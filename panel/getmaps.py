"""Get maps: browse GameBanana's Counter-Strike 1.6 maps and install them.

Only map content is copied into the game (the map, its textures, models,
sounds, sprites, sky and radar images). Anything that could run code or
change settings (.dll, .exe, .amxx, .cfg, plugins...) is skipped, and files
you already have are never replaced. Each install is recorded in
installed_maps.json so it can be uninstalled cleanly.
"""

import hashlib
import html
import json
import resource
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
import urllib.parse
import urllib.request

API = "https://gamebanana.com/apiv11"
MAPS_CATEGORY = 5474          # Counter-Strike 1.6 > Maps
PER_PAGE = 24
MAX_DOWNLOAD = 1536 * 1024 * 1024
MAX_UNPACKED = 3 * 1024 * 1024 * 1024     # stops "zip bombs" from filling the disk
# Map names end up in console commands (changelevel, votes), so only plain names are installed
SAFE_MAP = re.compile(r"^[A-Za-z0-9_.\-]{1,31}$")
USER_AGENT = "CZControlPanel/1.0 (local map browser)"
DOWNLOAD_HOSTS = ("gamebanana.com", "files.gamebanana.com")

# GameBanana map types, in the order the panel shows them
CATEGORIES = [
    (5474, "All maps"), (5484, "Bomb defusal"), (5493, "Hostage rescue"), (5483, "Deathmatch"),
    (5488, "Zombie"), (5510, "Zombie escape"), (5503, "Gun Game"), (5501, "Surf"), (5521, "Scout"),
    (5486, "AWP"), (5497, "Aim"), (5485, "Fight yard"), (5502, "Knife"), (5507, "Bunny hop"),
    (5504, "Climb"), (5492, "Deathrun"), (5489, "Jailbreak"), (5491, "Soccer Jam"),
    (5495, "Hide and seek"), (5513, "Vehicles"), (5496, "Fun"), (5506, "Map packs"),
]
SORTS = {"popular": "Generic_MostDownloaded", "new": "Generic_Newest", "liked": "Generic_MostLiked"}

# Where each kind of file belongs inside cstrike/, and which extensions are allowed there
CONTENT_DIRS = {
    "maps": {".bsp", ".nav", ".txt", ".res"},
    "gfx": {".tga", ".bmp"},
    "models": {".mdl"},
    "sound": {".wav", ".mp3"},
    "sprites": {".spr"},
    "overviews": {".txt", ".bmp", ".tga"},
}
ARCHIVES = (".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2")
SKY_SIDES = ("up", "dn", "lf", "rt", "ft", "bk")


def _get_json(path, params=None, timeout=20):
    url = API + path + ("?" + urllib.parse.urlencode(params) if params else "")
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def _image(media, size="_sFile530"):
    imgs = (media or {}).get("_aImages") or []
    if not imgs:
        return None
    i = imgs[0]
    return "%s/%s" % (i["_sBaseUrl"], i.get(size) or i["_sFile"])


def _plain(text, limit=1200):
    """GameBanana descriptions are HTML; the panel only shows text."""
    text = re.sub(r"(?i)<br\s*/?>|</p>|</li>|</h\d>", "\n", text or "")
    text = html.unescape(re.sub(r"<[^>]+>", "", text))
    text = re.sub(r"\n{3,}", "\n\n", re.sub(r"[ \t]+", " ", text)).strip()
    return text[:limit] + ("…" if len(text) > limit else "")


