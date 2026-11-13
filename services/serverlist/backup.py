#!/usr/bin/env python3
"""Daily backup of the community database (and the server list), run by lonerstrike-list-backup.timer.

Takes a consistent copy while the service is running (SQLite's backup API, not a file copy that can
catch a write half done), checks the copy opens and passes SQLite's integrity check, and keeps the
newest KEEP_DAYS days. Backups live next to the database, in <state>/backups/, so they belong to
the service's own user. They are on the same machine: they cover mistakes and a broken database,
not the machine being lost (pull a copy off it now and then; see the runbook).
"""
import datetime
import os
import shutil
import sqlite3
import sys

KEEP_DAYS = 14


def backup(state_dir, keep_days=KEEP_DAYS, today=None):
    today = today or datetime.date.today()
    db = os.path.join(state_dir, "community.db")
    out = os.path.join(state_dir, "backups")
    os.makedirs(out, mode=0o700, exist_ok=True)
    stamp = today.isoformat()
    made = []
    if os.path.exists(db):
        dest = os.path.join(out, f"community-{stamp}.db")
        tmp = dest + ".part"
        if os.path.exists(tmp):
            os.remove(tmp)
        src = sqlite3.connect(db)  # same user as the service, so SQLite may touch its -wal/-shm files safely
        dst = sqlite3.connect(tmp)
        try:
            src.backup(dst)
            ok = dst.execute("PRAGMA integrity_check").fetchone()[0]
        finally:
            dst.close()
            src.close()
        if ok != "ok":
            os.remove(tmp)
            raise SystemExit(f"The backup copy failed SQLite's integrity check: {ok}")
        os.chmod(tmp, 0o600)
        os.replace(tmp, dest)
        made.append(dest)
    lst = os.path.join(state_dir, "list.json")
    if os.path.exists(lst):
        dest = os.path.join(out, f"list-{stamp}.json")
        shutil.copyfile(lst, dest)
        os.chmod(dest, 0o600)
        made.append(dest)
    cutoff = today - datetime.timedelta(days=keep_days)
    removed = []
    for name in sorted(os.listdir(out)):
        for prefix, suffix in (("community-", ".db"), ("list-", ".json")):
            if name.startswith(prefix) and name.endswith(suffix):
                try:
                    day = datetime.date.fromisoformat(name[len(prefix):-len(suffix)])
                except ValueError:
                    continue
                if day < cutoff:
                    os.remove(os.path.join(out, name))
                    removed.append(name)
    return made, removed


if __name__ == "__main__":
    state = sys.argv[1] if len(sys.argv) > 1 else "/var/lib/lonerstrike-list"
    made, removed = backup(state)
    for p in made:
        print(f"Backed up {os.path.basename(p)} ({os.path.getsize(p)} bytes).")
    if removed:
        print(f"Removed {len(removed)} backup(s) older than {KEEP_DAYS} days.")
    if not made:
        print("Nothing to back up yet (no community.db or list.json).")
