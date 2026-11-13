#!/bin/sh
# Puts the Loner-Strike server list (and its community creations part) on this machine, or
# updates it. Run it from this folder on the machine, as root or with sudo:
#   scp -r services/serverlist scripts/r2-community.sh you@machine:/tmp/serverlist-new/
#   ssh you@machine 'sudo sh /tmp/serverlist-new/serverlist/install.sh'
# It listens on 127.0.0.1:8100. Point a hostname at it: in Cloudflare's dashboard, add a
# public hostname to the machine's tunnel (like list.example.com, service http://localhost:8100).
# Community settings and secrets: /etc/lonerstrike-list/community.env (made once from
# community.env.example with a fresh admin token; never overwritten). Database migrations run by
# themselves when the service starts (community.db, PRAGMA user_version).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
[ "$(id -u)" = 0 ] || { echo "Run it with sudo."; exit 1; }
[ -x /usr/bin/python3 ] || { echo "Python 3 isn't installed (apt install python3)."; exit 1; }
# fast Ed25519 checks of Discord's signed button clicks (without it a slower built-in check is used)
python3 -c 'import cryptography' 2>/dev/null || DEBIAN_FRONTEND=noninteractive apt-get install -y -q python3-cryptography >/dev/null \
  || echo "Couldn't install python3-cryptography; the slower built-in signature check will be used."
install -d -o root -g root -m 755 /opt/lonerstrike-list
for f in serverlist.py community.py modbot.py lsmod.py; do
  install -o root -g root -m 644 "$here/$f" "/opt/lonerstrike-list/$f"
done
install -o root -g root -m 755 "$here/set-env.sh" /opt/lonerstrike-list/set-env.sh
for f in "$here/r2-community.sh" "$here/../r2-community.sh" "$here/../../scripts/r2-community.sh"; do
  [ -f "$f" ] && { install -o root -g root -m 755 "$f" /opt/lonerstrike-list/r2-community.sh; break; }
done
install -d -o root -g root -m 700 /etc/lonerstrike-list
umask 077
if [ ! -f /etc/lonerstrike-list/community.env ]; then
  token=$(python3 -c 'import secrets; print(secrets.token_urlsafe(36))')
  publish=$(python3 -c 'import secrets; print(secrets.token_urlsafe(36))')
  sed -e "s|^COMMUNITY_ADMIN_TOKEN=.*|COMMUNITY_ADMIN_TOKEN=$token|" -e "s|^COMMUNITY_PUBLISH_TOKEN=.*|COMMUNITY_PUBLISH_TOKEN=$publish|" \
    "$here/community.env.example" > /etc/lonerstrike-list/community.env
  # the publish job's own file: its read-only token and (filled in by you) an R2 token, nothing else
  printf 'LS_COMMUNITY_TOKEN=%s\nR2_ACCESS_KEY_ID=\nR2_SECRET_ACCESS_KEY=\nR2_ENDPOINT=\nR2_BUCKET=\n' "$publish" > /etc/lonerstrike-list/publish.env
  echo "Made /etc/lonerstrike-list/community.env (new admin and publish tokens) and publish.env."
fi
chmod 600 /etc/lonerstrike-list/community.env
[ -f /etc/lonerstrike-list/publish.env ] && chmod 600 /etc/lonerstrike-list/publish.env
for u in lonerstrike-list.service lonerstrike-community-publish.service lonerstrike-community-publish.timer; do
  install -o root -g root -m 644 "$here/$u" "/etc/systemd/system/$u"
done
systemctl daemon-reload
systemctl enable lonerstrike-list >/dev/null 2>&1
systemctl restart lonerstrike-list
sleep 2
if python3 -c 'import urllib.request; [urllib.request.urlopen("http://127.0.0.1:8100" + p, timeout=5) for p in ("/servers", "/community/health")]'; then
  echo "The server list and community creations are running on 127.0.0.1:8100."
else
  echo "It didn't start: $(journalctl -u lonerstrike-list -n 3 --no-pager -o cat)"
  exit 1
fi