class MapStore:
    def __init__(self, game, here):
        self.game = game
        self.cstrike = os.path.join(game.hl, "cstrike")
        self.record_path = os.path.join(here, "installed_maps.json")
        self.shots = os.path.join(here, "cache", "gbshots")
        self.work = os.path.join(here, "cache", "downloads")
        os.makedirs(self.shots, exist_ok=True)
        self.lock = threading.Lock()
        self._lookups = threading.BoundedSemaphore(3)
        self.jobs = {}

    # ---- install record -------------------------------------------------
    def records(self):
        try:
            with open(self.record_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, ValueError):
            return {}

    def _save_records(self, rec):
        tmp = self.record_path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(rec, f, indent=1)
        os.replace(tmp, self.record_path)

    def authors(self):
        """map name -> author, for maps installed from here."""
        return {m.lower(): r["author"] for r in self.records().values() for m in r.get("maps", [])}

    def shot_for(self, name):
        p = os.path.join(self.shots, name.lower() + ".jpg")
        return p if os.path.exists(p) else None

    # ---- pictures for maps that came without one ------------------------
    def find_shot(self, name):
        """Looks for the map on GameBanana and saves its first screenshot.

        Only takes a page whose title is the map's own name, so a search for
        "aim_map" doesn't pick up "aim_map_usp". Misses are remembered for a
        month so the browser doesn't ask GameBanana again on every visit.
        """
        name = name.lower()
        if self.shot_for(name):
            return self.shot_for(name)
        missing_path = os.path.join(self.shots, "not_found.json")
        with self.lock:
            try:
                with open(missing_path) as f:
                    missing = json.load(f)
            except (OSError, ValueError):
                missing = {}
        if time.time() - missing.get(name, 0) < 30 * 86400:
            return None
        with self._lookups:   # a few at a time, the map grid asks for many at once
            image = None
            try:
                # CZ remakes (de_dust_cz) share the original map's page
                for want in [name] + ([name[:-3]] if name.endswith("_cz") else []):
                    d = _get_json("/Mod/Index", {"_nPage": 1, "_nPerpage": 40,
                                                  "_aFilters[Generic_Category]": MAPS_CATEGORY,
                                                  "_aFilters[Generic_Name]": "contains," + want})
                    key = re.sub(r"[^a-z0-9]", "", want)
                    for r in d.get("_aRecords", []):
                        words = re.split(r"[\s\[(,]+", r["_sName"].strip().lower())
                        if key in (re.sub(r"[^a-z0-9]", "", w) for w in words) or \
                                re.sub(r"[^a-z0-9]", "", r["_sName"].lower()) == key:
                            image = _image(r.get("_aPreviewMedia"))
                            if image:
                                break
                    if image:
                        break
                if image:
                    req = urllib.request.Request(image, headers={"User-Agent": USER_AGENT})
                    with urllib.request.urlopen(req, timeout=20) as resp:
                        data = resp.read(4 * 1024 * 1024)
                    with open(os.path.join(self.shots, name + ".jpg"), "wb") as f:
                        f.write(data)
                    return self.shot_for(name)
            except (OSError, ValueError, KeyError):
                return None   # offline or GameBanana busy: try again next time
        with self.lock:
            try:
                with open(missing_path) as f:
                    missing = json.load(f)
            except (OSError, ValueError):
                missing = {}
            missing[name] = time.time()
            with open(missing_path, "w") as f:
                json.dump(missing, f)
        return None

    # ---- browsing -------------------------------------------------------
    def browse(self, category=MAPS_CATEGORY, sort="popular", query="", page=1):
        if category not in {c for c, _ in CATEGORIES}:
            raise ValueError("unknown category")
        params = {
            "_nPage": max(1, min(int(page), 500)),
            "_nPerpage": PER_PAGE,
            "_aFilters[Generic_Category]": category,
            "_sSort": SORTS.get(sort, SORTS["popular"]),
        }
        query = query.strip()[:60]
        if query:
            params["_aFilters[Generic_Name]"] = "contains," + query
        d = _get_json("/Mod/Index", params)
        installed = self.records()
        have = {m.lower() for m in self._installed_map_names()}
        items = []
        for r in d.get("_aRecords", []):
            mid = str(r["_idRow"])
            guess = re.split(r"[\s\[(]", r["_sName"].strip())[0].lower()
            items.append({
                "id": r["_idRow"],
                "name": r["_sName"],
                "author": (r.get("_aSubmitter") or {}).get("_sName", ""),
                "type": (r.get("_aSubCategory") or {}).get("_sName", ""),
                "image": _image(r.get("_aPreviewMedia"), "_sFile220"),
                "likes": r.get("_nLikeCount", 0),
                "views": r.get("_nViewCount", 0),
                "added": r.get("_tsDateAdded", 0),
                "installed": mid in installed or guess in have,
            })
        meta = d.get("_aMetadata", {})
        return {"items": items, "total": meta.get("_nRecordCount", 0), "done": meta.get("_bIsComplete", True),
                "page": params["_nPage"], "categories": CATEGORIES}

    def details(self, mod_id):
        mod_id = int(mod_id)
        d = _get_json("/Mod/%d/ProfilePage" % mod_id)
        rec = self.records().get(str(mod_id))
        images = [("%s/%s" % (i["_sBaseUrl"], i.get("_sFile530") or i["_sFile"]))
                  for i in (d.get("_aPreviewMedia") or {}).get("_aImages", [])[:6]]
        files = [{
            "id": f["_idRow"], "file": f["_sFile"], "size": f.get("_nFilesize", 0),
            "downloads": f.get("_nDownloadCount", 0), "added": f.get("_tsDateAdded", 0),
            "supported": f["_sFile"].lower().endswith(ARCHIVES + (".bsp",)),
            "virus": (f.get("_sAvResult") or "").lower() not in ("", "clean"),
        } for f in d.get("_aFiles") or []]
        return {
            "id": mod_id, "name": d.get("_sName", ""), "author": (d.get("_aSubmitter") or {}).get("_sName", ""),
            "type": (d.get("_aCategory") or {}).get("_sName", ""), "text": _plain(d.get("_sText", "")),
            "images": images, "files": files, "downloads": d.get("_nDownloadCount", 0),
            "likes": d.get("_nLikeCount", 0), "url": d.get("_sProfileUrl", ""),
            "installed": rec,
        }

    def _installed_map_names(self):
        d = os.path.join(self.cstrike, "maps")
        names = [f[:-4] for f in os.listdir(d) if f.lower().endswith(".bsp")] if os.path.isdir(d) else []
        cz = os.path.join(self.game.cz, "maps")
        if os.path.isdir(cz):
            names += [f[:-4] for f in os.listdir(cz) if f.lower().endswith(".bsp")]
        return names

    # ---- installing -----------------------------------------------------
    def start_install(self, mod_id, file_id):
        mod_id, file_id = int(mod_id), int(file_id)
        with self.lock:
            for j in self.jobs.values():
                if j["state"] in ("queued", "downloading", "unpacking") and j["mod"] == mod_id:
                    return j["id"]
            job_id = "%d-%d-%d" % (mod_id, file_id, int(time.time()))
            self.jobs[job_id] = {"id": job_id, "mod": mod_id, "file": file_id, "state": "queued",
                                 "got": 0, "size": 0, "message": "Waiting", "maps": [], "kept": [], "skipped": []}
        threading.Thread(target=self._install, args=(job_id,), daemon=True).start()
        return job_id

    def job_list(self):
        with self.lock:
            # finished jobs are kept for ten minutes so the panel can show the result
            now = time.time()
            for k in [k for k, j in self.jobs.items() if j.get("ended") and now - j["ended"] > 600]:
                del self.jobs[k]
            return [dict(j) for j in self.jobs.values()]

    def _update(self, job_id, **kw):
        with self.lock:
            self.jobs[job_id].update(kw)

    def _install(self, job_id):
        job = self.jobs[job_id]
        tmp = None
        try:
            info = self.details(job["mod"])
            f = next((x for x in info["files"] if x["id"] == job["file"]), None)
            if not f:
                raise ValueError("That file is no longer on GameBanana.")
            if not f["supported"]:
                raise ValueError("%s isn't an archive the panel can open." % f["file"])
            if f["size"] > MAX_DOWNLOAD:
                raise ValueError("That download is over 1.5 GB.")
            os.makedirs(self.work, exist_ok=True)
            tmp = tempfile.mkdtemp(prefix="gb-", dir=self.work)
            archive = os.path.join(tmp, "download" + os.path.splitext(f["file"])[1].lower())
            self._download(job_id, job["file"], archive, f["size"])

            self._update(job_id, state="unpacking", message="Unpacking")
            src = os.path.join(tmp, "x")
            os.makedirs(src)
            if archive.endswith(".bsp"):
                shutil.move(archive, os.path.join(src, os.path.basename(f["file"])))
            else:
                self._extract(archive, src)
                self._extract_nested(src)

            self._update(job_id, state="unpacking", message="Copying map files")
            plan, skipped = self._plan(src)
            if not any(dest.lower().endswith(".bsp") for _, dest in plan):
                raise ValueError("No map (.bsp) found in that download.")
            installed, kept, maps, already, shared = [], [], [], [], []
            for path, dest in plan:
                full = os.path.join(self.cstrike, dest)
                if os.path.exists(full):
                    if dest.lower().endswith(".bsp"):
                        already.append(os.path.basename(dest)[:-4])
                    if not _same_file(path, full):
                        kept.append(dest)
                    else:
                        shared.append(dest)
                    continue
                os.makedirs(os.path.dirname(full), exist_ok=True)
                shutil.copy2(path, full)
                installed.append(dest)
                if dest.lower().endswith(".bsp"):
                    maps.append(os.path.basename(dest)[:-4])

            if maps or installed:
                self._save_shot(info, maps)
                with self.lock:
                    rec = self.records()
                    old = rec.get(str(job["mod"]), {})
                    rec[str(job["mod"])] = {
                        "name": info["name"], "author": info["author"] or "GameBanana", "url": info["url"],
                        "file": f["file"], "date": int(time.time()),
                        "maps": sorted(set(old.get("maps", []) + maps)),
                        "files": sorted(set(old.get("files", []) + installed)),
                        "shared": sorted(set(old.get("shared", []) + shared)),
                    }
                    self._save_records(rec)
                self.game.maps(refresh=True)
            if maps:
                msg = "Installed " + ", ".join(maps)
                if already:
                    msg += ". You already had " + ", ".join(already) + "."

            elif already:
                msg = "You already have " + ", ".join(already) + ". Nothing was replaced."
            else:
                msg = "Nothing new to install."
            self._update(job_id, state="done", message=msg, maps=maps, kept=kept,
                         skipped=skipped, ended=time.time())
        except Exception as e:  # report every failure to the panel instead of dying quietly
            self._update(job_id, state="error", message=str(e) or e.__class__.__name__, ended=time.time())
        finally:
            if tmp:
                shutil.rmtree(tmp, ignore_errors=True)

    def _download(self, job_id, file_id, out, size):
        url = "https://gamebanana.com/dl/%d" % file_id
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        self._update(job_id, state="downloading", message="Downloading", size=size)
        with urllib.request.urlopen(req, timeout=30) as r:
            host = urllib.parse.urlparse(r.geturl()).hostname or ""
            if not any(host == h or host.endswith("." + h) for h in DOWNLOAD_HOSTS):
                raise ValueError("The download was redirected somewhere unexpected (%s)." % host)
            got = 0
            with open(out, "wb") as f:
                while True:
                    chunk = r.read(256 * 1024)
                    if not chunk:
                        break
                    got += len(chunk)
                    if got > MAX_DOWNLOAD:
                        raise ValueError("That download is over 1.5 GB.")
                    f.write(chunk)
                    self._update(job_id, got=got)

    @staticmethod
    def _extract(archive, dest, budget=None):
        """Unpacks archive into dest, refusing more than budget bytes (default 3 GB)."""
        budget = MAX_UNPACKED if budget is None else budget
        if archive.endswith(".zip"):
            import zipfile
            try:
                with zipfile.ZipFile(archive) as z:
                    if sum(m.file_size for m in z.infolist()) > budget:
                        raise ValueError("That download unpacks to more than 3 GB.")
                    written = 0       # the sizes in a zip's index can lie, so count while writing too
                    for m in z.infolist():
                        name = m.filename.replace("\\", "/")
                        target = os.path.normpath(os.path.join(dest, name))
                        if not target.startswith(dest + os.sep) or m.is_dir():
                            continue
                        os.makedirs(os.path.dirname(target), exist_ok=True)
                        with z.open(m) as s, open(target, "wb") as t:
                            while True:
                                chunk = s.read(1024 * 1024)
                                if not chunk:
                                    break
                                written += len(chunk)
                                if written > budget:
                                    raise ValueError("That download unpacks to more than 3 GB.")
                                t.write(chunk)
                return
            except zipfile.BadZipFile:
                pass  # some ".zip" files are really rar; let bsdtar try
        # bsdtar (built into macOS) reads most rar, 7z and tarballs and refuses ../ paths.
        # A partial unpack counts as a failure: a map missing its models crashes the game.
        listing = subprocess.run(["/usr/bin/tar", "-tvf", archive], capture_output=True, text=True)
        total, links, readable = 0, False, listing.returncode == 0
        for line in listing.stdout.splitlines():
            cols = line.split()
            if len(cols) < 5 or not cols[4].isdigit():
                readable = False          # can't tell what's inside: don't let bsdtar unpack it
                break
            total += int(cols[4])
            links = links or line[:1] in ("l", "h")
        r = None
        if readable and not links:
            if total > budget:
                raise ValueError("That download unpacks to more than 3 GB.")
            r = _run_limited(["/usr/bin/tar", "-xf", archive, "-C", dest], dest, budget)
            _strip_links(dest)
            if r.returncode == 0:
                return
        unar = _tool("unar")
        lsar = _tool("lsar")
        if unar and lsar:
            entries = _lsar_entries(lsar, archive)
            if entries is not None:
                if any(e.get("XADIsLink") or e.get("XADIsHardLink") for e in entries):
                    raise ValueError("That download contains links to other files, so it wasn't unpacked.")
                if sum(int(e.get("XADFileSize") or 0) for e in entries) > budget:
                    raise ValueError("That download unpacks to more than 3 GB.")
                shutil.rmtree(dest, ignore_errors=True)
                os.makedirs(dest)
                r2 = _run_limited([unar, "-q", "-f", "-D", "-o", dest, archive], dest, budget)
                _strip_links(dest)
                if r2.returncode == 0:
                    return
        if readable and links:
            raise ValueError("That download contains links to other files, so it wasn't unpacked.")
        why = ((r.stderr if r else listing.stderr).strip().splitlines() or ["unknown error"])[-1]
        hint = "" if unar else " Installing unar (brew install unar) lets the panel open newer RAR files."
        raise ValueError("Couldn't fully unpack the download (%s).%s" % (why, hint))

    def _extract_nested(self, root):
        """Some uploads put the real archive inside the archive."""
        for path in list(_walk(root)):
            if path.lower().endswith(ARCHIVES):
                inner = tempfile.mkdtemp(prefix="nested-", dir=root)
                left = MAX_UNPACKED - _tree_size(root)
                if left <= 0:
                    raise ValueError("That download unpacks to more than 3 GB.")
                try:
                    self._extract(path, inner, left)
                except ValueError:
                    pass

    def _plan(self, root):
        """[(source file, path relative to cstrike/)] for every map file in the download."""
        plan, seen, skipped = [], set(), []
        bsps = {os.path.basename(p).lower()[:-4] for p in _walk(root) if p.lower().endswith(".bsp")}
        for path in _walk(root):
            rel = os.path.relpath(path, root).replace("\\", "/")
            parts = [p for p in rel.split("/") if p not in ("", ".")]
            low = [p.lower() for p in parts]
            name, ext = low[-1], os.path.splitext(low[-1])[1]
            dest = None
            # the outermost folder that takes this kind of file, so models/dx/maps/x.mdl
            # stays a model rather than counting as something under maps/
            idx = next((i for i, p in enumerate(low[:-1]) if p in CONTENT_DIRS and ext in CONTENT_DIRS[p]),
                       max((i for i, p in enumerate(low[:-1]) if p in CONTENT_DIRS), default=-1))
            if idx >= 0 and low[idx] == "maps" and ext in CONTENT_DIRS["maps"]:
                dest = "maps/" + parts[-1]       # the game only loads maps straight from maps/
            elif idx >= 0 and ext in CONTENT_DIRS[low[idx]]:
                dest = "/".join([low[idx]] + parts[idx + 1:])
            elif idx >= 0 and low[idx] == "maps" and ext == ".wad":
                dest = parts[-1]
            elif ext == ".wad":
                dest = parts[-1]
            elif ext in (".bsp", ".nav", ".res"):
                dest = "maps/" + parts[-1]
            elif ext == ".txt" and name[:-4] in bsps:
                dest = "maps/" + parts[-1]
            elif ext == ".spr":
                dest = "sprites/" + parts[-1]
            elif ext in (".tga", ".bmp") and name[:-4].endswith(SKY_SIDES):
                dest = "gfx/env/" + parts[-1]
            if dest is None:
                if not name.endswith(ARCHIVES):
                    skipped.append(rel)
                continue
            dest = os.path.normpath(dest).replace("\\", "/")
            if dest.startswith("..") or dest.lower() in seen:
                continue
            if dest.startswith("maps/") and not SAFE_MAP.match(os.path.splitext(parts[-1])[0]):
                skipped.append(rel + " (name has characters the game console can't take)")
                continue
            seen.add(dest.lower())
            plan.append((path, dest))
        return plan, skipped[:40]

    def _save_shot(self, info, maps):
        if not info["images"]:
            return
        try:
            req = urllib.request.Request(info["images"][0], headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=20) as r:
                data = r.read(4 * 1024 * 1024)
            for m in maps:
                with open(os.path.join(self.shots, m.lower() + ".jpg"), "wb") as f:
                    f.write(data)
        except OSError:
            pass  # a missing picture isn't worth failing the install over

    # ---- uninstalling ---------------------------------------------------
    def uninstall(self, mod_id):
        with self.lock:
            rec = self.records()
            r = rec.pop(str(int(mod_id)), None)
            if not r:
                raise ValueError("That download isn't installed from the panel.")
            # files another download also needs stay
            needed = {f.lower() for o in rec.values() for f in o.get("files", []) + o.get("shared", [])}
            for dest in r.get("files", []):
                if dest.lower() in needed:
                    continue
                full = os.path.normpath(os.path.join(self.cstrike, dest))
                if full.startswith(self.cstrike + os.sep) and os.path.isfile(full):
                    os.remove(full)
            for m in r.get("maps", []):
                # navigation the bots learned for it, and the cached picture
                for p in (os.path.join(self.cstrike, "maps", m + ".nav"),
                          os.path.join(self.game.cz, "maps", m + ".nav"),
                          os.path.join(self.shots, m.lower() + ".jpg")):
                    if os.path.isfile(p):
                        os.remove(p)
            self._save_records(rec)
        self.game.maps(refresh=True)
        return r.get("maps", [])


