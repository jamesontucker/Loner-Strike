#!/usr/bin/env python3
# Loner-Strike community creations: the server side of the app's Community browser
# (docs/COMMUNITY.md, docs/COMMUNITY-SERVER.md). Part of the server list service
# (serverlist.py), turned on with --community-db.
#
#   POST /community/submit                      a registration from the app's Publish page
#   GET  /community/submission/<mod|spray>/<id> where a registration stands (listed, pending...)
#   POST /community/report                      a player's report from the app's Report button
#   GET  /community/health                      {"ok": true}
#   /community/admin/...                        moderation, admin token only (lsmod.py)
#
# A registration is never taken on the app's word: the GameBanana page is fetched again here
# (only from gamebanana.com's public API), and it must be in Counter-Strike 1.6, carry the tag
# "Software Used: LonerStrike", be the kind it says it is, and have the file with the given id
# and MD5, scanned clean. The package's file list is read too (its zip directory, by range
# requests to GameBanana's own file hosts): nothing that runs code may be in it. For 1.0 only
# content and Game Mode Studio modes (settings and graphs; the app makes their code itself)
# are shared, never plugins. Whatever passes is listed at once and gets a card in the team's
# private Discord channel with a [Pull] button (modbot.py); creators the team blocked wait for
# a moderator instead. Players' reports get cards too. The index (community/index.json on
# dl.lonerstrike.com) is built from what's listed; a daily re-check pulls pages that vanish or
# drop the tag.
#
# Standard library only, like serverlist.py. Nothing secret is logged (no tokens, no webhook
# addresses, no visitor addresses).
import hashlib
import hmac
import io
import json
import re
import sqlite3
import sys
import threading
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
import zipfile

SCHEMA = 1
CS16_GAME = 4254
TAG_TITLE = "Software Used"
TAG_VALUE = "LonerStrike"
TAG_ALSO = "Loner-Strike"
GB_HOST = "gamebanana.com"
GB_API = f"https://{GB_HOST}/apiv11"
USER_AGENT = "LonerStrike/1.0 (community server; +https://lonerstrike.com)"
KINDS = ("map", "mode", "prefab", "spray", "texture")
MODELS = {"mod": "Mod", "mods": "Mod", "spray": "Spray", "sprays": "Spray"}
# GameBanana's CS 1.6 categories (COMMUNITY.md section 1)
CAT_MAPS, CAT_PREFABS, CAT_TEXTURES, CAT_AMXX = 5474, 451, 681, 3521
SHOT_HOSTS = ("images.gamebanana.com", "dl.lonerstrike.com")
DISCORD_HOST = "discord.com"

MAX_BODY = 64 * 1024          # a registration's body
MAX_PAGE = 4 << 20            # a GameBanana answer
MAX_INDEX = 8 << 20           # the index the app reads (it refuses bigger)
MAX_ITEMS = 5000
MAX_SHOTS = 6
MAX_FILE = 1 << 30            # the package (Publish refuses bigger)
MAX_PENDING = 500             # waiting for a person, all together
MAX_REASON = 300
SUBMIT_PER_IP_HOUR = 10
SUBMIT_PER_SUBMITTER_DAY = 10
ADMIN_FAILS_PER_HOUR = 10
GB_FETCHES_PER_MIN = 30
MIN_TOKEN = 32
REPORTS_PER_IP_HOUR = 5
REPORTS_PER_DAY = 300         # all together; more and the button says "try later"
REPORT_REASONS = ("broken", "malware", "stolen", "offensive", "not-loner-strike", "other")
MAX_NOTE = 500
MAX_LISTING = 4 << 20         # bytes read to list a package's files
MAX_ENTRIES = 20000
PLUGIN_REFUSAL = "Plugin modes can't be shared yet. Make it in Game Mode Studio, which builds a mode's code on each player's computer."
# files that run code (or hide other files) never go out in a shared package
CODE_EXT = (".amxx", ".amx", ".sma", ".inc", ".dll", ".so", ".dylib", ".exe", ".com", ".scr", ".msi", ".bat", ".cmd",
            ".sh", ".ps1", ".py", ".pyc", ".js", ".vbs", ".jar", ".lua", ".pl", ".rb", ".php", ".app", ".command")
ARCHIVE_EXT = (".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2", ".xz", ".cab")
# a page's file may be one of these on its own (nothing to look inside)
SINGLE_OK = (".bsp", ".wad", ".bmp", ".tga", ".png", ".jpg", ".jpeg")
# a re-check that finds more than this share of pages gone changes nothing (GameBanana's API
# having a bad day, or changing shape) and tells the team instead
RECHECK_GONE_SHARE = 0.25

STATES = ("pending", "listed", "hidden", "rejected", "pulled")


def log(msg):
    print(f"community: {msg}", file=sys.stderr, flush=True)


def now():
    return int(time.time())


class Reject(Exception):
    """A refusal the app (or the moderator) is told in plain words."""

    def __init__(self, code, message):
        super().__init__(message)
        self.code = code
        self.message = message


# ---- input checks ---------------------------------------------------------------------------

_HEX32 = re.compile(r"^[0-9a-f]{32}$")
_HEX64 = re.compile(r"^[0-9a-f]{64}$")
_MODE_ID = re.compile(r"^[A-Za-z0-9_]{1,32}$")
_SAFE_ARG = re.compile(r"^[A-Za-z0-9_.$-]{1,64}$")


def one_line(s, n):
    """Text on one line, without control or direction-changing characters, at most n characters."""
    out = "".join(" " if unicodedata.category(c) in ("Cc", "Cf", "Cs", "Co", "Zl", "Zp") else c for c in s)
    return " ".join(out.split())[:n]


def safe_arg(s):
    """The app's maps::safe_arg: a name the game's console takes as one word."""
    return isinstance(s, str) and bool(_SAFE_ARG.match(s))


def _bad(field):
    raise Reject(400, f"The registration's {field} isn't right.")


def _str(d, k, n, required=False):
    v = d.get(k, "")
    if v is None:
        v = ""
    if not isinstance(v, str) or len(v) > n * 4:
        _bad(k)
    v = one_line(v, n)
    if required and not v:
        _bad(k)
    return v


def _int(d, k, lo, hi, default=0):
    v = d.get(k, default)
    if v is None:
        v = default
    if isinstance(v, bool) or not isinstance(v, int) or not lo <= v <= hi:
        _bad(k)
    return v


def _num(d, k, lo, hi, default=0.0, nullable=False):
    v = d.get(k, default)
    if v is None and nullable:
        return None
    if v is None:
        v = default
    if isinstance(v, bool) or not isinstance(v, (int, float)) or v != v or not lo <= v <= hi:
        _bad(k)
    return round(float(v), 3)


def _bool(d, k):
    v = d.get(k, False)
    if v is None:
        return False
    if not isinstance(v, bool):
        _bad(k)
    return v


def _list(d, k, n):
    v = d.get(k, [])
    if v is None:
        return []
    if not isinstance(v, list) or len(v) > n:
        _bad(k)
    return v


def _obj(d, k):
    v = d.get(k)
    if v is None:
        return None
    if not isinstance(v, dict):
        _bad(k)
    return v


def parse_model(s):
    return MODELS.get(s.lower()) if isinstance(s, str) else None


def key_of(model, gb):
    return f"{model.lower()}:{gb}"


def parse_key(text):
    """'mod:123', 'spray:45' or a GameBanana link: (model, id), or None."""
    t = (text or "").strip()
    m = re.match(r"^(mod|mods|spray|sprays):(\d{1,8})$", t, re.I)
    if not m:
        m = re.match(r"^(?:https?://)?(?:www\.)?gamebanana\.com/(mods|sprays)/(\d{1,8})(?:[/?#].*)?$", t, re.I)
    if not m:
        return None
    gb = int(m.group(2))
    return (parse_model(m.group(1)), gb) if gb > 0 else None


