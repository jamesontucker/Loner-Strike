#!/usr/bin/env python3
# Community roles (docs/notes/Roles.md, docs/ROLES.md): servers claimed by their hosts, the
# Steam <-> Discord link, Supporter VIP from Polar, and server moderator applications with invites.
#
# Kept in its own SQLite database (--roles-db). Everything that arrives is untrusted: the Polar
# webhook (Standard Webhooks signature, checked in constant time, each delivery once), server
# requests (a server token, kept only as SHA-256), codes (made here with `secrets`, kept only as
# SHA-256, one use, short-lived) and Discord interactions (modbot.py checks Discord's signature;
# this checks who may do what). Rate limits on everything a stranger can call.
#
# HTTP (under /roles/, through serverlist.py):
#   POST /roles/claim/start, /roles/claim/finish           a host proves they run a server
#   GET|POST|DELETE /roles/server, POST /roles/server/owner-code
#   POST /roles/link/code                                  a code for a player who typed /link
#   GET /roles/vip, POST /roles/vip/check                  supporters' SteamIDs for servers
#   GET /roles/applications, POST /roles/applications/<id>/(invite|redeemed|revoke)
#   POST /roles/polar/webhook
# Discord (through modbot.py): /link, /unlink, /server claim, "Server moderator" in Apply, the cards.
import base64
import hashlib
import hmac
import json
import re
import secrets
import sqlite3
import threading
import time
import urllib.parse

import community as cm

CODE_SECS = 600                 # link codes, owner codes and claims
INVITE_SECS = 24 * 3600 + 120   # a shared-control invite works a day (a little slack for clocks)
SUPPORTER_DAYS = 30             # per one-time donation, stacking (0: for good)
SUPPORTER_MIN_CENTS = 300       # a donation counts from this much (net of discounts), in cents
FOREVER = 253402300799          # 9999-12-31: "for good" (SUPPORTER_DAYS 0)
# currencies a donation counts in, by their own cents: each is worth at least a US dollar, so the
# minimum is never undercut. Others aren't counted automatically (logged for the team).
CURRENCIES = ("usd", "eur", "gbp", "chf")
WEBHOOK_SKEW = 300
MAX_STEAMIDS = 64
MAX_BODY = 16 * 1024
COOLDOWN = 14 * 86400           # after a no, before the same server can be asked again
PER_DAY = 5
EPHEMERAL = 64
TOKEN_PREFIX = "lsr1_"
# Crockford base32: no I, L, O, U
ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
STEAMID = re.compile(r"^STEAM_[0-5]:[01]:\d{1,10}$")
SNOWFLAKE = re.compile(r"^\d{15,21}$")
SETTINGS = ("applications", "discord_mods_decide", "honor_supporters")
MOD_ROLE_NAME = "Server Moderator"
SUPPORTER_ROLE_NAME = "Supporter"
SYNC_EVERY = 3600               # the supporters' Discord role checked for everyone known
SYNC_PAUSE = 0.5                # between Discord calls in a check (well inside its rate limits)
SYNC_MAX = 500                  # accounts in one check (the rest at the next)
STEAMS_PER_DAY = 20
# the Discord side's actions (signed button ids, modbot.Mod.custom_id)
PUBLIC_ACTIONS = ("smpick", "smform")
DECIDER_ACTIONS = ("smok", "smno", "smnom")
AVAIL_MAX, TEXT_MAX, WHY_MIN = 300, 1000, 20

MIGRATIONS = [
    """
    CREATE TABLE servers (id INTEGER PRIMARY KEY AUTOINCREMENT, address TEXT UNIQUE NOT NULL, name TEXT NOT NULL DEFAULT '',
        token_hash TEXT NOT NULL, official INTEGER NOT NULL DEFAULT 0, owner_discord TEXT NOT NULL DEFAULT '',
        owner_code_hash TEXT NOT NULL DEFAULT '', owner_code_expires INTEGER NOT NULL DEFAULT 0,
        settings TEXT NOT NULL DEFAULT '{}', claimed_at INTEGER NOT NULL);
    CREATE INDEX servers_token ON servers(token_hash);
    CREATE TABLE claims (id TEXT PRIMARY KEY, address TEXT NOT NULL, nonce TEXT NOT NULL, expires INTEGER NOT NULL);
    CREATE TABLE link_codes (code_hash TEXT PRIMARY KEY, steamid TEXT NOT NULL, name TEXT NOT NULL DEFAULT '',
        server_id INTEGER NOT NULL, expires INTEGER NOT NULL);
    CREATE TABLE links (discord_id TEXT PRIMARY KEY, steamid TEXT UNIQUE NOT NULL, name TEXT NOT NULL DEFAULT '',
        server_id INTEGER NOT NULL, official INTEGER NOT NULL DEFAULT 0, linked_at INTEGER NOT NULL);
    CREATE TABLE polar_events (id TEXT PRIMARY KEY, at INTEGER NOT NULL);
    CREATE TABLE polar_customers (customer_id TEXT PRIMARY KEY, discord_id TEXT NOT NULL);
    CREATE TABLE polar_grants (grant_id TEXT PRIMARY KEY, customer_id TEXT NOT NULL, discord_id TEXT NOT NULL,
        monthly INTEGER NOT NULL, granted INTEGER NOT NULL, at INTEGER NOT NULL);
    CREATE TABLE polar_orders (order_id TEXT PRIMARY KEY, customer_id TEXT NOT NULL, applied INTEGER NOT NULL DEFAULT 0,
        at INTEGER NOT NULL);
    CREATE TABLE supporters (discord_id TEXT PRIMARY KEY, until INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE sm_apps (id INTEGER PRIMARY KEY AUTOINCREMENT, server_id INTEGER NOT NULL, discord_id TEXT NOT NULL,
        discord_name TEXT NOT NULL, steamid TEXT NOT NULL, linked_on TEXT NOT NULL DEFAULT '', official_link INTEGER NOT NULL DEFAULT 0,
        availability TEXT NOT NULL, experience TEXT NOT NULL DEFAULT '', why TEXT NOT NULL, other TEXT NOT NULL DEFAULT '',
        state TEXT NOT NULL, decider TEXT NOT NULL DEFAULT '', decider_name TEXT NOT NULL DEFAULT '', decided_at INTEGER NOT NULL DEFAULT 0,
        reason TEXT NOT NULL DEFAULT '', created INTEGER NOT NULL, invite_hash TEXT NOT NULL DEFAULT '', invite_expires INTEGER NOT NULL DEFAULT 0,
        member TEXT NOT NULL DEFAULT '', cards TEXT NOT NULL DEFAULT '[]', note TEXT NOT NULL DEFAULT '');
    CREATE INDEX sm_apps_server ON sm_apps(server_id, state);
    CREATE INDEX sm_apps_user ON sm_apps(discord_id);
    CREATE TABLE roles_log (id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL, server_id INTEGER NOT NULL DEFAULT 0,
        actor TEXT NOT NULL, action TEXT NOT NULL, subject TEXT NOT NULL DEFAULT '', detail TEXT NOT NULL DEFAULT '')
    """,
    # when Polar says a grant changed (events can come out of order)
    """
    ALTER TABLE polar_grants ADD COLUMN event_at REAL NOT NULL DEFAULT 0
    """,
    # refunds: a one-time donation's 30 days come off again (supporters.until is worked out from the orders)
    """
    ALTER TABLE polar_orders ADD COLUMN refunded INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE polar_orders ADD COLUMN applied_at REAL NOT NULL DEFAULT 0;
    UPDATE polar_orders SET applied_at=at WHERE applied=1
    """,
    # each donation keeps the Discord account it counted for (a customer's account can change), and
    # what it was (amounts under the minimum, or in another currency, never count)
    """
    ALTER TABLE polar_orders ADD COLUMN applied_to TEXT NOT NULL DEFAULT '';
    ALTER TABLE polar_orders ADD COLUMN amount_cents INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE polar_orders ADD COLUMN currency TEXT NOT NULL DEFAULT '';
    ALTER TABLE polar_orders ADD COLUMN ignored INTEGER NOT NULL DEFAULT 0;
    UPDATE polar_orders SET applied_to=COALESCE((SELECT discord_id FROM polar_customers c WHERE c.customer_id=polar_orders.customer_id), '') WHERE applied=1
    """,
    # the Supporter role: who Polar or our bot gave it to (only those are ever taken away again; a role
    # given by hand is never in here), checks to try again, and where the hourly check got to
    """
    CREATE TABLE role_granted (discord_id TEXT PRIMARY KEY, source TEXT NOT NULL, at INTEGER NOT NULL);
    CREATE TABLE role_pending (discord_id TEXT PRIMARY KEY, tries INTEGER NOT NULL DEFAULT 0, next_at INTEGER NOT NULL, why TEXT NOT NULL DEFAULT '');
    CREATE TABLE roles_meta (k TEXT PRIMARY KEY, v TEXT NOT NULL);
    INSERT OR IGNORE INTO role_granted(discord_id, source, at) SELECT DISTINCT discord_id, 'polar', at FROM polar_grants WHERE granted=1
    """,
]


def sha(s):
    return hashlib.sha256(s.encode()).hexdigest()


def new_code():
    return "".join(secrets.choice(ALPHABET) for _ in range(8))


def show_code(c):
    return f"{c[:4]}-{c[4:]}"


