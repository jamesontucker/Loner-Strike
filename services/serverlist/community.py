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
import os
import signal
import subprocess
import tempfile
import hmac
import io
import ipaddress
import json
import re
import secrets
import sqlite3
import struct
import sys
import threading
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
import zipfile
import zlib

SCHEMA = 1
CS16_GAME = 4254
TAG_TITLE = "Software Used"
TAG_VALUE = "LonerStrike"
TAG_ALSO = "Loner-Strike"
GB_HOST = "gamebanana.com"
GB_API = f"https://{GB_HOST}/apiv11"
USER_AGENT = "LonerStrike/1.0 (community server; +https://lonerstrike.com)"
KINDS = ("map", "mode", "prefab", "spray", "texture", "creature")
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
SUBMIT_PER_IP_DAY = 30
ADMIN_FAILS_PER_HOUR = 10
GB_FETCHES_PER_MIN = 30
MIN_TOKEN = 32
REPORTS_PER_IP_HOUR = 5
UNLIST_PER_IP_HOUR = 20       # unlist/list-again tries (each needs the page's code anyway)
UNLIST_CODE_TTL = 86400       # a code is good for a day
UNLIST_HOW = ("Put {code} anywhere in your GameBanana page's description (Edit, then save), then click again here. "
              "It shows the list that you're the page's owner. You can take it off afterwards.")
REPORTS_PER_DAY = 300         # all together; more and the button says "try later"
REPORT_REASONS = ("broken", "malware", "stolen", "offensive", "not-loner-strike", "other")
MAX_NOTE = 500
MAX_LISTING = 1 << 20         # bytes read to list a package's files (its directory and manifest)
MAX_ENTRIES = 5000            # files in one package
PACKAGE_CHECKS = 2            # packages looked into at the same time
LOCAL_SLACK = 96              # bytes allowed between one zip entry's data and the next header
HEADER_SAMPLES = 16           # local headers compared with the directory, spread over the zip
GB_PER_VISITOR_MIN = 10       # a visitor's share of the GameBanana checks a minute
REPORTS_PER_VISITOR_DAY = 20  # a visitor's share of the day's reports
STATUS_PER_MIN = 60
APPLICATION_DAYS = 180        # role applications are deleted this long after the decision (or after sending, if undecided)
HISTORY_DAYS = 730            # moderation history, cards, uid records and delisted entries: kept two years
REPORT_DAYS = 90
INTERACTION_DAYS = 1
CHECK_MAX = 256 << 20         # [Caps] biggest package whose files are checked one by one (bigger: "too big to check"); the biggest real CS 1.6 item found is a 132.7 MB mappack
CHECK_ENTRY_MAX = 256 << 20   # biggest single file inside it
CHECK_SECONDS = 180           # the whole check (download and files) must finish in this
CHECK_BLOCK = 4 << 20         # download piece size
PUBLISH_PER_HOUR = 60         # the publish workflow asks twice every 10 minutes
PLUGIN_REFUSAL = "Plugin modes can't be shared yet. Make it in Game Mode Studio, which builds a mode's code on each player's computer."
# files that run code (or hide other files) never go out in a shared package
# files that run code: named in the refusal (plugins get the Game Mode Studio message)
CODE_EXT = (".amxx", ".amx", ".sma", ".inc", ".dll", ".so", ".dylib", ".exe", ".com", ".scr", ".msi", ".bat", ".cmd",
            ".sh", ".ps1", ".py", ".pyc", ".js", ".vbs", ".jar", ".lua", ".pl", ".rb", ".php", ".app", ".command")
# game settings a package must never bring (they'd change how the server runs)
SETTINGS_FILES = ("liblist.gam", "server.cfg", "listenserver.cfg", "autoexec.cfg", "config.cfg", "userconfig.cfg")
STOCK_WADS = ("halflife.wad", "cstrike.wad", "czero.wad", "decals.wad", "tempdecal.wad", "pldecal.wad", "gfx.wad",
              "fonts.wad", "cached.wad", "liquids.wad", "xeno.wad", "spraypaint.wad")
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

# custom creatures a map, mode, prefab or creature package carries: the rules live in formats.py
# (the checker child reads the fragment itself)
from formats import BUILTIN_CREATURES, CREATURES_FILE, CREATURES_TYPES, creature_name_ok, creatures_ini_problem, creatures_ini_sections, creatures_ini_types, namespace_is  # noqa: E402,F401


def _creatures(raw, kind):
    """The custom creatures an item says it carries: [{name, model, sounds}] (names checked)."""
    out = []
    for c in _list(raw, "creatures", CREATURES_TYPES):
        if not isinstance(c, dict) or not creature_name_ok(c.get("name")):
            _bad("creatures")
        if kind not in ("map", "mode", "prefab", "creature"):
            _bad("creatures")
        if any(x["name"] == c["name"] for x in out):
            continue
        out.append({"name": c["name"], "model": _bool(c, "model"), "sounds": _int(c, "sounds", 0, 64)})
    return out
# a mode's (or kit's, or library item's) permanent id, beside its short game id (entries that already
# had one when the uids table came in are its owners from the start: migration 4 backfills them)
_UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
_SAFE_ARG = re.compile(r"^[A-Za-z0-9_.$-]{1,64}$")


def one_line(s, n):
    """Text on one line, without control or direction-changing characters, at most n characters."""
    out = "".join(" " if unicodedata.category(c) in ("Cc", "Cf", "Cs", "Co", "Zl", "Zp") else c for c in s)
    return " ".join(out.split())[:n]


def safe_arg(s):
    """The app's maps::safe_arg: a name the game's console takes as one word."""
    return isinstance(s, str) and bool(_SAFE_ARG.match(s))


def _bad(field):
    raise Reject(400, f"The registration's {field} isn't right. Update Loner-Strike and try again.")


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


_ASSET = re.compile(r"^[a-z0-9_./ -]{1,160}\.(mdl|spr|wav|mp3)$")


def _requirements(r):
    """A mode's maps and content (the app's community::ModeRequirements): plain map names, clean
    model/sound/sprite paths, links to GameBanana pages; anything else is dropped or refused."""
    dm = r.get("defaultMap", "") or ""
    if not isinstance(dm, str):
        _bad("requirements")
    maps = [m for m in dict.fromkeys(_list(r, "maps", 24)) if isinstance(m, str) and safe_arg(m)]
    content = []
    for c in _list(r, "content", 64):
        if not isinstance(c, dict) or not isinstance(c.get("path"), str):
            _bad("requirements content")
        p = c["path"].strip().replace("\\", "/").lower().lstrip("/")
        if ".." in p or not _ASSET.match(p):
            continue
        kind = "model" if p.endswith(".mdl") else "sprite" if p.endswith(".spr") else "sound"
        e = {"path": p, "kind": kind}
        link = c.get("link")
        if isinstance(link, dict) and link.get("model") in ("Mod", "Spray") and isinstance(link.get("id"), int) and 0 < link["id"] < 100_000_000:
            name = link.get("name", "")
            e["link"] = {"model": link["model"], "id": link["id"], "name": name[:80] if isinstance(name, str) else ""}
        content.append(e)
    out = {"defaultMap": dm if safe_arg(dm) else "", "maps": maps, "content": content}
    needs = _needs(r.get("needs"))
    if needs:
        out["needs"] = needs
    return out


_WORD = re.compile(r"^[A-Za-z0-9_-]{1,32}$")


def _needs(n):
    """What a mode needs beyond the game (the app's community::ModeNeeds, QA4-G F6): Loner-Strike's
    components, creature types and bot-brain features with the brain versions they need. Junk is
    dropped; a wrong shape is refused."""
    if n is None:
        return None
    if not isinstance(n, dict):
        _bad("requirements needs")

    def ver(k):
        v = n.get(k, 0)
        if isinstance(v, bool) or not isinstance(v, int) or v < 0:
            _bad("requirements needs")
        return min(v, 9)

    words = lambda k: [w for w in dict.fromkeys(_list(n, k, 32)) if isinstance(w, str) and _WORD.match(w)]
    feats = []
    for f in _list(n, "features", 12):
        if not isinstance(f, dict):
            _bad("requirements needs")
        fid, name, brain, v = f.get("id"), f.get("name", ""), f.get("brain"), f.get("ver", 0)
        if not (isinstance(fid, str) and _WORD.match(fid) and brain in ("mode", "creature") and isinstance(v, int) and not isinstance(v, bool) and v >= 0):
            continue
        feats.append({"id": fid, "name": " ".join(name.split())[:80] if isinstance(name, str) else "", "brain": brain, "ver": min(v, 9)})
    out = {"componentsPlugin": _bool(n, "componentsPlugin"), "components": words("components"), "creatures": words("creatures"),
           "modeVer": ver("modeVer"), "creatureVer": ver("creatureVer"), "features": feats}
    return out if out["componentsPlugin"] or out["components"] or out["creatures"] or feats else None


# what a Game Mode Studio mode can do beyond its own game (keep in step with community.rs RISKS)
MODE_CAPABILITIES = ("server_commands", "amx_commands", "plugin_calls", "kicks_players")


def _list(d, k, n):
    v = d.get(k, [])
    if v is None:
        return []
    if not isinstance(v, list) or len(v) > n:
        _bad(k)
    return v


