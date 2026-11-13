#!/usr/bin/env python3
"""Moderation for Loner-Strike's community creations, from a terminal (no website needed).

Talks to the list service's admin endpoints (community.py) with the admin token, read from
$LS_COMMUNITY_TOKEN or ~/.config/lonerstrike/community-admin.token (readable only by you).

Everything that passes the automatic checks is listed at once; the usual place to act is the
private Discord channel's cards ([Pull], [Keep], [Block creator], [Undo]). This is the same from
a terminal, and the way to publish the index.

  lsmod.py queue [--state pending|listed|hidden|rejected|pulled|all]   (default: what waits)
  lsmod.py show <page>                 everything about one page, with its history
  lsmod.py pull <page> --reason ".."   pull any page, even one never registered: apps hide it
  lsmod.py restore <page>              bring back a hidden or pulled page
  lsmod.py hide <page> [--reason ..]   keep it out of lists, still openable by link
  lsmod.py approve <page>              a blocked creator's submission (checks the page again)
  lsmod.py reject <page> --reason ".." turn down what's waiting (the author sees the reason)
  lsmod.py block <page> [--reason ..]  that page's creator: their next submissions wait
  lsmod.py unblock <page>
  lsmod.py blocked                     who is blocked
  lsmod.py reports [<page>]            players' reports (the last 100)
  lsmod.py feature <page> on|off
  lsmod.py use-file <page> <file id>   list another of the page's files (an older one too: only you can)
  lsmod.py recheck [<page>]            check GameBanana again now (all pages: in the background)
  lsmod.py status                      counts, and whether the index changed since publishing
  lsmod.py publish [--out DIR] [--upload]
        writes DIR/index.json and DIR/index.json.sha256 (default ./community-index); only with
        --upload are they put on dl.lonerstrike.com (scripts/r2-community.sh, R2 keys from
        ~/.config/lonerstrike/r2.env)

<page> is mod:<id>, spray:<id> or the GameBanana link. The admin API answers only on the box:
from a Mac, open a tunnel first (ssh -N -L 8100:127.0.0.1:8100 cs@<box>) and use the default
--server http://127.0.0.1:8100. Publishing again is needed after a pull for apps to see it (or the
optional publish timer on the box does it every few minutes; docs/COMMUNITY-SERVER.md).
"""
import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from community import parse_key  # noqa: E402

# moderation answers only on the box itself: open a tunnel first,  ssh -N -L 8100:127.0.0.1:8100 cs@<box>
DEFAULT_SERVER = "http://127.0.0.1:8100"
TOKEN_FILE = os.path.expanduser("~/.config/lonerstrike/community-admin.token")
# beside this file when installed on the box, in the repo's scripts/ on a Mac
UPLOADS = [os.path.join(HERE, "r2-community.sh"), os.path.join(HERE, "..", "..", "scripts", "r2-community.sh")]


class Failed(Exception):
    pass


def token(path):
    # COMMUNITY_ADMIN_TOKEN: the box's own env file (the publish timer)
    t = (os.environ.get("LS_COMMUNITY_TOKEN") or os.environ.get("COMMUNITY_ADMIN_TOKEN") or "").strip()
    if t:
        return t
    try:
        mode = os.stat(path).st_mode
        if mode & (stat.S_IRWXG | stat.S_IRWXO):
            print(f"Warning: {path} can be read by others; chmod 600 it.", file=sys.stderr)
        with open(path) as f:
            return f.read().strip()
    except OSError:
        raise Failed(f"No admin token: set LS_COMMUNITY_TOKEN or put it in {path} (chmod 600).")


def check_server(server):
    """https anywhere, plain http only to this machine (a token must never cross a network in the clear)."""
    u = urllib.parse.urlsplit(server)
    if u.scheme == "https" and u.hostname:
        return
    if u.scheme == "http" and u.hostname in ("127.0.0.1", "localhost", "::1"):
        return
    raise Failed(f"{server} isn't https (plain http is only for 127.0.0.1, through an ssh tunnel).")


