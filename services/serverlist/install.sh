#!/bin/sh
# Puts the Loner-Strike server list on this machine, or updates it. Run it from this folder
# on the machine, as root or with sudo:
#   scp -r services/serverlist you@machine:/tmp/ && ssh you@machine 'sudo sh /tmp/serverlist/install.sh'
# It listens on 127.0.0.1:8100. Point a hostname at it: in Cloudflare's dashboard, add a
# public hostname to the machine's tunnel (like list.example.com, service http://localhost:8100).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
[ "$(id -u)" = 0 ] || { echo "Run it with sudo."; exit 1; }
[ -x /usr/bin/python3 ] || { echo "Python 3 isn't installed (apt install python3)."; exit 1; }
install -d -o root -g root -m 755 /opt/lonerstrike-list
install -o root -g root -m 644 "$here/serverlist.py" /opt/lonerstrike-list/serverlist.py
install -o root -g root -m 644 "$here/lonerstrike-list.service" /etc/systemd/system/lonerstrike-list.service
systemctl daemon-reload
systemctl enable lonerstrike-list >/dev/null 2>&1
systemctl restart lonerstrike-list
sleep 2
if python3 -c 'import urllib.request; urllib.request.urlopen("http://127.0.0.1:8100/servers", timeout=5)'; then
  echo "The server list is running on 127.0.0.1:8100."
else
  echo "It didn't start: $(journalctl -u lonerstrike-list -n 3 --no-pager -o cat)"
  exit 1
fi
