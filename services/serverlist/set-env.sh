#!/bin/sh
# Sets one value in /etc/lonerstrike-list/community.env, read from stdin so it never shows up on a
# command line, in ps or in shell history. Keeps the file root-only (0600). (No R2 keys ever go on
# the box: the bucket also holds the app's updates; publishing runs on GitHub Actions.)
#   ssh you@your-box 'sudo /opt/lonerstrike-list/set-env.sh DISCORD_BOT_TOKEN' < ~/.config/loner-discord-token
set -eu
[ "$(id -u)" = 0 ] || { echo "Run it with sudo."; exit 1; }
key=${1:?Usage: set-env.sh KEY < value}
case "$key" in R2_*|AWS_*) echo "R2 keys never go on this machine."; exit 1 ;; esac
file=community
# the value comes in on stdin; the Python below comes in on a here-document, which takes stdin
# over, so the value is handed on as fd 3 first (3<&0 must come before the <<)
exec python3 -I - "$key" "/etc/lonerstrike-list/$file.env" 3<&0 <<'PY'
import os, re, sys, tempfile
key, path = sys.argv[1], sys.argv[2]
if not re.fullmatch(r"[A-Z][A-Z0-9_]{1,40}", key):
    sys.exit("That isn't a setting name.")
value = os.fdopen(3).read().strip()
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
print(f"{key} set in {path} (value not shown). For community.env, restart: sudo systemctl restart lonerstrike-list")
PY
