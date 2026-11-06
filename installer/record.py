"""Every file Loner-Strike writes into a game folder, so it can all be undone.

The record lives in <game>/addons/loner-strike/record.json. Before a file that
was already there (and not ours) is changed, a copy goes to
<game>/addons/loner-strike/backup/, and uninstalling puts it back.
"""

import json
import os
import shutil
import time

RECORD_DIR = os.path.join("addons", "loner-strike")


class Record:
    def __init__(self, game_dir, dry_run=False):
        self.game = game_dir
        self.dry = dry_run
        self.dir = os.path.join(game_dir, RECORD_DIR)
        self.path = os.path.join(self.dir, "record.json")
        try:
            with open(self.path, encoding="utf-8") as f:
                self.data = json.load(f)
        except (OSError, ValueError):
            self.data = {"files": {}, "components": {}}

    # ---- writing files --------------------------------------------------
    def _prepare(self, rel):
        """Backs up a file we're about to change, the first time we touch it."""
        rel = rel.replace("\\", "/")
        full = os.path.join(self.game, rel)
        if rel not in self.data["files"]:
            backup = None
            if os.path.exists(full):
                backup = "backup/" + rel
                if not self.dry:
                    dst = os.path.join(self.dir, backup)
                    os.makedirs(os.path.dirname(dst), exist_ok=True)
                    shutil.copy2(full, dst)
            self.data["files"][rel] = {"backup": backup, "components": [self.component]}
        else:
            # several components can ship the same file (ZP 4.3 and ZP 5 share sounds)
            owners = _owners(self.data["files"][rel])
            if self.component not in owners:
                self.data["files"][rel]["components"] = owners + [self.component]
                self.data["files"][rel].pop("component", None)
        if not self.dry:
            os.makedirs(os.path.dirname(full), exist_ok=True)
        return full

    def copy(self, src, rel):
        full = self._prepare(rel)
        if not self.dry:
            shutil.copy2(src, full)

    def write(self, rel, text, encoding="latin-1"):
        full = self._prepare(rel)
        if not self.dry:
            with open(full, "w", encoding=encoding, newline="") as f:
                f.write(text)

    def read(self, rel, encoding="latin-1"):
        try:
            with open(os.path.join(self.game, rel), encoding=encoding, newline="") as f:
                return f.read()
        except OSError:
            return None

    def exists(self, rel):
        return os.path.exists(os.path.join(self.game, rel))

    # ---- components -----------------------------------------------------
    component = None

    def begin(self, component):
        self.component = component

    def done(self, component, version=1):
        self.data["components"][component] = {"version": version, "date": int(time.time())}
        self.component = None
        self.save()

    def installed(self, component):
        return component in self.data["components"]

    def save(self):
        if self.dry:
            return
        os.makedirs(self.dir, exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(self.data, f, indent=1)
        os.replace(tmp, self.path)

    # ---- undoing --------------------------------------------------------
    def uninstall(self, components=None):
        """Removes our files (all, or only those of the given components) and restores backups."""
        keep = {}
        for rel, info in sorted(self.data["files"].items(), key=lambda x: -len(x[0])):
            if components is not None:
                left = [c for c in _owners(info) if c not in components]
                if left:                  # another component still needs it
                    info["components"] = left
                    info.pop("component", None)
                    keep[rel] = info
                    continue
            full = os.path.join(self.game, rel)
            if not self.dry:
                if info.get("backup"):
                    shutil.copy2(os.path.join(self.dir, info["backup"]), full)
                elif os.path.exists(full):
                    os.remove(full)
                    _prune(os.path.dirname(full), self.game)
        self.data["files"] = keep
        for c in list(self.data["components"]):
            if components is None or c in components:
                del self.data["components"][c]
        if components is None and not self.dry:
            shutil.rmtree(self.dir, ignore_errors=True)
            _remove_empty(os.path.join(self.game, "addons"))
        else:
            self.save()


def _owners(info):
    return list(info.get("components") or [info.get("component")])


def _remove_empty(root):
    """Removes empty folders left under root (bottom up), and root itself if empty."""
    for d, dirs, files in os.walk(root, topdown=False):
        try:
            os.rmdir(d)
        except OSError:
            pass


def _prune(d, stop):
    """Removes folders we emptied, up to the game folder."""
    while os.path.abspath(d) != os.path.abspath(stop):
        try:
            os.rmdir(d)
        except OSError:
            return
        d = os.path.dirname(d)
