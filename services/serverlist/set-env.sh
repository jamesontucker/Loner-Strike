#!/bin/sh
# Sets one value in /etc/lonerstrike-list/community.env, read from stdin so it never shows up on
# a command line, in ps or in shell history. Keeps the file root-only (0600).
#   cat ~/.config/loner-discord-token | ssh you@box 'sudo /opt/lonerstrike-list/set-env.sh DISCORD_BOT_TOKEN'
set -eu
[ "$(id -u)" = 0 ] || { echo "Run it with sudo."; exit 1; }
key=${1:?Usage: set-env.sh KEY < value}
exec python3 -I - "$key" <<'PY'
import os, re, sys, tempfile
key, path = sys.argv[1], "/etc/lonerstrike-list/community.env"
if not re.fullmatch(r"[A-Z][A-Z0-9_]{1,40}", key):
    sys.exit("That isn't a setting name.")
value = open("/dev/stdin").read().strip()
if "\n" in value or not value:
    sys.exit("The value must be one non-empty line.")
lines = open(path).read().splitlines() if os.path.exists(path) else []
out, done = [], False
for l in lines:
    if l.startswith(key + "="):
        if not done:
            out.append(f"{key}={value}")
            done = True
        continue
    out.append(l)
if not done:
    out.append(f"{key}={value}")
fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path))
with os.fdopen(fd, "w") as f:
    f.write("\n".join(out) + "\n")
os.chmod(tmp, 0o600)
os.replace(tmp, path)
print(f"{key} set (value not shown). Restart the service: sudo systemctl restart lonerstrike-list")
PY
