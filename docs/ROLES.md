# Community roles: linking, Supporter VIP and server moderators

Three things that tie Loner-Strike's game servers to its Discord: players **link** their SteamID to
their Discord account, **supporters** get a cosmetic VIP on official servers (and on servers whose host
honors them), and players **apply to moderate** a server, with an invite from its owner when approved.
Design, API and security notes: docs/notes/Roles.md. Code: `services/serverlist/roles.py` (the list
service and the bot), `app/src-tauri/src/roles.rs` (the host's app), `app/server-agent/src/roles.rs`
(ls-control on a Linux server), `plugins/scripting/gm_extras.sma` (/link) and `gm_vip.sma` (VIP).

## For players
- **Link:** on any Loner-Strike server whose host claimed it (every official server), say `/link` in the
  game. You get a code like `ABCD-EFGH` for 10 minutes. In the Loner-Strike Discord type
  `/link code:ABCD-EFGH`. `/link` alone says what you're linked to, `/unlink` undoes it. One Discord account
  ↔ one SteamID. A link made on an official server is *verified*; it replaces an unverified one of the same
  SteamID.
- **Supporter VIP:** supporting Loner-Strike on Polar gives the Supporter role in Discord; with your SteamID
  linked, official servers show you as a VIP within 5 minutes: the [VIP] tag, the scoreboard mark, a kept
  slot and a join message. Never gear, money or abilities. One-time or monthly, it's the same Supporter: each payment
  from the minimum counts (with the box's setting, the first one makes you a supporter for good, and cancelling a
  monthly support takes nothing away).
- **Moderate a server:** in the Discord's #apply channel, Apply for a role › Server moderator (needs a link;
  official servers need a verified one). Pick the server and answer the questions. The owner decides; when
  approved, the bot DMs you an invite: in Loner-Strike, Settings › Shared control › Accept an invite.

## For hosts
- **Claim** your server: Shared control › Loner-Strike roles › Claim (the server must be running and reachable
  from the internet). Then type the owner code it shows in the Discord: `/server claim code:…`, so moderator
  applications reach you by DM.
- Switches: **Accept moderator applications**, **Let Loner-Strike Discord moderators decide** (they can approve
  and deny for you), **Honor Loner-Strike supporters** (off by default: only the supporters among the players on
  your server are asked about, never a list).
- Supporters' perks: Admins › VIP & donations › Supporters (tag, scoreboard, join message, reserved slots).
- Approved applicants show under Shared control › Moderator applications: **Create moderator invite** (you, or an
  Admin you gave shared control to) and **Revoke** (app access, the in-game role and the Discord role go).
- On a server on this computer, /link and supporters work while Loner-Strike is open. On a Linux server they
  run on the server itself (ls-control's roles unit), so they work with your computer off; the server needs
  `curl`.

## For the Loner-Strike team (the box, Discord, Polar)
Nothing here is live until these are done (each needs the user's OK):
1. **Box:** deploy with `services/serverlist/install.sh` (it copies roles.py and the unit now runs with
   `--roles-db`); set `DISCORD_SERVERMOD_DECIDERS` (default: the Moderator role) and, later,
   `POLAR_WEBHOOK_SECRET` with `set-env.sh` from stdin; restart. The daily backup includes roles.db.
2. **Discord:** create the roles **Supporter** (for Polar's bot) and **Server Moderator** (ours), with the
   Loner-Strike bot's role above Server Moderator. Register the commands:
   `python3 discord-setup.py --apply --roles` (adds /link, /unlink, /server claim beside /community).
3. **Polar:** Benefits › New benefit › Discord: connect the server, role Supporter (Polar's bot above it);
   attach it to the monthly and the one-time products. Settings › Webhooks › Add endpoint
   `https://list.lonerstrike.com/roles/polar/webhook`, format Raw, events `benefit_grant.created`,
   `benefit_grant.updated`, `benefit_grant.cycled`, `benefit_grant.revoked`, `order.paid`, `order.refunded`; put its
   secret on the box (step 1); send a test event: Polar shows 202. A full refund of a one-time donation takes its 30
   days off again. A donation counts from `POLAR_SUPPORTER_MIN_CENTS` (default 300, net of discounts; a $0
   pay-what-you-want download never counts) and gives `POLAR_SUPPORTER_DAYS` days, stacking (default 30; 0: for
   good).
4. **Official servers:** claim each one from the app, then `lsmod.py roles official <server> on`. Turn on
   Accept moderator applications. Set Supporters › Reserved slots to 1 in its Admins page.
5. Moderation from a terminal: `lsmod.py roles servers | log | show <id> | supporters | unlink <steamid>`.