def clean_item(raw):
    """The item of a registration, checked field by field and typed exactly as the app's index
    reader wants it. Fields only this server may set (author, screenshots, review, featured,
    status, dates) are not taken from it. Refuses (400) anything that isn't right."""
    if not isinstance(raw, dict):
        _bad("item")
    kind = raw.get("kind")
    if kind not in KINDS:
        _bad("kind")
    model = parse_model(raw.get("model", "Mod"))
    if not model or (model == "Spray") != (kind == "spray"):
        _bad("model")
    it = {
        "kind": kind,
        "model": model,
        "gb": _int(raw, "gb", 1, 99_999_999),
        "file": _int(raw, "file", 1, 99_999_999),
        "md5": _str(raw, "md5", 32).lower(),
        "name": _str(raw, "name", 80, required=True),
        "summary": _str(raw, "summary", 400),
    }
    if not _HEX32.match(it["md5"]):
        _bad("md5")
    games = _list(raw, "games", 4)
    if any(g not in ("cstrike", "czero") for g in games):
        _bad("games")
    it["games"] = sorted(set(games)) or ["cstrike"]
    tags = []
    for t in _list(raw, "tags", 12):
        if not isinstance(t, str) or len(t) > 100:
            _bad("tags")
        t = one_line(t, 24)
        if t and t not in tags:
            tags.append(t)
    it["tags"] = tags

    m = _obj(raw, "map")
    if kind == "map" and m is None:
        _bad("map")
    if m is not None:
        if kind != "map":
            _bad("map")
        name = m.get("name")
        if not safe_arg(name) or len(name) > 31:
            _bad("map name")
        sha = _str(m, "bspSha256", 64).lower()
        if sha and not _HEX64.match(sha):
            _bad("bspSha256")
        modes = _list(m, "modes", 16)
        if any(not isinstance(x, str) or not _MODE_ID.match(x) for x in modes):
            _bad("modes")
        it["map"] = {"name": name, "bspSize": _int(m, "bspSize", 0, MAX_FILE), "bspSha256": sha,
                     "modes": list(dict.fromkeys(modes)),
                     **{f: _bool(m, f) for f in ("nav", "places", "lsv", "res", "overview")}}

    mo = _obj(raw, "mode")
    if kind == "mode" and mo is None:
        _bad("mode")
    if mo is not None:
        if kind != "mode":
            _bad("mode")
        mid = mo.get("id")
        if not isinstance(mid, str) or not _MODE_ID.match(mid):
            _bad("mode id")
        if _list(mo, "plugins", 64):
            raise Reject(422, PLUGIN_REFUSAL)
        maps = _list(mo, "maps", 32)
        if any(not safe_arg(x) for x in maps):
            _bad("mode maps")
        # plugins and their review stay in the format for later; for now never any
        it["mode"] = {"id": mid, "plugins": [], "review": "none", "maps": list(dict.fromkeys(maps))}

    h = _obj(raw, "health")
    if h is not None:
        verdict = h.get("verdict")
        if verdict not in ("good", "warn", "bad"):
            _bad("health verdict")
        mode = h.get("mode", "")
        if mode and (not isinstance(mode, str) or not _MODE_ID.match(mode)):
            _bad("health mode")
        it["health"] = {"verdict": verdict, "when": _int(h, "when", 0, 4_102_444_800), "mode": mode or "",
                        "minutes": _int(h, "minutes", 0, 10_000), "bots": _int(h, "bots", 0, 64),
                        "stuckPct": _num(h, "stuckPct", 0, 100), "falls": _int(h, "falls", 0, 1_000_000),
                        "killsPerBotMin": _num(h, "killsPerBotMin", 0, 1000),
                        "coveragePct": _num(h, "coveragePct", 0, 100, nullable=True)}
    return it


def parse_submission(body):
    """The app's POST body {"schema": 1, "app": "<version>", "item": {...}} -> (item, app)."""
    if len(body) > MAX_BODY:
        raise Reject(413, "The registration is too big.")
    try:
        d = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        raise Reject(400, "The registration couldn't be read.")
    if not isinstance(d, dict):
        raise Reject(400, "The registration couldn't be read.")
    schema = d.get("schema")
    if isinstance(schema, bool) or not isinstance(schema, int) or schema < 1:
        raise Reject(400, "The registration couldn't be read.")
    if schema > SCHEMA:
        raise Reject(400, "This registration is from a newer Loner-Strike than the list knows.")
    app = d.get("app", "")
    app = one_line(app, 32) if isinstance(app, str) else ""
    return clean_item(d.get("item")), app


# ---- GameBanana ---------------------------------------------------------------------------

class FetchError(Exception):
    pass


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    # a redirect could point anywhere: it's an error instead
    def redirect_request(self, *args, **kwargs):
        return None


_OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}), _NoRedirect())


def http_get(url, limit, hosts=(GB_HOST,), schemes=("https",), timeout=15):
    """(status, body) for a GET to one of `hosts` only (exact host[:port]), no redirects, at
    most `limit` bytes. FetchError when it couldn't be asked or answered junk."""
    u = urllib.parse.urlsplit(url)
    if u.scheme not in schemes or u.netloc not in hosts or "@" in u.netloc:
        raise FetchError("not an allowed address")
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"})
    try:
        r = _OPENER.open(req, timeout=timeout)
    except urllib.error.HTTPError as e:
        r = e
    except (OSError, ValueError) as e:
        raise FetchError(type(e).__name__)
    try:
        data = r.read(limit + 1)
    except OSError as e:
        raise FetchError(type(e).__name__)
    finally:
        r.close()
    if len(data) > limit:
        raise FetchError("too big")
    return r.status if hasattr(r, "status") else r.code, data


def page_url(model, gb):
    if model not in ("Mod", "Spray") or not isinstance(gb, int) or not 0 < gb < 100_000_000:
        raise FetchError("bad page")
    return f"{GB_API}/{model}/{gb}/ProfilePage"


_FILE_HOST = re.compile(r"^(files|filecache\d{1,3})\.gamebanana\.com$")


def file_url_ok(url):
    """A GameBanana download (gamebanana.com/dl/<id>) or one of its own file hosts it sends to."""
    u = urllib.parse.urlsplit(url)
    if u.scheme != "https" or "@" in u.netloc or u.port is not None:
        return False
    return (u.netloc == GB_HOST and re.match(r"^/dl/\d{1,9}$", u.path) is not None) or bool(_FILE_HOST.match(u.netloc))


def range_get(url, start, end, hops=4):
    """(bytes start..end of a GameBanana download, the address that answered). Redirects are
    followed only to GameBanana's own file hosts."""
    for _ in range(hops):
        if not file_url_ok(url):
            raise FetchError("not an allowed address")
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Range": f"bytes={start}-{end}"})
        try:
            r = _OPENER.open(req, timeout=20)
        except urllib.error.HTTPError as e:
            loc = e.headers.get("Location")
            e.close()
            if e.code in (301, 302, 303, 307, 308) and loc:
                url = urllib.parse.urljoin(url, loc)
                continue
            raise FetchError(f"status {e.code}")
        except (OSError, ValueError) as e:
            raise FetchError(type(e).__name__)
        with r:
            if r.status != 206:
                raise FetchError("the file host doesn't answer ranges")
            data = r.read(end - start + 2)
        if len(data) != end - start + 1:
            raise FetchError("short answer")
        return data, url
    raise FetchError("too many redirects")


class RangeFile(io.RawIOBase):
    """A remote file read by range requests, enough for zipfile to read the directory at its
    end without downloading the whole thing. Reads at most `budget` bytes in all."""

    def __init__(self, url, size, get=range_get, budget=MAX_LISTING, block=64 * 1024):
        self.url, self.size, self.get, self.budget, self.block = url, size, get, budget, block
        self.pos, self.cache, self.read_total = 0, (0, b""), 0

    def readable(self):
        return True

    def seekable(self):
        return True

    def tell(self):
        return self.pos

    def seek(self, off, whence=0):
        self.pos = max(0, {0: off, 1: self.pos + off, 2: self.size + off}[whence])
        return self.pos

    def read(self, n=-1):
        n = self.size - self.pos if n is None or n < 0 else min(n, self.size - self.pos)
        if n <= 0:
            return b""
        start, data = self.cache
        if not (start <= self.pos and self.pos + n <= start + len(data)):
            want = min(self.size, self.pos + max(n, self.block)) - 1
            self.read_total += want - self.pos + 1
            if self.read_total > self.budget:
                raise FetchError("the package's file list is too big")
            data, self.url = self.get(self.url, self.pos, want)
            start = self.pos
            self.cache = (start, data)
        out = data[self.pos - start:self.pos - start + n]
        self.pos += len(out)
        return out

    def readinto(self, b):
        d = self.read(len(b))
        b[:len(d)] = d
        return len(d)