def _uid(d, k="uid"):
    """An optional UUID (lower case); "" when absent. Entries without one keep working."""
    v = d.get(k, "")
    if v is None or v == "":
        return ""
    if not isinstance(v, str) or not _UUID.match(v.strip().lower()):
        _bad(k)
    return v.strip().lower()


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
    uid = _uid(raw)
    if uid:
        it["uid"] = uid
    # how well bots play it (game modes), on the app's own scale (games.ts BotRating); "" not said
    # who made the assets it ships (the app's tools/credits.rs AssetCredit): <= 64, short strings,
    # plain game paths, GameBanana links only
    credits = []
    for c in _list(raw, "credits", 64):
        if not isinstance(c, dict):
            _bad("credits")
        files = _list(c, "files", 64)
        if any(not isinstance(f, str) or not f or len(f) > 128 or ".." in f or f.startswith("/") or "\\" in f for f in files) or not files:
            _bad("credits")
        page = _str(c, "page", 200)
        if page and not page.startswith("https://gamebanana.com/"):
            _bad("credits")
        authors = []
        for a in _list(c, "authors", 24):
            if not isinstance(a, dict):
                _bad("credits")
            url = _str(a, "url", 200)
            if url and not url.startswith("https://gamebanana.com/"):
                _bad("credits")
            name = _str(a, "name", 60)
            if name:
                authors.append({"name": name, "role": _str(a, "role", 60), "url": url})
        perm = c.get("permission", "")
        if perm not in ("", None, "granted"):
            _bad("credits")
        credits.append({"files": [one_line(f, 128) for f in files], "page": page, "name": _str(c, "name", 80), "authors": authors,
                        "permission": perm or "", "note": _str(c, "note", 300)})
    if credits:
        it["credits"] = credits
    bots = raw.get("bots", "")
    if bots not in ("", None, "great", "ok", "weak", "none"):
        _bad("bots")
    if bots and kind == "mode":
        it["bots"] = bots
    r = _obj(raw, "remixOf")
    if r is not None:
        # credit for a remix: which creation it came from (shown with its name and author)
        rm = parse_model(r.get("model", "Mod"))
        if not rm:
            _bad("remixOf")
        it["remixOf"] = {"model": rm, "gb": _int(r, "gb", 0, 99_999_999), "uid": _uid(r),
                         "name": _str(r, "name", 80), "author": _str(r, "author", 40)}
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
    # P5: a map shipped with its editable source (source/: the project, a .map, its own textures)
    src = raw.get("source", False)
    if not isinstance(src, bool) or (src and kind != "map"):
        _bad("source")
    if src:
        it["source"] = True
    creatures = _creatures(raw, kind)
    if kind == "creature" and not creatures:
        _bad("creatures")
    if creatures:
        it["creatures"] = creatures

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
        refs = []
        for x in _list(m, "modeRefs", 16):
            if not isinstance(x, dict):
                _bad("modeRefs")
            mid = x.get("id", "")
            if not isinstance(mid, str) or not _MODE_ID.match(mid):
                _bad("modeRefs")
            ref = {"uid": _uid(x), "id": mid, "name": _str(x, "name", 40)}
            if not any(r2["id"] == mid and r2["uid"] == ref["uid"] for r2 in refs):
                refs.append(ref)
        it["map"] = {"name": name, "bspSize": _int(m, "bspSize", 0, MAX_FILE), "bspSha256": sha,
                     "modes": list(dict.fromkeys(modes)), "modeRefs": refs,
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
        # what a Studio mode does beyond its own game (the app's community::RISKS; it asks before
        # installing and shows a badge each); unknown words are dropped, the app re-checks the graph
        caps = [c for c in dict.fromkeys(_list(mo, "capabilities", 8)) if c in MODE_CAPABILITIES]
        # plugins and their review stay in the format for later; for now never any
        it["mode"] = {"id": mid, "uid": _uid(mo), "plugins": [], "review": "none", "maps": list(dict.fromkeys(maps))}
        if caps:
            it["mode"]["capabilities"] = caps
        req = _obj(mo, "requirements")
        if req is not None:
            it["mode"]["requirements"] = _requirements(req)

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
    """The app's POST body -> (model, page id, file id, app version). Only the ids are read:
    {"schema": 1, "app": "<version>", "page": {"model": "Mod", "id": 123, "file": 456}} (the
    older {"item": {"model", "gb", "file", ...}} is read the same way). Everything else about a
    creation comes from GameBanana and from the package itself, never from the request."""
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
    page = d.get("page") if isinstance(d.get("page"), dict) else None
    old = d.get("item") if isinstance(d.get("item"), dict) else None
    if page is None and old is None:
        raise Reject(400, "The registration doesn't say which GameBanana page it is.")
    src = page if page is not None else dict(old, id=old.get("gb"))
    model = parse_model(src.get("model", "Mod"))
    if not model:
        _bad("model")
    return model, _int(src, "id", 1, 99_999_999), _int(src, "file", 1, 99_999_999), app


def package_item(manifest, kind):
    """The Loner-Strike part of an entry (summary, games, tags, map and mode details) from the
    package's own loner-strike.json, checked with the same rules as everything else. Health
    numbers are dropped: nothing proves them, so they show as unchecked."""
    try:
        d = json.loads(manifest.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        raise Reject(422, "The package's loner-strike.json couldn't be read. Make the package again with Publish.")
    if not isinstance(d, dict) or not isinstance(d.get("item"), dict):
        raise Reject(422, "The package's loner-strike.json isn't one the app made. Make the package again with Publish.")
    schema = d.get("schema")
    if isinstance(schema, bool) or not isinstance(schema, int) or not 1 <= schema <= SCHEMA:
        raise Reject(422, "The package was made by a newer Loner-Strike than the list knows.")
    m = dict(d["item"])
    if m.get("kind", kind) != kind:
        raise Reject(422, f"The package is a {m.get('kind')} but the page is in the category for a {kind}. Move one of them.")
    # what the checks need but the package can't know yet; replaced by the verified values after
    m.update(kind=kind, model="Spray" if kind == "spray" else "Mod", gb=1, file=1, md5="0" * 32, name=m.get("name") or "x", health=None)
    try:
        it = clean_item(m)
    except Reject as e:
        if e.code == 422:
            raise
        raise Reject(422, e.message.replace("The registration's", "The package's loner-strike.json:"))
    return it


def map_facts(meta, sizes):
    """A map's details checked against the files really in the package: the .bsp must be there
    (its size from the zip), nav, knowledge, .res and overview only count when they ship."""
    have = {n.replace("\\", "/").lower(): sz for n, sz in sizes.items()}
    name = meta["name"]
    bsp = have.get(f"maps/{name.lower()}.bsp")
    if bsp is None:
        raise Reject(422, f"The package says it's the map {name}, but maps/{name}.bsp isn't in it.")
    nav = f"maps/{name.lower()}.nav" in have
    return dict(meta, bspSize=bsp, nav=nav, places=bool(meta.get("places")) and nav,
                lsv=nav and f"maps/{name.lower()}.lsv" in have, res=f"maps/{name.lower()}.res" in have,
                overview=any(f"overviews/{name.lower()}.{e}" in have for e in ("bmp", "tga", "txt")))


# ---- GameBanana ---# ---- GameBanana ---------------------------------------------------------------------------

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

    def peek(self, off, n):
        """Exactly n bytes at off in one small request (a local header), within the budget."""
        n = min(n, self.size - off)
        if n <= 0:
            return b""
        self.read_total += n
        if self.read_total > self.budget:
            raise FetchError("the package's file list is too big")
        data, self.url = self.get(self.url, off, off + n - 1)
        return data


MAX_MANIFEST = 64 * 1024
_EOCD = b"PK\x05\x06"
_LOCAL = b"PK\x03\x04"


def zip_directory(rf):
    """(entries, directory offset) from a zip's end record, checked before anything is built:
    too many entries, a directory too big to read, or zip64 (never needed for a package under
    1 GB with a few thousand files) are refused."""
    tail_len = min(rf.size, 22 + 65535)
    rf.seek(rf.size - tail_len)
    tail = rf.read(tail_len)
    at = tail.rfind(_EOCD)
    if at < 0 or len(tail) - at < 22:
        raise Reject(422, "The zip on the page couldn't be read. Upload the zip the app made, unchanged.")
    disk, cd_disk, here, total, cd_size, cd_off = struct.unpack("<HHHHII", tail[at + 4:at + 20])
    if disk or cd_disk or here != total or 0xFFFF in (here, total) or 0xFFFFFFFF in (cd_size, cd_off):
        raise Reject(422, "The zip is split or in zip64 form. Upload the zip the app made, unchanged.")
    if total > MAX_ENTRIES:
        raise Reject(422, f"The package has too many files ({total}; at most {MAX_ENTRIES}).")
    if cd_size > MAX_LISTING or cd_off + cd_size > rf.size:
        raise Reject(422, "The package's file list is too big or damaged. Upload the zip the app made, unchanged.")
    return total, cd_off


def check_layout(rf, infos, cd_off):
    """The zip's directory and its entries must agree: entries start at 0, follow each other
    with only a few bytes between them (no hidden extra entries an unzipper could find), and the
    local headers sampled across the file name the same files with the same sizes."""
    def name_len(i):
        return len(i.orig_filename.encode("utf-8" if i.flag_bits & 0x800 else "cp437"))
    order = sorted(infos, key=lambda i: i.header_offset)
    pos = 0
    for i in order:
        gap = i.header_offset - pos
        if gap < 0 or gap > (0 if pos == 0 else LOCAL_SLACK):
            raise Reject(422, "The zip has data outside its file list. Upload the zip the app made, unchanged.")
        pos = i.header_offset + 30 + name_len(i) + i.compress_size
    if not 0 <= cd_off - pos <= LOCAL_SLACK:
        raise Reject(422, "The zip has data outside its file list. Upload the zip the app made, unchanged.")
    picks = order if len(order) <= HEADER_SAMPLES else [order[round(k * (len(order) - 1) / (HEADER_SAMPLES - 1))] for k in range(HEADER_SAMPLES)]
    for i in picks:
        head = rf.peek(i.header_offset, 30 + name_len(i))
        if len(head) < 30 or head[:4] != _LOCAL:
            raise Reject(422, "The zip's entries don't match its file list. Upload the zip the app made, unchanged.")
        flags, method, _, _, _, csize, _, nlen, xlen = struct.unpack("<HHHHIIIHH", head[6:30])
        same_size = flags & 0x08 or csize == i.compress_size
        if method != i.compress_type or nlen != name_len(i) or head[30:] != i.orig_filename.encode("utf-8" if i.flag_bits & 0x800 else "cp437") \
                or not same_size or xlen > LOCAL_SLACK:
            raise Reject(422, "The zip's entries don't match its file list. Upload the zip the app made, unchanged.")


def read_package(f, get=range_get):
    """(sizes {path: bytes}, loner-strike.json's bytes or None) of a page's file, read from the
    zip's directory and that one small entry by range requests (a lone .wad/picture is itself).
    Reject(422) when it can't be looked inside; FetchError on network trouble."""
    low = f["name"].lower()
    if low.endswith(SINGLE_OK):
        return {f["name"]: f["size"]}, None
    if not low.endswith(".zip"):
        raise Reject(422, "Upload the zip the app made (Publish), unchanged: the list can only look inside zips.")
    rf = RangeFile(f"https://{GB_HOST}/dl/{f['id']}", f["size"], get)
    try:
        total, cd_off = zip_directory(rf)
        rf.seek(0)
        with zipfile.ZipFile(rf) as z:
            infos = z.infolist()
            if len(infos) != total:
                raise Reject(422, "The zip's file list is damaged. Upload the zip the app made, unchanged.")
            check_layout(rf, infos, cd_off)
            # the same name twice (one would hide the other; check_package catches case and slash variants)
            if len({i.filename for i in infos}) != len(infos):
                raise Reject(422, "The package has the same file twice. Make it again with Publish.")
            sizes = {i.filename: i.file_size for i in infos}
            manifest = None
            found = [i for i in infos if i.filename.lower() == "loner-strike.json"]
            if len(found) > 1:
                raise Reject(422, "The package has more than one loner-strike.json.")
            if found:
                i = found[0]
                if i.flag_bits & 1 or i.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                    raise Reject(422, "The package's loner-strike.json is encrypted or packed in an odd way. Make it with Publish.")
                if i.file_size > MAX_MANIFEST or i.compress_size > MAX_MANIFEST:
                    raise Reject(422, "The package's loner-strike.json is too big.")
                with z.open(i) as fh:
                    # at most this much comes out, whatever the header claims (no zip bombs)
                    manifest = fh.read(MAX_MANIFEST + 1)
                if len(manifest) > MAX_MANIFEST:
                    raise Reject(422, "The package's loner-strike.json is too big.")
    except (zipfile.BadZipFile, zipfile.LargeZipFile, EOFError, ValueError, NotImplementedError, KeyError, zlib.error, struct.error):
        raise Reject(422, "The zip on the page couldn't be read. Upload the zip the app made, unchanged.")
    return sizes, manifest


def package_names(f, get=range_get):
    """The paths inside a page's file."""
    return list(read_package(f, get)[0])


# a path part the app's strict archive rule takes: plain ASCII letters, digits, _ . space and -
_PART = re.compile(r"^[A-Za-z0-9 _.\-]{1,64}$")
# an 8.3 short name ("progra~1"): Windows could resolve it to another file
_SHORT_NAME = re.compile(r"~[0-9]")
# names Windows keeps for devices, with or without an extension (setup/record.rs windows_unsafe)
_DEVICE = re.compile(r"^(con|prn|aux|nul|conin\$|conout\$|com[1-9]|lpt[1-9])$")


# the page's own file (the zip as GameBanana names it; never installed by that name) keeps the
# installer's older, wider set
_OUTER = re.compile(r"^[A-Za-z0-9 _.\-()+!#&',]{1,128}$")


def _outer_name_ok(name):
    return isinstance(name, str) and "/" not in name and "\\" not in name and not name.startswith(".") \
        and not name.endswith((" ", ".")) and bool(_OUTER.match(name))


# the games' own files (the app's stock lists: Steam's depot manifests for Half-Life, CS 1.6 and
# Condition Zero; kept in step with app/src-tauri/src/tools/stock_*.txt)
_STOCK = None


def is_stock(path):
    global _STOCK
    if _STOCK is None:
        here = os.path.join(os.path.dirname(os.path.abspath(__file__)), "stock")
        got = set()
        for n in ("stock_valve.txt", "stock_cstrike.txt", "stock_czero.txt"):
            with open(os.path.join(here, n), encoding="utf-8") as f:
                got.update(line.strip().lower() for line in f if line.strip())
        _STOCK = got
    return path.lower() in _STOCK


def _part_ok(p):
    stem = p.split(".", 1)[0].rstrip(" ").lower()
    return not (p in ("", ".", "..") or p.startswith(".") or p.endswith((" ", ".")) or not _PART.match(p)
                or _SHORT_NAME.search(p) or _DEVICE.match(stem))


def norm_path(name):
    """A package path made plain (lower case, '/'), or None when it isn't one the app's
    installer would write: backslashes, '..', empty or dot parts, NUL, trailing dots or spaces,
    any character outside plain ASCII letters, digits, _ . space and -, 8.3 short names (x~1)
    and Windows device names (CON, NUL, COM1..., with or without an extension). Twins are
    compared lower-cased (check_package)."""
    if not isinstance(name, str) or "\\" in name or "\x00" in name or name.startswith("/") or len(name) > 255:
        return None
    parts = name[:-1].split("/") if name.endswith("/") else name.split("/")
    for p in parts:
        if not _part_ok(p):
            return None
    return "/".join(parts).lower() + ("/" if name.endswith("/") else "")


def _ext(p):
    base = p.rsplit("/", 1)[-1]
    return base[base.rfind("."):] if "." in base else ""


# where content goes and what it may be: the app's installer's own allow-list (archive.rs
# content_exts), plus .lsv map knowledge
CONTENT = {"maps": (".bsp", ".nav", ".txt", ".res", ".lsv"), "gfx": (".tga", ".bmp"), "models": (".mdl",),
           "sound": (".wav", ".mp3"), "sprites": (".spr",), "overviews": (".txt", ".bmp", ".tga")}


def allowed_path(kind, p):
    """May this (normalised) path be in a package of this kind? An allow-list: anything not
    named here is refused."""
    if p.endswith("/"):
        return True  # a folder entry (its files are checked one by one)
    parts, ext = p.split("/"), _ext(p)
    top = parts[0]
    if p in ("readme.txt", "loner-strike.json"):
        return True
    if p == CREATURES_FILE:
        return kind in ("map", "mode", "prefab", "creature")
    if kind == "creature":
        # one shared type: its creatures.ini section(s), its models and sounds, a preview picture
        assets = len(parts) >= 2 and top in ("models", "sound") and ext in (".mdl", ".wav")
        preview = top == "creatures" and len(parts) == 2 and parts[1].startswith("preview") and ext in (".png", ".jpg", ".jpeg")
        return assets or preview
    root_wad = len(parts) == 1 and ext == ".wad" and p not in STOCK_WADS
    content = len(parts) >= 2 and top in CONTENT and ext in CONTENT[top]
    if kind == "map":
        # (P5: the editable source: the editor project, a .map, its own textures, a note)
        source = top == "source" and len(parts) == 2 and ext in (".lsmap", ".map", ".wad", ".txt")
        return content or root_wad or source
    if kind == "texture":
        return root_wad
    if kind == "spray":
        return p == "tempdecal.wad" or root_wad or (top == "logos" and len(parts) == 2 and ext == ".bmp") \
            or (len(parts) == 1 and ext in (".bmp", ".tga", ".png", ".jpg", ".jpeg"))
    if kind == "prefab":
        # (its creatures' models and sounds too)
        creature_files = len(parts) >= 2 and top in ("models", "sound") and ext in (".mdl", ".wav")
        return root_wad or creature_files or (top == "prefabs" and len(parts) == 2 and ext in (".map", ".json", ".png"))
    if kind == "mode":
        # Game Mode Studio data (provisional until the Studio's package format is fixed): its
        # graphs and settings, and the content it uses; never plugins or their lists
        data = (top == "modes" and len(parts) >= 3 and ext in (".json", ".ini", ".txt")) or \
               (p.startswith("addons/amxmodx/configs/gamemodes/") and len(parts) == 5 and ext == ".ini")
        return (data or content or root_wad) and not parts[-1].startswith("plugins")
    return False


def code_files(names):
    """The paths in a package that run code or hide other files (named in the refusal)."""
    bad = []
    for n in names:
        low = n.replace("\\", "/").lower()
        if low.endswith("/"):
            continue
        base = low.rsplit("/", 1)[-1]
        if base.endswith(CODE_EXT) or base.endswith((".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2", ".xz", ".cab")) \
                or "addons/metamod/" in low or "/dlls/" in "/" + low:
            bad.append(n)
    return bad


def check_package(names, kind="map"):
    """Every path in the package must be plain and on the allow-list for its kind."""
    bad = code_files(names)
    if any(b.lower().endswith((".amxx", ".amx", ".sma", ".inc")) for b in bad):
        raise Reject(422, PLUGIN_REFUSAL)
    if bad:
        raise Reject(422, f"The package has files that run code or can't be checked ({', '.join(bad[:3])}). "
                          "Only maps, textures, models, sounds, sprays and Game Mode Studio settings can be shared.")
    odd = [n for n in names if norm_path(n) is None]
    if odd:
        raise Reject(422, f"The package has file names the app won't install ({', '.join(repr(n) for n in odd[:3])}). "
                          "Make it again with Publish.")
    paths = [norm_path(n) for n in names]
    if len(set(paths)) != len(paths):
        raise Reject(422, "The package has the same file twice (names differing only in case).")
    settings = [n for n, p in zip(names, paths) if p.rsplit("/", 1)[-1] in SETTINGS_FILES or p.endswith(".cfg") or p.endswith(".gam")
                or p.rsplit("/", 1)[-1].startswith("plugins") and p.endswith(".ini")]
    if settings:
        raise Reject(422, f"The package has game settings files ({', '.join(settings[:3])}); shared creations can't change "
                          "how a server runs.")
    other = [n for n, p in zip(names, paths) if not allowed_path(kind, p)]
    if other:
        raise Reject(422, f"The package has files the app doesn't install for a {kind} ({', '.join(other[:3])}).")


def fetch_page(model, gb, get=http_get):
    """("ok", page) / ("gone" | "private" | "withheld", reason) / ("error", reason). Those three
    only on GameBanana's own clear answer, never on a network problem: "gone" = no such page or
    deleted (author or moderator, GameBanana doesn't say), "private" = the author hid it,
    "withheld" = GameBanana's moderators took it down."""
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
    # (what kind of gone matters: GameBanana withholding a page is moderation, fail closed; a page the
    # author made private comes back with them; deleted/trashed can't be told apart from a moderator's
    # removal, so it waits for ours)
    if page.get("_bIsWithheld") is True:
        return "withheld", "GameBanana withheld the page."
    if page.get("_bIsTrashed") is True:
        return "gone", "The page was deleted."
    if page.get("_bIsPrivate") is True:
        return "private", "The page is private."
    return "ok", page


def has_tag(tags):
    """The app's community::has_tag: "Software Used: LonerStrike" in either of GameBanana's shapes."""
    def ours(v):
        return v.strip().lower() in (TAG_VALUE.lower(), TAG_ALSO.lower())


    for t in tags if isinstance(tags, list) else []:
        if isinstance(t, str):
            a, sep, b = t.partition(":")
            if any(ours(x) for x in (b if sep else t).split(",")):
                return True
        elif isinstance(t, dict) and isinstance(t.get("_sValue"), str):
            # the value is what counts (a GameBanana Custom Tag's title is optional)
            if any(ours(x) for x in t["_sValue"].split(",")):
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
    # a Loner-Strike creature (a tagged page among the game's skins and models)
    if root in ("skins", "models"):
        return "creature"
    return None


def page_files(page):
    out = []
    for f in page.get("_aFiles") or []:
        if isinstance(f, dict) and isinstance(f.get("_idRow"), int):
            out.append({"id": f["_idRow"], "name": str(f.get("_sFile") or ""),
                        "size": f.get("_nFilesize") if isinstance(f.get("_nFilesize"), int) else 0,
                        "md5": str(f.get("_sMd5Checksum") or "").lower(),
                        "av": str(f.get("_sAvResult") or "").lower(),
                        "added": f.get("_tsDateAdded") if isinstance(f.get("_tsDateAdded"), int) else 0,
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


def page_facts(page, model):
    """Checks a profile page (game, flags, tag, category, name, submitter). Returns its facts;
    Reject(422) with what to fix."""
    if _d(page.get("_aGame")).get("_idRow") != CS16_GAME:
        raise Reject(422, "That page isn't in GameBanana's Counter-Strike 1.6 section. Move it there, then try again.")
    if page.get("_bIsFlagged"):
        raise Reject(422, "GameBanana flagged that page. Sort it out on GameBanana, then try again.")
    if not has_tag(page.get("_aTags")):
        raise Reject(422, f"The page needs the {TAG_VALUE} tag. On GameBanana's Technical tab, under Tags, add a Custom Tag with the value {TAG_VALUE} (the title can stay empty), then press + and save, and try again.")
    kind = page_kind(page, model)
    if not kind:
        raise Reject(422, "The page isn't in a category the list takes: Maps, Prefabs, Textures, Resources > AMXX Plugins & Mods "
                          "(Game Mode Studio modes), Skins (creatures) or Sprays.")
    title = one_line(str(page.get("_sName") or ""), 80)
    if not title:
        raise Reject(422, "The page has no name.")
    sub = _d(page.get("_aSubmitter"))
    sid = sub.get("_idRow") if isinstance(sub.get("_idRow"), int) else 0
    name = one_line(str(sub.get("_sName") or ""), 40)
    if sid <= 0 or not name:
        raise Reject(422, "GameBanana didn't say who made the page. Try again in a minute.")
    return {"kind": kind, "title": title, "submitterId": sid, "author": name, "screenshots": page_shots(page)}


def file_ok(f):
    """Reject(422) unless a page's file is scanned clean and of a sane size."""
    if not _HEX32.match(f["md5"]):
        raise Reject(422, "GameBanana doesn't list the file's checksum yet. Try again in a few minutes.")
    if f["av"] != "clean":
        if f["av"] in ("", "pending") or f["avState"] not in ("", "done"):
            raise Reject(422, "GameBanana is still checking the file. Try again in a few minutes.")
        raise Reject(422, "GameBanana's virus check didn't pass the file.")
    if not 0 < f["size"] <= MAX_FILE:
        raise Reject(422, "The file on the page is empty or too big.")


def newest_first(files):
    """A page's files, newest first (GameBanana's date added, then the id)."""
    return sorted(files, key=lambda f: (f["added"], f["id"]), reverse=True)


def check_page(page, model, gb, file_id):
    """The page's facts with one of its files (moderators restoring a given file, and tests)."""
    facts = page_facts(page, model)
    f = next((f for f in page_files(page) if f["id"] == file_id), None)
    if not f:
        raise Reject(422, "That file isn't on the page. Upload the zip the app made unchanged, then try again.")
    file_ok(f)
    return dict(facts, file=f)


def build_item(model, gb, facts, sizes, manifest):
    """The entry, from verified sources only: GameBanana's page (title, author, pictures, the
    file and its MD5) and the package's own loner-strike.json and file list."""
    kind, f = facts["kind"], facts["file"]
    if manifest is not None:
        it = package_item(manifest, kind)
    elif kind in ("map", "mode", "prefab", "creature"):
        raise Reject(422, "The package has no loner-strike.json. Make it with the app's Publish and upload that zip unchanged.")
    else:
        it = {"kind": kind, "summary": "", "games": ["cstrike"], "tags": []}
    if kind == "map":
        if not it.get("map"):
            raise Reject(422, "The package's loner-strike.json doesn't name its map. Make it again with Publish.")
        it["map"] = map_facts(it["map"], sizes)
        # P5: "with its source" only when the package really has the project in source/
        if it.get("source") and not any(p.lower().startswith("source/") and p.lower().endswith(".lsmap") for p in sizes):
            it.pop("source", None)
    # custom creatures: listed only when the package really carries creatures/creatures.ini (its
    # contents are checked with the other files, creatures_ini_problem)
    if it.get("creatures") and not any(p.replace("\\", "/").lower() == CREATURES_FILE for p in sizes):
        raise Reject(422, "The package lists custom creatures but doesn't include them. Make it again with Publish.")
    it.pop("health", None)
    it.update(kind=kind, model=model, gb=gb, file=f["id"], md5=f["md5"], name=facts["title"], author=facts["author"],
              screenshots=facts["screenshots"], featured=False, status="listed")
    return it


# ---- rate limits ----------------------------------------------------------------------------

def visitor_key(ip):
    """What the rate limits count by: an IPv4 address, or an IPv6 address's /64 (one home or
    server usually has a whole /64, so counting single v6 addresses would be no limit at all)."""
    try:
        a = ipaddress.ip_address((ip or "").strip())
    except ValueError:
        return (ip or "?")[:64]
    if a.version == 6 and a.ipv4_mapped:
        return str(a.ipv4_mapped)
    if a.version == 6:
        return str(ipaddress.ip_network(f"{a}/64", strict=False))
    return str(a)


class Window:
    """At most `n` hits per `secs` per key, in memory (lost on restart, which is fine). The
    table keeps at most `max_keys` keys, dropping the ones heard from longest ago."""

    def __init__(self, n, secs, max_keys=20000):
        self.n, self.secs, self.max_keys = n, secs, max_keys
        self.lock = threading.Lock()
        self.hits = {}

    def allow(self, who, t=None):
        t = time.time() if t is None else t
        with self.lock:
            recent = [x for x in self.hits.pop(who, []) if t - x < self.secs]
            ok = len(recent) < self.n
            if ok:
                recent.append(t)
            self.hits[who] = recent  # re-inserted: dicts keep order, so the oldest keys come first
            if len(self.hits) > self.max_keys:
                for k in list(self.hits)[: len(self.hits) - int(self.max_keys * 0.9)]:
                    del self.hits[k]
            return ok

    def full(self, who, t=None):
        """True when `who` has used up its hits (without counting one)."""
        t = time.time() if t is None else t
        with self.lock:
            return len([x for x in self.hits.get(who, []) if t - x < self.secs]) >= self.n


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
    # 3: role applications from the Discord server (applications.py)
    """
    CREATE TABLE applications (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id TEXT NOT NULL,
        user_name TEXT NOT NULL DEFAULT '',
        role_key TEXT NOT NULL,
        state TEXT NOT NULL,
        why TEXT NOT NULL,
        links TEXT NOT NULL DEFAULT '',
        joined TEXT NOT NULL DEFAULT '',
        msg_id TEXT NOT NULL DEFAULT '',
        reason TEXT NOT NULL DEFAULT '',
        decider TEXT NOT NULL DEFAULT '',
        decider_name TEXT NOT NULL DEFAULT '',
        note TEXT NOT NULL DEFAULT '',
        at INTEGER NOT NULL,
        decided_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX applications_user ON applications(user_id, role_key)
    """,
    # 4: who owns a permanent id (uid): the GameBanana member who first registered it. Updates in the
    # app match by uid only within one owner, so nobody can pass their package off as someone
    # else's new version; a moderator moves a uid when a creator changes accounts.
    """
    CREATE TABLE uids (
        uid TEXT PRIMARY KEY,
        owner_id INTEGER NOT NULL,
        owner TEXT NOT NULL DEFAULT '',
        key TEXT NOT NULL DEFAULT '',
        at INTEGER NOT NULL,
        by TEXT NOT NULL DEFAULT ''
    );
    INSERT OR IGNORE INTO uids (uid, owner_id, owner, key, at, by)
        SELECT lower(trim(json_extract(item, '$.uid'))), submitter_id, submitter, key, added, 'backfill' FROM entries
        WHERE json_valid(item) AND submitter_id > 0 AND lower(trim(COALESCE(json_extract(item, '$.uid'), ''))) GLOB '[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'
        ORDER BY added, key;
    INSERT OR IGNORE INTO uids (uid, owner_id, owner, key, at, by)
        SELECT lower(trim(json_extract(item, '$.mode.uid'))), submitter_id, submitter, key, added, 'backfill' FROM entries
        WHERE json_valid(item) AND submitter_id > 0 AND lower(trim(COALESCE(json_extract(item, '$.mode.uid'), ''))) GLOB '[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f]-[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'
        ORDER BY added, key
    """,
    # 5: a uid is settled by evidence, not by who registered first: the date of the earliest
    # GameBanana upload carrying it (file_at, 0 unknown), a moderator's pin, and the conflicts the
    # moderators are asked to settle (a card each, with a button per side)
    """
    ALTER TABLE uids ADD COLUMN file_at INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE uids ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
    CREATE TABLE uid_conflicts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        uid TEXT NOT NULL,
        holder TEXT NOT NULL,
        other TEXT NOT NULL,
        why TEXT NOT NULL DEFAULT '',
        at INTEGER NOT NULL,
        settled TEXT NOT NULL DEFAULT '',
        settled_by TEXT NOT NULL DEFAULT ''
    )
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
#   re-check           untagged/flagged/moved/withheld by GameBanana -> pulled (waiting: rejected),
#                      unlisted ones too; page private or the listed file removed -> withdrawn, back
#                      to listed by itself when they return; page deleted (author or moderator,
#                      GameBanana can't tell) -> withdrawn for review: only a moderator lists it
#                      again (restore; the creator's new file waits for one); listed file replaced
#                      -> hidden; a team hold stays marked as the team's
#   creator (proven)   unlist (listed) -> unlisted; list again (unlisted) -> listed, after the page
#                      is checked again (anything that pulls it pulls it). Proof: a code
#                      the service gives, put on the GameBanana page by its submitter (unlist()).
#                      Never undoes a team hold. A new file for an unlisted page keeps it unlisted.


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

    def put(self, row, action, detail="", claim=None):
        """Writes a whole entry and a log line in one go. `claim` (uids, owner id, owner name, the
        upload's GameBanana date): settles those uids in the same transaction (settle_uid). A uid
        another member keeps is left out of this entry, which is listed all the same. Returns the
        ids of the uid conflicts it found, for the moderators' cards."""
        cols = list(row)
        conflicts = []
        with self.lock:
            self.db.execute("BEGIN")
            try:
                if claim:
                    # settled here, with the entry write, so nothing half-done is ever kept
                    uids, owner_id, owner, file_at = claim["uids"], claim["owner_id"], claim["owner"], claim["file_at"]
                    may_take = claim.get("may_take", True)
                    lost = []
                    for uid in uids:
                        r = self.db.execute("SELECT * FROM uids WHERE uid=?", (uid,)).fetchone()
                        holder = dict(r) if r else None
                        held = None
                        if holder:
                            hr = self.db.execute("SELECT state FROM entries WHERE key=?", (holder["key"],)).fetchone()
                            held = hr["state"] if hr else None
                        h_at = (claim.get("holder_at") or {}).get(uid) or 0
                        verdict = settle_uid(holder, owner_id, file_at, h_at, held == "listed", may_take)
                        if holder and h_at > 0 and (not holder["file_at"] or h_at < holder["file_at"]):
                            # an earlier upload of the holder's proven now: kept (only ever lower)
                            self.db.execute("UPDATE uids SET file_at=? WHERE uid=?", (h_at, uid))
                            holder["file_at"] = h_at
                        if not may_take and verdict != "same":
                            # a held submission: noted for the moderator, settled when it's approved
                            conflicts.append(self._conflict(uid, holder["key"], row["key"],
                                                            f"{row['key']} waits for a moderator and declares it too"))
                            continue
                        if holder is None:
                            if not may_take:
                                continue  # claimed when a moderator approves it
                            self.db.execute("INSERT INTO uids (uid, owner_id, owner, key, at, by, file_at) VALUES (?,?,?,?,?,?,?)",
                                            (uid, owner_id, owner, row["key"], now(), "registration", file_at or 0))
                        elif verdict == "same":
                            if file_at and (not holder["file_at"] or file_at < holder["file_at"]):
                                self.db.execute("UPDATE uids SET file_at=? WHERE uid=?", (file_at, uid))
                        elif verdict == "take":
                            # the earlier upload wins: the holder's entry loses the uid
                            self.db.execute("UPDATE uids SET owner_id=?, owner=?, key=?, at=?, by=?, file_at=? WHERE uid=?",
                                            (owner_id, owner, row["key"], now(), "earlier upload", file_at, uid))
                            self._strip_row(holder["key"], uid)
                            conflicts.append(self._conflict(uid, row["key"], holder["key"],
                                                            f"{row['key']}'s upload carrying it is older; {holder['key']} lost it"))
                        else:
                            lost.append(uid)
                            why = ("a moderator gave it to " + holder["key"]) if holder.get("pinned") else (
                                f"{row['key']}'s upload carrying it is older, but {holder['key']} is listed with it: decide" if verdict == "flag"
                                else f"{holder['key']} has it; this upload isn't provably older")
                            conflicts.append(self._conflict(uid, holder["key"], row["key"], why))
                    if lost:
                        row = dict(row)
                        for col in ("item", "proposed"):
                            if row.get(col):
                                row[col] = json.dumps(strip_uids(_j(row[col]), lost), ensure_ascii=False)
                        cols = list(row)
                self.db.execute(f"INSERT OR REPLACE INTO entries ({','.join(cols)}) VALUES ({','.join('?' * len(cols))})",
                                tuple(row[c] for c in cols))
                self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)", (row["key"], now(), action, detail[:500]))
                self.db.execute("COMMIT")
            except Exception:
                self.db.execute("ROLLBACK")
                raise
        return conflicts

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

    # -- who owns a uid --

    def uid_owner(self, uid):
        with self.lock:
            r = self.db.execute("SELECT * FROM uids WHERE uid=?", (uid,)).fetchone()
            return dict(r) if r else None

    def _strip_row(self, key, uid):
        """(inside a transaction) Takes a uid out of an entry's stored item."""
        r = self.db.execute("SELECT item, proposed FROM entries WHERE key=?", (key,)).fetchone()
        if not r:
            return
        vals = [json.dumps(strip_uids(_j(v), [uid]), ensure_ascii=False) if v else v for v in (r["item"], r["proposed"])]
        self.db.execute("UPDATE entries SET item=?, proposed=? WHERE key=?", (*vals, key))
        self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)", (key, now(), "uid-lost", uid))

    def _conflict(self, uid, holder, other, why):
        """(inside a transaction) Notes a uid conflict for the moderators; its id."""
        return self.db.execute("INSERT INTO uid_conflicts (uid, holder, other, why, at) VALUES (?,?,?,?,?)",
                               (uid, holder, other, why[:300], now())).lastrowid

    def uid_conflict(self, cid):
        with self.lock:
            r = self.db.execute("SELECT * FROM uid_conflicts WHERE id=?", (cid,)).fetchone()
        return dict(r) if r else None

    def give_uid(self, uid, key, by):
        """A moderator settles a uid on one page: that page's submitter owns it (pinned), that entry
        carries it again, every other entry that declares it loses it."""
        with self.lock:
            self.db.execute("BEGIN")
            try:
                row = self.db.execute("SELECT * FROM entries WHERE key=?", (key,)).fetchone()
                if not row or not row["submitter_id"]:
                    raise Reject(404, "That page isn't registered.")
                self.db.execute("INSERT OR REPLACE INTO uids (uid, owner_id, owner, key, at, by, file_at, pinned) VALUES (?,?,?,?,?,?,COALESCE((SELECT file_at FROM uids WHERE uid=?), 0),1)",
                                (uid, row["submitter_id"], row["submitter"], key, now(), by, uid))
                for r in self.db.execute("SELECT key, item, proposed FROM entries WHERE key<>?", (key,)).fetchall():
                    if uid in item_uids(_j(r["item"]) or {}) or uid in item_uids(_j(r["proposed"]) or {}):
                        self._strip_row(r["key"], uid)
                vals = [json.dumps(restore_uid(_j(v), uid), ensure_ascii=False) if v else v for v in (row["item"], row["proposed"])]
                self.db.execute("UPDATE entries SET item=?, proposed=? WHERE key=?", (*vals, key))
                self.db.execute("UPDATE uid_conflicts SET settled=?, settled_by=? WHERE uid=? AND settled=''", (key, by, uid))
                self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)", (f"uid:{uid}", now(), "uid-owner", f"to {key} by {by}"[:500]))
                self.db.execute("COMMIT")
            except Exception:
                self.db.execute("ROLLBACK")
                raise

    def set_uid_owner(self, uid, owner_id, owner, by):
        """A moderator moves a uid to another member (a creator who changed accounts)."""
        with self.lock:
            self.db.execute("BEGIN")
            self.db.execute("INSERT OR REPLACE INTO uids (uid, owner_id, owner, key, at, by, file_at, pinned) VALUES (?,?,?,COALESCE((SELECT key FROM uids WHERE uid=?), ''),?,?,COALESCE((SELECT file_at FROM uids WHERE uid=?), 0),1)",
                            (uid, owner_id, owner, uid, now(), by, uid))
            self.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)",
                            (f"uid:{uid}", now(), "uid-owner", f"to member {owner_id} {owner} by {by}"[:500]))
            self.db.execute("COMMIT")

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

    # -- role applications --

    def housekeeping(self, at=None):
        """The daily retention pass (the privacy page's promises). Returns what went, by table.
        - applications: 180 days after the decision (or after sending, undecided);
        - reports: 90 days; Discord interaction ids: 1 day;
        - entries out of the list (pulled, turned down, hidden) for two years: the whole entry (with
          its creator, credits and reasons) and its uid records; a pulled page can then be registered
          again like any new one;
        - moderation history: two years, except a creator's block history while the block is in place
          (after it's lifted, two years from the lifting, its newest line);
        - moderators' cards and settled uid disputes: two years;
        - uid ownership records whose entry is gone."""
        t = now() if at is None else at
        cut = t - HISTORY_DAYS * 86400
        out = {"applications": self.prune_applications(t)}
        with self.lock:
            self.db.execute("BEGIN")
            try:
                ex = lambda sql, args=(): self.db.execute(sql, args).rowcount
                out["reports"] = ex("DELETE FROM reports WHERE at < ?", (t - REPORT_DAYS * 86400,))
                out["interactions"] = ex("DELETE FROM interactions WHERE at < ?", (t - INTERACTION_DAYS * 86400,))
                out["entries"] = ex("DELETE FROM entries WHERE state IN ('pulled', 'rejected', 'hidden') AND updated < ?", (cut,))
                # (a moderator's assignment with no page yet: two years like the rest of the history)
                out["uids"] = ex("DELETE FROM uids WHERE (key <> '' AND key NOT IN (SELECT key FROM entries)) OR (key = '' AND at < ?)", (cut,))
                out["uid_conflicts"] = ex("DELETE FROM uid_conflicts WHERE at < ? OR (holder NOT IN (SELECT key FROM entries) AND other NOT IN (SELECT key FROM entries))", (cut,))
                out["cards"] = ex("DELETE FROM cards WHERE at < ?", (cut,))
                # a creator's block history stays while the block does; lifted, it counts from its newest line
                out["log"] = ex(
                    "DELETE FROM log WHERE at < ? AND NOT (key LIKE 'member:%' AND ("
                    "CAST(substr(key, 8) AS INTEGER) IN (SELECT submitter_id FROM blocked) "
                    "OR (SELECT MAX(l2.at) FROM log l2 WHERE l2.key = log.key) >= ?))",
                    (cut, cut),
                )
                self.db.execute("COMMIT")
            except Exception:
                self.db.execute("ROLLBACK")
                raise
        return out

    def prune_applications(self, at=None):
        """Deletes role applications 180 days after their decision, and undecided ones 180 days after
        they were sent (the privacy page's promise). Moderation history (log) is left as it is."""
        cut = (now() if at is None else at) - APPLICATION_DAYS * 86400
        with self.lock:
            return self.db.execute("DELETE FROM applications WHERE (decided_at > 0 AND decided_at < ?) OR (decided_at = 0 AND at < ?)",
                                   (cut, cut)).rowcount

    def add_application(self, **cols):
        cols.setdefault("at", now())
        self.prune_applications()
        with self.lock:
            return self.db.execute(f"INSERT INTO applications ({','.join(cols)}) VALUES ({','.join('?' * len(cols))})",
                                   tuple(cols.values())).lastrowid

    def application(self, app_id):
        with self.lock:
            r = self.db.execute("SELECT * FROM applications WHERE id=?", (app_id,)).fetchone()
        return dict(r) if r else None

    def update_application(self, app_id, **cols):
        with self.lock:
            self.db.execute(f"UPDATE applications SET {', '.join(f'{k}=?' for k in cols)} WHERE id=?", (*cols.values(), app_id))

    def applications_of(self, user_id, role_key):
        with self.lock:
            return [dict(r) for r in self.db.execute("SELECT * FROM applications WHERE user_id=? AND role_key=? ORDER BY id DESC",
                                                      (user_id, role_key))]

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

