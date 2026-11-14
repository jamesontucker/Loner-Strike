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
for f in serverlist.py community.py modbot.py lsmod.py formats.py formats_runner.py applications.py roles.py backup.py; do
  install -o root -g root -m 644 "$here/$f" "/opt/lonerstrike-list/$f"
done
install -o root -g root -m 755 "$here/set-env.sh" /opt/lonerstrike-list/set-env.sh
# the games' own files (creature packages may not ship them)
install -d -o root -g root -m 755 /opt/lonerstrike-list/stock
for f in stock_valve.txt stock_cstrike.txt stock_czero.txt; do
  install -o root -g root -m 644 "$here/stock/$f" "/opt/lonerstrike-list/stock/$f"
done
rm -f /opt/lonerstrike-list/r2-community.sh
install -d -o root -g root -m 700 /etc/lonerstrike-list
umask 077
if [ ! -f /etc/lonerstrike-list/community.env ]; then
  token=$(python3 -c 'import secrets; print(secrets.token_urlsafe(36))')
  publish=$(python3 -c 'import secrets; print(secrets.token_urlsafe(36))')
  sed -e "s|^COMMUNITY_ADMIN_TOKEN=.*|COMMUNITY_ADMIN_TOKEN=$token|" -e "s|^COMMUNITY_PUBLISH_TOKEN=.*|COMMUNITY_PUBLISH_TOKEN=$publish|" \
    "$here/community.env.example" > /etc/lonerstrike-list/community.env
  echo "Made /etc/lonerstrike-list/community.env (new admin and publish tokens)."
fi
chmod 600 /etc/lonerstrike-list/community.env
# publishing runs on GitHub Actions (R2 keys never on this machine): the earlier box timer goes
if [ -f /etc/systemd/system/lonerstrike-community-publish.timer ] || [ -f /etc/lonerstrike-list/publish.env ]; then
  systemctl disable --now lonerstrike-community-publish.timer >/dev/null 2>&1 || true
  rm -f /etc/systemd/system/lonerstrike-community-publish.timer /etc/systemd/system/lonerstrike-community-publish.service \
        /etc/lonerstrike-list/publish.env
  echo "Removed the old box publish timer and publish.env (publishing is a GitHub workflow now)."
fi
install -o root -g root -m 644 "$here/lonerstrike-list.service" /etc/systemd/system/lonerstrike-list.service
systemctl daemon-reload
systemctl enable lonerstrike-list >/dev/null 2>&1
systemctl restart lonerstrike-list
# daily backup of community.db and list.json into <state>/backups (14 days kept; same machine only)
install -o root -g root -m 644 "$here/lonerstrike-list-backup.service" /etc/systemd/system/lonerstrike-list-backup.service
install -o root -g root -m 644 "$here/lonerstrike-list-backup.timer" /etc/systemd/system/lonerstrike-list-backup.timer
systemctl daemon-reload
systemctl enable --now lonerstrike-list-backup.timer >/dev/null 2>&1
sleep 2
if python3 -c 'import urllib.request; [urllib.request.urlopen("http://127.0.0.1:8100" + p, timeout=5) for p in ("/servers", "/community/health")]'; then
  echo "The server list and community creations are running on 127.0.0.1:8100."
  if systemctl start lonerstrike-list-backup.service; then
    echo "Backup: $(journalctl -u lonerstrike-list-backup -n 20 --no-pager -o cat | grep -E "^(Backed up|Nothing)" | tail -2 | tr "\n" " ") Daily at about 04:30; kept 14 days."
  else
    echo "The backup didn't run: $(journalctl -u lonerstrike-list-backup -n 3 --no-pager -o cat)"
    exit 1
  fi
else
  echo "It didn't start: $(journalctl -u lonerstrike-list -n 3 --no-pager -o cat)"
  exit 1
fi