def norm_code(text):
    """What a person typed, as a code: case, spaces and hyphens ignored, O read as 0 and I/L as 1."""
    t = re.sub(r"[\s-]", "", str(text or "")).upper().translate(str.maketrans("OIL", "011"))
    return t if len(t) == 8 and all(c in ALPHABET for c in t) else None


def valid_steamid(s):
    return isinstance(s, str) and bool(STEAMID.match(s))


def canon(s):
    """One form per account: STEAM_1:1:123 and STEAM_0:1:123 are the same, kept as STEAM_0:1:123
    (None when it isn't a SteamID)."""
    return "STEAM_0" + s[7:] if valid_steamid(s) else None


class Store:
    def __init__(self, path):
        self.lock = threading.RLock()
        self.db = sqlite3.connect(path, check_same_thread=False, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA journal_mode=WAL")
        self.db.execute("PRAGMA busy_timeout=5000")
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
            self.canonical_steamids()

    def canonical_steamids(self):
        """Every SteamID in its one form (STEAM_0:Y:Z), on every start. Two links that turn out to be
        one account: the official one is kept, else the older one; the other is dropped."""
        with self.lock:
            for table in ("link_codes", "sm_apps"):
                self.db.execute(f"UPDATE {table} SET steamid='STEAM_0'||substr(steamid, 8) WHERE steamid GLOB 'STEAM_[1-5]:*'")
            for r in self.db.execute("SELECT * FROM links WHERE steamid GLOB 'STEAM_[1-5]:*'").fetchall():
                c = "STEAM_0" + r["steamid"][7:]
                have = self.db.execute("SELECT * FROM links WHERE steamid=?", (c,)).fetchone()
                if have and (have["official"], -have["linked_at"]) >= (r["official"], -r["linked_at"]):
                    self.db.execute("DELETE FROM links WHERE discord_id=?", (r["discord_id"],))
                    continue
                if have:
                    self.db.execute("DELETE FROM links WHERE discord_id=?", (have["discord_id"],))
                self.db.execute("UPDATE links SET steamid=? WHERE discord_id=?", (c, r["discord_id"]))

    def one(self, sql, args=()):
        with self.lock:
            r = self.db.execute(sql, args).fetchone()
        return dict(r) if r else None

    def all(self, sql, args=()):
        with self.lock:
            return [dict(r) for r in self.db.execute(sql, args).fetchall()]

    def run(self, sql, args=()):
        with self.lock:
            return self.db.execute(sql, args)


class Roles:
    def __init__(self, store, clean=None, asker=None, players=None, polar_secret="", mod=None, deciders=(), days=SUPPORTER_DAYS,
                 min_cents=SUPPORTER_MIN_CENTS, products=()):
        self.s = store
        # serverlist.clean_address, serverlist.check and serverlist.players (A2S), passed in to keep
        # the imports one way
        self.clean, self.asker, self.players = clean, asker, players
        self.polar_key = webhook_key(polar_secret)
        self.mod = mod
        self.deciders = {str(r) for r in deciders if r}
        # days per one-time donation (stacking); 0: a donation makes them a supporter for good
        self.days = max(0, int(days))
        self.min_cents = max(0, int(min_cents))
        # the Polar products that make a Supporter (POLAR_SUPPORTER_PRODUCTS); anything else the
        # organization sells never does. None set: nothing counts (fails closed)
        self.products = {str(p).strip() for p in products if str(p).strip()}
        if self.polar_key and not self.products:
            cm.log("roles: POLAR_SUPPORTER_PRODUCTS isn't set, so no Polar payment or grant makes a Supporter. "
                   "Set the support products' ids in the env file.")
        self.lock = threading.RLock()
        if mod is not None:
            mod.rolesvc = self
        self.claim_limit = cm.Window(10, 3600)
        self.code_per_steam = cm.Window(3, CODE_SECS)
        self.code_per_server = cm.Window(60, CODE_SECS)
        # distinct SteamIDs a server may make codes for in a day (a host can write any SteamID in
        # their own log: this caps how many they could try to squat)
        self.steams_per_server = {}
        self.link_fails = cm.Window(5, 3600)
        self.owner_fails = cm.Window(5, 3600)
        self.per_token = cm.Window(120, 60)
        self.bad_token = cm.Window(20, 3600)
        self.webhook_limit = cm.Window(120, 60)
        self.per_day = cm.Window(PER_DAY, 86400)

    # ---- the log ----

    def log(self, action, actor="", server=0, subject="", detail=""):
        self.s.run("INSERT INTO roles_log(at, server_id, actor, action, subject, detail) VALUES (?,?,?,?,?,?)",
                   (cm.now(), int(server or 0), str(actor)[:64], action, str(subject)[:64], cm.one_line(str(detail), 300)))
        cm.log(f"roles: {action} {subject}".strip())

    def audit(self, line):
        if self.mod:
            self.mod.audit(line)

    # ---- servers ----

    def server_by_token(self, token):
        if not isinstance(token, str) or not token.startswith(TOKEN_PREFIX) or len(token) > 80:
            return None
        h = sha(token)
        row = self.s.one("SELECT * FROM servers WHERE token_hash=?", (h,))
        return row if row and hmac.compare_digest(row["token_hash"], h) else None

    @staticmethod
    def settings_of(row):
        try:
            s = json.loads(row["settings"] or "{}")
        except ValueError:
            s = {}
        out = {k: bool(s.get(k)) for k in SETTINGS}
        if row["official"]:
            out["discord_mods_decide"] = True
        return out

    def server_view(self, row):
        return {"server": row["id"], "name": row["name"], "address": row["address"], "official": bool(row["official"]),
                "owner_linked": bool(row["owner_discord"]), "settings": self.settings_of(row)}

    def claim_start(self, body, ip):
        if not self.claim_limit.allow(cm.visitor_key(ip)):
            raise cm.Reject(429, "Too many claims from here. Try again in an hour.")
        found = self.clean(body.get("address")) if self.clean and isinstance(body.get("address"), str) else None
        if not found:
            raise cm.Reject(400, "That isn't an address on the internet, like play.example.com:27015.")
        cid, nonce = secrets.token_hex(8), "lsc-" + secrets.token_hex(8)
        self.s.run("DELETE FROM claims WHERE expires < ?", (cm.now(),))
        self.s.run("INSERT INTO claims(id, address, nonce, expires) VALUES (?,?,?,?)", (cid, found[0], nonce, cm.now() + CODE_SECS))
        return {"claim": cid, "nonce": nonce, "cvar": "ls_claim", "expires": cm.now() + CODE_SECS}

    def claim_finish(self, body, ip):
        if not self.claim_limit.allow(cm.visitor_key(ip) + ":finish"):
            raise cm.Reject(429, "Too many claims from here. Try again in an hour.")
        cid = body.get("claim") if isinstance(body.get("claim"), str) else ""
        c = self.s.one("SELECT * FROM claims WHERE id=?", (cid,)) if re.fullmatch(r"[0-9a-f]{16}", cid) else None
        if not c or c["expires"] < cm.now():
            raise cm.Reject(404, "That claim ran out. Start again.")
        info, rules = self.asker(c["address"]) if self.asker else (None, None)
        if not info:
            raise cm.Reject(409, "The server didn't answer from the internet. Check it's running and its port is open.")
        got = str((rules or {}).get("ls_claim") or "")
        if not hmac.compare_digest(got.encode(), c["nonce"].encode()):
            raise cm.Reject(409, "The server's ls_claim isn't set yet. Try again in a few seconds.")
        token = TOKEN_PREFIX + secrets.token_urlsafe(32)
        owner = new_code()
        name = cm.one_line(str(info.get("name") or c["address"]), 64)
        with self.lock:
            self.s.run("DELETE FROM claims WHERE id=?", (cid,))
            old = self.s.one("SELECT * FROM servers WHERE address=?", (c["address"],))
            removed = self.retire(old) if old else []
            # whoever runs the server now starts from nothing: not official, default settings, no
            # applications or moderators (those belonged to the old server, which keeps its own id)
            cur = self.s.run("INSERT INTO servers(address, name, token_hash, owner_code_hash, owner_code_expires, claimed_at) VALUES (?,?,?,?,?,?)",
                             (c["address"], name, sha(token), sha(owner), cm.now() + CODE_SECS, cm.now()))
            sid = cur.lastrowid
        self.log("claim", "host", sid, c["address"], f"claimed again (was server {old['id']})" if old else "")
        if old:
            what = (f"Official server {cm.discord_text(old['name'], 60)} ({old['address']}) was re-claimed; official status removed"
                    if old["official"] else f"Server {cm.discord_text(old['name'], 60)} ({old['address']}) was re-claimed by whoever runs it now")
            self.audit(what + (f"; {len(removed)} moderator(s) removed" if removed else "") + ". `lsmod.py roles servers`.")
        return {"server": sid, "token": token, "owner_code": show_code(owner), "expires": cm.now() + CODE_SECS}

    def retire(self, old):
        """A server claimed again (by a new host, or the same one starting over): the old row stops
        working and keeps its history under its own id; its open applications close and its
        moderators lose the Discord role. Their app access and in-game role are kept by the old
        host's app and game files, which the service can't reach: the old host's app finds its
        claim gone. Returns the removed moderators' Discord ids."""
        self.s.run("UPDATE servers SET address=?, token_hash='', official=0, owner_code_hash='', owner_code_expires=0 WHERE id=?",
                   (f"{old['address']}#retired-{old['id']}", old["id"]))
        removed = [x["discord_id"] for x in self.end_server(old["id"], "reclaim", "the server was claimed again")]
        self.log("retire", "reclaim", old["id"], old["address"], ("official, " if old["official"] else "") + f"{len(removed)} moderator(s) removed")
        return removed

    def owner_code(self, srv):
        code = new_code()
        self.s.run("UPDATE servers SET owner_code_hash=?, owner_code_expires=? WHERE id=?", (sha(code), cm.now() + CODE_SECS, srv["id"]))
        return {"owner_code": show_code(code), "expires": cm.now() + CODE_SECS}

    def set_settings(self, srv, body):
        given = body.get("settings")
        if not isinstance(given, dict):
            raise cm.Reject(400, "Send the settings.")
        cur = self.settings_of(srv)
        for k in SETTINGS:
            if k in given:
                if not isinstance(given[k], bool):
                    raise cm.Reject(400, f"{k} must be true or false.")
                cur[k] = given[k]
        self.s.run("UPDATE servers SET settings=? WHERE id=?", (json.dumps(cur), srv["id"]))
        self.log("settings", "host", srv["id"], "", json.dumps(cur))
        return self.server_view(self.s.one("SELECT * FROM servers WHERE id=?", (srv["id"],)))

    def unclaim(self, srv):
        """The owner's app ends the claim: its moderators are revoked like revoke() does; the answer
        lists them so the app takes their app access and in-game role away too."""
        revoked = self.end_server(srv["id"], "unclaim", "the server's claim ended")
        self.s.run("DELETE FROM servers WHERE id=?", (srv["id"],))
        self.log("unclaim", "host", srv["id"], srv["address"], f"{len(revoked)} moderator(s) removed")
        return {"ok": True, "revoked": revoked}

    def end_server(self, server_id, actor, why):
        """A server's claim ends (unclaimed, or claimed again): open applications close, invites stop
        and moderators are revoked (Discord role gone unless they moderate another server). Returns
        [{application, member, steamid}] for the ones revoked."""
        self.s.run("UPDATE sm_apps SET state='closed', invite_hash='', invite_expires=0 WHERE server_id=? AND state IN ('pending','approved')", (server_id,))
        revoked = []
        for a in self.s.all("SELECT * FROM sm_apps WHERE server_id=? AND state IN ('invited','active')", (server_id,)):
            self.s.run("UPDATE sm_apps SET state='revoked', invite_hash='', invite_expires=0, note=? WHERE id=?", (why[:1].upper() + why[1:] + ".", a["id"]))
            rid = self.mod_role()
            if rid and not self.moderating(a["discord_id"]):
                self.mod.bot.api("DELETE", f"/guilds/{self.mod.guild}/members/{a['discord_id']}/roles/{rid}",
                                 reason=f"Application #{a['id']}: {why}")
            self.log("revoke", actor, server_id, a["discord_id"], f"application #{a['id']}: {why}")
            revoked.append({"application": a["id"], "member": a["member"], "steamid": a["steamid"], "discord_id": a["discord_id"]})
        return revoked

    def set_official(self, server, on):
        """lsmod.py servers official <id> on|off (admin)."""
        row = self.s.one("SELECT * FROM servers WHERE id=? OR address=?", (int(server) if str(server).isdigit() else -1, str(server)))
        if not row:
            raise cm.Reject(404, "No claimed server like that.")
        self.s.run("UPDATE servers SET official=? WHERE id=?", (1 if on else 0, row["id"]))
        self.log("official", "admin", row["id"], row["address"], "on" if on else "off")
        return self.server_view(self.s.one("SELECT * FROM servers WHERE id=?", (row["id"],)))

    # ---- link ----

    def link_code(self, srv, body):
        steamid = canon(body.get("steamid"))
        if not steamid:
            raise cm.Reject(400, "That isn't a SteamID a link can use (bots, LAN and pending ids can't link).")
        name = body.get("name") if isinstance(body.get("name"), str) else ""
        if not name.strip() or len(name) > 64:
            raise cm.Reject(400, "Send the player's name as the server shows it.")
        if not self.code_per_server.allow(str(srv["id"])):
            raise cm.Reject(429, "Too many link codes from this server. Try again in a few minutes.")
        if not self.new_steamid_ok(srv["id"], steamid):
            raise cm.Reject(429, f"This server made link codes for {STEAMS_PER_DAY} players today. Try again tomorrow, or /link on an official server.")
        # the player is on the server right now, as the list service sees it itself (A2S gives names, not
        # SteamIDs: this checks someone of that name is playing there)
        on = self.players(srv["address"]) if self.players else None
        if on is None:
            raise cm.Reject(409, "The list service couldn't see who's on the server right now. Try again in a minute.")
        if name not in on:
            raise cm.Reject(409, "You don't seem to be on this server right now (the list service checks its player list). "
                                 "Stay in the game and try /link again.")
        if not self.code_per_steam.allow(steamid):
            raise cm.Reject(429, "You asked for a few codes already. Use the newest one, or wait 10 minutes.")
        name = cm.one_line(name, 32)
        code = new_code()
        with self.lock:
            self.s.run("DELETE FROM link_codes WHERE steamid=? OR expires < ?", (steamid, cm.now()))
            self.s.run("INSERT INTO link_codes(code_hash, steamid, name, server_id, expires) VALUES (?,?,?,?,?)",
                       (sha(code), steamid, name, srv["id"], cm.now() + CODE_SECS))
        return {"code": show_code(code), "expires": cm.now() + CODE_SECS}

    def new_steamid_ok(self, server_id, steamid, t=None):
        """At most STEAMS_PER_DAY different SteamIDs per server per day (the same one again is fine)."""
        t = time.time() if t is None else t
        with self.lock:
            seen = {k: v for k, v in self.steams_per_server.get(server_id, {}).items() if t - v < 86400}
            if steamid not in seen and len(seen) >= STEAMS_PER_DAY:
                self.steams_per_server[server_id] = seen
                return False
            seen.setdefault(steamid, t)
            self.steams_per_server[server_id] = seen
            return True

    def moderating(self, discord_id, steamid=None):
        q = "SELECT a.*, s.name AS server_name FROM sm_apps a LEFT JOIN servers s ON s.id=a.server_id WHERE a.discord_id=? AND a.state IN ('invited','active')"
        rows = self.s.all(q, (discord_id,))
        return [r for r in rows if steamid is None or r["steamid"] == steamid]

    def link(self, discord_id, text):
        """/link code:<code> in Discord. Returns what to tell them."""
        if self.link_fails.full(discord_id):
            raise cm.Reject(429, "Too many wrong codes. Try again in an hour.")
        code = norm_code(text)
        row = self.s.one("SELECT * FROM link_codes WHERE code_hash=?", (sha(code),)) if code else None
        if not row or not hmac.compare_digest(row["code_hash"], sha(code)):
            self.link_fails.allow(discord_id)
            raise cm.Reject(404, "That code isn't right. Type /link in game for a new one (codes work for 10 minutes).")
        if row["expires"] < cm.now():
            self.s.run("DELETE FROM link_codes WHERE code_hash=?", (row["code_hash"],))
            raise cm.Reject(410, "That code ran out. Type /link in game for a new one.")
        evicted = None
        with self.lock:
            srv = self.s.one("SELECT * FROM servers WHERE id=?", (row["server_id"],))
            official = 1 if srv and srv["official"] else 0
            other = self.s.one("SELECT * FROM links WHERE steamid=?", (row["steamid"],))
            if other and other["discord_id"] != discord_id:
                # a link made on an official server wins over an unverified one (a host can make a code
                # for any SteamID on their own server); two official links need /unlink
                if not official or other["official"]:
                    raise cm.Reject(409, "That SteamID is linked to another Discord account. They can /unlink first"
                                         + ("." if official else ", or /link on an official Loner-Strike server (that link wins)."))
            mine = self.s.one("SELECT * FROM links WHERE discord_id=?", (discord_id,))
            if mine and mine["steamid"] != row["steamid"] and self.moderating(discord_id, mine["steamid"]):
                m = self.moderating(discord_id, mine["steamid"])[0]
                raise cm.Reject(409, f"You moderate {m['server_name'] or 'a server'} with your linked SteamID. Ask its owner to remove you first.")
            if other and other["discord_id"] != discord_id:
                self.s.run("DELETE FROM links WHERE discord_id=?", (other["discord_id"],))
                evicted = other
            self.s.run("DELETE FROM link_codes WHERE code_hash=?", (row["code_hash"],))
            self.s.run("INSERT OR REPLACE INTO links(discord_id, steamid, name, server_id, official, linked_at) VALUES (?,?,?,?,?,?)",
                       (discord_id, row["steamid"], row["name"], row["server_id"], official, cm.now()))
        self.log("link", discord_id, row["server_id"], row["steamid"], "relinked" if mine else "")
        if evicted:
            self.log("evict", discord_id, row["server_id"], row["steamid"], f"unverified link of {evicted['discord_id']} replaced by an official one")
            self.audit(f"An unverified link of {row['steamid']} (<@{evicted['discord_id']}>) was replaced by one made on an official server by <@{discord_id}>")
            self.dm(evicted["discord_id"], f"Your link to {row['steamid']} was replaced: its player linked it on an official Loner-Strike "
                                           f"server. If that SteamID is yours, type /link on an official server.")
        where = "an official Loner-Strike server" if official else (srv["name"] if srv else "a server")
        return f"Linked: this Discord account ↔ {row['steamid']} (seen on {cm.discord_text(where, 64)})."

    def link_status(self, discord_id):
        r = self.s.one("SELECT * FROM links WHERE discord_id=?", (discord_id,))
        if not r:
            return "You're not linked. Type /link in game on a Loner-Strike server for a code."
        sup = " You're a supporter: thank you!" if self.is_supporter(discord_id) else ""
        return f"Linked to {r['steamid']}" + (" (on an official server)." if r["official"] else ".") + sup

    def unlink(self, discord_id):
        r = self.s.one("SELECT * FROM links WHERE discord_id=?", (discord_id,))
        if not r:
            return "You weren't linked."
        m = self.moderating(discord_id, r["steamid"])
        if m:
            raise cm.Reject(409, f"You moderate {m[0]['server_name'] or 'a server'} with this SteamID. Ask its owner to remove you first.")
        self.s.run("DELETE FROM links WHERE discord_id=?", (discord_id,))
        self.log("unlink", discord_id, r["server_id"], r["steamid"])
        return "Unlinked."

    def claim_owner(self, discord_id, text):
        """/server claim code:<owner code>: this Discord account is the server's owner."""
        if self.owner_fails.full(discord_id):
            raise cm.Reject(429, "Too many wrong codes. Try again in an hour.")
        code = norm_code(text)
        row = self.s.one("SELECT * FROM servers WHERE owner_code_hash=?", (sha(code),)) if code else None
        if not row or row["owner_code_expires"] < cm.now() or not hmac.compare_digest(row["owner_code_hash"], sha(code)):
            self.owner_fails.allow(discord_id)
            raise cm.Reject(404, "That code isn't right or ran out. Make a new one in the app (the server's Shared control › Loner-Strike roles).")
        self.s.run("UPDATE servers SET owner_discord=?, owner_code_hash='', owner_code_expires=0 WHERE id=?", (discord_id, row["id"]))
        self.log("owner", discord_id, row["id"], row["address"])
        return f"You're the owner of {cm.discord_text(row['name'], 64)} here: moderator applications for it come to you by DM."

    def evict(self, steamid, reason=""):
        """lsmod.py roles unlink <steamid>: a moderator takes a squatted link away (logged)."""
        steamid = canon(steamid)
        if not steamid:
            raise cm.Reject(400, "That isn't a SteamID.")
        r = self.s.one("SELECT * FROM links WHERE steamid=?", (steamid,))
        if not r:
            raise cm.Reject(404, "Nobody is linked to that SteamID.")
        self.s.run("DELETE FROM links WHERE steamid=?", (steamid,))
        self.log("evict", "moderator", r["server_id"], steamid, f"link of {r['discord_id']} removed" + (f": {reason}" if reason else ""))
        self.audit(f"Link of {steamid} (<@{r['discord_id']}>) removed by a moderator" + (f": {cm.discord_text(reason, 200)}" if reason else ""))
        return {"ok": True, "discord_id": r["discord_id"]}

    # ---- supporters (Polar) ----

    def is_supporter(self, discord_id, at=None):
        at = cm.now() if at is None else at
        if self.s.one("SELECT 1 AS x FROM polar_grants WHERE discord_id=? AND monthly=1 AND granted=1", (discord_id,)):
            return True
        r = self.s.one("SELECT until FROM supporters WHERE discord_id=?", (discord_id,))
        return bool(r and r["until"] > at)

    def supporter_steamids(self):
        rows = self.s.all("SELECT discord_id, steamid FROM links")
        return sorted(r["steamid"] for r in rows if self.is_supporter(r["discord_id"]))

    def _apply(self, order_id, discord_id, when):
        """A one-time donation counts for `discord_id` from `when`; their time is worked out again."""
        self.s.run("UPDATE polar_orders SET applied=1, applied_at=?, applied_to=? WHERE order_id=?", (when, discord_id, order_id))
        return self._recompute(discord_id)

    def _recompute(self, discord_id):
        """A supporter's time from the one-time donations that counted for this Discord account (applied
        to it, not refunded): each adds `days`, from the later of when it came and the end of what they
        had; with days 0, any one makes them a supporter for good."""
        if not discord_id:
            return 0
        orders = self.s.all("SELECT applied_at FROM polar_orders WHERE applied_to=? AND applied=1 AND refunded=0 ORDER BY applied_at", (discord_id,))
        until = 0
        for o in orders:
            until = FOREVER if self.days == 0 else max(int(o["applied_at"]), until) + self.days * 86400
        self.s.run("INSERT OR REPLACE INTO supporters(discord_id, until) VALUES (?,?)", (discord_id, until))
        return until

    def webhook(self, headers, body, ip):
        """POST /roles/polar/webhook: (status, answer)."""
        if not self.webhook_limit.allow(cm.visitor_key(ip)):
            return 429, {"error": "slow down"}
        if not self.polar_key:
            return 503, {"error": "not set up"}
        wid = headers.get("webhook-id") or ""
        ts = headers.get("webhook-timestamp") or ""
        sigs = headers.get("webhook-signature") or ""
        if not re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", wid) or not re.fullmatch(r"\d{1,12}", ts):
            return 401, {"error": "bad signature"}
        if abs(time.time() - int(ts)) > WEBHOOK_SKEW:
            return 401, {"error": "stale"}
        mac = base64.b64encode(hmac.new(self.polar_key, f"{wid}.{ts}.".encode() + body, hashlib.sha256).digest())
        ok = False
        for part in sigs.split():
            v, _, s = part.partition(",")
            if v == "v1" and hmac.compare_digest(s.encode(), mac):
                ok = True
        if not ok:
            return 401, {"error": "bad signature"}
        try:
            ev = json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            return 400, {"error": "bad body"}
        if not isinstance(ev, dict) or not isinstance(ev.get("data"), dict):
            return 400, {"error": "bad body"}
        with self.lock:
            try:
                self.s.run("INSERT INTO polar_events(id, at) VALUES (?,?)", (wid, cm.now()))
            except sqlite3.IntegrityError:
                return 202, {"ok": True, "again": True}
            self.polar_event(str(ev.get("type") or ""), ev["data"], float(ts))
        return 202, {"ok": True}

    def polar_event(self, kind, d, sent=None):
        at = cm.now()
        # when the grant changed, by Polar (modified_at), else when the webhook was sent: an event older
        # than what's kept is ignored, and a revoke is only undone by a newer grant
        when = event_time(d.get("modified_at")) or event_time(d.get("revoked_at")) or event_time(d.get("granted_at")) or sent or at
        if kind.startswith("benefit_grant."):
            props = d.get("properties") if isinstance(d.get("properties"), dict) else {}
            benefit = d.get("benefit") if isinstance(d.get("benefit"), dict) else {}
            account = str(props.get("account_id") or "")
            gid, cust = str(d.get("id") or ""), str(d.get("customer_id") or "")
            if (benefit.get("type") not in (None, "discord")) or not SNOWFLAKE.match(account) or not gid or not cust:
                return
            granted = 1 if d.get("is_granted", True) and not d.get("is_revoked") and not d.get("revoked_at") and kind != "benefit_grant.revoked" else 0
            # a monthly grant counts while it's granted only when it's for one of the support products (the
            # Discord account it names is kept either way: it's whose payments those are)
            monthly = 1 if d.get("subscription_id") and self.is_support_product(grant_products(d)) else 0
            if d.get("subscription_id") and not monthly:
                self.log("supporter", "polar", 0, account, f"grant {gid}: other product: ignored for the monthly status")
            have = self.s.one("SELECT * FROM polar_grants WHERE grant_id=?", (gid,))
            if have and (when < have["event_at"] or (when == have["event_at"] and granted and not have["granted"])):
                self.log("supporter", "polar", 0, account, f"{kind} older than what's kept: ignored")
                return
            before = self.s.one("SELECT discord_id FROM polar_customers WHERE customer_id=?", (cust,))
            self.s.run("INSERT OR REPLACE INTO polar_customers(customer_id, discord_id) VALUES (?,?)", (cust, account))
            if granted:
                # Polar's Discord benefit gives the role: ours to take away again when they don't qualify
                self.granted_by(account, "polar")
            if before and before["discord_id"] != account:
                # the customer's Discord account changed: their donations go with them
                self.s.run("UPDATE polar_orders SET applied_to=? WHERE customer_id=? AND applied=1", (account, cust))
                self._recompute(before["discord_id"])
                self._recompute(account)
                self.sync_later(before["discord_id"])
                self.sync_later(account)
                self.log("supporter", "polar", 0, account, f"customer {cust}: Discord account changed from {before['discord_id']}; donations moved")
            self.s.run("INSERT OR REPLACE INTO polar_grants(grant_id, customer_id, discord_id, monthly, granted, at, event_at) VALUES (?,?,?,?,?,?,?)",
                       (gid, cust, account, monthly, granted, at, when))
            # donations that came before Polar told us whose Discord this customer is
            # (amounts checked again: orders kept before amounts were, have none and don't count)
            for o in self.s.all("SELECT * FROM polar_orders WHERE customer_id=? AND applied=0 AND refunded=0 AND ignored=0 AND amount_cents>=? AND amount_cents>0 ORDER BY at",
                                (cust, self.min_cents)):
                self._apply(o["order_id"], account, o["at"] if o["at"] > at - 86400 else at)
            self.log("supporter", "polar", 0, account, f"{kind} {'monthly' if monthly else 'one-time'} {'granted' if granted else 'revoked'}")
            # Polar's own Discord benefit takes the role away on a revoke; someone who's a supporter for
            # good by their payments gets it back from our bot
            if not granted:
                self.sync_later(account)
        elif kind == "order.paid":
            # one-time donations and each paid month of a subscription alike: every payment from the minimum
            # counts (with POLAR_SUPPORTER_DAYS 0 the first makes them a supporter for good, so cancelling,
            # the grant's revoke, takes nothing they paid for)
            oid, cust = str(d.get("id") or ""), str(d.get("customer_id") or "")
            if not oid or not cust:
                return
            cents, currency, why = self.order_counts(d)
            if why is None and not self.is_support_product(order_products(d)):
                why = "other product: ignored"
            try:
                self.s.run("INSERT INTO polar_orders(order_id, customer_id, at, amount_cents, currency, ignored) VALUES (?,?,?,?,?,?)",
                           (oid, cust, at, cents, currency, 0 if why is None else 1))
            except sqlite3.IntegrityError:
                return
            if why is not None:
                self.log("donation", "polar", 0, cust, f"order {oid}: {why}, not counted")
                return
            c = self.s.one("SELECT discord_id FROM polar_customers WHERE customer_id=?", (cust,))
            if c:
                until = self._apply(oid, c["discord_id"], at)
                self.log("donation", "polar", 0, c["discord_id"], f"order {oid}: supporter until {time.strftime('%Y-%m-%d', time.gmtime(until))}")
            else:
                self.log("donation", "polar", 0, cust, f"order {oid}: waiting for the Discord benefit")
        elif kind == "order.refunded":
            # any payment, one-time or a month of a subscription
            oid, cust = str(d.get("id") or ""), str(d.get("customer_id") or "")
            if not oid or not cust:
                return
            total = d.get("total_amount", d.get("amount"))
            refunded_amount = d.get("refunded_amount")
            full = d.get("status") == "refunded" or (isinstance(total, (int, float)) and isinstance(refunded_amount, (int, float))
                                                    and total > 0 and refunded_amount >= total)
            if not full:
                self.log("refund", "polar", 0, cust, f"order {oid}: partly refunded, its days stay")
                return
            row = self.s.one("SELECT * FROM polar_orders WHERE order_id=?", (oid,))
            if not row:
                # the refund came before the payment: the payment, when it comes, finds it and counts nothing
                self.s.run("INSERT INTO polar_orders(order_id, customer_id, at, refunded) VALUES (?,?,?,1)", (oid, cust, at))
                self.log("refund", "polar", 0, cust, f"order {oid}: refunded before its payment arrived")
                return
            if row["refunded"]:
                return
            self.s.run("UPDATE polar_orders SET refunded=1 WHERE order_id=?", (oid,))
            if row["applied"] and row["applied_to"]:
                until = self._recompute(row["applied_to"])
                self.sync_later(row["applied_to"])
                self.log("refund", "polar", 0, row["applied_to"], f"order {oid}: refunded, supporter until "
                                                                 + (time.strftime('%Y-%m-%d', time.gmtime(until)) if until else "none"))
            else:
                self.log("refund", "polar", 0, cust, f"order {oid}: refunded (it hadn't counted yet)")

    def is_support_product(self, ids):
        return bool(self.products) and any(i in self.products for i in ids)

    def order_counts(self, d):
        """(cents, currency, why not) for a payment (one-time, or a month of a subscription): it counts (why None) when what was paid,
        net of discounts and before tax, is at least the minimum, in a currency worth at least a dollar."""
        def num(k):
            v = d.get(k)
            return v if isinstance(v, int) and not isinstance(v, bool) and v >= 0 else None
        cents = num("net_amount")
        if cents is None:
            amount, discount = num("amount"), num("discount_amount") or 0
            cents = max(0, amount - discount) if amount is not None else num("total_amount")
        currency = str(d.get("currency") or "usd").lower()[:8]
        if cents is None:
            return 0, currency, "no amount"
        if currency not in CURRENCIES:
            return cents, currency, f"paid in {currency.upper()} (check by hand)"
        if cents < self.min_cents or cents == 0:
            return cents, currency, f"{cents} {currency} cents, under the minimum of {self.min_cents}"
        return cents, currency, None

    def vip(self, srv):
        if not srv["official"]:
            raise cm.Reject(403, "Only official servers get the whole supporter list. Turn on Honor supporters to check players.")
        return {"steamids": self.supporter_steamids(), "ttl": 300}

    def vip_check(self, srv, body):
        if not self.settings_of(srv)["honor_supporters"] and not srv["official"]:
            raise cm.Reject(403, "Turn on Honor Loner-Strike supporters for this server first.")
        ids = body.get("steamids")
        if not isinstance(ids, list) or len(ids) > MAX_STEAMIDS or not all(valid_steamid(x) for x in ids):
            raise cm.Reject(400, f"Send at most {MAX_STEAMIDS} SteamIDs.")
        out = []
        for sid in sorted(set(ids)):
            # answered in the form the server sent (its users.ini compares its own form)
            r = self.s.one("SELECT discord_id FROM links WHERE steamid=?", (canon(sid),))
            if r and self.is_supporter(r["discord_id"]):
                out.append(sid)
        return {"steamids": out}

    # ---- applications (server side) ----

    def app_view(self, a):
        state = a["state"]
        if state == "invited" and a["invite_expires"] < cm.now():
            state = "approved"
        return {"id": a["id"], "state": state, "discord_name": a["discord_name"], "steamid": a["steamid"],
                "linked_on": a["linked_on"], "official_link": bool(a["official_link"]),
                "link": "verified" if a["official_link"] else "unverified", "availability": a["availability"],
                "answers": {"experience": a["experience"], "why": a["why"], "other": a["other"]},
                "decided_by": a["decider_name"], "decided_at": a["decided_at"], "created": a["created"],
                "invite": {"state": "sent" if state == "invited" else ("used" if state in ("active", "revoked") else ""),
                           "expires": a["invite_expires"]}, "member": a["member"]}

    def applications(self, srv):
        rows = self.s.all("SELECT * FROM sm_apps WHERE server_id=? AND (state NOT IN ('denied','closed') OR decided_at > ?) ORDER BY id DESC LIMIT 100",
                          (srv["id"], cm.now() - 30 * 86400))
        return {"applications": [self.app_view(a) for a in rows]}

    def _app_of(self, srv, aid):
        a = self.s.one("SELECT * FROM sm_apps WHERE id=? AND server_id=?", (aid, srv["id"]))
        if not a:
            raise cm.Reject(404, "That application isn't for this server.")
        return a

    def send_invite(self, srv, aid, body):
        a = self._app_of(srv, aid)
        state = self.app_view(a)["state"]
        if state != "approved":
            raise cm.Reject(409, f"That application is {state}, not waiting for an invite.")
        invite = body.get("invite")
        if not isinstance(invite, str) or not invite.startswith("LS1.") or len(invite) > 4000:
            raise cm.Reject(400, "That isn't a Loner-Strike invite.")
        expires = body.get("expires")
        if not isinstance(expires, int) or not cm.now() < expires <= cm.now() + INVITE_SECS:
            expires = cm.now() + 24 * 3600
        self.s.run("UPDATE sm_apps SET state='invited', invite_hash=?, invite_expires=? WHERE id=?", (sha(invite), expires, a["id"]))
        text = (f"Your moderator invite for **{cm.discord_text(srv['name'], 64)}** (works once, for 24 hours):\n"
                f"```{invite}```\nIn Loner-Strike: Settings › Shared control › Accept an invite, and paste it. "
                f"You'll moderate as {a['steamid']} in game.")
        dm = self.dm(a["discord_id"], text)
        self.log("invite", body.get("by") or "host", srv["id"], a["discord_id"], f"application #{a['id']}" + ("" if dm else ", DMs closed"))
        self.audit(f"Server moderator invite for application #{a['id']} ({cm.discord_text(srv['name'], 40)})" + ("" if dm else ": their DMs are closed, the host sends it"))
        return {"ok": True, "dm": dm}

    def guild_role(self, name):
        """A Discord role's id by its name (cached ten minutes), or None."""
        if not self.mod or not self.mod.bot:
            return None
        cache = getattr(self, "_roles_cache", None)
        if cache is None:
            cache = self._roles_cache = {}
        hit = cache.get(name)
        if hit and time.time() - hit[1] < 600:
            return hit[0]
        code, ans = self.mod.bot.api("GET", f"/guilds/{self.mod.guild}/roles")
        found = None
        if code == 200 and isinstance(ans, list):
            want = "".join(c for c in name.lower() if c.isalnum())
            for r in ans:
                if isinstance(r, dict) and "".join(c for c in str(r.get("name", "")).lower() if c.isalnum() and c.isascii()) == want:
                    found = str(r.get("id"))
        if found:
            cache[name] = (found, time.time())
        return found

    def mod_role(self):
        return self.guild_role(MOD_ROLE_NAME)

    # ---- the Supporter role in Discord (ours to keep right; Polar's own benefit takes it away
    # when a monthly support ends, while a payment made them a supporter for good) ----

    def role_wanted(self, discord_id):
        """True: they should have Supporter; False: our bot takes it away (Polar or our bot gave it to
        them, or a payment of theirs was refunded, and they don't qualify any more); None: not ours to
        say (a role given by hand, or someone we never gave it to)."""
        if self.is_supporter(discord_id):
            return True
        if self.s.one("SELECT 1 AS x FROM role_granted WHERE discord_id=?", (discord_id,)):
            return False
        refunded = self.s.one("SELECT 1 AS x FROM polar_orders WHERE applied_to=? AND refunded=1", (discord_id,))
        return False if refunded else None

    def granted_by(self, discord_id, source):
        self.s.run("INSERT OR IGNORE INTO role_granted(discord_id, source, at) VALUES (?,?,?)", (discord_id, source, cm.now()))

    def _retry(self, discord_id, why):
        """Tried again later: 2, 4, 8... minutes, at most six hours apart."""
        r = self.s.one("SELECT tries FROM role_pending WHERE discord_id=?", (discord_id,))
        tries = (r["tries"] if r else 0) + 1
        self.s.run("INSERT OR REPLACE INTO role_pending(discord_id, tries, next_at, why) VALUES (?,?,?,?)",
                   (discord_id, tries, cm.now() + min(60 * 2 ** tries, 6 * 3600), why))

    def sync_role(self, discord_id):
        """Gives or takes the Supporter role as our rules say. 'added', 'removed', 'ok', 'gone' (not in
        the server), 'limited' (Discord asked us to slow down), 'failed' or 'skip'. Whatever didn't get
        done is kept and tried again (role_pending)."""
        out = self._sync_role(discord_id)
        if out in ("gone", "limited", "failed"):
            self._retry(discord_id, out)
        else:
            self.s.run("DELETE FROM role_pending WHERE discord_id=?", (discord_id,))
        return out

    def _sync_role(self, discord_id):
        want = self.role_wanted(discord_id)
        if want is None:
            return "skip"
        rid = self.guild_role(SUPPORTER_ROLE_NAME)
        if not rid:
            return "failed" if self.mod and self.mod.bot else "skip"
        bot, guild = self.mod.bot, self.mod.guild
        code, member = bot.api("GET", f"/guilds/{guild}/members/{discord_id}")
        if code == 404:
            return "gone"
        if code == 429:
            return "limited"
        if code != 200 or not isinstance(member, dict):
            return "failed"
        has = rid in {str(x) for x in member.get("roles") or []}
        if want == has:
            if not want:
                self.s.run("DELETE FROM role_granted WHERE discord_id=?", (discord_id,))
            return "ok"
        code, _ = bot.api("PUT" if want else "DELETE", f"/guilds/{guild}/members/{discord_id}/roles/{rid}",
                          reason="Loner-Strike supporter" if want else "Loner-Strike: not a supporter any more")
        if code == 429:
            return "limited"
        if code not in (200, 204):
            self.log("discord-role", "bot", 0, discord_id, f"Supporter {'add' if want else 'remove'} failed (status {code})")
            return "failed"
        if want:
            self.granted_by(discord_id, "bot")
        else:
            self.s.run("DELETE FROM role_granted WHERE discord_id=?", (discord_id,))
        self.log("discord-role", "bot", 0, discord_id, "Supporter added" if want else "Supporter removed (not a supporter any more)")
        return "added" if want else "removed"

    def sync_later(self, discord_id):
        if self.mod and self.mod.bot and discord_id:
            self.mod._later(lambda: self.sync_role(discord_id))

    def known_accounts(self):
        return sorted(r["discord_id"] for r in self.s.all(
            "SELECT discord_id FROM polar_customers UNION SELECT applied_to AS discord_id FROM polar_orders WHERE applied_to != '' "
            "UNION SELECT discord_id FROM supporters UNION SELECT discord_id FROM role_granted") if r["discord_id"])

    def sync_all(self, pause=SYNC_PAUSE, budget=SYNC_MAX):
        """One check: first what's waiting to be tried again (when due), then the next accounts from
        where the last check stopped (round and round), at most `budget` in all. Returns the counts."""
        counts = {}

        def one(d):
            out = self.sync_role(d)
            counts[out] = counts.get(out, 0) + 1
            if pause:
                time.sleep(pause)
            return out
        for r in self.s.all("SELECT discord_id FROM role_pending WHERE next_at<=? ORDER BY next_at LIMIT ?", (cm.now(), budget)):
            budget -= 1
            if one(r["discord_id"]) == "limited":
                return counts
        ids = self.known_accounts()
        if not ids or budget <= 0:
            return counts
        cur = (self.s.one("SELECT v FROM roles_meta WHERE k='role_cursor'") or {}).get("v", "")
        start = next((i for i, d in enumerate(ids) if d > cur), 0)
        for d in (ids[start:] + ids[:start])[:budget]:
            out = one(d)
            self.s.run("INSERT OR REPLACE INTO roles_meta(k, v) VALUES ('role_cursor', ?)", (d,))
            if out == "limited":
                break
        return counts

    def sync_loop(self, every=SYNC_EVERY):
        """Hourly, for as long as the service runs (someone who left the server gets the role again
        at the first check after they're back: the bot doesn't hear joins)."""
        while True:
            time.sleep(every)
            try:
                self.sync_all()
            except Exception as e:  # never let the loop die
                cm.log(f"roles: supporter role check: {type(e).__name__}")

    def redeemed(self, srv, aid, body):
        a = self._app_of(srv, aid)
        if a["state"] == "active":
            return {"ok": True}
        if a["state"] != "invited":
            raise cm.Reject(409, "That application has no invite out.")
        member = cm.one_line(body.get("member") if isinstance(body.get("member"), str) else "", 64)
        self.s.run("UPDATE sm_apps SET state='active', member=? WHERE id=?", (member, a["id"]))
        rid = self.mod_role()
        note = ""
        if rid:
            code, _ = self.mod.bot.api("PUT", f"/guilds/{self.mod.guild}/members/{a['discord_id']}/roles/{rid}",
                                       reason=f"Server moderator of {srv['name'][:60]} (application #{a['id']})")
            if code not in (200, 204):
                note = f"Discord didn't give the {MOD_ROLE_NAME} role (status {code})."
        elif self.mod:
            note = f"There's no {MOD_ROLE_NAME} role in the Discord."
        if note:
            self.s.run("UPDATE sm_apps SET note=? WHERE id=?", (note, a["id"]))
        self.log("redeem", member or "member", srv["id"], a["discord_id"], f"application #{a['id']}" + (f"; {note}" if note else ""))
        self.audit(f"Server moderator of {cm.discord_text(srv['name'], 40)}: <@{a['discord_id']}> (application #{a['id']})")
        return {"ok": True, "note": note}

    def revoke(self, srv, aid, body):
        a = self._app_of(srv, aid)
        if a["state"] not in ("invited", "active", "approved"):
            raise cm.Reject(409, f"That application is {a['state']}.")
        self.s.run("UPDATE sm_apps SET state='revoked', invite_hash='', invite_expires=0 WHERE id=?", (a["id"],))
        rid = self.mod_role()
        if rid and not self.moderating(a["discord_id"]):
            self.mod.bot.api("DELETE", f"/guilds/{self.mod.guild}/members/{a['discord_id']}/roles/{rid}",
                             reason=f"No longer a server moderator (application #{a['id']})")
        by = cm.one_line(body.get("by") if isinstance(body.get("by"), str) else "host", 40)
        self.log("revoke", by, srv["id"], a["discord_id"], f"application #{a['id']}")
        self.audit(f"Server moderator removed from {cm.discord_text(srv['name'], 40)}: <@{a['discord_id']}> (application #{a['id']}, by {cm.discord_text(by, 40)})")
        return {"ok": True}

    # ---- HTTP ----

    def handle(self, h, method, ip, read_body, reply):
        parts = [p for p in urllib.parse.urlsplit(h.path).path.split("/") if p][1:]
        try:
            if parts == ["polar", "webhook"] and method == "POST":
                code, ans = self.webhook(h.headers, read_body(h), ip)
                return reply(h, code, ans, cors=False)
            body = {}
            if method in ("POST", "DELETE"):
                raw = read_body(h)
                if len(raw) > MAX_BODY:
                    raise cm.Reject(413, "The request is too big.")
                try:
                    body = json.loads(raw.decode("utf-8") or "{}")
                except (UnicodeDecodeError, ValueError):
                    raise cm.Reject(400, "The request couldn't be read.")
                if not isinstance(body, dict):
                    raise cm.Reject(400, "The request couldn't be read.")
            if parts == ["claim", "start"] and method == "POST":
                return reply(h, 200, self.claim_start(body, ip), cors=False)
            if parts == ["claim", "finish"] and method == "POST":
                return reply(h, 200, self.claim_finish(body, ip), cors=False)
            srv = self._authorized(h, ip)
            if parts == ["server"]:
                if method == "GET":
                    return reply(h, 200, self.server_view(srv), cors=False)
                if method == "POST":
                    return reply(h, 200, self.set_settings(srv, body), cors=False)
                if method == "DELETE":
                    return reply(h, 200, self.unclaim(srv), cors=False)
            if parts == ["server", "owner-code"] and method == "POST":
                return reply(h, 200, self.owner_code(srv), cors=False)
            if parts == ["link", "code"] and method == "POST":
                return reply(h, 200, self.link_code(srv, body), cors=False)
            if parts == ["vip"] and method == "GET":
                return reply(h, 200, self.vip(srv), cors=False)
            if parts == ["vip", "check"] and method == "POST":
                return reply(h, 200, self.vip_check(srv, body), cors=False)
            if parts == ["applications"] and method == "GET":
                return reply(h, 200, self.applications(srv), cors=False)
            if len(parts) == 3 and parts[0] == "applications" and parts[1].isdigit() and method == "POST":
                fn = {"invite": self.send_invite, "redeemed": self.redeemed, "revoke": self.revoke}.get(parts[2])
                if fn:
                    return reply(h, 200, fn(srv, int(parts[1]), body), cors=False)
            return reply(h, 404, {"error": "not found"}, cors=False)
        except cm.Reject as e:
            return reply(h, e.code, {"error": e.message}, cors=False)

    def _authorized(self, h, ip):
        who = cm.visitor_key(ip)
        if self.bad_token.full(who):
            raise cm.Reject(429, "Too many wrong tokens from here. Try again later.")
        auth = h.headers.get("Authorization") or ""
        srv = self.server_by_token(auth[7:].strip() if auth[:7].lower() == "bearer " else "")
        if not srv:
            self.bad_token.allow(who)
            raise cm.Reject(401, "This server isn't claimed (or its claim was replaced). Claim it again in the app.")
        if not self.per_token.allow(str(srv["id"])):
            raise cm.Reject(429, "Too many requests from this server. Try again in a minute.")
        return srv

    # ---- Discord ----

    def dm(self, uid, text):
        if not self.mod or not self.mod.bot:
            return False
        code, ans = self.mod.bot.api("POST", "/users/@me/channels", {"recipient_id": uid})
        if code != 200 or not isinstance(ans, dict) or not ans.get("id"):
            return False
        return self.mod.bot.post_to(str(ans["id"]), {"content": text[:1900]}) is not None

    def command(self, d, uid):
        """/link, /unlink, /server claim (anyone in the guild). Returns the text to tell them."""
        data = d.get("data") if isinstance(d.get("data"), dict) else {}
        name = data.get("name")
        opts = data.get("options") if isinstance(data.get("options"), list) else []
        if name == "server":
            sub = opts[0] if opts and isinstance(opts[0], dict) else {}
            opts = sub.get("options") if isinstance(sub.get("options"), list) else []
            name = "server " + str(sub.get("name") or "")
        vals = {o.get("name"): o.get("value") for o in opts if isinstance(o, dict)}
        code = vals.get("code") if isinstance(vals.get("code"), str) else ""
        if name == "link":
            return self.link(uid, code) if code else self.link_status(uid)
        if name == "unlink":
            return self.unlink(uid)
        if name == "server claim":
            return self.claim_owner(uid, code)
        raise cm.Reject(400, "Unknown command.")

    def apply_start(self, uid):
        """"Server moderator" picked in Apply: which server."""
        link = self.s.one("SELECT * FROM links WHERE discord_id=?", (uid,))
        if not link:
            raise cm.Reject(409, "Link your SteamID first: type /link in game on any Loner-Strike server.")
        rows = self.s.all("SELECT * FROM servers ORDER BY official DESC, name LIMIT 200")
        rows = [r for r in rows if self.settings_of(r)["applications"] and (r["owner_discord"] or r["official"])
                and (link["official"] or not r["official"])]
        if not rows:
            raise cm.Reject(409, "No server is taking moderator applications right now"
                            + ("" if link["official"] else " (official servers need a link made on an official server: type /link there)") + ".")
        options = [{"label": cm.one_line(r["name"], 90) or r["address"], "value": str(r["id"]),
                    "description": ("Official · " if r["official"] else "") + r["address"][:80]} for r in rows[:25]]
        return {"type": 4, "data": {"flags": EPHEMERAL, "allowed_mentions": {"parse": []},
                                    "content": "Which server would you like to moderate?",
                                    "components": [{"type": 1, "components": [{"type": 3, "custom_id": self.mod.custom_id("smpick", 0),
                                                                               "placeholder": "Pick a server", "options": options}]}]}}

    def _check_can_apply(self, uid, srv):
        link = self.s.one("SELECT * FROM links WHERE discord_id=?", (uid,))
        if not link:
            raise cm.Reject(409, "Link your SteamID first: type /link in game on any Loner-Strike server.")
        if not srv or not self.settings_of(srv)["applications"]:
            raise cm.Reject(409, "That server isn't taking moderator applications.")
        if srv["official"] and not link["official"]:
            raise cm.Reject(409, "Official servers need a link made on an official server: type /link there, then /link the code here.")
        for a in self.s.all("SELECT * FROM sm_apps WHERE discord_id=? AND server_id=?", (uid, srv["id"])):
            if a["state"] in ("pending", "approved", "invited", "active"):
                raise cm.Reject(409, f"You already have an application for {srv['name']} ({self.app_view(a)['state']}).")
            if a["state"] == "denied" and a["decided_at"] + COOLDOWN > cm.now():
                raise cm.Reject(409, f"You can apply for {srv['name']} again from {time.strftime('%Y-%m-%d', time.gmtime(a['decided_at'] + COOLDOWN))}.")
        return link

    def pick(self, d, uid):
        values = (d.get("data") or {}).get("values")
        sid = values[0] if isinstance(values, list) and values and isinstance(values[0], str) and values[0].isdigit() else ""
        srv = self.s.one("SELECT * FROM servers WHERE id=?", (int(sid),)) if sid else None
        self._check_can_apply(uid, srv)
        return {"type": 9, "data": {
            "custom_id": self.mod.custom_id("smform", srv["id"]), "title": f"Moderate: {srv['name']}"[:45],
            "components": [
                {"type": 1, "components": [{"type": 4, "custom_id": "availability", "style": 1, "required": True, "min_length": 5,
                                            "max_length": AVAIL_MAX, "label": "When can you play? (days, hours, time zone)"}]},
                {"type": 1, "components": [{"type": 4, "custom_id": "experience", "style": 2, "required": False,
                                            "max_length": TEXT_MAX, "label": "Have you moderated before? Where?"}]},
                {"type": 1, "components": [{"type": 4, "custom_id": "why", "style": 2, "required": True, "min_length": WHY_MIN,
                                            "max_length": TEXT_MAX, "label": "Why this server? What would you do?"}]},
                {"type": 1, "components": [{"type": 4, "custom_id": "other", "style": 2, "required": False,
                                            "max_length": TEXT_MAX, "label": "Anything else the owner should know?"}]}]}}

    def submit(self, d, sid, user, member):
        uid = str(user.get("id"))
        srv = self.s.one("SELECT * FROM servers WHERE id=?", (sid,))
        link = self._check_can_apply(uid, srv)
        fields = {}
        for row in (d.get("data") or {}).get("components") or []:
            for comp in (row or {}).get("components") or []:
                if isinstance(comp, dict) and isinstance(comp.get("value"), str):
                    fields[comp.get("custom_id")] = comp["value"]
        avail = cm.one_line(fields.get("availability", ""), AVAIL_MAX)
        why = cm.one_line(fields.get("why", ""), TEXT_MAX)
        if len(avail) < 5 or len(why) < WHY_MIN:
            raise cm.Reject(400, f"Say when you can play, and a little more about why (at least {WHY_MIN} characters).")
        if not self.per_day.allow(uid):
            raise cm.Reject(429, "That's a lot of applications for one day. Try again tomorrow.")
        name = cm.one_line(str(user.get("global_name") or user.get("username") or uid), 40)
        on = self.s.one("SELECT name, official FROM servers WHERE id=?", (link["server_id"],))
        linked_on = "an official server" if link["official"] else (on["name"] if on else "a server that's gone")
        cur = self.s.run("INSERT INTO sm_apps(server_id, discord_id, discord_name, steamid, linked_on, official_link, availability, experience, why, other, state, created) "
                         "VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
                         (srv["id"], uid, name, link["steamid"], linked_on, link["official"], avail,
                          cm.one_line(fields.get("experience", ""), TEXT_MAX), why, cm.one_line(fields.get("other", ""), TEXT_MAX),
                          "pending", cm.now()))
        aid = cur.lastrowid
        self.log("apply", uid, srv["id"], link["steamid"], f"application #{aid}")
        self._post_cards(aid)
        return self.mod.tell(f"Thanks! Your application for {srv['name']} went to its owner. You'll hear back by DM.")

    def _post_cards(self, aid):
        a = self.s.one("SELECT * FROM sm_apps WHERE id=?", (aid,))
        srv = self.s.one("SELECT * FROM servers WHERE id=?", (a["server_id"],))
        cards = []
        if srv["owner_discord"]:
            code, ans = self.mod.bot.api("POST", "/users/@me/channels", {"recipient_id": srv["owner_discord"]})
            if code == 200 and isinstance(ans, dict) and ans.get("id"):
                msg = self.mod.bot.post_to(str(ans["id"]), self.card(aid))
                if msg:
                    cards.append([str(ans["id"]), msg])
        if self.settings_of(srv)["discord_mods_decide"] and self.mod.apps and self.mod.apps.channel:
            msg = self.mod.bot.post_to(self.mod.apps.channel, self.card(aid))
            if msg:
                cards.append([self.mod.apps.channel, msg])
        self.s.run("UPDATE sm_apps SET cards=? WHERE id=?", (json.dumps(cards), aid))
        if not cards:
            self.mod.say(f"**Server moderator application #{aid}** couldn't reach anyone who decides (the owner's DMs are closed?). `lsmod.py roles show {aid}`.")

    def card(self, aid):
        a = self.s.one("SELECT * FROM sm_apps WHERE id=?", (aid,))
        srv = self.s.one("SELECT * FROM servers WHERE id=?", (a["server_id"],)) or {"name": "a server that's gone", "official": 0}
        import applications as ap
        desc = (f"<@{a['discord_id']}> ({cm.discord_text(a['discord_name'], 40)}) · account made {ap.created(a['discord_id'])}\n"
                f"SteamID {a['steamid']} · linked on {cm.discord_text(a['linked_on'], 64)}"
                + ("" if a["official_link"] else " · **unverified** (that server's host could have made the code)"))
        fields = [{"name": "Server", "value": cm.discord_text(srv["name"], 64) + (" (official)" if srv["official"] else ""), "inline": False},
                  {"name": "When they play", "value": cm.discord_text(a["availability"], AVAIL_MAX) or "-", "inline": False},
                  {"name": "Why", "value": cm.discord_text(a["why"], TEXT_MAX) or "-", "inline": False}]
        if a["experience"]:
            fields.append({"name": "Experience", "value": cm.discord_text(a["experience"], TEXT_MAX), "inline": False})
        if a["other"]:
            fields.append({"name": "Anything else", "value": cm.discord_text(a["other"], TEXT_MAX), "inline": False})
        embed = {"title": f"Server moderator application #{a['id']}", "description": desc, "fields": fields, "color": 0x4169E1}
        buttons = []
        if a["state"] == "pending":
            buttons = [{"type": 2, "style": 3, "label": "Approve", "custom_id": self.mod.custom_id("smok", a["id"])},
                       {"type": 2, "style": 4, "label": "Deny", "custom_id": self.mod.custom_id("smno", a["id"])}]
        else:
            word = {"approved": "Approved", "denied": "Denied", "invited": "Approved, invite sent", "active": "Moderating",
                    "revoked": "Removed", "closed": "Closed (the server's claim ended)"}.get(a["state"], a["state"])
            embed["description"] += f"\n\n**{word}**" + (f" by <@{a['decider']}>" if a["decider"] else "") + \
                (f" · {cm.discord_text(a['reason'], 200)}" if a["reason"] else "")
            embed["color"] = 0x2E8B57 if a["state"] in ("approved", "invited", "active") else 0x808080
        return {"embeds": [embed], "components": [{"type": 1, "components": buttons}] if buttons else []}

    def may_decide(self, uid, roles, srv):
        if srv["owner_discord"] and uid == srv["owner_discord"]:
            return True
        if self.mod and uid == self.mod.owner:
            return True
        return self.settings_of(srv)["discord_mods_decide"] and bool(set(roles or ()) & self.deciders)

    def decide(self, action, aid, d, actor):
        a = self.s.one("SELECT * FROM sm_apps WHERE id=?", (aid,))
        if not a:
            raise cm.Reject(404, "That application is gone.")
        srv = self.s.one("SELECT * FROM servers WHERE id=?", (a["server_id"],))
        if not srv:
            raise cm.Reject(409, "That server's claim ended.")
        if not self.may_decide(actor["id"], actor.get("roles"), srv):
            raise cm.Reject(403, "You can't decide on this server's applications.")
        if actor["id"] == a["discord_id"]:
            raise cm.Reject(403, "You can't decide on your own application.")
        if a["state"] != "pending":
            raise cm.Reject(409, f"Already {a['state']} by {a['decider_name'] or 'someone'}.")
        if action == "smno":
            return {"type": 9, "data": {"custom_id": self.mod.custom_id("smnom", aid), "title": "Deny"[:45],
                                        "components": [{"type": 1, "components": [{
                                            "type": 4, "custom_id": "reason", "style": 2, "max_length": cm.MAX_REASON,
                                            "label": "Why? (optional; they're told by DM)", "required": False}]}]}}
        reason = ""
        if action == "smnom":
            for row in (d.get("data") or {}).get("components") or []:
                for comp in (row or {}).get("components") or []:
                    if isinstance(comp, dict) and comp.get("custom_id") == "reason" and isinstance(comp.get("value"), str):
                        reason = cm.one_line(comp["value"], cm.MAX_REASON)
        state = "approved" if action == "smok" else "denied"
        self.s.run("UPDATE sm_apps SET state=?, decider=?, decider_name=?, decided_at=?, reason=? WHERE id=?",
                   (state, actor["id"], actor["name"], cm.now(), reason, aid))
        self.log("approve" if state == "approved" else "deny", actor["id"], srv["id"], a["discord_id"], f"application #{aid}")
        self.audit(f"Server moderator application #{aid} ({cm.discord_text(srv['name'], 40)}) {state} by <@{actor['id']}>")
        name = cm.discord_text(srv["name"], 64)

        def later():
            if state == "approved":
                self.dm(a["discord_id"], f"Good news: your application to moderate {name} was approved. The server's team will send "
                                         f"your invite here soon.")
            else:
                self.dm(a["discord_id"], f"Thanks for applying to moderate {name}. The owner said no this time"
                                         + (f": {cm.one_line(reason, cm.MAX_REASON)}" if reason else ".")
                                         + f" You can apply again from {time.strftime('%Y-%m-%d', time.gmtime(cm.now() + COOLDOWN))}.")
            card = self.card(aid)
            here = str(d.get("channel_id") or "")
            for ch, msg in json.loads(a["cards"] or "[]"):
                if ch != here:
                    self.mod.bot.edit_in(ch, msg, card)
        self.mod._later(later)
        return {"type": 7, "data": self.card(aid)}

    # ---- admin (lsmod.py, through community.py's admin endpoints) ----

    def admin(self, method, parts, body, query):
        if parts == ["servers"] and method == "GET":
            return {"servers": [self.server_view(r) | {"claimed_at": r["claimed_at"]} for r in self.s.all("SELECT * FROM servers ORDER BY id")]}
        if len(parts) == 3 and parts[0] == "servers" and parts[2] == "official" and method == "POST":
            return self.set_official(urllib.parse.unquote(parts[1]), bool(body.get("on")))
        if parts == ["log"] and method == "GET":
            n = (query.get("n") or ["100"])[0]
            return {"log": self.s.all("SELECT * FROM roles_log ORDER BY id DESC LIMIT ?", (min(int(n) if n.isdigit() else 100, 1000),))}
        if len(parts) == 2 and parts[0] == "application" and parts[1].isdigit() and method == "GET":
            a = self.s.one("SELECT * FROM sm_apps WHERE id=?", (int(parts[1]),))
            if not a:
                raise cm.Reject(404, "No such application.")
            return self.app_view(a) | {"server": a["server_id"], "discord_id": a["discord_id"], "note": a["note"]}
        if parts == ["unlink"] and method == "POST":
            return self.evict(body.get("steamid"), cm.one_line(str(body.get("reason") or ""), 200))
        if parts == ["supporters"] and method == "GET":
            return {"steamids": self.supporter_steamids(),
                    "supporters": self.s.all("SELECT * FROM supporters ORDER BY until DESC LIMIT 500")}
        raise cm.Reject(404, "not found")


def _ids(*vals):
    return [str(v) for v in vals if isinstance(v, (str, int)) and not isinstance(v, bool) and str(v)]


def order_products(d):
    """The product ids an order is for: product_id, product.id, its items', its subscription's."""
    sub = d.get("subscription") if isinstance(d.get("subscription"), dict) else {}
    prod = d.get("product") if isinstance(d.get("product"), dict) else {}
    items = [i for i in d.get("items") or [] if isinstance(i, dict)] if isinstance(d.get("items"), list) else []
    return _ids(d.get("product_id"), prod.get("id"), sub.get("product_id"), *[i.get("product_id") for i in items])


def grant_products(d):
    """The product ids a benefit grant came from: through its subscription or its order."""
    sub = d.get("subscription") if isinstance(d.get("subscription"), dict) else {}
    order = d.get("order") if isinstance(d.get("order"), dict) else {}
    return _ids(d.get("product_id"), sub.get("product_id"), order.get("product_id"))


def event_time(v):
    """An ISO 8601 time from Polar ("2026-11-14T10:00:00.123Z") as unix seconds, or None."""
    if not isinstance(v, str) or len(v) > 40:
        return None
    try:
        import datetime
        t = datetime.datetime.fromisoformat(v.replace("Z", "+00:00"))
        return (t if t.tzinfo else t.replace(tzinfo=datetime.timezone.utc)).timestamp()
    except ValueError:
        return None


def webhook_key(secret):
    """The HMAC key for a Polar (Standard Webhooks) secret. Standard Webhooks' `whsec_<base64>` is the
    key in base64; Polar's own secrets are used as they are (its SDK base64-encodes them first, then
    decodes them again)."""
    secret = (secret or "").strip()
    if not secret:
        return None
    if secret.startswith("whsec_"):
        try:
            return base64.b64decode(secret[6:], validate=True)
        except ValueError:
            return None
    return secret.encode()