def settle_uid(holder, owner_id, file_at, holder_at=None, holder_listed=False, may_take=True):
    """Who gets a uid another member already holds: "same" (the same member), "keep" (the holder
    keeps it; the newcomer goes without it), "take" (the newcomer's earliest upload carrying it is
    provably older and the holder's entry isn't listed) or "flag" (the newcomer's upload is older
    but the holder is listed: nothing listed loses a uid automatically, a moderator decides).
    Both sides' dates are their earliest upload carrying the uid (`holder_at`, worked out the same
    way as the newcomer's); a moderator's pin beats any date; a held submission (`may_take` false)
    never takes; anything unproven stays with the holder."""
    if holder is None or holder["owner_id"] == owner_id:
        return "same"
    if holder.get("pinned"):
        return "keep"
    # the holder's earliest proven date: what's stored only ever goes down (a deleted or unread old
    # file never weakens it), and a fresh look can only lower it further
    known = [d for d in (holder_at or 0, holder.get("file_at") or 0) if d > 0]
    a = min(known) if known else 0
    b = file_at or 0
    if a > 0 and b > 0 and b < a:
        return "take" if (may_take and not holder_listed) else "flag"
    return "keep"


def strip_uids(item, uids):
    """An entry without the given uids (listed, but linked to nothing). What it declared is kept
    aside (`heldBack`, not indexed) for a moderator to give back."""
    it = dict(item)
    back = dict(it.get("heldBack") or {})
    if norm_uid(it.get("uid")) in uids:
        back["uid"] = norm_uid(it.pop("uid", None))
    if isinstance(it.get("mode"), dict) and norm_uid(it["mode"].get("uid")) in uids:
        back["modeUid"] = norm_uid(it["mode"].get("uid"))
        it["mode"] = dict(it["mode"], uid="")
    if back:
        it["heldBack"] = back
    return it


