#!/usr/bin/env python3
# Moderation cards in the team's private Discord channel, with buttons (community.py).
#
# The bot application ("Loner-Strike Setup") posts a card for every new listing ([Pull]), every
# report ([Pull] [Keep] [Block creator]) and every submission from a blocked creator ([Approve]
# [Reject]). Discord sends button clicks to POST /discord/interactions on this service; each one
# is checked (Discord's Ed25519 signature, a fresh timestamp, our guild, a moderator), done once,
# and the card is edited to say who did what, with [Undo]. An audit line goes to the channel.
# Without a bot token the plain webhook (if set) gets the same news as text, without buttons.
#
# Standard library only; Ed25519 uses the `cryptography` package when it's installed and an
# RFC 8032 verifier written out here otherwise (slow-ish, a few ms, plenty for button clicks).
import hashlib
import hmac
import json
import re
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

import applications
import community as cm

API = "https://discord.com/api/v10"
MAX_SKEW = 300            # seconds a signed request may be old (Discord's own advice: 5 min)
EPHEMERAL = 64
# interaction types and response types (Discord's numbers)
PING, COMMAND, COMPONENT, MODAL = 1, 2, 3, 5
R_PONG, R_MESSAGE, R_UPDATE, R_MODAL = 1, 4, 7, 9


# ---- Ed25519 (verification) -------------------------------------------------------------------