def package_names(f, get=range_get):
    """The paths inside a page's file (the file itself when it's a single map, wad or image).
    Reject(422) when it can't be looked inside; FetchError on network trouble."""
    low = f["name"].lower()
    if low.endswith(SINGLE_OK):
        return [f["name"]]
    if not low.endswith(".zip"):
        raise Reject(422, "Upload the zip the app made (Publish), unchanged: the list can only look inside zips.")
    try:
        with zipfile.ZipFile(RangeFile(f"https://{GB_HOST}/dl/{f['id']}", f["size"], get)) as z:
            names = z.namelist()
    except (zipfile.BadZipFile, zipfile.LargeZipFile, EOFError, ValueError, NotImplementedError):
        raise Reject(422, "The zip on the page couldn't be read. Upload the zip the app made, unchanged.")
    if len(names) > MAX_ENTRIES:
        raise Reject(422, "The package has too many files.")
    return names


def code_files(names):
    """The paths in a package that run code or hide other files."""
    bad = []
    for n in names:
        low = n.replace("\\", "/").lower()
        if low.endswith("/"):
            continue
        base = low.rsplit("/", 1)[-1]
        if base.endswith(CODE_EXT) or base.endswith(ARCHIVE_EXT) or "addons/metamod/" in low or "/dlls/" in "/" + low:
            bad.append(n)
    return bad


def check_package(names):
    bad = code_files(names)
    if not bad:
        return
    if any(b.lower().endswith((".amxx", ".amx", ".sma", ".inc")) for b in bad):
        raise Reject(422, PLUGIN_REFUSAL)
    raise Reject(422, f"The package has files that run code or can't be checked ({', '.join(bad[:3])}). "
                      "Only maps, textures, models, sounds, sprays and Game Mode Studio settings can be shared.")


def fetch_page(model, gb, get=http_get):
    """("ok", page) / ("gone", reason) / ("error", reason). "gone" only on GameBanana's own
    clear answer (no such page, trashed, private, withheld), never on a network problem."""
    try:
        status, data = get(page_url(model, gb), MAX_PAGE)
    except (FetchError, OSError) as e:
        return "error", str(e) or type(e).__name__
    try:
        page = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        return "error", f"status {status}, not JSON"
    if not isinstance(page, dict):
        return "error", "not an object"
    if page.get("_sErrorCode") == "NO_SUCH_RECORD" or (status == 404 and "_sErrorCode" in page):
        return "gone", "The page isn't on GameBanana (any more)."
    if status != 200 or "_sErrorCode" in page:
        return "error", f"status {status}"
    if page.get("_idRow") != gb:
        return "error", "a different page answered"
    for flag, why in (("_bIsTrashed", "was deleted"), ("_bIsPrivate", "is private"), ("_bIsWithheld", "was withheld by GameBanana")):
        if page.get(flag) is True:
            return "gone", f"The page {why}."
    return "ok", page


def has_tag(tags):
    """The app's community::has_tag: "Software Used: LonerStrike" in either of GameBanana's shapes."""
    def ours(v):
        return v.strip().lower() in (TAG_VALUE.lower(), TAG_ALSO.lower())

    def want(t, v):
        return t.strip().lower() == TAG_TITLE.lower() and any(ours(x) for x in v.split(","))

    for t in tags if isinstance(tags, list) else []:
        if isinstance(t, str):
            a, sep, b = t.partition(":")
            if (want(a, b) if sep else ours(t)):
                return True
        elif isinstance(t, dict) and isinstance(t.get("_sTitle"), str) and isinstance(t.get("_sValue"), str):
            if want(t["_sTitle"], t["_sValue"]):
                return True
    return False


def _d(v):
    return v if isinstance(v, dict) else {}


def page_kind(page, model):
    """What a profile page is, from its item type and categories (as the app's kind_of)."""
    if model == "Spray":
        return "spray"
    cat, sup = _d(page.get("_aCategory")), _d(page.get("_aSuperCategory"))
    ids = {cat.get("_idRow"), sup.get("_idRow")}
    root = str(sup.get("_sName") or cat.get("_sName") or "").lower()
    sub = str(cat.get("_sName") or "").lower()
    if CAT_MAPS in ids or root == "maps":
        return "map"
    if CAT_PREFABS in ids or root == "prefabs":
        return "prefab"
    if CAT_TEXTURES in ids or root == "textures":
        return "texture"
    if CAT_AMXX in ids or "amxx" in sub:
        return "mode"
    return None


def page_files(page):
    out = []
    for f in page.get("_aFiles") or []:
        if isinstance(f, dict) and isinstance(f.get("_idRow"), int):
            out.append({"id": f["_idRow"], "name": str(f.get("_sFile") or ""),
                        "size": f.get("_nFilesize") if isinstance(f.get("_nFilesize"), int) else 0,
                        "md5": str(f.get("_sMd5Checksum") or "").lower(),
                        "av": str(f.get("_sAvResult") or "").lower(),
                        "avState": str(f.get("_sAvState") or "").lower()})
    return out


def page_shots(page):
    out = []
    for i in _d(page.get("_aPreviewMedia")).get("_aImages") or []:
        if not isinstance(i, dict):
            continue
        u = f"{i.get('_sBaseUrl', '')}/{i.get('_sFile', '')}"
        host = u[8:].split("/", 1)[0] if u.startswith("https://") else ""
        if host in SHOT_HOSTS and len(u) < 400 and not any(c in u for c in "\"<> \\"):
            out.append(u)
        if len(out) >= MAX_SHOTS:
            break
    return out


def check_page(page, model, gb, item):
    """Checks a profile page against a registration. Returns the page's facts
    (submitter, author name, screenshots, the file); Reject(422) with what to fix."""
    if _d(page.get("_aGame")).get("_idRow") != CS16_GAME:
        raise Reject(422, "That page isn't in GameBanana's Counter-Strike 1.6 section.")
    if page.get("_bIsFlagged"):
        raise Reject(422, "GameBanana flagged that page.")
    if not has_tag(page.get("_aTags")):
        raise Reject(422, f'Add the tag "{TAG_TITLE}: {TAG_VALUE}" to the page (Edit > Attributes), then try again.')
    kind = page_kind(page, model)
    if kind != item["kind"]:
        where = {"map": "a Maps category", "prefab": "Prefabs", "texture": "Textures",
                 "mode": "Resources > AMXX Plugins & Mods", "spray": "Sprays"}[item["kind"]]
        raise Reject(422, f"The page is in the wrong category for a {item['kind']}: put it in {where}.")
    f = next((f for f in page_files(page) if f["id"] == item["file"]), None)
    if not f or f["md5"] != item["md5"]:
        raise Reject(422, "None of the page's files is the package the app made. Upload that zip unchanged, then try again.")
    if f["av"] != "clean":
        if f["av"] in ("", "pending") or f["avState"] not in ("", "done"):
            raise Reject(422, "GameBanana is still checking the file. Try again in a few minutes.")
        raise Reject(422, "GameBanana's virus check didn't pass the file.")
    if not 0 < f["size"] <= MAX_FILE:
        raise Reject(422, "The file on the page is empty or too big.")
    sub = _d(page.get("_aSubmitter"))
    sid = sub.get("_idRow") if isinstance(sub.get("_idRow"), int) else 0
    name = one_line(str(sub.get("_sName") or ""), 40)
    if sid <= 0 or not name:
        raise Reject(422, "GameBanana didn't say who made the page. Try again in a minute.")
    return {"submitterId": sid, "author": name, "screenshots": page_shots(page), "file": f}


# ---- rate limits ----------------------------------------------------------------------------

class Window:
    """At most `n` hits per `secs` per key, in memory (lost on restart, which is fine)."""

    def __init__(self, n, secs):
        self.n, self.secs = n, secs
        self.lock = threading.Lock()
        self.hits = {}

    def allow(self, who, t=None):
        t = time.time() if t is None else t
        with self.lock:
            recent = [x for x in self.hits.get(who, []) if t - x < self.secs]
            ok = len(recent) < self.n
            if ok:
                recent.append(t)
            self.hits[who] = recent
            if len(self.hits) > 10000:
                self.hits = {k: v for k, v in self.hits.items() if v and t - v[-1] < self.secs}
            return ok


# ---- Discord --------------------------------------------------------------------------------

def webhook_ok(url):
    u = urllib.parse.urlsplit(url or "")
    return u.scheme == "https" and u.netloc == DISCORD_HOST and u.path.startswith("/api/webhooks/") and not u.query and not u.fragment


def discord_text(s, n=120):
    """Outside text (a page's name, a report's note) made harmless in a Discord message: one
    line, capped, no markup, no mentions (allowed_mentions is empty as well)."""
    s = re.sub(r"([\\*_~`|>\[\]()#<:-])", r"\\\1", one_line(s, n))
    return s.replace("@", "@​")