def restore_uid(item, uid):
    """Gives an entry back a uid it declared and had held back."""
    it = dict(item)
    back = dict(it.get("heldBack") or {})
    if back.get("uid") == uid:
        it["uid"] = uid
        back.pop("uid")
    if back.get("modeUid") == uid and isinstance(it.get("mode"), dict):
        it["mode"] = dict(it["mode"], uid=uid)
        back.pop("modeUid")
    if back:
        it["heldBack"] = back
    else:
        it.pop("heldBack", None)
    return it


def norm_uid(u):
    """A uid as stored and compared: stripped, lower case, a UUID; "" for anything else."""
    u = u.strip().lower() if isinstance(u, str) else ""
    return u if _UUID.match(u) else ""


def item_uids(it):
    """The permanent ids an entry declares (its own, its mode's), normalised, without repeats."""
    out = []
    for u in (it.get("uid"), (it.get("mode") or {}).get("uid")):
        u = norm_uid(u)
        if u and u not in out:
            out.append(u)
    return out


def index_item(row):
    """One index entry in exactly the app's schema (community.rs Item): every field with the
    type the app's reader wants (a wrong type or an unknown enum value would make it drop the
    whole list)."""
    it = _j(row["item"])
    status = {"listed": "listed", "hidden": "hidden", "pulled": "revoked", "unlisted": "unlisted", "withdrawn": "withdrawn"}[row["state"]]
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
    if it.get("uid"):
        out["uid"] = it["uid"]
    if it.get("bots"):
        out["bots"] = it["bots"]
    if it.get("credits"):
        out["credits"] = it["credits"]
    # the GameBanana member the entry is registered to: the app matches updates by uid only within one owner
    out["ownerId"] = int(row["submitter_id"] or 0)
    if it.get("remixOf"):
        out["remixOf"] = it["remixOf"]
    if it.get("source"):
        out["source"] = True
    if it.get("creatures"):
        out["creatures"] = it["creatures"]
    out.update(screenshots=list(it.get("screenshots") or [])[:MAX_SHOTS], tags=list(it.get("tags") or []),
               featured=bool(row["featured"]), added=int(row["added"]), updated=int(row["updated"]), status=status)
    return out


