#!/usr/bin/env python3
"""Live test harness for QA (docs/notes/QA4.md, "Live test harness"). Run on the Mac.

Everything it makes is named QA- (or qa-), is recorded in ~/.config/lonerstrike/qa-live.json
(mode 600) and goes again with `cleanup`. It never prints a token.

  live.py discord-setup [--owner <discord id>]   private category QA-tests + channel #qa-live (only the
                                                bot and the owner see them) and the role QA-Tester
  live.py discord-check [--target <discord id>]  the bot posts, edits and deletes a message in #qa-live,
                                                and gives QA-Tester to the target and takes it back
  live.py community-add <mod:ID> --file <file id>  registers a test GameBanana page on the live list
                                                (the page must exist and carry the LonerStrike tag;
                                                an agent can't upload to GameBanana: the user makes it)
  live.py community-check                       where every recorded test entry stands
  live.py link-add <steamid> <discord id>       records a test link to remove at cleanup (made in game)
  live.py cleanup                               removes every QA- role, channel, recorded entry and link
  live.py verify-clean                          checks it all left (exit 1 when something's still there)

Needs: the bot token in ~/.config/loner-discord-token; for the list, the admin tunnel
(ssh -f -N -L 8100:127.0.0.1:8100 cs@<box>) and lsmod.py's admin token; ssh to the box.
"""
import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
SERVICE = os.path.dirname(HERE)
API = "https://discord.com/api/v10"
GUILD = "1568421500636233748"
OWNER = "310619637727363072"
BOX = os.environ.get("LS_BOX", "cs@178.156.241.180")
LIST = "https://list.lonerstrike.com"
STATE = os.path.expanduser("~/.config/lonerstrike/qa-live.json")
TOKEN_FILE = os.path.expanduser("~/.config/loner-discord-token")
VIEW, SEND, HISTORY, MANAGE = 1 << 10, 1 << 11, 1 << 16, 1 << 13
CATEGORY, CHANNEL, ROLE = "QA-tests", "qa-live", "QA-Tester"


def state():
    try:
        return json.load(open(STATE))
    except (OSError, ValueError):
        return {"channels": [], "roles": [], "entries": [], "links": []}


def save(s):
    os.makedirs(os.path.dirname(STATE), exist_ok=True)
    with open(STATE, "w") as f:
        json.dump(s, f, indent=2)
    os.chmod(STATE, 0o600)


def discord(method, path, body=None):
    tok = open(TOKEN_FILE).read().strip()
    req = urllib.request.Request(API + path, data=json.dumps(body).encode() if body is not None else None, method=method,
                                 headers={"Authorization": f"Bot {tok}", "Content-Type": "application/json",
                                          "User-Agent": "DiscordBot (https://lonerstrike.com, 1)", "X-Audit-Log-Reason": "Loner-Strike QA"})
    for _ in range(3):
        try:
            with urllib.request.urlopen(req, timeout=20) as r:
                raw = r.read()
                return r.status, json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            if e.code == 429:
                time.sleep(float((json.loads(e.read() or b"{}") or {}).get("retry_after", 2)))
                continue
            return e.code, json.loads(e.read() or b"{}")
    return 429, {}


def ok(code, what):
    print(f"{'ok  ' if 200 <= code < 300 else 'FAIL'} {what} ({code})")
    return 200 <= code < 300


def discord_setup(a):
    s = state()
    code, me = discord("GET", "/users/@me")
    if not ok(code, "the bot answers"):
        return 1
    bot = me["id"]
    allow = str(VIEW | SEND | HISTORY)
    over = [{"id": GUILD, "type": 0, "deny": str(VIEW)},
            {"id": bot, "type": 1, "allow": str(VIEW | SEND | HISTORY | MANAGE)},
            {"id": a.owner, "type": 1, "allow": allow}]
    code, chans = discord("GET", f"/guilds/{GUILD}/channels")
    have = {c["name"]: c for c in chans} if code == 200 else {}
    cat = have.get(CATEGORY)
    if not cat:
        code, cat = discord("POST", f"/guilds/{GUILD}/channels", {"name": CATEGORY, "type": 4, "permission_overwrites": over})
        ok(code, f"category {CATEGORY}")
        s["channels"].append(cat["id"])
    ch = next((c for c in chans if c["name"] == CHANNEL and c.get("parent_id") == cat["id"]), None)
    if not ch:
        code, ch = discord("POST", f"/guilds/{GUILD}/channels", {"name": CHANNEL, "type": 0, "parent_id": cat["id"],
                                                              "topic": "Loner-Strike QA (removed after the tests)", "permission_overwrites": over})
        ok(code, f"channel #{CHANNEL}")
        s["channels"].insert(0, ch["id"])
    code, roles = discord("GET", f"/guilds/{GUILD}/roles")
    role = next((r for r in roles if r["name"] == ROLE), None) if code == 200 else None
    if not role:
        code, role = discord("POST", f"/guilds/{GUILD}/roles", {"name": ROLE, "permissions": "0", "mentionable": False})
        ok(code, f"role {ROLE}")
        s["roles"].append(role["id"])
    s.update(channel=ch["id"], category=cat["id"], role=role["id"])
    save(s)
    print(f"#{CHANNEL} {ch['id']} (only the bot and {a.owner} see it); role {ROLE} {role['id']}")
    return 0


