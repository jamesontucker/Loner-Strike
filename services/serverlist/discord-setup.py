#!/usr/bin/env python3
"""One-time Discord setup for the community mod channel's buttons (modbot.py). Run on your Mac.

  python3 discord-setup.py                  shows what it would do (reads nothing secret beyond
                                            asking Discord for the app's id and public key)
  python3 discord-setup.py --apply          does it:
     1. registers the guild slash command /community pull|keep|block|status <page> [reason]
     2. sets the application's Interactions Endpoint URL to
        https://list.lonerstrike.com/discord/interactions
        (Discord sends a signed PING there at once, so the list service must already be running
        with DISCORD_PUBLIC_KEY set, or Discord refuses the URL)

Prints the application id and public key (both public) for the service's env file. The bot token
is read from ~/.config/loner-discord-token (or --token-file) and never printed.
"""
import argparse
import json
import os
import sys
import urllib.error
import urllib.request

API = "https://discord.com/api/v10"
GUILD = "1568421500636233748"
ENDPOINT = "https://list.lonerstrike.com/discord/interactions"
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
    if not a.apply:
        print(f"Would register /community in guild {GUILD} and set the endpoint to {a.endpoint}. Add --apply to do it.")
        return
    call(token, "PUT", f"/applications/{app['id']}/guilds/{GUILD}/commands", [COMMAND])
    print("Registered /community (pull, keep, block, status).")
    call(token, "PATCH", "/applications/@me", {"interactions_endpoint_url": a.endpoint})
    print(f"Interactions endpoint set to {a.endpoint} (Discord's signed PING was answered).")


if __name__ == "__main__":
    main()