def build_index(store):
    """community/index.json as bytes (UTF-8, compact) and its sha256. The same database gives
    the same bytes, so an unchanged index isn't uploaded again."""
    items, revoked = [], set()
    for row in store.rows(("listed", "hidden", "pulled", "unlisted", "withdrawn")):
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
    def __init__(self, store, token="", notifier=None, get=http_get, get_range=range_get, admin_local=False, publish_token=""):
        self.store = store
        self.token_digest = hashlib.sha256(token.encode()).digest() if len(token) >= MIN_TOKEN else None
        if token and not self.token_digest:
            log(f"the admin token is shorter than {MIN_TOKEN} characters; moderation is off")
        # the publish workflow's token: it may read the published index (GET /community/publish/...), nothing else
        self.publish_digest = hashlib.sha256(publish_token.encode()).digest() if len(publish_token) >= MIN_TOKEN else None
        self.notifier = notifier or Notifier("")
        self.get = get
        self.get_range = get_range
        self.admin_local = admin_local
        self.ip_limit = Window(SUBMIT_PER_IP_HOUR, 3600)
        # per visitor, never per GameBanana member (a stranger mustn't use up a creator's quota)
        self.ip_day = Window(SUBMIT_PER_IP_DAY, 86400)
        self.admin_fails = Window(ADMIN_FAILS_PER_HOUR, 3600)
        self.gb_limit = Window(GB_FETCHES_PER_MIN, 60)
        self.gb_share = Window(GB_PER_VISITOR_MIN, 60)
        self.status_limit = Window(STATUS_PER_MIN, 60)
        self.publish_limit = Window(PUBLISH_PER_HOUR, 3600)
        self.publish_fails = Window(ADMIN_FAILS_PER_HOUR, 3600)
        self.pkg_slots = threading.BoundedSemaphore(PACKAGE_CHECKS)
        self.check_slot = threading.BoundedSemaphore(1)  # files are checked for one package at a time
        self.verify_md5 = True
        self.checker_cmd = None  # (tests put a stand-in child here)
        self.checking = None     # the running checker child, if any
        self.report_ip = Window(REPORTS_PER_IP_HOUR, 3600)
        self.unlist_ip = Window(UNLIST_PER_IP_HOUR, 3600)
        self.unlist_codes = {}  # (key, action) -> (code, valid until)
        self.report_same = Window(1, 86400)
        self.report_all = Window(REPORTS_PER_DAY, 86400)
        self.report_day = Window(REPORTS_PER_VISITOR_DAY, 86400)
        self.report_said = Window(1, 1800)
        # reporters are told apart in the mod channel by a hash of their address with a salt that
        # lives only in memory: the same person reads the same for a while, nobody can reverse it
        self.salt = hashlib.sha256(str(time.time_ns()).encode() + bytes(16)).digest()
        self.recheck_lock = threading.Lock()

    def reporter(self, ip):
        return hmac.new(self.salt, ip.encode(), hashlib.sha256).hexdigest()[:8]

    # -- registrations --

    def _page(self, model, gb):
        got, page = fetch_page(model, gb, self.get)
        if got in ("gone", "private", "withheld"):
            raise Reject(422, page)
        if got == "error":
            log(f"GameBanana didn't answer for {key_of(model, gb)}: {page}")
            raise Reject(502, "GameBanana didn't answer. Try again in a few minutes.")
        return page

    def _entry(self, model, gb, facts, f):
        """The entry for one of the page's files after the file and package checks."""
        file_ok(f)
        if not _outer_name_ok(f["name"]):
            raise Reject(422, "The file's name isn't one the app would install. Upload the zip the app made, unchanged.")
        # only a couple of packages are looked into at once (memory on a small box)
        if not self.pkg_slots.acquire(timeout=0.5):
            raise Reject(429, "The list is checking other packages. Try again in a minute.")
        try:
            sizes, manifest = read_package(f, self.get_range)
        except FetchError as e:
            log(f"couldn't read the package of {key_of(model, gb)}: {e}")
            raise Reject(502, "GameBanana's file server didn't answer. Try again in a few minutes.")
        finally:
            self.pkg_slots.release()
        check_package(list(sizes), facts["kind"])
        item = build_item(model, gb, dict(facts, file=f), sizes, manifest)
        # the files themselves, one by one (kept with the entry; not part of the index)
        item["checks"] = self.check_files(model, gb, f, facts["kind"])
        # every creature the entry lists must be defined in the package's creatures.ini
        missing = [c["name"] for c in item.get("creatures", []) if c["name"] not in item["checks"].get("creatures", [])]
        if missing:
            raise Reject(422, f"The package lists {missing[0]} but its creatures.ini doesn't define it. Make it again with Publish.")
        self._creature_names_owned(item, facts)
        self._creature_files_owned(item, facts, sizes)
        # the upload's GameBanana date: the evidence when two pages declare the same uid (not indexed)
        item["fileAt"] = f.get("added") or 0
        return item

    def _creature_names_owned(self, item, facts):
        """Every type a package defines is named after its uploader: gb<GameBanana member id>.<name>
        (the verified submitter of the page), or is an exact copy (the same section) of a type that
        member's own listed entry defines (a creation carrying someone else's creature it uses). A
        remix is published under the remixer's id; remixOf keeps the credit."""
        secs = item.get("checks", {}).get("creatureSections", {})
        member = int(facts["submitterId"])
        for n, h in secs.items():
            if namespace_is(n, member):
                continue
            part = n.partition(".")[0]
            owner = int(part[2:]) if part.startswith("gb") and part[2:].isdigit() else 0
            ok = owner and owner != member and any(
                ((_j(r["item"]) or {}).get("checks") or {}).get("creatureSections", {}).get(n) == h
                for r in self.store.rows(("listed",)) if r["submitter_id"] == owner)
            if not ok:
                raise Reject(422, f"{n} isn't yours (yours are named gb{member}.…) and isn't the same as that creator's listed one. "
                                  "Publish your own creatures under your id, or carry the listed one unchanged.")

    def check_files(self, model, gb, f, kind):
        """Downloads the page's file (range requests to GameBanana's file hosts, at most
        CHECK_MAX, to a temporary file), checks its MD5 is the one GameBanana lists, and has a
        CHILD PROCESS (formats_runner.py: CPU and memory limits, lower priority) check every file
        in it. The child is killed at CHECK_SECONDS; the slot (one check at a time) is let go only
        once the child has exited. Returns a summary; Reject(422) names the file that failed."""
        if f["size"] > CHECK_MAX:
            raise Reject(422, f"The package is too big to check ({f['size'] >> 20} MB; the list checks up to {CHECK_MAX >> 20} MB). "
                              "Leave out what the map doesn't need, or ask in the Discord.")
        if not self.check_slot.acquire(timeout=1):
            raise Reject(429, "The list is checking another package. Try again in a minute.")
        try:
            deadline = time.monotonic() + CHECK_SECONDS
            with tempfile.TemporaryDirectory(prefix="ls-check-") as tmp:
                path = os.path.join(tmp, "package")
                md5 = hashlib.md5()
                url = f"https://{GB_HOST}/dl/{f['id']}"
                try:
                    with open(path, "wb") as fh:
                        for start in range(0, f["size"], CHECK_BLOCK):
                            if time.monotonic() > deadline:
                                raise Reject(422, "The package took too long to download. Try again later.")
                            data, url = self.get_range(url, start, min(f["size"], start + CHECK_BLOCK) - 1)
                            fh.write(data)
                            md5.update(data)
                except FetchError as e:
                    raise Reject(502, f"GameBanana's file server didn't answer ({e}). Try again in a few minutes.")
                if self.verify_md5 and md5.hexdigest() != f["md5"]:
                    raise Reject(422, "The file GameBanana sent isn't the one its page lists (its checksum differs). Try again later.")
                left = max(1.0, deadline - time.monotonic())
                result = self.run_checker(path, f["name"], left, tmp)
        finally:
            self.check_slot.release()
        if not result.get("ok"):
            raise Reject(422, result.get("error") or "The package couldn't be checked.")
        log(f"checked {key_of(model, gb)}: {result['files']} files")
        out = {"files": result["files"], "kinds": result["kinds"], "bytes": f["size"], "at": now()}
        if isinstance(result.get("npcFiles"), list):
            out["npcFiles"] = [r for r in result["npcFiles"] if isinstance(r, str)][:64]
        if isinstance(result.get("creatures"), list):
            out["creatures"] = [n for n in result["creatures"] if isinstance(n, str)][:64]
            secs = result.get("creatureSections")
            if isinstance(secs, dict):
                out["creatureSections"] = {k: v for k, v in list(secs.items())[:64] if isinstance(k, str) and isinstance(v, str)}
            refs = result.get("creatureRefs")
            if isinstance(refs, list):
                out["creatureRefs"] = [r for r in refs if isinstance(r, str)][:1024]
            got = result.get("creatureFiles")
            if isinstance(got, dict):
                out["creatureFiles"] = {k: v for k, v in list(got.items())[:2048] if isinstance(k, str) and isinstance(v, str)}
        return out

    def run_checker(self, path, name, seconds, cwd):
        """Runs the checker child and waits for it to exit (killed and reaped at `seconds`)."""
        cmd = self.checker_cmd or [sys.executable, "-I", os.path.join(os.path.dirname(os.path.abspath(__file__)), "formats_runner.py")]
        proc = subprocess.Popen(cmd + [path, name, str(int(seconds))], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, cwd=cwd, start_new_session=True)
        self.checking = proc
        try:
            out, _ = proc.communicate(timeout=seconds + 2)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except OSError:
                proc.kill()
            proc.wait()  # reaped before the slot is let go
            log("a package check took too long and was stopped")
            return {"ok": False, "error": "The package took too long to check. Make it smaller, or ask in the Discord."}
        finally:
            self.checking = None
        try:
            result = json.loads(out.decode("utf-8").strip().splitlines()[-1])
            if isinstance(result, dict):
                return result
        except (UnicodeDecodeError, ValueError, IndexError):
            pass
        # no answer: killed by its own limits (CPU time, memory) or it crashed
        return {"ok": False, "error": "The package needs too much time or memory to check. Make it smaller, or ask in the Discord."}

    def _choose(self, model, gb, row):
        """(page facts, the entry or None, a note). The server picks the file, never the request:
        the page's newest file that passes every check, and for a page already registered only a
        file newer than the one it has (None: keep it; the note says why when a newer one failed).
        Going back to an older file is a moderator's call."""
        page = self._page(model, gb)
        facts = page_facts(page, model)
        files = newest_first(page_files(page))
        have = (_j(row["item"]) or _j(row["proposed"]) or {}) if row else {}
        cur = next((f for f in files if f["id"] == have.get("file")), None)
        floor = (cur["added"], cur["id"]) if cur else None
        newer = [f for f in files if floor is None or (f["added"], f["id"]) > floor][:3]
        refusals = []
        for f in newer:
            try:
                return facts, self._entry(model, gb, facts, f), ""
            except Reject as e:
                if e.code == 502:
                    raise
                refusals.append(e)
        if cur:
            return facts, None, (f"A newer file on the page wasn't used: {refusals[0].message}" if refusals else "")
        if refusals:
            raise refusals[0]
        raise Reject(422, "The page has no file. Upload the zip the app made, then try again.")

    def _verify_file(self, model, gb, file_id):
        """(facts, entry) for one given file (a moderator approving or restoring)."""
        page = self._page(model, gb)
        facts = page_facts(page, model)
        f = next((f for f in page_files(page) if f["id"] == file_id), None)
        if not f:
            raise Reject(422, "That file isn't on the page any more.")
        return facts, self._entry(model, gb, facts, f)

    def submit(self, body, ip):
        """(status, answer) for the app's POST /community/submit. Anyone may register any
        tagged page (the tag is the creator's say-so); what gets listed is always the page's and
        the package's own data, so a stranger can at most make the entry match them again."""
        ip = visitor_key(ip)
        if not self.ip_limit.allow(ip):
            return 429, {"error": "Too many registrations from here. Try again in an hour."}
        if not self.ip_day.allow(ip):
            return 429, {"error": "Too many registrations from here today. Try again tomorrow."}
        try:
            model, gb, _hint, app = parse_submission(body)  # the file id is only a hint: the server picks
            key = key_of(model, gb)
            row = self.store.get(key)
            if row and row["state"] == "pulled":
                raise Reject(409, "The Loner-Strike team pulled this page, so it can't be registered again. Ask in the Discord.")
            # a visitor's fair share of GameBanana checks first, so one source can't use them all up
            if not self.gb_share.allow(ip) or not self.gb_limit.allow("gb"):
                raise Reject(429, "The list is busy. Try again in a few minutes.")
            facts, item, note = self._choose(model, gb, row)
            if item is None:
                # nothing newer that passes: the entry stays as it is
                if note:
                    self.store.touch(key, row["checked"], note)
                    log(f"registration {key}: kept the current file ({note})")
                waiting = bool(row["proposed"])
                return 200, dict({"ok": True, "key": key, "state": "pending" if waiting else row["state"]}, **({"note": note} if note else {}))
            if row and row["state"] == "rejected" and row["hidden_by"] == "team" and row["rejected_md5"] == item["md5"]:
                raise Reject(409, f"The Loner-Strike team turned this file down: {row['reason'] or 'no reason given'}. Upload a new version to try again.")
            # creators the team blocked wait for a moderator, and so does a new file for a page the
            # team hid or turned down; everyone else is listed at once
            # a permanent id belongs to the member who first registered it: someone else's package
            # declaring it would otherwise show as that creation's "update" in every app
            # a uid another member holds never stops a listing: it's settled by evidence when the
            # entry is written (settle_uid), and conflicts go to the moderators
            uids = item_uids(item)
            file_at = self._uid_evidence(model, gb, facts, item, uids)
            # (a page deleted on GameBanana and back waits for a moderator too: hidden_by "review")
            team_hold = bool(row) and row["hidden_by"] in ("team", "review")
            blocked = self.store.is_blocked(facts["submitterId"])
            look = blocked or team_hold
            if look and not (row and row["proposed"]) and self.store.count("pending") >= MAX_PENDING:
                raise Reject(503, "The list has too many creations waiting for a look. Try again in a day or two.")
        except Reject as e:
            return e.code, {"error": e.message}
        t = now()
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
            # (the author's own unlisting stays until they list it again)
            if was == "unlisted":
                row.update(item=data, proposed=None, rejected_md5="", check_note="")
            else:
                row.update(item=data, proposed=None, rejected_md5="", state="listed", hidden_by="", reason="", check_note="")
            action = "list"
        try:
            conflicts = self.store.put(row, action, f"md5 {item['md5']} file {item['file']} app {app}",
                                       claim=self._claim(uids, facts["submitterId"], facts["author"], file_at, may_take=not look))
        except Reject as e:
            return e.code, {"error": e.message}
        for cid in conflicts:
            self.notify_uid_conflict(cid)
        log(f"registration {key} {row['state']} ({'new' if fresh else 'update'})")
        self.notify_new(key, item, look, not fresh and was in ("listed", "hidden"), fresh,
                        why="blocked creator" if look and blocked else f"{was} by the team" if look else "")
        return 200, {"ok": True, "key": key, "state": "pending" if look else row["state"]}

    def _earliest_carrying(self, model, gb, kind, uids, at):
        """The date of a page's earliest upload whose package declares all `uids`, starting from a
        file dated `at` that does (up to 3 older files read, newest first; stops at the first that
        can't be read or doesn't carry them)."""
        try:
            files = page_files(self._page(model, gb))
        except Reject:
            return at
        older = sorted([x for x in files if x.get("added") and at and x["added"] < at], key=lambda x: x["added"], reverse=True)[:3]
        for x in older:
            try:
                _sizes, manifest = read_package(x, self.get_range)
                if not manifest or not set(uids) <= set(item_uids(package_item(manifest, kind))):
                    break
                at = x["added"]
            except (Reject, FetchError, ValueError):
                break
        return at

    def _claim(self, uids, owner_id, owner, file_at, may_take=True):
        """What Store.put needs to settle uids: the newcomer's date, and each held uid's holder's
        earliest upload carrying it, worked out the same way (symmetric evidence)."""
        holder_at = {}
        for uid in uids:
            h = self.store.uid_owner(uid)
            if not h or h["owner_id"] == owner_id or h.get("pinned"):
                continue
            hrow = self.store.get(h["key"]) if h.get("key") else None
            hit = (_j(hrow["item"]) or {}) if hrow else {}
            p = parse_key(h["key"]) if h.get("key") else None
            start = hit.get("fileAt") or h.get("file_at") or 0
            holder_at[uid] = self._earliest_carrying(p[0], p[1], hit.get("kind") or "mode", [uid], start) if p and start else (h.get("file_at") or 0)
        return {"uids": uids, "owner_id": owner_id, "owner": owner, "file_at": file_at, "holder_at": holder_at, "may_take": may_take}

    def _uid_evidence(self, model, gb, facts, item, uids):
        """The GameBanana date of this page's earliest upload that carries the entry's uids: the file
        being registered, or (only when another member holds one of them) an older file of the same
        page whose package declares them too."""
        # (looked up for every uid, not only disputed ones: the earliest date proven now is stored and
        # stays the owner's evidence even if that old file later leaves the page)
        at = item.get("fileAt") or 0
        return self._earliest_carrying(model, gb, facts["kind"], uids, at) if uids else at

    def notify_uid_conflict(self, cid):
        c = self.store.uid_conflict(cid)
        if not c:
            return
        text = (f"**uid conflict** `{c['uid']}` between `{c['holder']}` (has it) and `{c['other']}`: {discord_text(c['why'], 300)}\n"
                f"Give it to one: `lsmod.py give-uid {c['uid']} {c['holder']}` or `lsmod.py give-uid {c['uid']} {c['other']}`")
        self.notifier.card("uid", c["holder"], text, report_id=cid)

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
            # the reason the moderators gave (written for the author); never who gave it
            out["reason"] = row["reason"]
        return out

    # -- reports --

    def unlist(self, body, ip, now_=None):
        """(status, answer) for the app's POST /community/unlist {key, action: unlist|list}: the
        author takes a creation off the list (or puts it back). The app never signs in to
        GameBanana, so the proof is the page itself, as for registrations: the first ask gets a
        short code; the author puts it anywhere in the page's description on GameBanana and asks
        again; the service reads the page and acts only when the code is there and the page's
        submitter is the member who registered it. A team hold is never undone."""
        ip = visitor_key(ip)
        if not self.unlist_ip.allow(ip):
            return 429, {"error": "Too many tries from here. Try again in an hour."}
        try:
            req = json.loads(body.decode("utf-8")) if len(body) <= 2048 else None
        except (UnicodeDecodeError, ValueError):
            req = None
        if not isinstance(req, dict) or req.get("action") not in ("unlist", "list") or not isinstance(req.get("key"), str):
            return 400, {"error": "Say which creation and what to do."}
        p = parse_key(req["key"])
        row = self.store.get(key_of(*p)) if p else None
        if not row:
            return 404, {"error": "That creation isn't on the list."}
        action = req["action"]
        want_from = "listed" if action == "unlist" else "unlisted"
        if row["hidden_by"] == "team" or row["state"] in ("pulled", "rejected"):
            return 409, {"error": "The Loner-Strike team has a hold on this one, so only they can change it. Ask in the Discord."}
        if row["state"] != want_from:
            return 409, {"error": "It's already unlisted." if action == "unlist" and row["state"] == "unlisted"
                         else "It's already on the list." if action == "list" and row["state"] == "listed"
                         else f"It can't be {'unlisted' if action == 'unlist' else 'listed again'} while it's {row['state']}."}
        t = now_ or now()
        ck = (row["key"], action)
        held = self.unlist_codes.get(ck)
        if not held or held[1] < t:
            code = f"ls-{'unlist' if action == 'unlist' else 'list'}-" + "".join(secrets.choice("abcdefghjkmnpqrstuvwxyz23456789") for _ in range(6))
            self.unlist_codes[ck] = (code, t + UNLIST_CODE_TTL)
            return 200, {"state": "needs-proof", "code": code, "how": UNLIST_HOW.format(code=code)}
        code = held[0]
        got, page = fetch_page(row["model"], row["gb"], self.get)
        if got != "ok":
            return 200, {"state": "needs-proof", "code": code, "how": UNLIST_HOW.format(code=code),
                         "note": "GameBanana didn't show the page just now. Try again in a minute."}
        # (straight from the page: page_facts refuses an untagged page, which List again must still see)
        submitter = _d(page.get("_aSubmitter")).get("_idRow")
        if code not in json.dumps(page, ensure_ascii=False).lower():
            return 200, {"state": "needs-proof", "code": code, "how": UNLIST_HOW.format(code=code),
                         "note": "The code isn't on the page yet (GameBanana can take a minute to show an edit)."}
        if not submitter or submitter != row["submitter_id"]:
            return 403, {"error": "That page's submitter isn't the GameBanana member who registered it, so it can't be changed from here. Ask in the Discord."}
        if action == "list":
            # the page as it is now: anything that would pull it pulls it; a missing or replaced
            # file means registering the new one first
            check = dict(row)
            check["state"] = "listed"
            verdict, why = self._verdict(check)
            if verdict == "pull":
                row.update(state="pulled", reason=why, updated=t, proposed=None, hidden_by="recheck")
                self.store.put(row, "recheck pull", why)
                return 409, {"error": f"It can't be listed again: {why}"}
            if verdict != "ok":
                return 409, {"error": f"It can't be listed again yet: {why or 'the page has changed'} Register the page's current file in Publish first."}
        del self.unlist_codes[ck]
        if action == "unlist":
            row.update(state="unlisted", hidden_by="creator", reason="Unlisted by the author.", updated=t)
        else:
            row.update(state="listed", hidden_by="", reason="", updated=t)
        self.store.put(row, f"creator {action}", f"code {code}")
        log(f"creator {action} {row['key']}")
        return 200, {"state": row["state"], "note": "You can take the code off the page now." }

    def report(self, body, ip):
        """(status, answer) for the app's POST /community/report: a player says something's wrong
        with a creation. Kept 90 days with a reason, an optional note and the reporter's hash."""
        ip = visitor_key(ip)
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
        if not self.report_day.allow(who) or not self.report_all.allow("all"):
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

    def block(self, submitter_id, name, by, reason, pull=False):
        """That GameBanana member's next submissions wait for a moderator; with `pull` (the
        repeat-infringer rule, docs/notes/T.md) their listings are pulled too. Returns what was pulled."""
        if not isinstance(submitter_id, int) or submitter_id <= 0:
            raise Reject(400, "A GameBanana member id, please.")
        self.store.set_blocked(submitter_id, one_line(name or "", 40), one_line(by, 40), one_line(reason or "", MAX_REASON), True)
        pulled = []
        if pull:
            for row in self.store.rows(("listed", "hidden", "pending")):
                if row["submitter_id"] == submitter_id:
                    # (one row that can't be written never stops the others)
                    try:
                        self.moderate(row["key"], "pull", {"reason": one_line(reason or "", MAX_REASON) or "Removed: the creator's listings were taken down."})
                        pulled.append(row["key"])
                    except Exception as e:  # noqa: BLE001
                        self._write_failed(row["key"], "pull", e)
        return pulled

    def _creature_files_owned(self, item, facts, sizes):
        """The files a creature-carrying package ships: none of the games' own files (stock lists);
        every file in a models/gb<id>/ or sound/gb<id>/ folder named by its creatures.ini, and
        every file it names that ships in such a folder; the uploader's own id, or another
        member's (a carried creature) only when byte-identical to that member's listed file. A
        creature package ships nothing in models/ or sound/ its creatures don't use."""
        checks = item.get("checks", {})
        if not checks.get("creatureSections") and not checks.get("npcFiles") and item.get("kind") != "creature":
            return
        member = int(facts["submitterId"])
        refs = set(checks.get("creatureRefs") or [])
        files = checks.get("creatureFiles") or {}
        shipped = [p for p in (norm_path(n) for n in sizes) if p and not p.endswith("/")]
        # (game content only: the package's own readme and data aren't installed into the game)
        stock = [p for p in shipped if p.partition("/")[0] not in ("creatures", "source", "prefabs", "modes")
                 and p not in ("readme.txt", "loner-strike.json") and is_stock(p)]
        if stock:
            raise Reject(422, f"The package ships the game's own {stock[0]}. Leave the game's files out: players have them.")
        listed = None
        for p in shipped:
            top, _, rest = p.partition("/")
            if top not in ("models", "sound"):
                continue
            folder = rest.partition("/")[0]
            ns = folder if re.match(r"^gb[1-9][0-9]{0,8}$", folder) and "/" in rest else ""
            if (ns or item.get("kind") == "creature") and p not in refs:
                raise Reject(422, f"{p} isn't used by any of the package's creatures. Make it again with Publish.")
            if p in refs and not ns:
                raise Reject(422, f"{p} must be in {top}/gb{member}/ (the creature files' own folder). Make it again with Publish.")
            if ns and ns != f"gb{member}":
                owner = int(ns[2:])
                if listed is None:
                    listed = [((_j(r["item"]) or {}).get("checks") or {}).get("creatureFiles") or {}
                              for r in self.store.rows(("listed",)) if r["submitter_id"] == owner]
                if not any(x.get(p) == files.get(p) and files.get(p) for x in listed):
                    raise Reject(422, f"{p} is in {ns}'s folder but isn't the same as that creator's listed file.")
                listed = None

    def _write_failed(self, key, what, e):
        """A moderation or re-check write that failed for one entry: logged and told to the
        moderators; the caller carries on with the rest."""
        msg = e.message if isinstance(e, Reject) else f"{type(e).__name__}: {e}"
        log(f"couldn't {what} {key}: {msg}")
        try:
            self.notifier.say(f"**Couldn't {what} `{key}`:** {discord_text(msg)}. The rest went on; look at it by hand.")
        except Exception:  # noqa: BLE001
            pass

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
        """The entry as the page and package are now, which must still be the same file."""
        try:
            facts, fresh = self._verify_file(row["model"], row["gb"], item.get("file", 0))
        except Reject as e:
            if e.code == 502:
                raise Reject(502, f"{e.message} Nothing changed.")
            raise Reject(409, f"Not done: the page doesn't pass the checks now. {e.message}")
        if fresh["md5"] != item.get("md5"):
            raise Reject(409, "Not done: the file on the page changed. A new registration brings the new one.")
        return fresh

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
                cand = self._verify_live(row, cand)
            row.update(item=json.dumps(cand, ensure_ascii=False), proposed=None, state="listed", reason="", updated=t, rejected_md5="",
                       hidden_by="", check_note="")
            # its uids are settled now, as for any listing (a held submission took none)
            uids = item_uids(cand)
            p = parse_key(key)
            at = self._earliest_carrying(p[0], p[1], cand.get("kind") or "mode", uids, cand.get("fileAt") or 0) if uids and live else (cand.get("fileAt") or 0)
            for cid in self.store.put(row, "approve", reason, claim=self._claim(uids, row["submitter_id"], row["submitter"], at)):
                self.notify_uid_conflict(cid)
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
            if row["state"] not in ("hidden", "pulled", "withdrawn") or not row["item"]:
                raise Reject(409, f"{row['key']} is {row['state']}; only a hidden, pulled or withdrawn entry that was listed can be restored.")
            item = self._verify_live(row, _j(row["item"]))
            row.update(item=json.dumps(item, ensure_ascii=False), state="listed", reason="", updated=t, check_note="", hidden_by="")
            self.store.put(row, "restore", reason)
        elif action == "hide":
            row = self._need(key)
            if row["state"] != "listed":
                raise Reject(409, f"{row['key']} is {row['state']}; only a listed entry can be hidden.")
            row.update(state="hidden", reason=reason, updated=t, hidden_by="team")
            self.store.put(row, "hide", reason)
        elif action == "usefile":
            # a moderator choosing which of the page's files is listed (an older one too)
            row = self._need(key)
            fid = body.get("file")
            if isinstance(fid, bool) or not isinstance(fid, int) or fid <= 0:
                raise Reject(400, "Which file? Its GameBanana file id.")
            if not row["item"]:
                raise Reject(409, f"{row['key']} isn't listed; approve it first.")
            try:
                _, item = self._verify_file(row["model"], row["gb"], fid)
            except Reject as e:
                raise Reject(e.code if e.code == 502 else 409, f"Not done: {e.message}")
            row.update(item=json.dumps(item, ensure_ascii=False), updated=t, check_note="")
            self.store.put(row, "usefile", f"file {fid} {reason}".strip())
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
        # (the daily housekeeping: what's past its time goes; see Store.housekeeping)
        try:
            gone = self.store.housekeeping()
            if any(gone.values()):
                log(f"housekeeping: {', '.join(f'{k} {v}' for k, v in gone.items() if v)}")
        except Exception as e:  # (never stops the re-check)
            log(f"housekeeping failed: {e}")
        try:
            rows = [self.store.get(k) for k in keys] if keys else self.store.rows(("pending", "listed", "hidden", "withdrawn", "unlisted"))
            rows = [r for r in rows if r and r["state"] in ("pending", "listed", "hidden", "withdrawn", "unlisted")]
            verdicts = []
            for i, row in enumerate(rows):
                if i and pause:
                    time.sleep(pause)
                verdicts.append((row, *self._verdict(row)))
            # many pages leaving the list at once is more likely GameBanana's API having a bad day (or
            # changing shape) than real: more than 3, or 2+ and over a quarter, waits for a person
            gone = sum(1 for _, v, _ in verdicts if v in ("pull", "reject", "hide", "withdraw", "withdraw-review"))
            suspicious = not keys and (gone > 3 or (gone >= 2 and gone > len(rows) * RECHECK_GONE_SHARE))
            changed, t = [], now()
            for row, verdict, why in verdicts:
                if verdict == "error":
                    self.store.touch(row["key"], row["checked"], f"couldn't check: {why}")
                    continue
                if suspicious and verdict in ("pull", "reject", "hide", "withdraw", "withdraw-review"):
                    self.store.touch(row["key"], row["checked"], f"would be {verdict}ed ({why}), held back")
                    continue
                self.store.touch(row["key"], t, why if verdict != "ok" else "")
                if verdict == "ok":
                    continue
                row = self.store.get(row["key"])
                # a hold the team put on stays the team's (only a moderator lifts it)
                by = "team" if row["hidden_by"] == "team" else "recheck"
                if verdict in ("pull", "reject", "hide", "withdraw"):
                    row["hidden_by"] = by
                if verdict == "review-back":
                    # (back on GameBanana, but the removal may have been a moderator's: ours decide)
                    self.notifier.say(f"**Back on GameBanana:** `{row['key']}` was deleted there and is back. "
                                      f"`lsmod.py restore {row['key']}` lists it again after a look.")
                    continue
                if verdict == "pull":
                    row.update(state="pulled", reason=why, updated=t, proposed=None)
                elif verdict == "reject":
                    row.update(state="rejected", reason=why, updated=t, proposed=None,
                               rejected_md5=(_j(row["proposed"]) or {}).get("md5", ""))
                elif verdict == "hide":
                    row.update(state="hidden", reason=why, updated=t)
                elif verdict == "drop":
                    row.update(proposed=None, updated=t)
                elif verdict == "withdraw":
                    row.update(state="withdrawn", reason=why, updated=t)
                elif verdict == "withdraw-review":
                    # hidden like a withdrawal, but only a moderator lists it again (restore)
                    row.update(state="withdrawn", reason=why, updated=t, hidden_by="review" if row["hidden_by"] != "team" else "team")
                elif verdict == "relist":
                    # (the page and its file came back; a team hold or a moderator review never lifts by itself)
                    if row["hidden_by"] in ("team", "review"):
                        continue
                    row.update(state="listed", reason="", hidden_by="", updated=t)
                try:
                    self.store.put(row, f"recheck {verdict}", why)
                except Exception as e:  # noqa: BLE001 (one bad row never stops the re-check)
                    self._write_failed(row["key"], f"re-check ({verdict})", e)
                    continue
                changed.append((row["key"], verdict, why))
            summary = {"at": t, "checked": len(rows), "changed": len(changed), "heldBack": suspicious}
            if not keys:
                self.store.set_meta("last_recheck", summary)
            if suspicious:
                self.notifier.say(f"**Re-check held back:** {gone} of {len(rows)} pages would have left the list at once, which "
                                  "is more than looks real. Nothing changed and publishing waits. Check GameBanana, then "
                                  "`lsmod.py release-hold` (or `lsmod.py recheck` again).")
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
        withdrawn = row["state"] == "withdrawn"
        unlisted = row["state"] == "unlisted"
        listed = row["state"] in ("listed", "hidden", "withdrawn", "unlisted")
        lose = "pull" if listed else "reject"
        if got == "withheld":
            # GameBanana's moderation: fail closed, whatever state it's in (installed copies are told)
            return lose, page
        if got in ("gone", "private"):
            if withdrawn or unlisted:
                return "ok", row["reason"] or page
            if not listed:
                return lose, page
            # a private page is the author's doing and comes back with them; a deleted one may have
            # been a moderator's: hidden the same, but only our moderators list it again
            return ("withdraw" if got == "private" else "withdraw-review"), page
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
            if not f:
                if withdrawn or unlisted:
                    return "ok", row["reason"] or ""
                return "withdraw", "The listed file was removed from GameBanana."
            if f["md5"] != it.get("md5"):
                if row["state"] in ("hidden", "unlisted"):
                    return "ok", row["check_note"] or ""
                return "hide", "The listed file was replaced on GameBanana."
            if withdrawn:
                if row["hidden_by"] == "review":
                    return "review-back", "The page is back on GameBanana; a moderator can list it again."
                return "relist", "The page and its file are back on GameBanana."
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

    def _authorized(self, h, ip, path):
        """None when the request carries the admin token, else (status, why)."""
        if self.token_digest is None:
            return 503, "Moderation is off (no admin token set)."
        if self.admin_local and (h.client_address[0] != "127.0.0.1" or h.headers.get("CF-Connecting-IP")):
            return 403, "Moderation only answers on this machine (ssh -L 8100:127.0.0.1:8100 to the box)."
        # too many wrong tokens from one place: even a right one is refused for the hour
        if self.admin_fails.full(ip):
            return 429, "Too many wrong tokens from here. Try again in an hour."
        auth = h.headers.get("Authorization") or ""
        given = hashlib.sha256((auth[7:].strip() if auth[:7].lower() == "bearer " else "").encode()).digest()
        # both sides hashed first, so the comparison takes the same time whatever the length
        if hmac.compare_digest(given, self.token_digest):
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
            if parts in (["submit"], ["report"], ["unlist"]):
                if method != "POST":
                    return reply(h, 405, {"error": "POST only."})
                body = read_body(h)
                code, answer = {"submit": self.submit, "report": self.report, "unlist": self.unlist}[parts[0]](body, ip)
                return reply(h, code, answer, cors=False)
            if len(parts) == 3 and parts[0] == "submission" and method == "GET":
                if not self.status_limit.allow(visitor_key(ip)):
                    return reply(h, 429, {"error": "Too many questions from here. Try again in a minute."})
                p = parse_key(f"{parts[1]}:{parts[2]}")
                if not p:
                    return reply(h, 404, {"error": "not found"})
                return reply(h, 200, self.status(*p))
            if parts[:1] == ["publish"] and method == "GET" and parts[1:] in (["index"], ["index.sha256"]):
                return self.publish_export(h, visitor_key(ip), parts[1] == "index.sha256")
            if parts[:1] == ["admin"]:
                denied = self._authorized(h, visitor_key(ip), (method, "/".join(parts[1:])))
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

    def publish_export(self, h, ip, sha_only):
        """GET /community/publish/index(.sha256) for the publish workflow: exactly what
        `lsmod.py publish` uploads, with X-Recheck-Held-Back (the workflow doesn't publish then).
        Only the publish token opens it (not the admin token: a token leaked from CI can't
        moderate), through the tunnel too, rate-limited, wrong tokens locking out for an hour."""
        if self.publish_digest is None:
            return reply(h, 503, {"error": "Publishing is off (no publish token set)."}, cors=False)
        if self.publish_fails.full(ip) or not self.publish_limit.allow(ip):
            return reply(h, 429, {"error": "Too many requests. Try again later."}, cors=False)
        auth = h.headers.get("Authorization") or ""
        given = hashlib.sha256((auth[7:].strip() if auth[:7].lower() == "bearer " else "").encode()).digest()
        if not hmac.compare_digest(given, self.publish_digest):
            self.publish_fails.allow(ip)
            return reply(h, 401, {"error": "Wrong or missing publish token."}, cors=False)
        data, sha = build_index(self.store)
        held = bool((self.store.meta("last_recheck", {}) or {}).get("heldBack"))
        headers = {"X-Index-Sha256": sha, "X-Recheck-Held-Back": "1" if held else "0"}
        if sha_only:
            h.send_response(200)
            body = f"{sha}  index.json\n".encode()
            h.send_header("Content-Type", "text/plain; charset=utf-8")
            h.send_header("Content-Length", str(len(body)))
            h.send_header("Cache-Control", "no-store")
            for k, v in headers.items():
                h.send_header(k, v)
            h.end_headers()
            if h.command != "HEAD":
                h.wfile.write(body)
            return None
        return reply(h, 200, None, cors=False, raw=data, headers=headers)

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
        if parts[:1] == ["roles"]:
            if not getattr(self, "rolesvc", None):
                raise Reject(404, "Community roles are off (no --roles-db).")
            return reply(h, 200, self.rolesvc.admin(method, parts[1:], body, query), cors=False)
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
                    self.block(sid, (row or {}).get("submitter", ""), "lsmod", body.get("reason") if isinstance(body.get("reason"), str) else "",
                               pull=body.get("pull") is True)
            return reply(h, 200, {"blocked": self.store.blocked()}, cors=False)
        if parts == ["give-uid"] and method == "POST":
            uid = norm_uid(body.get("uid"))
            p = parse_key(body.get("page")) if isinstance(body.get("page"), str) else None
            if not uid or not p:
                raise Reject(400, "Send {\"uid\": \"<uuid>\", \"page\": \"mod:<id>\"}.")
            self.store.give_uid(uid, key_of(*p), "lsmod")
            return reply(h, 200, {"ok": True, "uid": uid, "owner": self.store.uid_owner(uid)}, cors=False)
        if parts == ["uid"] and method == "POST":
            uid = norm_uid(body.get("uid"))
            member = body.get("member")
            if not uid or isinstance(member, bool) or not isinstance(member, int) or member <= 0:
                raise Reject(400, "Send {\"uid\": \"<uuid>\", \"member\": <GameBanana member id>}.")
            name = one_line(body.get("name") if isinstance(body.get("name"), str) else "", 40)
            self.store.set_uid_owner(uid, member, name, "lsmod")
            return reply(h, 200, {"ok": True, "uid": uid, "owner": self.store.uid_owner(uid)}, cors=False)
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
        if parts == ["hold"] and method == "POST":
            # a person looked after a held-back re-check: publishing may go on
            if body.get("held") is not False:
                raise Reject(400, "Send {\"held\": false} to let publishing go on.")
            last = self.store.meta("last_recheck", {}) or {}
            self.store.set_meta("last_recheck", dict(last, heldBack=False, released=now()))
            return reply(h, 200, {"ok": True}, cors=False)
        if parts == ["published"] and method == "POST":
            sha = body.get("sha256")
            if not isinstance(sha, str) or not _HEX64.match(sha):
                raise Reject(400, "sha256, please.")
            self.store.set_meta("published", {"sha256": sha, "at": now()})
            return reply(h, 200, {"ok": True}, cors=False)
        return reply(h, 404, {"error": "not found"}, cors=False)


def read_body(h):
    """The request body, at most MAX_BODY bytes: by Content-Length, or chunked (Cloudflare's
    tunnel may pass a body on that way)."""
    te = (h.headers.get("Transfer-Encoding") or "").strip().lower()
    if te:
        if te != "chunked" or h.headers.get("Content-Length"):
            raise Reject(400, "Unsupported Transfer-Encoding.")
        out = b""
        while True:
            line = h.rfile.readline(64)
            try:
                n = int(line.split(b";")[0].strip(), 16)
            except ValueError:
                raise Reject(400, "Bad chunked body.")
            if n < 0 or len(out) + n > MAX_BODY:
                raise Reject(413, "The request is too big.")
            if n == 0:
                while h.rfile.readline(1024).strip():
                    pass  # trailers
                return out
            out += h.rfile.read(n)
            h.rfile.readline(8)
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
