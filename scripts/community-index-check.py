#!/usr/bin/env python3
"""Checks a community index before the publish workflow uploads it (.github/workflows/community-publish.yml).

  community-index-check.py <new index.json> <new index.json.sha256> <header sha> [<live index.json>] [--force]

Exit 0: publish it. Exit 3: the same as what's live, nothing to do. Exit 1: refused (the reason on
stderr): it doesn't match its sha256, isn't a schema 1 index the app reads, is over 8 MB or 5000
entries, or many listed creations would disappear at once (more than 3, or 2 or more and over a
quarter of what's live, the re-check guard's rule) unless --force.
"""
import hashlib
import json
import re
import sys

MAX_BYTES = 8 << 20
MAX_ITEMS = 5000
KINDS = {"map", "mode", "prefab", "spray", "texture"}
STATUSES = {"listed", "hidden", "revoked"}
KEY = re.compile(r"^(mod|spray):[1-9][0-9]{0,7}$")


class Refused(Exception):
    pass


def check(data, sha_text, header_sha):
    """The parsed index, or Refused."""
    if len(data) > MAX_BYTES:
        raise Refused(f"the index is {len(data)} bytes; the app reads at most {MAX_BYTES}")
    sha = hashlib.sha256(data).hexdigest()
    want = (sha_text.split() or [""])[0].lower()
    if sha != want or sha != (header_sha or "").strip().lower():
        raise Refused("the index doesn't match its sha256")
    try:
        doc = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        raise Refused("the index isn't JSON")
    if not isinstance(doc, dict) or doc.get("schema") != 1:
        raise Refused("the index isn't schema 1")
    items, revoked = doc.get("items"), doc.get("revoked")
    if not isinstance(items, list) or not isinstance(revoked, list) or not isinstance(doc.get("verifiedAuthors", []), list):
        raise Refused("the index's lists are missing")
    if len(items) > MAX_ITEMS:
        raise Refused(f"the index has {len(items)} entries; the app reads at most {MAX_ITEMS}")
    if not all(isinstance(r, str) and KEY.match(r) for r in revoked):
        raise Refused("the index's pulled list has something that isn't a page")
    for it in items:
        ok = (isinstance(it, dict) and it.get("kind") in KINDS and it.get("model") in ("Mod", "Spray")
              and type(it.get("gb")) is int and 0 < it["gb"] < 100_000_000 and it.get("status") in STATUSES
              and isinstance(it.get("name"), str) and it["name"].strip())
        if not ok:
            raise Refused("the index has an entry the app couldn't read")
    return doc, sha


def listed(doc):
    return sum(1 for it in doc.get("items", []) if isinstance(it, dict) and it.get("status") == "listed")


def main(argv):
    force = "--force" in argv
    args = [a for a in argv if a != "--force"]
    if len(args) not in (3, 4):
        print(__doc__, file=sys.stderr)
        return 2
    data = open(args[0], "rb").read()
    try:
        doc, sha = check(data, open(args[1]).read(), args[2])
        live = None
        if len(args) == 4:
            try:
                raw = open(args[3], "rb").read()
                live = json.loads(raw.decode("utf-8")) if raw else None
                if live is not None and hashlib.sha256(raw).hexdigest() == sha:
                    print("The index is the same as the live one.")
                    return 3
            except (OSError, UnicodeDecodeError, ValueError):
                live = None
        if isinstance(live, dict):
            before, after = listed(live), listed(doc)
            drop = before - after
            if (drop > 3 or (drop >= 2 and drop > before * 0.25)) and not force:
                raise Refused(f"{drop} of {before} listed creations would disappear at once; check with lsmod.py, "
                              "then run the workflow by hand with force")
            print(f"Listed: {before} live, {after} new.")
    except Refused as e:
        print(f"Refused: {e}", file=sys.stderr)
        return 1
    print(f"OK to publish: {len(data)} bytes, {len(doc['items'])} entries, sha256 {sha}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