class Notifier:
    """Posts to the team's moderation channel through a webhook; off without one. Never blocks
    the caller, never logs the address."""

    def __init__(self, url, post=None):
        self.url = url if webhook_ok(url) else ""
        if url and not self.url:
            log("the Discord webhook isn't a discord.com webhook address; notifications are off")
        self._post = post or self._send

    def _send(self, url, body):
        req = urllib.request.Request(url, data=json.dumps(body).encode(), method="POST",
                                     headers={"Content-Type": "application/json",
                                              "User-Agent": "DiscordBot (https://lonerstrike.com, 1)"})
        try:
            with _OPENER.open(req, timeout=10) as r:
                return r.status
        except urllib.error.HTTPError as e:
            return e.code
        except (OSError, ValueError):
            return 0

    def card(self, kind, key, text, report_id=None):
        """A moderation card (listing, report, queue); a plain message here, with buttons in modbot.Mod."""
        self.say(text)

    def say(self, text, wait=False):
        if not self.url:
            return
        body = {"content": text[:1900], "allowed_mentions": {"parse": []}, "username": "Loner-Strike creations"}

        def go():
            code = self._post(self.url, body)
            if code not in (200, 204):
                log(f"Discord didn't take the message (status {code})")
        if wait:
            go()
        else:
            threading.Thread(target=go, daemon=True).start()


def gb_link(model, gb):
    return f"https://gamebanana.com/{'sprays' if model == 'Spray' else 'mods'}/{gb}"


# ---- the store ------------------------------------------------------------------------------

MIGRATIONS = [
    # 1: entries, the moderation log, settings, reports
    """
    CREATE TABLE entries (
        key TEXT PRIMARY KEY,
        model TEXT NOT NULL,
        gb INTEGER NOT NULL,
        kind TEXT NOT NULL DEFAULT '',
        state TEXT NOT NULL,
        item TEXT,
        proposed TEXT,
        rejected_md5 TEXT NOT NULL DEFAULT '',
        featured INTEGER NOT NULL DEFAULT 0,
        reason TEXT NOT NULL DEFAULT '',
        submitter_id INTEGER NOT NULL DEFAULT 0,
        submitter TEXT NOT NULL DEFAULT '',
        app TEXT NOT NULL DEFAULT '',
        added INTEGER NOT NULL,
        updated INTEGER NOT NULL,
        checked INTEGER NOT NULL DEFAULT 0,
        check_note TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX entries_state ON entries(state);
    CREATE TABLE log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        key TEXT NOT NULL,
        at INTEGER NOT NULL,
        action TEXT NOT NULL,
        detail TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX log_key ON log(key);
    CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT NOT NULL);
    CREATE TABLE reports (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        key TEXT NOT NULL,
        at INTEGER NOT NULL,
        reason TEXT NOT NULL,
        note TEXT NOT NULL DEFAULT '',
        reporter TEXT NOT NULL DEFAULT '',
        app TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX reports_key ON reports(key);
    CREATE TABLE blocked (
        submitter_id INTEGER PRIMARY KEY,
        name TEXT NOT NULL DEFAULT '',
        at INTEGER NOT NULL,
        by TEXT NOT NULL DEFAULT '',
        reason TEXT NOT NULL DEFAULT ''
    );
    CREATE TABLE cards (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL,
        key TEXT NOT NULL,
        report_id INTEGER,
        msg_id TEXT NOT NULL DEFAULT '',
        action TEXT NOT NULL DEFAULT '',
        actor TEXT NOT NULL DEFAULT '',
        actor_name TEXT NOT NULL DEFAULT '',
        reason TEXT NOT NULL DEFAULT '',
        before TEXT NOT NULL DEFAULT '',
        after_hash TEXT NOT NULL DEFAULT '',
        at INTEGER NOT NULL
    );
    CREATE TABLE interactions (id TEXT PRIMARY KEY, at INTEGER NOT NULL);
    """,
    # 2: who took an entry out of the list ('team' | 'recheck' | ''), recorded instead of guessed.
    # Rows already hidden, pulled or turned down count as the team's: only a moderator lifts them.
    """
    ALTER TABLE entries ADD COLUMN hidden_by TEXT NOT NULL DEFAULT '';
    UPDATE entries SET hidden_by = 'team' WHERE state IN ('hidden', 'pulled', 'rejected')
    """,
]

# Every way an entry's state changes, and who may do it (a creator's registration never undoes a
# moderator's pull, hide, block or turning down; tests in test_community.Transitions):
#   creator registers  new page             -> listed (blocked creator: pending)
#                      listed               -> listed, new file (blocked: stays listed, file waits)
#                      hidden by re-check   -> listed again, if the page passes every check now
#                      hidden by the team   -> stays hidden; the new file waits for a moderator
#                      rejected by re-check -> listed (blocked: pending)
#                      rejected by the team -> pending (a moderator looks at the new file)
#                      same file as rejected, or pulled by anyone -> refused (409), no change
#   moderator          approve (pending/waiting file) -> listed; restore (hidden/pulled) -> listed;
#                      hide -> hidden; pull -> pulled; reject -> rejected (or drops a waiting file);
#                      block/unblock a creator; Undo puts back the exact row before the action
#   re-check           gone/untagged/flagged -> pulled (waiting: rejected); listed file replaced
#                      -> hidden; a team hold stays marked as the team's


class Store:
    def __init__(self, path):
        self.lock = threading.RLock()
        self.db = sqlite3.connect(path, check_same_thread=False, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA busy_timeout=5000")
        self.migrate()

    def migrate(self):
        """Brings the database up to the newest version (PRAGMA user_version), one step at a time."""
        with self.lock:
            have = self.db.execute("PRAGMA user_version").fetchone()[0]
            for n, sql in enumerate(MIGRATIONS[have:], start=have + 1):
                self.db.execute("BEGIN")
                try:
                    for stmt in [x for x in sql.split(";") if x.strip()]:
                        self.db.execute(stmt)
                    self.db.execute(f"PRAGMA user_version={n}")
                    self.db.execute("COMMIT")
                except Exception:
                    self.db.execute("ROLLBACK")
                    raise
                log(f"database at version {n}")

    def get(self, key):
        with self.lock:
            r = self.db.execute("SELECT * FROM entries WHERE key=?", (key,)).fetchone()
        return dict(r) if r else None

    def rows(self, states=None):
        with self.lock:
            if states:
                q = f"SELECT * FROM entries WHERE state IN ({','.join('?' * len(states))}) ORDER BY updated DESC"
                return [dict(r) for r in self.db.execute(q, tuple(states))]
            return [dict(r) for r in self.db.execute("SELECT * FROM entries ORDER BY updated DESC")]

    def count(self, state):
        with self.lock:
            return self.db.execute("SELECT COUNT(*) FROM entries WHERE state=?", (state,)).fetchone()[0]

    def put(self, row, action, detail=""):
        """Writes a whole entry and a log line in one go."""
        cols = list(row)
        with self.lock:
            self.db.execute("BEGIN")
            try:
                self.db.execute(f"INSERT OR REPLACE INTO entries ({','.join(cols)}) VALUES ({','.join('?' * len(cols))})",
                                tuple(row[c] for c in cols))
                self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)", (row["key"], now(), action, detail[:500]))
                self.db.execute("COMMIT")
            except Exception:
                self.db.execute("ROLLBACK")
                raise

    def touch(self, key, checked, note):
        with self.lock:
            self.db.execute("UPDATE entries SET checked=?, check_note=? WHERE key=?", (checked, note[:200], key))

    def history(self, key, n=50):
        with self.lock:
            return [dict(r) for r in self.db.execute("SELECT at, action, detail FROM log WHERE key=? ORDER BY id DESC LIMIT ?", (key, n))]

    def meta(self, k, default=None):
        with self.lock:
            r = self.db.execute("SELECT v FROM meta WHERE k=?", (k,)).fetchone()
        return json.loads(r[0]) if r else default

    def set_meta(self, k, v):
        with self.lock:
            self.db.execute("INSERT OR REPLACE INTO meta (k, v) VALUES (?,?)", (k, json.dumps(v)))

    def last_change(self):
        with self.lock:
            r = self.db.execute("SELECT MAX(at) FROM log").fetchone()[0]
        return r or 0

    def add_report(self, key, reason, note, reporter, app):
        with self.lock:
            cur = self.db.execute("INSERT INTO reports (key, at, reason, note, reporter, app) VALUES (?,?,?,?,?,?)",
                                  (key, now(), reason, note, reporter, app))
            # reports are kept 90 days
            self.db.execute("DELETE FROM reports WHERE at < ?", (now() - 90 * 86400,))
            return cur.lastrowid

    def report(self, rid):
        with self.lock:
            r = self.db.execute("SELECT * FROM reports WHERE id=?", (rid,)).fetchone()
        return dict(r) if r else None

    # -- blocked creators --

    def is_blocked(self, submitter_id):
        with self.lock:
            return self.db.execute("SELECT 1 FROM blocked WHERE submitter_id=?", (submitter_id,)).fetchone() is not None

    def set_blocked(self, submitter_id, name, by, reason, on):
        with self.lock:
            self.db.execute("BEGIN")
            if on:
                self.db.execute("INSERT OR REPLACE INTO blocked (submitter_id, name, at, by, reason) VALUES (?,?,?,?,?)",
                                (submitter_id, name, now(), by, reason))
            else:
                self.db.execute("DELETE FROM blocked WHERE submitter_id=?", (submitter_id,))
            self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)",
                            (f"member:{submitter_id}", now(), "block" if on else "unblock", f"{name} by {by}: {reason}"[:500]))
            self.db.execute("COMMIT")

    def blocked(self):
        with self.lock:
            return [dict(r) for r in self.db.execute("SELECT * FROM blocked ORDER BY at DESC")]

    # -- Discord cards --

    def new_card(self, kind, key, report_id=None):
        with self.lock:
            return self.db.execute("INSERT INTO cards (kind, key, report_id, at) VALUES (?,?,?,?)", (kind, key, report_id, now())).lastrowid

    def card(self, card_id):
        with self.lock:
            r = self.db.execute("SELECT * FROM cards WHERE id=?", (card_id,)).fetchone()
        return dict(r) if r else None

    def update_card(self, card_id, **cols):
        with self.lock:
            self.db.execute(f"UPDATE cards SET {', '.join(f'{k}=?' for k in cols)} WHERE id=?", (*cols.values(), card_id))

    def first_time(self, interaction_id):
        """True the first time an interaction id is seen (Discord may deliver one twice)."""
        with self.lock:
            self.db.execute("DELETE FROM interactions WHERE at < ?", (now() - 86400,))
            try:
                self.db.execute("INSERT INTO interactions (id, at) VALUES (?,?)", (interaction_id, now()))
                return True
            except sqlite3.IntegrityError:
                return False

    def restore(self, key, row, why):
        """Puts an entry back as it was (Undo); no row: the entry didn't exist."""
        if row:
            row = dict(row, updated=now())
            self.put(row, "undo", why)
            return
        with self.lock:
            self.db.execute("BEGIN")
            self.db.execute("DELETE FROM entries WHERE key=?", (key,))
            self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)", (key, now(), "undo", why[:500]))
            self.db.execute("COMMIT")

    def reports(self, key=None, n=100):
        with self.lock:
            if key:
                q = self.db.execute("SELECT * FROM reports WHERE key=? ORDER BY id DESC LIMIT ?", (key, n))
            else:
                q = self.db.execute("SELECT * FROM reports ORDER BY id DESC LIMIT ?", (n,))
            return [dict(r) for r in q]

    def report_count(self, key, since):
        with self.lock:
            return self.db.execute("SELECT COUNT(*) FROM reports WHERE key=? AND at>=?", (key, since)).fetchone()[0]


