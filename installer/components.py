"""Installs catalog components: game modes and add-ons made by the community.

A component (catalog/components.json) can have:
  vendor   folder in this repo whose contents mirror the game folder: the mod's
           sources with Loner-Strike's bot patches, its configs and language files
  compile  plugins to build: {"src": path under scripting/, "out": name, "include": dir or null}
  fetch    resources from the original project, each file checked by sha256:
           {"archive": url, "strip": "prefix/inside/archive/", "files": {"rel/path": sha256}}
           {"url": url, "to": "rel/path", "sha256": "..."}
           a file's value can also be {"from": "path/in/archive", "sha256": "..."} to place it elsewhere
  patches  unified diffs (paths relative to the game folder) applied to fetched files: Loner-Strike's
           changes to mods whose code has no license, so only our changes live in this repo
  maps     maps from GameBanana: {"mod": id, "file": id, "maps": [names]}
  dirs     empty folders the mod expects
Models, sounds and maps that already exist are never replaced.
"""

import json
import os
import shutil
import sys
import tempfile

from . import fetch
from .core import REPO, compile_plugin

CATALOG = os.path.join(REPO, "catalog", "components.json")


def load_catalog():
    """components.json plus one file per packaged mod in catalog/parts/ (those replace
    the entry with the same id)."""
    with open(CATALOG, encoding="utf-8") as f:
        comps = {c["id"]: c for c in json.load(f)["components"]}
    parts = os.path.join(os.path.dirname(CATALOG), "parts")
    for name in sorted(os.listdir(parts)) if os.path.isdir(parts) else []:
        if name.endswith(".json"):
            with open(os.path.join(parts, name), encoding="utf-8") as f:
                part = json.load(f)
            comps[part["id"]] = part
    return list(comps.values())


def _copy_vendor(rec, comp):
    root = os.path.join(REPO, comp["vendor"])
    for d, _, files in os.walk(root):
        for f in files:
            full = os.path.join(d, f)
            rec.copy(full, os.path.relpath(full, root).replace(os.sep, "/"))


def _place_resource(rec, src, rel, sha):
    """Copies one fetched file, unless the player already has a file there."""
    if sha and fetch.sha256(src) != sha:
        raise fetch.FetchError("%s changed upstream (checksum differs)." % rel)
    if rec.exists(rel) and rel not in rec.data["files"]:
        return False
    rec.copy(src, rel)
    return True


def _fetch(rec, comp, say):
    with tempfile.TemporaryDirectory() as tmp:
        for i, item in enumerate(comp.get("fetch", [])):
            if "archive" in item:
                where = fetch.unpack(fetch.download(item["archive"], item.get("sha256")), os.path.join(tmp, str(i)))
                for rel, sha in item["files"].items():
                    origin = rel
                    if isinstance(sha, dict):
                        origin, sha = sha["from"], sha.get("sha256")
                    src = os.path.join(where, item.get("strip", "") + origin)
                    if not os.path.isfile(src):
                        raise fetch.FetchError("%s is missing from %s" % (rel, item["archive"]))
                    _place_resource(rec, src, rel, sha)
            else:
                _place_resource(rec, fetch.download(item["url"], item.get("sha256")), item["to"], item.get("sha256"))


def map_store(install):
    """The panel's map store, so maps installed here can be removed from the panel too."""
    sys.path.insert(0, os.path.join(REPO, "panel"))
    import getmaps

    class Game:      # the little the map store needs from the panel's Game
        hl = install.hl
        def maps(self, refresh=False):
            return []

    return getmaps.MapStore(Game(), os.path.join(REPO, "panel"))


def _maps(rec, install, comp, say):
    """Installs the component's GameBanana maps and remembers the ones that are new,
    so uninstalling the component removes them again."""
    if not comp.get("maps"):
        return
    store = map_store(install)
    import time
    ours = rec.data.setdefault("maps", {}).setdefault(comp["id"], [])
    for m in comp["maps"]:
        if all(os.path.exists(os.path.join(install.hl, g, "maps", n + ".bsp"))
               for n in m["maps"] for g in ("cstrike",)):
            continue
        say("    map: %s" % ", ".join(m["maps"]))
        if str(m["mod"]) not in store.records() and m["mod"] not in ours:
            ours.append(m["mod"])
        job = store.start_install(m["mod"], m["file"])
        while store.jobs[job]["state"] not in ("done", "error"):
            time.sleep(0.5)
        if store.jobs[job]["state"] == "error":
            say("      couldn't install it: %s" % store.jobs[job]["message"])


def _patch(rec, comp):
    from . import patch
    for p in comp.get("patches", []):
        with open(os.path.join(REPO, p), encoding="latin-1", newline="") as f:
            files = patch.parse(f.read())
        for rel, hunks in files.items():
            text = rec.read(rel)
            if text is None:
                if rec.dry:
                    continue
                raise fetch.FetchError("%s needs %s, which wasn't downloaded" % (p, rel))
            try:
                rec.write(rel, patch.apply(text, hunks))
            except patch.PatchError as e:
                raise fetch.FetchError("%s: %s (%s)" % (p, e, rel))


def install_component(rec, install, comp, say):
    say(comp["name"])
    rec.begin(comp["id"])
    if comp.get("vendor"):
        _copy_vendor(rec, comp)
    for d in comp.get("dirs", []):
        if not rec.dry:
            os.makedirs(os.path.join(rec.game, d), exist_ok=True)
    _fetch(rec, comp, say)
    _patch(rec, comp)
    ok = True
    for c in comp.get("compile", []):
        ok = compile_plugin(rec, install, c["src"], c["out"], c.get("include"), say) and ok
    if not rec.dry:
        _maps(rec, install, comp, say)
    rec.done(comp["id"], comp.get("version", 1))
    return ok
