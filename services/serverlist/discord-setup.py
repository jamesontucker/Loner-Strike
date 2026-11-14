#!/usr/bin/env python3
"""One-time Discord setup for the list service's buttons (modbot.py, applications.py). Run on your Mac.

  python3 discord-setup.py                  shows what it would do (asks Discord for the app's id,
                                            public key, channels and roles; changes nothing)
  python3 discord-setup.py --apply          does it:
     1. registers the guild slash command /community pull|keep|block|status <page> [reason]
     2. sets the application's Interactions Endpoint URL to
        https://list.lonerstrike.com/discord/interactions
        (Discord sends a signed PING there at once, so the list service must already be running
        with DISCORD_PUBLIC_KEY set, or Discord refuses the URL)
     3. "Apply for a role": makes (or reuses) a public, read-only #📝・apply channel with one pinned
        message and an [Apply for a role] button, and a private #🛂・applications channel for the
        team (Moderator, Dev, G-Man and the bot can see it). Running it again changes nothing that's
        already there.

Prints the application id and public key and the applications channel id (all public) for the
service's env file, and warns when the bot's role is below a role people can apply for (Discord
then refuses to give it). The bot token is read from ~/.config/loner-discord-token (or
--token-file) and never printed.
"""
import argparse
import json
import os
import sys
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

API = "https://discord.com/api/v10"
GUILD = "1568421500636233748"
ENDPOINT = "https://list.lonerstrike.com/discord/interactions"
TEAM_ROLES = ("1568425985504968768", "1568425984288362637", "1570702999754379264")  # Moderator, Dev, G-Man
APPLY_NAME, APPLICATIONS_NAME = "📝・apply", "🛂・applications"
STRING, SUB = 3, 1
PAGE = {"type": STRING, "name": "page", "description": "mod:<id>, spray:<id> or the GameBanana link", "required": True}
REASON = {"type": STRING, "name": "reason", "description": "Why (kept with the entry)", "required": False, "max_length": 300}
COMMAND = {
    "name": "community",
    "description": "Moderate Loner-Strike community creations",
    "type": 1,
    # a hint for who sees it (Manage Messages); the service itself only obeys Moderator, Dev and the owner
    "default_member_permissions": str(1 << 13),
    "dm_permission": False,
    "options": [
        {"type": SUB, "name": "pull", "description": "Pull a page: apps hide it", "options": [PAGE, REASON]},
        {"type": SUB, "name": "keep", "description": "Note that a reported page stays", "options": [PAGE]},
        {"type": SUB, "name": "block", "description": "The page's creator: their next submissions wait", "options": [PAGE, REASON]},
        {"type": SUB, "name": "status", "description": "Where a page stands", "options": [PAGE]},
    ],
}
# permissions (Discord's bit numbers)
VIEW, SEND, EMBED, HISTORY, REACT, MANAGE_MESSAGES = 1 << 10, 1 << 11, 1 << 14, 1 << 16, 1 << 6, 1 << 13
THREADS = (1 << 35) | (1 << 36) | (1 << 38)
APPLY_TEXT = ("**Apply for a role**\n"
              "Want to help test, make maps or modes, hunt bugs, or moderate? Press the button, pick a role and tell us a little about yourself. The team reads "
              "every application; you'll hear back here or by DM.")


