#!/usr/bin/env python3
# "Apply for a role" in the Loner-Strike Discord (through modbot.py's interactions endpoint).
#
# A pinned message in the public #apply channel has an [Apply for a role] button (posted by
# discord-setup.py --apply). Anyone in the server may press it: they pick a role, then fill in a
# small form (why, links). The application lands as a card in the private applications channel
# with [Approve] [Deny]; only Moderator, Dev, G-Man (DISCORD_APPLY_ROLES) or the owner may act.
# Approve gives that one role and nothing else (a Moderator application only ever gets the
# Moderator role; whatever that role may do is set in Discord). Deny can carry a reason and the
# applicant is told by DM when their DMs are open. Every action edits the card, is logged to the
# channel and has [Undo]. Applications are kept in the community database (table applications).
import time

import community as cm

ROLES = (("playtester", "Playtester"), ("map_maker", "Map Maker"), ("mode_maker", "Mode Maker"),
         ("bug_hunter", "Bug Hunter"), ("surf_legend", "Surf Legend"), ("deathrun_legend", "Deathrun Legend"),
         ("moderator", "Moderator"))
NAMES = dict(ROLES)
WHY_MIN, WHY_MAX, LINKS_MAX = 20, 1000, 400
COOLDOWN = 14 * 86400        # after a no, before the same role can be asked for again
PER_DAY = 5                  # applications a person may send a day (all roles)
ROLE_CACHE = 600
EPHEMERAL = 64
PUBLIC_ACTIONS = ("apply", "applypick", "applyform")
DECIDER_ACTIONS = ("appok", "appno", "appnom", "appundo")
MISSING_PERMISSIONS = 50013


def norm(name):
    """A role name compared without emoji, punctuation or case ("🗺️・Map Maker" -> "mapmaker")."""
    return "".join(c for c in name.lower() if c.isalnum() and c.isascii())


def created(snowflake):
    """When a Discord account was made (from its id), as a date."""
    try:
        return time.strftime("%Y-%m-%d", time.gmtime(((int(snowflake) >> 22) + 1420070400000) / 1000))
    except (TypeError, ValueError):
        return "?"