def _run_limited(cmd, dest, budget):
    """Runs an unpacker, stopping it as soon as dest grows past budget bytes.

    The listing checked beforehand can lie about sizes, so the folder is
    watched while the unpacker works, and no single file may exceed the budget.
    """
    def limit():
        resource.setrlimit(resource.RLIMIT_FSIZE, (budget, budget))
    # its own process group, so stopping it also stops anything it started
    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
                            preexec_fn=limit, start_new_session=True)
    try:
        while True:
            try:
                _, err = proc.communicate(timeout=0.5)
                break
            except subprocess.TimeoutExpired:
                if _tree_size(dest) > budget:
                    raise ValueError("That download unpacks to more than 3 GB.")
    finally:
        if proc.poll() is None:       # whatever went wrong, never leave it running
            try:
                os.killpg(proc.pid, 9)
            except OSError:
                proc.kill()
            proc.wait()
    if _tree_size(dest) > budget:
        raise ValueError("That download unpacks to more than 3 GB.")
    return subprocess.CompletedProcess(cmd, proc.returncode, "", err or "")


def _tool(name):
    return shutil.which(name) or next((p for p in ("/opt/homebrew/bin/" + name, "/usr/local/bin/" + name)
                                       if os.path.exists(p)), None)


def _lsar_entries(lsar, archive):
    """The archive's entries as lsar reports them, or None if it can't read the archive."""
    r = subprocess.run([lsar, "-j", archive], capture_output=True, text=True)
    try:
        return json.loads(r.stdout).get("lsarContents") if r.returncode == 0 else None
    except ValueError:
        return None


def _strip_links(root):
    """Removes symbolic links an archive brought along, so nothing points outside the download."""
    for d, dirs, files in os.walk(root):
        for n in dirs + files:
            p = os.path.join(d, n)
            if os.path.islink(p):
                os.unlink(p)


def _tree_size(root):
    total = 0
    for d, _, files in os.walk(root):
        for f in files:
            try:      # unpackers rename and remove temporary files as they go
                st = os.lstat(os.path.join(d, f))
            except OSError:
                continue
            total += st.st_size
    return total


def _walk(root):
    for d, dirs, files in os.walk(root):
        for f in files:
            p = os.path.join(d, f)
            if os.path.isfile(p) and not os.path.islink(p):
                yield p


def _same_file(a, b):
    if os.path.getsize(a) != os.path.getsize(b):
        return False
    h = []
    for p in (a, b):
        with open(p, "rb") as f:
            h.append(hashlib.md5(f.read()).digest())
    return h[0] == h[1]