def discord_check(a):
    s = state()
    if not s.get("channel"):
        print("Run discord-setup first.")
        return 1
    good = True
    code, m = discord("POST", f"/channels/{s['channel']}/messages", {"content": "QA: the bot can post here", "allowed_mentions": {"parse": []}})
    good &= ok(code, "post")
    if code == 200:
        good &= ok(discord("PATCH", f"/channels/{s['channel']}/messages/{m['id']}", {"content": "QA: and edit"})[0], "edit")
        good &= ok(discord("DELETE", f"/channels/{s['channel']}/messages/{m['id']}")[0], "delete")
    target = a.target or OWNER
    good &= ok(discord("PUT", f"/guilds/{GUILD}/members/{target}/roles/{s['role']}")[0], f"give {ROLE} to {target}")
    code, mem = discord("GET", f"/guilds/{GUILD}/members/{target}")
    good &= ok(code if s["role"] in mem.get("roles", []) else 500, "the member has it")
    good &= ok(discord("DELETE", f"/guilds/{GUILD}/members/{target}/roles/{s['role']}")[0], f"take {ROLE} back")
    return 0 if good else 1


def lsmod(*args):
    r = subprocess.run([sys.executable, os.path.join(SERVICE, "lsmod.py"), *args], capture_output=True, text=True)
    return r.returncode, (r.stdout + r.stderr).strip()


def community_add(a):
    s = state()
    model, _, gid = a.page.partition(":")
    body = {"schema": 1, "app": "qa-live", "page": {"model": "Mod" if model == "mod" else model, "id": int(gid), "file": a.file}}
    req = urllib.request.Request(f"{LIST}/community/submit", data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json", "User-Agent": "Loner-Strike QA"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            ans = json.loads(r.read())
    except urllib.error.HTTPError as e:
        ans = json.loads(e.read() or b"{}")
    print(json.dumps(ans)[:400])
    if a.page not in s["entries"]:
        s["entries"].append(a.page)
        save(s)
    return 0 if "error" not in ans else 1


def community_check(a):
    for key in state()["entries"]:
        model, _, gid = key.partition(":")
        try:
            with urllib.request.urlopen(f"{LIST}/community/submission/{model}/{gid}", timeout=20) as r:
                print(key, json.loads(r.read()))
        except urllib.error.HTTPError as e:
            print(key, e.code)
    return 0


def link_add(a):
    s = state()
    s["links"].append({"steamid": a.steamid, "discord": a.discord})
    save(s)
    print(f"recorded: {a.steamid} goes at cleanup")
    return 0


def cleanup(a):
    s = state()
    for cid in list(s["channels"]):
        code, _ = discord("DELETE", f"/channels/{cid}")
        if code in (200, 404):
            s["channels"].remove(cid)
        ok(code if code != 404 else 200, f"channel {cid} removed")
    for rid in list(s["roles"]):
        code, _ = discord("DELETE", f"/guilds/{GUILD}/roles/{rid}")
        if code in (200, 204, 404):
            s["roles"].remove(rid)
        ok(code if code != 404 else 204, f"role {rid} removed")
    for key in list(s["entries"]):
        code, out = lsmod("pull", key, "--reason", "QA test entry")
        print(out.splitlines()[0] if out else key)
        # gone from the database too (a test entry, so its history goes with it)
        r = subprocess.run(["ssh", "-o", "BatchMode=yes", BOX, "sudo python3 -c \"import sqlite3,sys; d=sqlite3.connect('/var/lib/lonerstrike-list/community.db'); "
                            "n=d.execute('DELETE FROM entries WHERE key=?', (sys.argv[1],)).rowcount; d.commit(); print(n)\" " + key],
                           capture_output=True, text=True)
        if r.returncode == 0:
            s["entries"].remove(key)
            ok(200, f"entry {key} removed ({r.stdout.strip()} row)")
    for ln in list(s["links"]):
        code, out = lsmod("roles", "unlink", ln["steamid"], "--reason", "QA test link")
        if code == 0 or "Nobody is linked" in out:
            s["links"].remove(ln)
        print(out.splitlines()[0] if out else ln["steamid"])
    for k in ("channel", "category", "role"):
        s.pop(k, None)
    save(s)
    return verify_clean(a)


def verify_clean(a):
    left = []
    code, chans = discord("GET", f"/guilds/{GUILD}/channels")
    left += [f"channel {c['name']}" for c in (chans if code == 200 else []) if c["name"].lower().startswith("qa-")]
    code, roles = discord("GET", f"/guilds/{GUILD}/roles")
    left += [f"role {r['name']}" for r in (roles if code == 200 else []) if r["name"].startswith("QA-")]
    s = state()
    left += [f"entry {k}" for k in s["entries"]] + [f"link {ln['steamid']}" for ln in s["links"]]
    for x in left:
        print(f"still there: {x}")
    print("clean" if not left else f"{len(left)} left")
    return 1 if left else 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("discord-setup").add_argument("--owner", default=OWNER)
    sub.add_parser("discord-check").add_argument("--target", default="")
    c = sub.add_parser("community-add")
    c.add_argument("page")
    c.add_argument("--file", type=int, required=True)
    sub.add_parser("community-check")
    ln = sub.add_parser("link-add")
    ln.add_argument("steamid")
    ln.add_argument("discord")
    sub.add_parser("cleanup")
    sub.add_parser("verify-clean")
    a = p.parse_args()
    return {"discord-setup": discord_setup, "discord-check": discord_check, "community-add": community_add,
            "community-check": community_check, "link-add": link_add, "cleanup": cleanup, "verify-clean": verify_clean}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