def call(token, method, path, body=None):
    req = urllib.request.Request(API + path, data=json.dumps(body).encode() if body is not None else None, method=method,
                                 headers={"Authorization": f"Bot {token}", "Content-Type": "application/json",
                                          "User-Agent": "DiscordBot (https://lonerstrike.com, 1)"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        detail = e.read()[:300].decode("utf-8", "replace")
        sys.exit(f"Discord said {e.code} to {method} {path}: {detail}")


def setup_apply(token, ask, apply, say=print):
    """The apply channel, its pinned button and the private applications channel. `ask(method,
    path, body)` talks to Discord. Returns the applications channel id (or None in a dry run)."""
    import applications
    import modbot
    me = ask("GET", "/users/@me")
    bot_id = str(me["id"])
    channels = ask("GET", f"/guilds/{GUILD}/channels")
    norm = applications.norm

    def find(name, kind=0):
        return next((c for c in channels if c.get("type") == kind and norm(c.get("name", "")) == norm(name)), None)

    def category(word):
        return next((c["id"] for c in channels if c.get("type") == 4 and word in norm(c.get("name", ""))), None)

    plans = (
        (APPLY_NAME, category("community"), "Apply for a Loner-Strike role (press the button in the pinned message).",
         [{"id": GUILD, "type": 0, "allow": str(VIEW | HISTORY), "deny": str(SEND | REACT | THREADS)},
          {"id": bot_id, "type": 1, "allow": str(VIEW | SEND | EMBED | HISTORY | MANAGE_MESSAGES), "deny": "0"}]),
        (APPLICATIONS_NAME, category("staff"), "Role applications (team only).",
         [{"id": GUILD, "type": 0, "allow": "0", "deny": str(VIEW)},
          {"id": bot_id, "type": 1, "allow": str(VIEW | SEND | EMBED | HISTORY), "deny": "0"}]
         + [{"id": r, "type": 0, "allow": str(VIEW | SEND | HISTORY), "deny": "0"} for r in TEAM_ROLES]),
    )
    ids = {}
    for name, parent, topic, overwrites in plans:
        have = find(name)
        if have:
            say(f"#{name} is there ({have['id']}).")
            ids[name] = have["id"]
            continue
        say(f"{'Making' if apply else 'Would make'} #{name}" + (" in its category" if parent else "") + ".")
        if apply:
            body = {"name": name, "type": 0, "topic": topic, "permission_overwrites": overwrites}
            if parent:
                body["parent_id"] = parent
            ids[name] = ask("POST", f"/guilds/{GUILD}/channels", body)["id"]
    if apply or APPLY_NAME in ids:
        pins = ask("GET", f"/channels/{ids[APPLY_NAME]}/pins") if APPLY_NAME in ids else []
        ours = [m for m in pins if str((m.get("author") or {}).get("id")) == bot_id
                and "ls1:apply:" in json.dumps(m.get("components") or [])]
        if ours:
            say("The Apply button is pinned already.")
        elif apply:
            button = {"type": 2, "style": 1, "label": "Apply for a role",
                      "custom_id": modbot.sign_id(modbot.button_secret(token), "apply", 0)}
            msg = ask("POST", f"/channels/{ids[APPLY_NAME]}/messages",
                      {"content": APPLY_TEXT, "components": [{"type": 1, "components": [button]}], "allowed_mentions": {"parse": []}})
            ask("PUT", f"/channels/{ids[APPLY_NAME]}/pins/{msg['id']}")
            say("Posted and pinned the Apply button.")
        else:
            say("Would post and pin the Apply button.")
    # Discord only lets the bot give roles below its own highest role
    roles = ask("GET", f"/guilds/{GUILD}/roles")
    member = ask("GET", f"/guilds/{GUILD}/members/{bot_id}")
    pos = {str(r["id"]): r.get("position", 0) for r in roles}
    top = max((pos.get(str(r), 0) for r in member.get("roles", [])), default=0)
    by_name = {norm(r.get("name", "")): r for r in roles}
    for _key, name in applications.ROLES:
        r = by_name.get(norm(name))
        if not r:
            say(f"  ! There's no role called {name} yet: make it, or approving {name} applications will say so.")
        elif r.get("position", 0) >= top:
            say(f"  ! The bot's role is below {r['name']}: drag the bot's role (\"Loner-Strike\") above it in Server Settings > Roles.")
    if APPLICATIONS_NAME in ids:
        say(f"For the service's env file:  DISCORD_APPLY_CHANNEL_ID={ids[APPLICATIONS_NAME]}  "
            f"DISCORD_APPLY_ROLES={','.join(TEAM_ROLES)}")
        return ids[APPLICATIONS_NAME]
    return None


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--token-file", default=os.path.expanduser("~/.config/loner-discord-token"))
    p.add_argument("--endpoint", default=ENDPOINT)
    p.add_argument("--apply", action="store_true")
    a = p.parse_args()
    try:
        token = open(a.token_file).read().strip()
    except OSError:
        sys.exit(f"No bot token at {a.token_file}.")
    app = call(token, "GET", "/applications/@me")
    print(f"Application: {app.get('name')}  id {app.get('id')}")
    print(f"For the service's env file:  DISCORD_APP_ID={app.get('id')}  DISCORD_PUBLIC_KEY={app.get('verify_key')}")
    print(f"Interactions endpoint now: {app.get('interactions_endpoint_url') or '(none)'}")
    if a.apply:
        call(token, "PUT", f"/applications/{app['id']}/guilds/{GUILD}/commands", [COMMAND])
        print("Registered /community (pull, keep, block, status).")
        call(token, "PATCH", "/applications/@me", {"interactions_endpoint_url": a.endpoint})
        print(f"Interactions endpoint set to {a.endpoint} (Discord's signed PING was answered).")
    else:
        print(f"Would register /community in guild {GUILD} and set the endpoint to {a.endpoint}.")
    setup_apply(token, lambda m, path, b=None: call(token, m, path, b), a.apply)
    if not a.apply:
        print("Nothing changed. Add --apply to do it.")


if __name__ == "__main__":
    main()