def new_row(model, gb, t):
    return {"key": key_of(model, gb), "model": model, "gb": gb, "kind": "", "state": "pending", "item": None,
            "proposed": None, "rejected_md5": "", "featured": 0, "reason": "",
            "submitter_id": 0, "submitter": "", "app": "", "added": t, "updated": t, "checked": 0, "check_note": "",
            "hidden_by": ""}


def _j(s):
    return json.loads(s) if s else None


def row_hash(row):
    """What an entry is, without the re-check's bookkeeping (to tell whether it changed since)."""
    if not row:
        return "none"
    keep = {k: v for k, v in row.items() if k not in ("checked", "check_note")}
    return hashlib.sha256(json.dumps(keep, sort_keys=True).encode()).hexdigest()


# ---- the index ------------------------------------------------------------------------------

def index_item(row):
    """One index entry in exactly the app's schema (community.rs Item): every field with the
    type the app's reader wants (a wrong type or an unknown enum value would make it drop the
    whole list)."""
    it = _j(row["item"])
    status = {"listed": "listed", "hidden": "hidden", "pulled": "revoked"}[row["state"]]
    out = {"kind": it["kind"], "model": row["model"], "gb": int(row["gb"]), "file": int(it.get("file", 0)),
           "md5": it.get("md5", ""), "name": it["name"], "author": it.get("author", ""),
           "summary": it.get("summary", ""), "games": list(it.get("games") or ["cstrike"])}
    if it.get("map"):
        out["map"] = it["map"]
    if it.get("mode"):
        # plugins and review stay in the format for a later version; none are shared for now
        out["mode"] = dict(it["mode"], plugins=[], review="none")
    if it.get("health"):
        out["health"] = it["health"]
    out.update(screenshots=list(it.get("screenshots") or [])[:MAX_SHOTS], tags=list(it.get("tags") or []),
               featured=bool(row["featured"]), added=int(row["added"]), updated=int(row["updated"]), status=status)
    return out