class Applications:
    def __init__(self, mod, channel, deciders, privileged=("moderator",)):
        self.mod, self.c, self.bot = mod, mod.c, mod.bot
        self.channel = str(channel or "")
        self.deciders = {str(r) for r in deciders if r}
        # roles with powers: only the owner or a holder may give them (DISCORD_APPLY_PRIVILEGED,
        # role keys or names; Moderator always counts)
        self.privileged = {"moderator"} | {k for k, n in ROLES for p in privileged if p and norm(p) in (norm(k), norm(n))}
        self.per_day = cm.Window(PER_DAY, 86400)
        self._roles, self._roles_at = {}, 0.0

    # -- roles in the server --

    def role_id(self, key):
        """The Discord role id for one of ROLES, looked up by name (cached a few minutes)."""
        if time.time() - self._roles_at > ROLE_CACHE or not self._roles:
            code, ans = self.bot.api("GET", f"/guilds/{self.mod.guild}/roles")
            if code == 200 and isinstance(ans, list):
                self._roles = {norm(str(r.get("name", ""))): str(r.get("id")) for r in ans if isinstance(r, dict)}
                self._roles_at = time.time()
        return self._roles.get(norm(NAMES[key]))

    def may_decide(self, uid, roles):
        return uid == self.mod.owner or bool(roles & self.deciders)

    # -- the applicant's side --

    def check_can_apply(self, uid, key, member_roles):
        rid = self.role_id(key)
        if rid and rid in member_roles:
            raise cm.Reject(409, f"You already have the {NAMES[key]} role.")
        for a in self.c.store.applications_of(uid, key):
            if a["state"] == "pending":
                raise cm.Reject(409, f"Your {NAMES[key]} application is waiting for the team. You'll hear back.")
            if a["state"] == "denied" and a["decided_at"] + COOLDOWN > cm.now():
                when = time.strftime("%Y-%m-%d", time.gmtime(a["decided_at"] + COOLDOWN))
                raise cm.Reject(409, f"You can apply for {NAMES[key]} again from {when}.")

    def start(self):
        """The button in #apply: a private message with the roles to pick from."""
        options = [{"label": name, "value": key} for key, name in ROLES]
        return {"type": 4, "data": {"flags": EPHEMERAL, "allowed_mentions": {"parse": []},
                                    "content": "Which role would you like to apply for?",
                                    "components": [{"type": 1, "components": [{
                                        "type": 3, "custom_id": self.mod.custom_id("applypick", 0),
                                        "placeholder": "Pick a role", "options": options}]}]}}

    def pick(self, d, uid, member_roles):
        """A role picked: the form (why, links)."""
        values = (d.get("data") or {}).get("values")
        key = values[0] if isinstance(values, list) and values and isinstance(values[0], str) else ""
        if key not in NAMES:
            raise cm.Reject(400, "Pick one of the roles in the list.")
        self.check_can_apply(uid, key, member_roles)
        index = [k for k, _ in ROLES].index(key)
        return {"type": 9, "data": {
            "custom_id": self.mod.custom_id("applyform", index), "title": f"Apply: {NAMES[key]}"[:45],
            "components": [
                {"type": 1, "components": [{"type": 4, "custom_id": "why", "style": 2, "label": "Why? What would you do with it?",
                                            "min_length": WHY_MIN, "max_length": WHY_MAX, "required": True}]},
                {"type": 1, "components": [{"type": 4, "custom_id": "links", "style": 2,
                                            "label": "Links (GameBanana, Steam, examples)", "max_length": LINKS_MAX,
                                            "required": False}]}]}}

    def submit(self, d, index, user, member):
        """The form sent: the application is kept and goes to the team as a card."""
        if not 0 <= index < len(ROLES):
            raise cm.Reject(400, "That form isn't one of ours.")
        key = ROLES[index][0]
        uid = str(user.get("id"))
        member_roles = {str(r) for r in member.get("roles") or [] if isinstance(r, (str, int))}
        fields = {}
        for row in (d.get("data") or {}).get("components") or []:
            for comp in (row or {}).get("components") or []:
                if isinstance(comp, dict) and isinstance(comp.get("value"), str):
                    fields[comp.get("custom_id")] = comp["value"]
        why = cm.one_line(fields.get("why", ""), WHY_MAX)
        links = cm.one_line(fields.get("links", ""), LINKS_MAX)
        if len(why) < WHY_MIN:
            raise cm.Reject(400, f"Say a little more about why (at least {WHY_MIN} characters).")
        self.check_can_apply(uid, key, member_roles)
        if not self.per_day.allow(uid):
            raise cm.Reject(429, "That's a lot of applications for one day. Try again tomorrow.")
        name = cm.one_line(str(user.get("global_name") or user.get("username") or uid), 40)
        app_id = self.c.store.add_application(user_id=uid, user_name=name, role_key=key, state="pending", why=why,
                                              links=links, joined=str(member.get("joined_at") or "")[:10])
        msg = self.bot.post_to(self.channel, self.card(app_id)) if self.channel else None
        if msg:
            self.c.store.update_application(app_id, msg_id=msg)
        else:
            self.mod.say(f"**Application #{app_id}** for {NAMES[key]} from {cm.discord_text(name, 40)} (the applications "
                         f"channel couldn't be reached; see `lsmod.py` or the database).")
        cm.log(f"application #{app_id} for {key}")
        return self.mod.tell(f"Thanks! Your {NAMES[key]} application is with the team. You'll hear back here or by DM.")

    # -- the team's side --

    def card(self, app_id):
        a = self.c.store.application(app_id)
        name = NAMES.get(a["role_key"], a["role_key"])
        desc = (f"<@{a['user_id']}> ({cm.discord_text(a['user_name'], 40)}) · account made {created(a['user_id'])}"
                + (f" · joined {cm.discord_text(a['joined'], 10)}" if a["joined"] else ""))
        fields = [{"name": "Why", "value": cm.discord_text(a["why"], WHY_MAX) or "-", "inline": False}]
        if a["links"]:
            fields.append({"name": "Links", "value": cm.discord_text(a["links"], LINKS_MAX), "inline": False})
        if a["role_key"] == "moderator":
            fields.append({"name": "Note", "value": "Approving gives the Moderator role only, nothing else.", "inline": False})
        embed = {"title": f"Application #{a['id']}: {name}", "description": desc, "fields": fields, "color": 0x4169E1}
        if a["state"] == "pending":
            buttons = [{"type": 2, "style": 3, "label": "Approve", "custom_id": self.mod.custom_id("appok", a["id"])},
                       {"type": 2, "style": 4, "label": "Deny", "custom_id": self.mod.custom_id("appno", a["id"])}]
        else:
            word = "Approved" if a["state"] == "approved" else "Denied"
            line = f"**{word}** by <@{a['decider']}>" + (f" · {cm.discord_text(a['reason'], 200)}" if a["reason"] else "")
            embed["description"] += "\n\n" + line
            embed["color"] = 0x2E8B57 if a["state"] == "approved" else 0x808080
            buttons = [{"type": 2, "style": 2, "label": "Undo", "custom_id": self.mod.custom_id("appundo", a["id"])}]
        if a["note"]:
            embed["description"] += f"\n\n⚠ {cm.one_line(a['note'], 400)}"  # (our own words; mentions are off anyway)
        return {"embeds": [embed], "components": [{"type": 1, "components": buttons}]}

    def _app(self, app_id):
        a = self.c.store.application(app_id)
        if not a:
            raise cm.Reject(404, "That application is gone.")
        return a

    def may_grant(self, actor, key, rid):
        """A role with powers (Moderator, DISCORD_APPLY_PRIVILEGED) only from the owner or someone
        who has it themselves (a Dev or G-Man can't make a Moderator); earned roles from any decider."""
        if key not in self.privileged:
            return True
        return actor["id"] == self.mod.owner or (rid is not None and rid in actor.get("roles", set()))

    def decide(self, action, app_id, d, actor):
        """Approve / Deny (form) / Undo on a card. Returns the interaction answer."""
        a = self._app(app_id)
        if actor["id"] == a["user_id"] and actor["id"] != self.mod.owner:
            raise cm.Reject(403, "You can't decide on your own application; someone else on the team will.")
        if action in ("appok", "appno", "appnom") and a["state"] != "pending":
            raise cm.Reject(409, f"Already {a['state']} by {a['decider_name'] or 'someone'}.")
        name = NAMES.get(a["role_key"], a["role_key"])
        if action in ("appok", "appundo"):
            rid = self.role_id(a["role_key"])
            if rid and not self.may_grant(actor, a["role_key"], rid):
                raise cm.Reject(403, f"Only someone who has the {name} role (or the owner) can give it or take it back.")
        if action == "appok":
            if not rid:
                self.c.store.update_application(app_id, note=f"There's no role called {name} in the server. Make it (or rename one), then press Approve again.")
                return {"type": 7, "data": self.card(app_id)}
            code, ans = self.bot.api("PUT", f"/guilds/{self.mod.guild}/members/{a['user_id']}/roles/{rid}",
                                     reason=f"Application #{app_id} approved by {actor['name']}")
            if code in (200, 204):
                self.c.store.update_application(app_id, state="approved", decider=actor["id"], decider_name=actor["name"],
                                                decided_at=cm.now(), note="", reason="")
                self.mod.audit(f"Application #{app_id} ({name}) approved by <@{actor['id']}>")
            elif code == 403:
                hint = (f"Discord refused to give the role: the bot's own role has to be ABOVE {name} in Server Settings > "
                        f"Roles (drag \"Loner-Strike Setup\" above it) and have Manage Roles. Then press Approve again.")
                self.c.store.update_application(app_id, note=hint if (ans or {}).get("code") in (MISSING_PERMISSIONS, None) else
                                                f"Discord refused to give the role ({(ans or {}).get('message', 'forbidden')}).")
            elif code == 404:
                self.c.store.update_application(app_id, note="They're not in the server any more.")
            else:
                self.c.store.update_application(app_id, note=f"Discord didn't answer (status {code}). Press Approve again in a minute.")
            return {"type": 7, "data": self.card(app_id)}
        if action == "appno":
            return {"type": 9, "data": {"custom_id": self.mod.custom_id("appnom", app_id), "title": f"Deny: {name}"[:45],
                                        "components": [{"type": 1, "components": [{
                                            "type": 4, "custom_id": "reason", "style": 2, "max_length": cm.MAX_REASON,
                                            "label": "Why? (optional; they're told by DM)", "required": False}]}]}}
        if action == "appnom":
            reason = ""
            for row in (d.get("data") or {}).get("components") or []:
                for comp in (row or {}).get("components") or []:
                    if isinstance(comp, dict) and comp.get("custom_id") == "reason" and isinstance(comp.get("value"), str):
                        reason = cm.one_line(comp["value"], cm.MAX_REASON)
            self.c.store.update_application(app_id, state="denied", decider=actor["id"], decider_name=actor["name"],
                                            decided_at=cm.now(), reason=reason, note="")
            told = self.dm(a["user_id"], name, reason)
            if not told:
                self.c.store.update_application(app_id, note="Couldn't tell them by DM (their DMs are closed).")
            self.mod.audit(f"Application #{app_id} ({name}) denied by <@{actor['id']}>")
            return {"type": 7, "data": self.card(app_id)}
        if action == "appundo":
            if a["state"] == "pending":
                raise cm.Reject(409, "Nothing to undo.")
            if a["state"] == "approved":
                code, _ = self.bot.api("DELETE", f"/guilds/{self.mod.guild}/members/{a['user_id']}/roles/{rid}",
                                       reason=f"Application #{app_id}: approval undone by {actor['name']}") if rid else (0, {})
                if code not in (200, 204):
                    raise cm.Reject(409, "Discord didn't take the role back (check the bot's role is above it). Nothing changed.")
            self.c.store.update_application(app_id, state="pending", decider="", decider_name="", decided_at=0, reason="",
                                            note="Undone (they weren't told again)." if a["state"] == "denied" else "")
            self.mod.audit(f"Application #{app_id} ({name}): {a['state']} undone by <@{actor['id']}>")
            return {"type": 7, "data": self.card(app_id)}
        raise cm.Reject(400, "Unknown action.")

    def dm(self, uid, name, reason):
        """Tells an applicant politely; False when their DMs are closed (or Discord says no)."""
        code, ans = self.bot.api("POST", "/users/@me/channels", {"recipient_id": uid})
        if code != 200 or not isinstance(ans, dict) or not ans.get("id"):
            return False
        text = (f"Hi! Thanks for applying for {name} in the Loner-Strike Discord. The team said no this time"
                + (f": {cm.one_line(reason, cm.MAX_REASON)}" if reason else ".")
                + f" You're welcome to apply again from {time.strftime('%Y-%m-%d', time.gmtime(cm.now() + COOLDOWN))}.")
        return self.bot.post_to(str(ans["id"]), {"content": text[:1900]}) is not None
