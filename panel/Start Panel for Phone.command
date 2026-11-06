#!/bin/sh
# Double-click to also reach the CZ Control Panel from your phone (same Wi-Fi).
# Open the link it prints on your phone. Close this window to stop phone access;
# the Mac panel at http://127.0.0.1:8027 keeps running either way.
cd "$(dirname "$0")" && exec python3 panel.py --lan --port 8028