def build_index(store):
    """community/index.json as bytes (UTF-8, compact) and its sha256. The same database gives
    the same bytes, so an unchanged index isn't uploaded again."""
    items, revoked = [], set()
    for row in store.rows(("listed", "hidden", "pulled")):
        if row["state"] == "pulled":
            revoked.add(row["key"])
        if row["item"]:
            items.append(index_item(row))
    # what people see first; pulled entries last (they're also in "revoked", which is what counts)
    items.sort(key=lambda i: (i["status"] == "revoked", not i["featured"], -i["updated"], i["model"], i["gb"]))
    items = items[:MAX_ITEMS]
    doc = {"schema": SCHEMA, "updated": store.last_change(), "revoked": sorted(revoked),
           "verifiedAuthors": [], "items": items}
    data = json.dumps(doc, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if len(data) > MAX_INDEX:
        raise Reject(507, f"The index would be {len(data)} bytes; the app reads at most {MAX_INDEX}.")
    return data, hashlib.sha256(data).hexdigest()


# ---- the service ----------------------------------------------------------------------------

class Community:
    def __init__(self, store, token="", notifier=None, get=http_get, get_range=range_get, admin_local=False):
        self.store = store
        self.token_digest = hashlib.sha256(token.encode()).digest() if len(token) >= MIN_TOKEN else None
        if token and not self.token_digest:
            log(f"the admin token is shorter than {MIN_TOKEN} characters; moderation is off")
        self.notifier = notifier or Notifier("")
        self.get = get
        self.get_range = get_range
        self.admin_local = admin_local
        self.ip_limit = Window(SUBMIT_PER_IP_HOUR, 3600)
        self.submitter_limit = Window(SUBMIT_PER_SUBMITTER_DAY, 86400)
        self.admin_fails = Window(ADMIN_FAILS_PER_HOUR, 3600)
        self.gb_limit = Window(GB_FETCHES_PER_MIN, 60)
        self.report_ip = Window(REPORTS_PER_IP_HOUR, 3600)
        self.report_same = Window(1, 86400)
        self.report_all = Window(REPORTS_PER_DAY, 86400)
        self.report_said = Window(1, 1800)
        # reporters are told apart in the mod channel by a hash of their address with a salt that
        # lives only in memory: the same person reads the same for a while, nobody can reverse it
        self.salt = hashlib.sha256(str(time.time_ns()).encode() + bytes(16)).digest()
        self.recheck_lock = threading.Lock()

    def reporter(self, ip):
        return hmac.new(self.salt, ip.encode(), hashlib.sha256).hexdigest()[:8]

    # -- registrations --

    def _verify(self, model, gb, item):
        """The page's facts after every automatic check (page, tag, kind, file, package contents)."""
        got, page = fetch_page(model, gb, self.get)
        if got == "gone":
            raise Reject(422, page)
        if got == "error":
            log(f"GameBanana didn't answer for {key_of(model, gb)}: {page}")
            raise Reject(502, "GameBanana didn't answer. Try again in a few minutes.")
        facts = check_page(page, model, gb, item)
        try:
            names = package_names(facts["file"], self.get_range)
        except FetchError as e:
            log(f"couldn't list the package of {key_of(model, gb)}: {e}")
            raise Reject(502, "GameBanana's file server didn't answer. Try again in a few minutes.")
        check_package(names)
        facts["files"] = len(names)
        return facts

    def submit(self, body, ip):
        """(status, answer) for the app's POST /community/submit."""
        if not self.ip_limit.allow(ip):
            return 429, {"error": "Too many registrations from here. Try again in an hour."}
        try:
            item, app = parse_submission(body)
            model, gb = item["model"], item["gb"]
            key = key_of(model, gb)
            row = self.store.get(key)
            if row:
                if row["state"] == "pulled":
                    raise Reject(409, "The Loner-Strike team pulled this page, so it can't be registered again. Ask in the Discord.")
                same = [x for x in (_j(row["proposed"]), _j(row["item"])) if x and x.get("md5") == item["md5"]]
                if row["state"] == "rejected" and row["hidden_by"] == "team" and row["rejected_md5"] == item["md5"]:
                    raise Reject(409, f"The Loner-Strike team turned this file down: {row['reason'] or 'no reason given'}. Upload a new version to try again.")
                if same and row["state"] in ("pending", "listed", "hidden"):
                    return 200, {"ok": True, "key": key, "state": row["state"]}
            if not self.gb_limit.allow("gb"):
                raise Reject(503, "The list is busy. Try again in a few minutes.")
            facts = self._verify(model, gb, item)
            if not self.submitter_limit.allow(facts["submitterId"]):
                raise Reject(429, "Too many registrations from this GameBanana member today. Try again tomorrow.")
            # creators the team blocked wait for a moderator, and so does a new file for a page the
            # team hid or turned down; everyone else is listed at once
            team_hold = bool(row) and row["hidden_by"] == "team"
            blocked = self.store.is_blocked(facts["submitterId"])
            look = blocked or team_hold
            if look and not (row and row["proposed"]) and self.store.count("pending") >= MAX_PENDING:
                raise Reject(503, "The list has too many creations waiting for a look. Try again in a day or two.")
        except Reject as e:
            return e.code, {"error": e.message}
        t = now()
        item.update(author=facts["author"], screenshots=facts["screenshots"], featured=False, status="listed")
        fresh = row is None
        row = row or new_row(model, gb, t)
        was = row["state"]
        row.update(kind=item["kind"], submitter_id=facts["submitterId"], submitter=facts["author"], app=app, updated=t)
        data = json.dumps(item, ensure_ascii=False)
        if look:
            # the team's reason and hold stay as they are; the file waits for a moderator
            row["proposed"] = data
            if was not in ("listed", "hidden"):
                row["state"] = "pending"
            action = "submit"
        else:
            # new, listed, or out of the list only by the re-check (and passing every check now)
            row.update(item=data, proposed=None, rejected_md5="", state="listed", hidden_by="", reason="", check_note="")
            action = "list"
        self.store.put(row, action, f"md5 {item['md5']} file {item['file']} app {app}")
        log(f"registration {key} {row['state']} ({'new' if fresh else 'update'})")
        self.notify_new(key, item, look, not fresh and was in ("listed", "hidden"), fresh,
                        why="blocked creator" if look and blocked else f"{was} by the team" if look else "")
        return 200, {"ok": True, "key": key, "state": "pending" if look else row["state"]}

    def notify_new(self, key, item, look, update, fresh=True, why=""):
        head = (f"**Waiting for a moderator ({why}){' (update)' if update else ''}:**" if look
                else f"**Listed {'update' if update else item['kind']}:**")
        lines = [f"{head} {discord_text(item['name'])} by {discord_text(item['author'])}",
                 f"<{gb_link(item['model'], item['gb'])}>"]
        if look:
            lines.append(f"Approve: `lsmod.py approve {key}`  ·  Turn down: `lsmod.py reject {key} --reason \"...\"`")
        else:
            lines.append(f"Passed the automatic checks. If it shouldn't be there: `lsmod.py pull {key} --reason \"...\"`, "
                         "then `lsmod.py publish --upload`.")
        self.notifier.card("queue" if look else "listing", key, "\n".join(lines))

    def status(self, model, gb):
        row = self.store.get(key_of(model, gb))
        if not row:
            return {"key": key_of(model, gb), "state": "unknown"}
        out = {"key": row["key"], "state": row["state"], "updated": row["updated"]}
        approved, proposed = _j(row["item"]), _j(row["proposed"])
        if approved:
            out["md5"] = approved.get("md5", "")
        if proposed:
            out["waiting"] = proposed.get("md5", "")
        if row["state"] in ("rejected", "pulled", "hidden") and row["reason"]:
            out["reason"] = row["reason"]
        return out

    # -- reports --

    def report(self, body, ip):
        """(status, answer) for the app's POST /community/report: a player says something's wrong
        with a creation. Kept 90 days with a reason, an optional note and the reporter's hash."""
        if not self.report_ip.allow(ip):
            return 429, {"error": "Too many reports from here. Try again in an hour."}
        try:
            if len(body) > 4096:
                raise Reject(413, "The report is too long.")
            try:
                d = json.loads(body.decode("utf-8"))
            except (UnicodeDecodeError, ValueError):
                raise Reject(400, "The report couldn't be read.")
            if not isinstance(d, dict) or d.get("schema") != 1:
                raise Reject(400, "The report couldn't be read.")
            model = parse_model(d.get("model"))
            gb = d.get("id")
            if not model or isinstance(gb, bool) or not isinstance(gb, int) or not 0 < gb < 100_000_000:
                raise Reject(400, "Which creation? The report doesn't say.")
            reason = d.get("reason")
            if reason not in REPORT_REASONS:
                raise Reject(400, f"The reason is one of {', '.join(REPORT_REASONS)}.")
            note = d.get("note") or ""
            if not isinstance(note, str) or len(note) > MAX_NOTE * 4:
                raise Reject(400, "The note isn't right.")
            note = one_line(note, MAX_NOTE)
            app = one_line(d["app"], 32) if isinstance(d.get("app"), str) else ""
        except Reject as e:
            return e.code, {"error": e.message}
        key = key_of(model, gb)
        who = self.reporter(ip)
        if not self.report_same.allow(f"{who}|{key}"):
            return 200, {"ok": True}  # already reported by them today: thanks, once is enough
        if not self.report_all.allow("all"):
            return 503, {"error": "Lots of reports right now. Try again later, or tell us in the Discord."}
        rid = self.store.add_report(key, reason, note, who, app)
        log(f"report for {key}: {reason}")
        if self.report_said.allow(key):
            self.notify_report(key, model, gb, reason, note, who, rid)
        return 200, {"ok": True}

    def notify_report(self, key, model, gb, reason, note, who, rid=None):
        row = self.store.get(key)
        it = (_j(row["item"]) or _j(row["proposed"]) or {}) if row else {}
        title = discord_text(it.get("name", "")) or "(not in our list: a tagged GameBanana page)"
        week = self.store.report_count(key, now() - 7 * 86400)
        lines = [f"**Report: {reason}** for {title} (`{key}`, {row['state'] if row else 'not registered'})",
                 f"<{gb_link(model, gb)}>"]
        if note:
            lines.append(f"> {discord_text(note, MAX_NOTE)}")
        lines.append(f"Reporter #{who} · {week} report(s) for this page in 7 days")
        lines.append(f"Pull: `lsmod.py pull {key} --reason \"...\"` then `lsmod.py publish --upload`  ·  All reports: `lsmod.py reports {key}`")
        self.notifier.card("report", key, "\n".join(lines), report_id=rid)

    def block(self, submitter_id, name, by, reason):
        """That GameBanana member's next submissions wait for a moderator."""
        if not isinstance(submitter_id, int) or submitter_id <= 0:
            raise Reject(400, "A GameBanana member id, please.")
        self.store.set_blocked(submitter_id, one_line(name or "", 40), one_line(by, 40), one_line(reason or "", MAX_REASON), True)

    def unblock(self, submitter_id, by):
        if isinstance(submitter_id, int) and submitter_id > 0:
            self.store.set_blocked(submitter_id, "", one_line(by, 40), "", False)

    # -- moderation --

    def _row(self, key):
        p = parse_key(key)
        if not p:
            raise Reject(400, "Name the page as mod:<id>, spray:<id> or its GameBanana link.")
        row = self.store.get(key_of(*p))
        return p, row

    def _need(self, key):
        p, row = self._row(key)
        if not row:
            raise Reject(404, f"Nothing is registered for {key_of(*p)}.")
        return row

    def _verify_live(self, row, item):
        try:
            return self._verify(row["model"], row["gb"], item)
        except Reject as e:
            if e.code == 502:
                raise Reject(502, f"{e.message} Nothing changed.")
            raise Reject(409, f"Not done: the page doesn't pass the checks now. {e.message}")

    def moderate(self, key, action, body, live=True):
        """One moderation step; returns the entry afterwards. live=False skips checking the page
        again when approving (Discord's buttons must answer within 3 seconds; the automatic checks
        already passed when it came in)."""
        t = now()
        reason = body.get("reason", "")
        if not isinstance(reason, str):
            raise Reject(400, "The reason isn't text.")
        reason = one_line(reason, MAX_REASON)
        if action == "approve":
            row = self._need(key)
            cand = _j(row["proposed"])
            if not cand:
                raise Reject(409, f"Nothing is waiting for {row['key']} (it's {row['state']}). Use restore to bring back a hidden or pulled entry.")
            if row["state"] == "pulled":
                raise Reject(409, "It's pulled. Restore it first if it should come back.")
            if live:
                facts = self._verify_live(row, cand)
                cand.update(author=facts["author"], screenshots=facts["screenshots"])
            row.update(item=json.dumps(cand, ensure_ascii=False), proposed=None, state="listed", reason="", updated=t, rejected_md5="",
                       hidden_by="", check_note="")
            self.store.put(row, "approve", reason)
        elif action == "reject":
            row = self._need(key)
            cand = _j(row["proposed"])
            if not reason:
                raise Reject(400, "Say why (--reason); the author sees it.")
            if not cand:
                raise Reject(409, f"Nothing is waiting for {row['key']}. To take a listed page out, pull it.")
            row.update(proposed=None, rejected_md5=cand["md5"], updated=t, reason=reason)
            if not row["item"]:
                row.update(state="rejected", hidden_by="team")
            self.store.put(row, "reject", reason)
        elif action == "pull":
            p, row = self._row(key)
            if not reason:
                raise Reject(400, "Say why (--reason); it's kept with the entry.")
            row = row or new_row(p[0], p[1], t)
            row.update(state="pulled", reason=reason, updated=t, proposed=None, hidden_by="team")
            self.store.put(row, "pull", reason)
        elif action == "restore":
            row = self._need(key)
            if row["state"] not in ("hidden", "pulled") or not row["item"]:
                raise Reject(409, f"{row['key']} is {row['state']}; only a hidden or pulled entry that was listed can be restored.")
            item = _j(row["item"])
            facts = self._verify_live(row, item)
            item.update(author=facts["author"], screenshots=facts["screenshots"])
            row.update(item=json.dumps(item, ensure_ascii=False), state="listed", reason="", updated=t, check_note="", hidden_by="")
            self.store.put(row, "restore", reason)
        elif action == "hide":
            row = self._need(key)
            if row["state"] != "listed":
                raise Reject(409, f"{row['key']} is {row['state']}; only a listed entry can be hidden.")
            row.update(state="hidden", reason=reason, updated=t, hidden_by="team")
            self.store.put(row, "hide", reason)
        elif action == "feature":
            row = self._need(key)
            on = body.get("featured")
            if not isinstance(on, bool):
                raise Reject(400, "featured is true or false.")
            row.update(featured=int(on), updated=t)
            self.store.put(row, "feature", "on" if on else "off")
        else:
            raise Reject(400, "Unknown action.")
        return self.detail(key)

    def detail(self, key):
        row = self._need(key)
        out = {k: row[k] for k in ("key", "model", "gb", "kind", "state", "reason", "submitter", "submitter_id",
                                   "app", "added", "updated", "checked", "check_note", "rejected_md5", "hidden_by")}
        out["featured"] = bool(row["featured"])
        out["link"] = gb_link(row["model"], row["gb"])
        out["listed"] = _j(row["item"])
        out["waiting"] = _j(row["proposed"])
        out["history"] = self.store.history(row["key"])
        out["reports"] = self.store.report_count(row["key"], now() - 90 * 86400)
        out["creatorBlocked"] = bool(row["submitter_id"]) and self.store.is_blocked(row["submitter_id"])
        return out

    def queue(self, state):
        if state and state not in STATES and state != "all":
            raise Reject(400, f"The state is one of {', '.join(STATES)} or all.")
        rows = self.store.rows(None if state == "all" else (state,) if state else ("pending",))
        if not state:
            rows += [r for r in self.store.rows(("listed", "hidden")) if r["proposed"]]
        out = []
        for r in rows:
            it = _j(r["proposed"]) or _j(r["item"]) or {}
            out.append({"key": r["key"], "state": r["state"], "kind": r["kind"], "name": it.get("name", ""),
                        "author": r["submitter"], "updated": r["updated"], "waiting": bool(r["proposed"]),
                        "reason": r["reason"], "checkNote": r["check_note"]})
        return {"entries": out, "pending": self.store.count("pending")}

    def status_report(self):
        data, sha = build_index(self.store)
        published = self.store.meta("published", {})
        counts = {s: self.store.count(s) for s in STATES}
        return {"counts": counts, "index": {"sha256": sha, "bytes": len(data)}, "published": published,
                "changedSincePublish": published.get("sha256") != sha, "lastRecheck": self.store.meta("last_recheck", {}),
                "reports7d": len([r for r in self.store.reports(n=10000) if r["at"] >= now() - 7 * 86400])}

    # -- the re-check --

    def recheck(self, keys=None, pause=2.0):
        """Asks GameBanana about every listed, hidden and waiting page again. A page gone or
        untagged is pulled (waiting ones turned down); a listed file that's no longer on the
        page is hidden. Network trouble changes nothing."""
        if not self.recheck_lock.acquire(blocking=False):
            raise Reject(409, "A re-check is already running.")
        try:
            rows = [self.store.get(k) for k in keys] if keys else self.store.rows(("pending", "listed", "hidden"))
            rows = [r for r in rows if r and r["state"] in ("pending", "listed", "hidden")]
            verdicts = []
            for i, row in enumerate(rows):
                if i and pause:
                    time.sleep(pause)
                verdicts.append((row, *self._verdict(row)))
            gone = sum(1 for _, v, _ in verdicts if v in ("pull", "reject"))
            suspicious = not keys and len(rows) >= 8 and gone > len(rows) * RECHECK_GONE_SHARE
            changed, t = [], now()
            for row, verdict, why in verdicts:
                if verdict == "error":
                    self.store.touch(row["key"], row["checked"], f"couldn't check: {why}")
                    continue
                if suspicious and verdict in ("pull", "reject"):
                    self.store.touch(row["key"], row["checked"], f"would be {verdict}ed ({why}), held back")
                    continue
                self.store.touch(row["key"], t, why if verdict != "ok" else "")
                if verdict == "ok":
                    continue
                row = self.store.get(row["key"])
                # a hold the team put on stays the team's (only a moderator lifts it)
                by = "team" if row["hidden_by"] == "team" else "recheck"
                if verdict in ("pull", "reject", "hide"):
                    row["hidden_by"] = by
                if verdict == "pull":
                    row.update(state="pulled", reason=why, updated=t, proposed=None)
                elif verdict == "reject":
                    row.update(state="rejected", reason=why, updated=t, proposed=None,
                               rejected_md5=(_j(row["proposed"]) or {}).get("md5", ""))
                elif verdict == "hide":
                    row.update(state="hidden", reason=why, updated=t)
                elif verdict == "drop":
                    row.update(proposed=None, updated=t)
                self.store.put(row, f"recheck {verdict}", why)
                changed.append((row["key"], verdict, why))
            summary = {"at": t, "checked": len(rows), "changed": len(changed), "heldBack": suspicious}
            if not keys:
                self.store.set_meta("last_recheck", summary)
            if suspicious:
                self.notifier.say(f"**Re-check held back:** {gone} of {len(rows)} pages looked gone or untagged, which is too many "
                                  "to be real. Nothing was pulled. Check GameBanana, then `lsmod.py recheck` again.")
            elif changed:
                lines = [f"**Re-check:** {len(changed)} change(s). `lsmod.py publish --upload` to put the index out."]
                lines += [f"- `{k}` {v}: {discord_text(w)}" for k, v, w in changed[:15]]
                self.notifier.say("\n".join(lines))
            log(f"re-check: {len(rows)} pages, {len(changed)} changed{', held back' if suspicious else ''}")
            return dict(summary, changes=[{"key": k, "verdict": v, "why": w} for k, v, w in changed])
        finally:
            self.recheck_lock.release()

    def _verdict(self, row):
        """("ok" | "pull" | "reject" | "hide" | "drop" | "error", why) for one entry. (The package
        contents aren't read again: a file's MD5 can't change without it being a new file.)"""
        got, page = fetch_page(row["model"], row["gb"], self.get)
        if got == "error":
            return "error", page
        listed = row["state"] in ("listed", "hidden")
        lose = "pull" if listed else "reject"
        if got == "gone":
            return lose, page
        if _d(page.get("_aGame")).get("_idRow") != CS16_GAME:
            return lose, "The page moved out of Counter-Strike 1.6."
        if page.get("_bIsFlagged"):
            return lose, "GameBanana flagged the page."
        if not has_tag(page.get("_aTags")):
            return lose, "The page dropped the LonerStrike tag."
        files = {f["id"]: f for f in page_files(page)}
        if listed:
            it = _j(row["item"])
            f = files.get(it.get("file"))
            if f and f["md5"] == it.get("md5") and f["av"] and f["av"] != "clean":
                return "pull", "GameBanana's virus check flagged the file."
            if not f or f["md5"] != it.get("md5"):
                if row["state"] == "hidden":
                    return "ok", row["check_note"] or ""
                return "hide", "The listed file was replaced or removed on GameBanana."
            prop = _j(row["proposed"])
            if prop and prop.get("md5") not in (x["md5"] for x in files.values()):
                return "drop", "The waiting update's file is no longer on the page."
            return "ok", ""
        prop = _j(row["proposed"]) or {}
        if prop.get("md5") not in (x["md5"] for x in files.values()):
            return "reject", "The file was removed from the page before anyone looked."
        return "ok", ""

    def recheck_loop(self, hours, first_delay=600):
        time.sleep(first_delay)
        while True:
            self._safe_recheck()
            time.sleep(max(3600, hours * 3600))

    def _safe_recheck(self):
        try:
            self.recheck()
        except Reject:
            pass
        except Exception as e:  # never let the loop die; the next run tries again
            log(f"re-check failed: {type(e).__name__}")

    # -- HTTP --

    def _authorized(self, h, ip):
        """None when the request carries the admin token, else (status, why)."""
        if self.token_digest is None:
            return 503, "Moderation is off (no admin token set)."
        if self.admin_local and (h.client_address[0] != "127.0.0.1" or h.headers.get("CF-Connecting-IP")):
            return 403, "Moderation only answers on this machine."
        # too many wrong tokens from one place: even a right one is refused for the hour
        with self.admin_fails.lock:
            recent = [x for x in self.admin_fails.hits.get(ip, []) if time.time() - x < 3600]
        if len(recent) >= ADMIN_FAILS_PER_HOUR:
            return 429, "Too many wrong tokens from here. Try again in an hour."
        auth = h.headers.get("Authorization") or ""
        given = auth[7:].strip() if auth[:7].lower() == "bearer " else ""
        # both sides hashed first, so the comparison takes the same time whatever the length
        if hmac.compare_digest(hashlib.sha256(given.encode()).digest(), self.token_digest):
            return None
        self.admin_fails.allow(ip)
        return 401, "Wrong or missing admin token."

    def handle(self, h, method, ip):
        """Answers a request under /community/ for the serverlist handler `h`."""
        path = urllib.parse.urlsplit(h.path)
        parts = [p for p in path.path.split("/") if p][1:]
        query = urllib.parse.parse_qs(path.query)
        try:
            if parts == ["health"] and method == "GET":
                return reply(h, 200, {"ok": True})
            if parts in (["submit"], ["report"]):
                if method != "POST":
                    return reply(h, 405, {"error": "POST only."})
                body = read_body(h)
                code, answer = (self.submit if parts == ["submit"] else self.report)(body, ip)
                return reply(h, code, answer, cors=False)
            if len(parts) == 3 and parts[0] == "submission" and method == "GET":
                p = parse_key(f"{parts[1]}:{parts[2]}")
                if not p:
                    return reply(h, 404, {"error": "not found"})
                return reply(h, 200, self.status(*p))
            if parts[:1] == ["admin"]:
                denied = self._authorized(h, ip)
                if denied:
                    return reply(h, denied[0], {"error": denied[1]}, cors=False)
                return self.admin(h, method, parts[1:], query)
            return reply(h, 404, {"error": "not found"})
        except Reject as e:
            return reply(h, e.code, {"error": e.message}, cors=False)

    def interactions(self, h):
        """POST /discord/interactions: button clicks from the mod channel (modbot.Mod)."""
        mod = self.notifier
        if not hasattr(mod, "interaction"):
            return reply(h, 404, {"error": "not found"}, cors=False)
        try:
            body = read_body(h)
        except Reject as e:
            return reply(h, e.code, {"error": e.message}, cors=False)
        code, answer = mod.interaction(h.headers, body)
        return reply(h, code, answer, cors=False)

    def admin(self, h, method, parts, query):
        body = {}
        if method == "POST":
            raw = read_body(h)
            try:
                body = json.loads(raw.decode("utf-8") or "{}")
            except (UnicodeDecodeError, ValueError):
                raise Reject(400, "The request couldn't be read.")
            if not isinstance(body, dict):
                raise Reject(400, "The request couldn't be read.")
        if parts == ["queue"] and method == "GET":
            return reply(h, 200, self.queue((query.get("state") or [""])[0]), cors=False)
        if parts == ["status"] and method == "GET":
            return reply(h, 200, self.status_report(), cors=False)
        if parts == ["reports"] and method == "GET":
            k = (query.get("key") or [""])[0]
            p = parse_key(k) if k else None
            if k and not p:
                raise Reject(400, "Name the page as mod:<id> or spray:<id>.")
            return reply(h, 200, {"reports": self.store.reports(key_of(*p) if p else None)}, cors=False)
        if len(parts) == 3 and parts[0] == "item":
            key = f"{parts[1]}:{parts[2]}"
            if method == "GET":
                return reply(h, 200, self.detail(key), cors=False)
            if method == "POST":
                action = body.get("action")
                return reply(h, 200, self.moderate(key, action if isinstance(action, str) else "", body), cors=False)
        if parts == ["blocked"]:
            if method == "POST":
                p = parse_key(body.get("page")) if isinstance(body.get("page"), str) else None
                row = self.store.get(key_of(*p)) if p else None
                sid = body.get("member") if isinstance(body.get("member"), int) else (row or {}).get("submitter_id", 0)
                if not sid:
                    raise Reject(400, "Name a registered page (or a GameBanana member id) whose creator to block.")
                if body.get("block") is False:
                    self.unblock(sid, "lsmod")
                else:
                    self.block(sid, (row or {}).get("submitter", ""), "lsmod", body.get("reason") if isinstance(body.get("reason"), str) else "")
            return reply(h, 200, {"blocked": self.store.blocked()}, cors=False)
        if parts == ["recheck"] and method == "POST":
            key = body.get("key")
            if key:
                p = parse_key(key) if isinstance(key, str) else None
                if not p:
                    raise Reject(400, "Name the page as mod:<id> or spray:<id>.")
                return reply(h, 200, self.recheck([key_of(*p)], pause=0), cors=False)
            if self.recheck_lock.locked():
                raise Reject(409, "A re-check is already running.")
            threading.Thread(target=self._safe_recheck, daemon=True).start()
            return reply(h, 202, {"ok": True, "started": True}, cors=False)
        if parts == ["index"] and method == "GET":
            data, sha = build_index(self.store)
            return reply(h, 200, None, cors=False, raw=data, headers={"X-Index-Sha256": sha})
        if parts == ["published"] and method == "POST":
            sha = body.get("sha256")
            if not isinstance(sha, str) or not _HEX64.match(sha):
                raise Reject(400, "sha256, please.")
            self.store.set_meta("published", {"sha256": sha, "at": now()})
            return reply(h, 200, {"ok": True}, cors=False)
        return reply(h, 404, {"error": "not found"}, cors=False)


def read_body(h):
    if h.headers.get("Transfer-Encoding"):
        raise Reject(411, "Send a Content-Length.")
    try:
        size = int(h.headers.get("Content-Length") or 0)
    except ValueError:
        raise Reject(400, "Bad Content-Length.")
    if size < 0:
        raise Reject(400, "Bad Content-Length.")
    if size > MAX_BODY:
        raise Reject(413, "The request is too big.")
    return h.rfile.read(size) if size else b""


def reply(h, code, body, cors=True, raw=None, headers=None):
    data = raw if raw is not None else json.dumps(body, ensure_ascii=False).encode("utf-8")
    h.send_response(code)
    h.send_header("Content-Type", "application/json; charset=utf-8")
    h.send_header("Content-Length", str(len(data)))
    h.send_header("Cache-Control", "no-store")
    h.send_header("X-Content-Type-Options", "nosniff")
    if cors:
        h.send_header("Access-Control-Allow-Origin", "*")
    for k, v in (headers or {}).items():
        h.send_header(k, v)
    h.end_headers()
    if h.command != "HEAD":
        h.wfile.write(data)