def call(server, tok, method, path, body=None, raw=False):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(server.rstrip("/") + "/community/admin/" + path, data=data, method=method,
                                 headers={"Authorization": f"Bearer {tok}", "Content-Type": "application/json",
                                          "User-Agent": "lsmod/1.0 (Loner-Strike moderation)"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            out = r.read()
            return (out, dict(r.headers)) if raw else json.loads(out or b"{}")
    except urllib.error.HTTPError as e:
        try:
            msg = json.loads(e.read()).get("error") or f"status {e.code}"
        except ValueError:
            msg = f"status {e.code}"
        raise Failed(msg)
    except (OSError, ValueError) as e:
        raise Failed(f"The list service didn't answer ({type(e).__name__}).")


def page_path(text):
    p = parse_key(text)
    if not p:
        raise Failed(f"{text!r} isn't a page: use mod:<id>, spray:<id> or the GameBanana link.")
    return f"item/{p[0].lower()}/{p[1]}"


def when(t):
    return time.strftime("%Y-%m-%d %H:%M", time.localtime(t)) if t else "-"


def show_entry(d):
    it = d.get("waiting") or d.get("listed") or {}
    print(f"{d['key']}  {d['state'].upper()}  {d['kind']}  {d['link']}")
    print(f"  name: {it.get('name', '')}   by {d['submitter']} (GameBanana member {d['submitter_id']})")
    if d.get("reason"):
        print(f"  reason: {d['reason']}" + (f"  (out of the list by the {d['hidden_by']})" if d.get("hidden_by") else ""))
    if d.get("check_note"):
        print(f"  last check: {d['check_note']} ({when(d['checked'])})")
    for label, x in (("listed", d.get("listed")), ("waiting", d.get("waiting"))):
        if x:
            print(f"  {label}: file {x.get('file')} md5 {x.get('md5')}")
    mode = it.get("mode")
    if mode:
        print(f"  mode {mode.get('id')}: settings only (Game Mode Studio)")
    if d.get("reports"):
        print(f"  reports: {d['reports']} in 90 days (lsmod.py reports {d['key']})")
    if d.get("creatorBlocked"):
        print("  the creator is blocked: their submissions wait for a moderator")
    if it.get("map"):
        m = it["map"]
        print(f"  map {m['name']}: modes {', '.join(m['modes']) or '-'}; nav {m['nav']} knowledge {m['lsv']} overview {m['overview']}")
    if it.get("summary"):
        print(f"  summary: {it['summary']}")
    print("  history:")
    for h in d.get("history", []):
        print(f"    {when(h['at'])}  {h['action']}  {h['detail']}")


def publish(server, tok, out, upload, if_changed=False, auto=False):
    if if_changed or auto:
        st = call(server, tok, "GET", "status")
        if auto and (st.get("lastRecheck") or {}).get("heldBack"):
            print("The last re-check was held back (too many pages leaving at once): not publishing until a person looks.")
            return
        if not st["changedSincePublish"]:
            print("The index hasn't changed since it was last published.")
            return
    data, headers = call(server, tok, "GET", "index", raw=True)
    sha = hashlib.sha256(data).hexdigest()
    if headers.get("X-Index-Sha256") != sha:
        raise Failed("The index arrived damaged (its sha256 doesn't match). Try again.")
    doc = json.loads(data)
    if doc.get("schema") != 1 or not isinstance(doc.get("items"), list):
        raise Failed("The index doesn't look right; nothing written.")
    os.makedirs(out, exist_ok=True)
    for name, content in (("index.json", data), ("index.json.sha256", f"{sha}  index.json\n".encode())):
        tmp = os.path.join(out, name + ".tmp")
        with open(tmp, "wb") as f:
            f.write(content)
        os.replace(tmp, os.path.join(out, name))
    listed = sum(1 for i in doc["items"] if i.get("status") == "listed")
    print(f"Wrote {out}/index.json ({len(data)} bytes, {listed} listed, {len(doc['revoked'])} pulled), sha256 {sha}.")
    if not upload:
        print("Nothing uploaded. Run again with --upload to put it on dl.lonerstrike.com.")
        return
    script = next((os.path.normpath(u) for u in UPLOADS if os.path.exists(u)), None)
    if not script:
        raise Failed("r2-community.sh isn't here; run publish from the Loner-Strike repo on your Mac.")
    r = subprocess.run(["bash", script, out, "--upload"])
    if r.returncode != 0:
        raise Failed("The upload didn't finish; the index online is unchanged or the previous one.")
    call(server, tok, "POST", "published", {"sha256": sha})
    print("Published: https://dl.lonerstrike.com/community/index.json (apps see it within 15 minutes).")


def main(argv=None):
    p = argparse.ArgumentParser(prog="lsmod.py", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--server", default=os.environ.get("LS_COMMUNITY_SERVER", DEFAULT_SERVER))
    p.add_argument("--token-file", default=TOKEN_FILE)
    sub = p.add_subparsers(dest="cmd", required=True)
    q = sub.add_parser("queue")
    q.add_argument("--state", default="")
    for name in ("show", "approve", "restore"):
        sub.add_parser(name).add_argument("page")
    for name, need in (("reject", True), ("pull", True), ("hide", False)):
        s = sub.add_parser(name)
        s.add_argument("page")
        s.add_argument("--reason", required=need, default="")
    s = sub.add_parser("block")
    s.add_argument("page")
    s.add_argument("--reason", default="")
    sub.add_parser("unblock").add_argument("page")
    sub.add_parser("blocked")
    sub.add_parser("reports").add_argument("page", nargs="?")
    s = sub.add_parser("use-file")
    s.add_argument("page")
    s.add_argument("file", type=int)
    s = sub.add_parser("feature")
    s.add_argument("page")
    s.add_argument("onoff", choices=("on", "off"))
    sub.add_parser("recheck").add_argument("page", nargs="?")
    sub.add_parser("status")
    s = sub.add_parser("publish")
    s.add_argument("--out", default="community-index")
    s.add_argument("--upload", action="store_true", help="really put it on dl.lonerstrike.com")
    s.add_argument("--if-changed", action="store_true", help="do nothing when it's the same as last published")
    s.add_argument("--auto", action="store_true", help="the publish timer: --if-changed, and nothing after a held-back re-check")
    a = p.parse_args(argv)
    try:
        srv = a.server
        check_server(srv)
        tok = token(a.token_file)
        if a.cmd == "queue":
            r = call(srv, tok, "GET", "queue" + (f"?state={urllib.parse.quote(a.state)}" if a.state else ""))
            if not r["entries"]:
                print("Nothing here.")
            for e in r["entries"]:
                extra = ""
                note = f"  ({e['reason'] or e['checkNote']})" if e["reason"] or e["checkNote"] else ""
                print(f"{e['key']:<16} {e['state'] + (' +update' if e['waiting'] and e['state'] != 'pending' else ''):<16} "
                      f"{e['kind']:<8} {e['name'][:40]:<40} {e['author'][:20]:<20} {when(e['updated'])}{extra}{note}")
            print(f"{r['pending']} waiting.")
        elif a.cmd == "show":
            show_entry(call(srv, tok, "GET", page_path(a.page)))
        elif a.cmd in ("approve", "restore", "reject", "pull", "hide"):
            d = call(srv, tok, "POST", page_path(a.page), {"action": a.cmd, "reason": getattr(a, "reason", "")})
            print(f"{d['key']} is now {d['state']}.")
            if a.cmd in ("approve", "restore", "pull", "hide"):  # the index changed
                print("Run `lsmod.py publish --upload` to put the change out.")
        elif a.cmd in ("block", "unblock", "blocked"):
            body = None
            if a.cmd != "blocked":
                page_path(a.page)
                body = {"page": a.page, "block": a.cmd == "block", "reason": getattr(a, "reason", "")}
            r = call(srv, tok, "POST" if body else "GET", "blocked", body)
            if not r["blocked"]:
                print("Nobody is blocked.")
            for b in r["blocked"]:
                print(f"{b['name'] or '?'} (GameBanana member {b['submitter_id']})  since {when(b['at'])} by {b['by']}  {b['reason']}")
        elif a.cmd == "reports":
            q = ""
            if a.page:
                page_path(a.page)
                q = "?key=" + urllib.parse.quote(a.page)
            r = call(srv, tok, "GET", "reports" + q)
            if not r["reports"]:
                print("No reports.")
            for x in r["reports"]:
                print(f"{when(x['at'])}  {x['key']:<16} {x['reason']:<16} #{x['reporter']}  {x['note']}")
        elif a.cmd == "use-file":
            d = call(srv, tok, "POST", page_path(a.page), {"action": "usefile", "file": a.file})
            print(f"{d['key']} now lists file {d['listed']['file']}. Run `lsmod.py publish --upload` to put it out.")
        elif a.cmd == "feature":
            d = call(srv, tok, "POST", page_path(a.page), {"action": "feature", "featured": a.onoff == "on"})
            print(f"{d['key']}: featured {d['featured']}.")
        elif a.cmd == "recheck":
            body = {"key": a.page} if a.page else {}
            r = call(srv, tok, "POST", "recheck", body)
            if r.get("started"):
                print("Re-check started; see `lsmod.py status` in a few minutes (the mod channel hears about changes).")
            else:
                print(f"Checked {r['checked']}, changed {r['changed']}.")
                for c in r.get("changes", []):
                    print(f"  {c['key']}: {c['verdict']} ({c['why']})")
        elif a.cmd == "status":
            r = call(srv, tok, "GET", "status")
            print("  ".join(f"{k} {v}" for k, v in r["counts"].items()))
            pub = r.get("published") or {}
            print(f"Index: {r['index']['bytes']} bytes, sha256 {r['index']['sha256']}")
            print(f"Published: {pub.get('sha256', 'never')} {when(pub.get('at', 0))}"
                  + ("  -> CHANGED, run `lsmod.py publish --upload`" if r["changedSincePublish"] else "  (up to date)"))
            lr = r.get("lastRecheck") or {}
            if lr:
                print(f"Last re-check {when(lr.get('at', 0))}: {lr.get('checked')} checked, {lr.get('changed')} changed"
                      + (", HELD BACK" if lr.get("heldBack") else ""))
        elif a.cmd == "publish":
            publish(srv, tok, a.out, a.upload, a.if_changed, a.auto)
    except Failed as e:
        print(e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