_P = 2 ** 255 - 19
_L = 2 ** 252 + 27742317777372353535851937790883648493
_D = -121665 * pow(121666, _P - 2, _P) % _P
_I = pow(2, (_P - 1) // 4, _P)


def _inv(x):
    return pow(x, _P - 2, _P)


def _recover_x(y, sign):
    if y >= _P:
        return None
    x2 = (y * y - 1) * _inv(_D * y * y + 1) % _P
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P:
        x = x * _I % _P
    if (x * x - x2) % _P:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


_GY = 4 * _inv(5) % _P
_GX = _recover_x(_GY, 0)
_G = (_GX, _GY, 1, _GX * _GY % _P)


def _add(a, b):
    A = (a[1] - a[0]) * (b[1] - b[0]) % _P
    B = (a[1] + a[0]) * (b[1] + b[0]) % _P
    C = 2 * a[3] * b[3] * _D % _P
    D = 2 * a[2] * b[2] % _P
    E, F, G, H = B - A, D - C, D + C, B + A
    return (E * F % _P, G * H % _P, F * G % _P, E * H % _P)


def _mul(s, pt):
    q = (0, 1, 1, 0)
    while s:
        if s & 1:
            q = _add(q, pt)
        pt = _add(pt, pt)
        s >>= 1
    return q


def _equal(a, b):
    return (a[0] * b[2] - b[0] * a[2]) % _P == 0 and (a[1] * b[2] - b[1] * a[2]) % _P == 0


def _compress(pt):
    zi = _inv(pt[2])
    x, y = pt[0] * zi % _P, pt[1] * zi % _P
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _decompress(b):
    y = int.from_bytes(b, "little")
    sign = y >> 255
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    return None if x is None else (x, y, 1, x * y % _P)


def _sha512_int(*parts):
    return int.from_bytes(hashlib.sha512(b"".join(parts)).digest(), "little")


def verify_pure(pub, msg, sig):
    if len(pub) != 32 or len(sig) != 64:
        return False
    a = _decompress(pub)
    r = _decompress(sig[:32])
    s = int.from_bytes(sig[32:], "little")
    if a is None or r is None or s >= _L:
        return False
    h = _sha512_int(sig[:32], pub, msg) % _L
    return _equal(_mul(s, _G), _add(r, _mul(h, a)))


def sign_pure(secret, msg):
    """For tests: an Ed25519 signature with a 32-byte secret key."""
    h = hashlib.sha512(secret).digest()
    a = int.from_bytes(h[:32], "little") & ((1 << 254) - 8) | (1 << 254)
    pub = _compress(_mul(a, _G))
    r = _sha512_int(h[32:], msg) % _L
    big_r = _compress(_mul(r, _G))
    s = (r + _sha512_int(big_r, pub, msg) % _L * a) % _L
    return big_r + s.to_bytes(32, "little")


def public_key(secret):
    h = hashlib.sha512(secret).digest()
    return _compress(_mul(int.from_bytes(h[:32], "little") & ((1 << 254) - 8) | (1 << 254), _G))


try:
    from cryptography.exceptions import InvalidSignature
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

    def verify(pub, msg, sig):
        try:
            Ed25519PublicKey.from_public_bytes(pub).verify(sig, msg)
            return True
        except (InvalidSignature, ValueError):
            return False
    FAST = True
except ImportError:  # the box's python3 without python3-cryptography: the version above
    verify = verify_pure
    FAST = False


# ---- the bot's side of Discord's API ----------------------------------------------------------

class Bot:
    """Posts and edits messages in one channel as the bot. Never logs the token."""

    def __init__(self, token, channel, send=None):
        self.token, self.channel = token, channel
        self._send = send or self._http

    # what the bot may ask of Discord: messages in channels, roles of members, a DM channel
    ALLOWED = ("/channels/", "/guilds/", "/users/@me/channels")

    def _http(self, method, path, body, reason=None):
        url = API + path
        if not path.startswith(self.ALLOWED) or ".." in path:
            raise ValueError("not a call the bot makes")
        headers = {"Authorization": f"Bot {self.token}", "User-Agent": "DiscordBot (https://lonerstrike.com, 1)"}
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        if reason:
            headers["X-Audit-Log-Reason"] = urllib.parse.quote(reason[:400])
        req = urllib.request.Request(url, data=data, method=method, headers=headers)
        try:
            with cm._OPENER.open(req, timeout=10) as r:
                raw = r.read()
                return r.status, json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            try:
                return e.code, json.loads(e.read() or b"{}")
            except ValueError:
                return e.code, {}
        except (OSError, ValueError):
            return 0, {}

    def api(self, method, path, body=None, reason=None):
        """(status, answer) for one call (see ALLOWED)."""
        return self._send(method, path, body, reason) if reason else self._send(method, path, body)

    def post_to(self, channel, payload):
        """The new message's id in another channel, or None."""
        code, ans = self._send("POST", f"/channels/{channel}/messages", dict(payload, allowed_mentions={"parse": []}))
        if code != 200:
            cm.log(f"the bot couldn't post (status {code})")
            return None
        return str(ans.get("id") or "") or None

    def edit_in(self, channel, msg_id, payload):
        code, _ = self._send("PATCH", f"/channels/{channel}/messages/{msg_id}", dict(payload, allowed_mentions={"parse": []}))
        return code == 200

    def post(self, payload):
        """The new message's id, or None."""
        code, ans = self._send("POST", f"/channels/{self.channel}/messages", dict(payload, allowed_mentions={"parse": []}))
        if code != 200:
            cm.log(f"the bot couldn't post (status {code})")
            return None
        return str(ans.get("id") or "") or None

    def edit(self, msg_id, payload):
        code, _ = self._send("PATCH", f"/channels/{self.channel}/messages/{msg_id}", dict(payload, allowed_mentions={"parse": []}))
        return code == 200


# ---- cards ----------------------------------------------------------------------------------

def button_secret(token):
    """The key button ids are signed with, from the bot token (discord-setup.py signs the
    pinned Apply button with the same)."""
    return hmac.new(token.encode() or b"none", b"ls-buttons-1", hashlib.sha256).digest()


def sign_id(secret, action, n):
    mac = hmac.new(secret, f"{action}:{n}".encode(), hashlib.sha256).hexdigest()[:20]
    return f"ls1:{action}:{n}:{mac}"


def _button(label, custom_id, style, disabled=False):
    return {"type": 2, "style": style, "label": label, "custom_id": custom_id, "disabled": disabled}


PRIMARY, SECONDARY, SUCCESS, DANGER = 1, 2, 3, 4
PULLED = "Pulled by the moderators"  # the default reason: neutral, never a moderator's name
DONE_WORDS = {"pull": "Pulled", "keep": "Kept", "block": "Creator blocked", "approve": "Approved", "reject": "Turned down"}


class Mod(cm.Notifier):
    """Cards and buttons. A Notifier, so community.py can use it in its place: card() posts a
    card through the bot (text through the webhook when the bot can't), say() a plain line."""

    def __init__(self, community, bot, public_key_hex, guild, roles, owner, webhook="", post=None, background=True,
                 apply_channel="", apply_roles=()):
        super().__init__(webhook, post=post)
        self.c = community
        self.bot = bot
        self.pub = bytes.fromhex(public_key_hex) if re.fullmatch(r"[0-9a-fA-F]{64}", public_key_hex or "") else None
        if public_key_hex and not self.pub:
            cm.log("DISCORD_PUBLIC_KEY isn't 64 hex characters; buttons are off")
        self.guild, self.roles, self.owner = str(guild or ""), {str(r) for r in roles if r}, str(owner or "")
        # button ids are signed so a forged click can't name another card or action
        self.secret = button_secret(bot.token if bot else "")
        self.lock = threading.RLock()
        self.background = background
        community.notifier = self
        # role applications (only when their private channel is set)
        self.apps = applications.Applications(self, apply_channel, apply_roles) if apply_channel and bot else None
        if self.pub and not FAST:
            cm.log("python3-cryptography isn't installed: Discord signatures are checked by the slower built-in "
                   "verifier (apt install python3-cryptography)")

    # -- posting --

    def _later(self, fn):
        if self.background:
            threading.Thread(target=fn, daemon=True).start()
        else:
            fn()

    def say(self, text, wait=False):
        def go():
            if self.bot and self.bot.post({"content": text[:1900]}):
                return
            cm.Notifier.say(self, text, wait=True)
        if wait:
            go()
        else:
            self._later(go)

    def card(self, kind, key, text, report_id=None):
        """A new card (listing, report or queue) for an entry; the text is the webhook's fallback."""
        card_id = self.c.store.new_card(kind, key, report_id)

        def go():
            msg = self.bot.post(self.payload(card_id)) if self.bot and self.pub else None
            if msg:
                self.c.store.update_card(card_id, msg_id=msg)
            else:
                cm.Notifier.say(self, text, wait=True)
        self._later(go)
        return card_id

    def custom_id(self, action, card_id):
        return sign_id(self.secret, action, card_id)

    def parse_id(self, cid):
        m = re.fullmatch(r"ls1:([a-z]{2,12}):(\d{1,12}):([0-9a-f]{20})", cid or "")
        if not m:
            return None
        action, card_id, mac = m.group(1), int(m.group(2)), m.group(3)
        good = hmac.new(self.secret, f"{action}:{card_id}".encode(), hashlib.sha256).hexdigest()[:20]
        return (action, card_id) if hmac.compare_digest(mac, good) else None

    def payload(self, card_id):
        """The card's message: an embed about the page, its state, and the buttons that fit."""
        card = self.c.store.card(card_id)
        row = self.c.store.get(card["key"])
        model, gb = cm.parse_key(card["key"])
        it = (cm._j(row["proposed"]) or cm._j(row["item"]) or {}) if row else {}
        name = it.get("name") or "(a tagged GameBanana page we haven't listed)"
        fields = [{"name": "Kind", "value": it.get("kind") or "?", "inline": True},
                  {"name": "Page", "value": f"`{card['key']}`", "inline": True},
                  {"name": "By", "value": cm.discord_text((row or {}).get("submitter") or "?", 40), "inline": True}]
        checks = it.get("checks") or {}
        if checks:
            what = ", ".join(f"{n} {k}{'s' if n != 1 else ''}" for k, n in sorted(checks.get("kinds", {}).items()))
            fields.append({"name": "Files checked", "value": f"{checks.get('files', 0)} passed ({what or 'none of a checked kind'})", "inline": False})
        embed = {"title": name[:200], "url": cm.gb_link(model, gb), "fields": fields}
        shots = it.get("screenshots") or []
        if shots and shots[0].startswith("https://images.gamebanana.com/"):
            embed["thumbnail"] = {"url": shots[0]}
        if card["kind"] == "listing":
            embed.update(color=0x2E8B57, description="Listed after the automatic checks.")
            buttons = [_button("Pull", self.custom_id("pull", card_id), DANGER)]
        elif card["kind"] == "report":
            rep = self.c.store.report(card["report_id"]) or {}
            week = self.c.store.report_count(card["key"], cm.now() - 7 * 86400)
            desc = f"**Report: {rep.get('reason', '?')}**"
            if rep.get("note"):
                desc += f"\n> {cm.discord_text(rep['note'], cm.MAX_NOTE)}"
            desc += f"\nReporter #{rep.get('reporter', '?')} · {week} report(s) for this page in 7 days"
            embed.update(color=0xD2691E, description=desc)
            blockable = bool(row and row["submitter_id"])
            buttons = [_button("Pull", self.custom_id("pull", card_id), DANGER),
                       _button("Keep", self.custom_id("keep", card_id), SECONDARY),
                       _button("Block creator", self.custom_id("block", card_id), SECONDARY, disabled=not blockable)]
        else:  # queue: a blocked creator's submission
            embed.update(color=0x4169E1, description="From a blocked creator: waits for a moderator.")
            buttons = [_button("Approve", self.custom_id("approve", card_id), SUCCESS),
                       _button("Reject", self.custom_id("reject", card_id), DANGER)]
        if card["action"]:
            line = f"**{DONE_WORDS.get(card['action'], card['action'])}** by <@{card['actor']}>"
            if card["reason"]:
                line += f" · {cm.discord_text(card['reason'], 200)}"
            embed["description"] = (embed.get("description", "") + "\n\n" + line).strip()
            embed["color"] = 0x808080
            buttons = [_button("Undo", self.custom_id("undo", card_id), SECONDARY)]
        return {"embeds": [embed], "components": [{"type": 1, "components": buttons}]}

    # -- interactions --

    def interaction(self, headers, body):
        """(status, answer) for POST /discord/interactions."""
        if not self.pub:
            return 503, {"error": "buttons are off"}
        sig, ts = headers.get("X-Signature-Ed25519") or "", headers.get("X-Signature-Timestamp") or ""
        if not re.fullmatch(r"[0-9a-fA-F]{128}", sig) or not re.fullmatch(r"\d{1,12}", ts):
            return 401, {"error": "bad signature"}
        if abs(time.time() - int(ts)) > MAX_SKEW:
            return 401, {"error": "stale"}
        if not verify(self.pub, ts.encode() + body, bytes.fromhex(sig)):
            return 401, {"error": "bad signature"}
        try:
            d = json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            return 400, {"error": "bad body"}
        if not isinstance(d, dict):
            return 400, {"error": "bad body"}
        if d.get("type") == PING:
            return 200, {"type": R_PONG}
        if str(d.get("guild_id") or "") != self.guild:
            return 200, self.tell("This only works in the Loner-Strike server.")
        member = d.get("member") if isinstance(d.get("member"), dict) else {}
        user = member.get("user") if isinstance(member.get("user"), dict) else {}
        uid = str(user.get("id") or "")
        roles = {str(r) for r in member.get("roles") or [] if isinstance(r, (str, int))}
        # role applications: anyone may apply; deciding has its own roles (applications.py)
        data = d.get("data") if isinstance(d.get("data"), dict) else {}
        parsed = self.parse_id(str(data.get("custom_id") or "")) if d.get("type") in (COMPONENT, MODAL) else None
        if parsed and self.apps and parsed[0] in applications.PUBLIC_ACTIONS + applications.DECIDER_ACTIONS:
            if not uid:
                return 200, self.tell("You can't do that.")
            if parsed[0] in applications.DECIDER_ACTIONS and not self.apps.may_decide(uid, roles):
                return 200, self.tell("You can't do that.")
            return 200, self.application(d, parsed, uid, roles, user, member)
        if not uid or not (uid == self.owner or roles & self.roles):
            return 200, self.tell("You can't do that.")
        actor = {"id": uid, "name": cm.one_line(str(user.get("global_name") or user.get("username") or uid), 40)}
        iid = str(d.get("id") or "")
        with self.lock:
            if not iid or not self.c.store.first_time(iid):
                return 200, self.tell("Already done.")
            try:
                if d.get("type") == COMPONENT:
                    return 200, self.click(d, actor)
                if d.get("type") == MODAL:
                    return 200, self.submit_modal(d, actor)
                if d.get("type") == COMMAND:
                    return 200, self.command(d, actor)
            except cm.Reject as e:
                return 200, self.tell(e.message)
            except (TypeError, AttributeError, KeyError, ValueError, IndexError):
                return 200, self.tell("That didn't make sense to the list service.")
        return 400, {"error": "unknown interaction"}

    def application(self, d, parsed, uid, roles, user, member):
        action, n = parsed
        actor = {"id": uid, "name": cm.one_line(str(user.get("global_name") or user.get("username") or uid), 40)}
        iid = str(d.get("id") or "")
        with self.lock:
            if not iid or not self.c.store.first_time(iid):
                return self.tell("Already done.")
            try:
                if action == "apply":
                    return self.apps.start()
                if action == "applypick":
                    return self.apps.pick(d, uid, roles)
                if action == "applyform":
                    return self.apps.submit(d, n, user, member)
                return self.apps.decide(action, n, d, actor)
            except cm.Reject as e:
                return self.tell(e.message)
            except (TypeError, AttributeError, KeyError, ValueError, IndexError):
                return self.tell("That didn't make sense to the list service.")

    def tell(self, text):
        return {"type": R_MESSAGE, "data": {"content": text[:1900], "flags": EPHEMERAL, "allowed_mentions": {"parse": []}}}

    def _card_for(self, d, field):
        data = d.get("data") if isinstance(d.get("data"), dict) else {}
        parsed = self.parse_id(str(data.get("custom_id") or ""))
        if not parsed:
            raise cm.Reject(400, "That button isn't one of ours.")
        action, card_id = parsed
        card = self.c.store.card(card_id)
        if not card:
            raise cm.Reject(404, "That card is gone.")
        return action, card, data

    def click(self, d, actor):
        action, card, _ = self._card_for(d, "custom_id")
        allowed = {"listing": ("pull",), "report": ("pull", "keep", "block"), "queue": ("approve", "reject")}[card["kind"]]
        if action == "undo":
            self.undo(card, actor)
        elif action not in allowed:
            raise cm.Reject(400, "That button doesn't fit this card.")
        elif card["action"]:
            raise cm.Reject(409, f"Already {DONE_WORDS.get(card['action'], card['action']).lower()} by {card['actor_name']}.")
        elif action in ("pull", "reject"):
            # a reason first, in a small form (needed to reject: the author sees it)
            return {"type": R_MODAL, "data": {"custom_id": self.custom_id(action + "m", card["id"]),
                                              "title": "Pull it" if action == "pull" else "Turn it down",
                                              "components": [{"type": 1, "components": [{
                                                  "type": 4, "custom_id": "reason", "style": 2, "label": "Why? (kept with the entry)",
                                                  "min_length": 0 if action == "pull" else 3, "max_length": cm.MAX_REASON,
                                                  "required": action == "reject"}]}]}}
        else:
            self.act(card, action, actor, "")
        return {"type": R_UPDATE, "data": self.payload(card["id"])}

    def submit_modal(self, d, actor):
        action, card, data = self._card_for(d, "custom_id")
        if action not in ("pullm", "rejectm"):
            raise cm.Reject(400, "That form isn't one of ours.")
        if card["action"]:
            raise cm.Reject(409, f"Already {DONE_WORDS.get(card['action'], card['action']).lower()} by {card['actor_name']}.")
        reason = ""
        for row in data.get("components") or []:
            for comp in (row or {}).get("components") or []:
                if isinstance(comp, dict) and comp.get("custom_id") == "reason" and isinstance(comp.get("value"), str):
                    reason = comp["value"]
        self.act(card, action[:-1], actor, reason)
        return {"type": R_UPDATE, "data": self.payload(card["id"])}

    def act(self, card, action, actor, reason):
        """Does a card's action, remembering the entry as it was so Undo can put it back."""
        key = card["key"]
        before = self.c.store.get(key)
        reason = cm.one_line(reason or "", cm.MAX_REASON)
        if action == "pull":
            self.c.moderate(key, "pull", {"reason": reason or PULLED})
        elif action == "approve":
            self.c.moderate(key, "approve", {"reason": ""}, live=False)
        elif action == "reject":
            self.c.moderate(key, "reject", {"reason": reason})
        elif action == "block":
            row = before or {}
            if not row.get("submitter_id"):
                raise cm.Reject(409, "We don't know who made this page, so there's nobody to block.")
            self.c.block(row["submitter_id"], row["submitter"], actor["name"], reason or "blocked from a report")
        after = self.c.store.get(key)
        self.c.store.update_card(card["id"], action=action, actor=actor["id"], actor_name=actor["name"], reason=reason,
                                 before=json.dumps(before) if before else "", after_hash=cm.row_hash(after))
        self.audit(f"{DONE_WORDS[action]} `{key}` by <@{actor['id']}>" + (f" · {cm.discord_text(reason, 200)}" if reason else ""))

    def undo(self, card, actor):
        if not card["action"]:
            raise cm.Reject(409, "Nothing to undo.")
        key = card["key"]
        now_row = self.c.store.get(key)
        if card["action"] not in ("keep", "block") and cm.row_hash(now_row) != card["after_hash"]:
            raise cm.Reject(409, f"`{key}` changed since; undo it with lsmod.py instead.")
        if card["action"] == "block":
            row = cm._j(card["before"]) or {}
            self.c.unblock(row.get("submitter_id", 0), actor["name"])
        elif card["action"] != "keep":
            self.c.store.restore(key, cm._j(card["before"]), f"undo {card['action']} by {actor['name']}")
        self.audit(f"Undid {DONE_WORDS[card['action']].lower()} on `{key}` (by <@{actor['id']}>)")
        self.c.store.update_card(card["id"], action="", actor="", actor_name="", reason="", before="", after_hash="")

    def audit(self, line):
        self._later(lambda: self.bot.post({"content": f"Audit: {line}"[:1900]}) if self.bot else None)

    # -- slash commands (/community pull|keep|block|status <page> [reason]) --

    def command(self, d, actor):
        data = d.get("data") if isinstance(d.get("data"), dict) else {}
        if data.get("name") != "community":
            raise cm.Reject(400, "Unknown command.")
        subs = data.get("options")
        sub = subs[0] if isinstance(subs, list) and subs and isinstance(subs[0], dict) else {}
        raw = sub.get("options") if isinstance(sub.get("options"), list) else []
        opts = {o.get("name"): o.get("value") for o in raw if isinstance(o, dict) and isinstance(o.get("name"), str)}
        page = opts.get("page") if isinstance(opts.get("page"), str) else ""
        reason = opts.get("reason") if isinstance(opts.get("reason"), str) else ""
        p = cm.parse_key(page)
        if not p:
            raise cm.Reject(400, "Name the page as mod:<id>, spray:<id> or its GameBanana link.")
        key = cm.key_of(*p)
        name = sub.get("name")
        if name == "status":
            st = self.c.status(*p)
            reps = self.c.store.report_count(key, cm.now() - 7 * 86400)
            return self.tell(f"`{key}`: {st['state']}" + (f" ({st['reason']})" if st.get("reason") else "") + f" · {reps} report(s) in 7 days")
        if name == "pull":
            self.c.moderate(key, "pull", {"reason": reason or PULLED})
            self.audit(f"Pulled `{key}` by <@{actor['id']}> (command)" + (f" · {cm.discord_text(reason, 200)}" if reason else ""))
            return self.tell(f"Pulled `{key}`. Run `lsmod.py publish --upload` (or wait for the next publish) to put it out.")
        if name == "keep":
            self.audit(f"Kept `{key}` by <@{actor['id']}> (command)")
            return self.tell(f"Noted: `{key}` stays.")
        if name == "block":
            row = self.c.store.get(key)
            if not row or not row["submitter_id"]:
                raise cm.Reject(409, "We don't know who made this page, so there's nobody to block.")
            self.c.block(row["submitter_id"], row["submitter"], actor["name"], reason or "blocked by command")
            self.audit(f"Creator of `{key}` ({cm.discord_text(row['submitter'], 40)}) blocked by <@{actor['id']}> (command)")
            return self.tell(f"{row['submitter']}'s next submissions wait for a moderator.")
        raise cm.Reject(400, "Unknown command.")
