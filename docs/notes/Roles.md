# Community roles: Steam ↔ Discord link, Supporter VIP, server moderator applications

Owner: **MCP** (list service, Discord bot, app backend routes, ls-control, plugins). **P5**: the app UI.
Status (2026-11-14): built and tested (list service and bot, the app's backend, ls-control's roles unit,
gm_extras /link, gm_vip 1.2). Nothing deployed: the box, the live Discord and Polar need the user's OK (docs/ROLES.md,
"For the Loner-Strike team"). The relay is gm_extras (not a new gm_link plugin), so no setup change was needed. On a
Linux server the relay and supporters run as their own unit (`ls-control roles`), because the shared control service
may only talk to this machine; an Admin's calls reach it through a spool in the service's folder.

## The pieces
1. **Server claim.** A host proves they run a server, the same way the server list already works: the list service
   reads a public cvar the host's app set. A claimed server gets a **server token**, which its owner's app (or, on a
   Linux server, ls-control) uses for everything below. LAN-only servers can't claim: the list service has to reach
   them.
2. **Steam ↔ Discord link.** In game, `/link` gives a one-time code; in the Loner-Strike Discord, `/link code:XXXX-XXXX`
   stores Discord id ↔ SteamID. Unlink and relink are possible.
3. **Supporter VIP.** Polar's Discord benefit gives the Supporter role (the user's setup, below). Polar's webhook tells
   the list service who supports; official servers pull the supporters' SteamIDs; hosts may opt in to honor them.
4. **Server moderator applications.** "Server moderator" in the existing Apply flow (needs a link), a card with
   [Approve] [Deny] for the server's owner (and Discord moderators when the owner lets them), then the owner's or a
   shared-control Admin's app makes a Moderator invite that the bot DMs to the applicant.

## 1. Server claim
- App: `POST /roles/claim/start {address}` → `{claim, nonce, expires}` (10 min). The app sets the public cvar
  `ls_claim <nonce>` on the running server (local: RCON; Linux: ls-control), then `POST /roles/claim/finish {claim}`.
  The service asks the server for its rules (A2S, like the server list), and when `ls_claim` matches it answers
  `{server, token, owner_code}`. The token (`lsr1_` + 43 base64url characters, 32 random bytes) is shown once; only
  its SHA-256 is kept. The app clears `ls_claim` afterwards.
- **Owner on Discord:** the owner types `/server claim code:<owner_code>` in the Loner-Strike Discord (code 10 min,
  one use) so the service knows their Discord id: application cards go to them by DM. A new owner code:
  `POST /roles/server/owner-code`.
- **Claimed again** (a new host on the same address, or the same host starting over; security review 2026-11-14): the
  new claim is a new server. The old one keeps its id and history but stops working: not official any more (only
  `lsmod.py roles official` gives it back, to the new one), default settings, its pending, approved and invited
  applications closed, its active moderators revoked (Discord role taken away; their app access and in-game role live
  in the old host's app and game files, which the service can't reach, and that app finds its claim gone). Logged
  (`retire`) with an audit line: "Official server X was re-claimed; official status removed".
- Where the token lives: a local server: `<app dir>/secrets/roles-<server id>` (0600 on the Mac/Linux, user-only
  ACL on Windows). A Linux server: `~/loner-strike/control/<id>/roles-token` (0600, the game user), written by the
  owner's app over SSH; ls-control uses it. The token never goes to helpers' apps.
- **Official servers:** the user marks them on the box: `lsmod.py servers official <server> on`.
- Unclaim: `DELETE /roles/server` (the owner's app). Its links stay (they're the players'); pending and approved
  applications close; invited and active ones are revoked as `revoke` does (invites stop, the Discord role goes unless
  they moderate another server); the answer lists them (`revoked: [{application, member, steamid}]`) and the app takes
  their app access and in-game role away.

## 2. Link
- In game: `say /link` (gm_extras.amxx, on every Loner-Strike server). The plugin logs
  `"Name<uid><STEAM_0:1:123><CT>" triggered "ls_link"` and tells the player "Getting your code…". The **relay**
  (the owner's app for a server on this computer, ls-control for a Linux server) sees the line in the log, asks
  `POST /roles/link/code {steamid, name}` (server token), and runs `ls_link_show <uid> <code>` over RCON; the plugin
  shows: "Your code: ABCD-EFGH. In the Loner-Strike Discord type /link ABCD-EFGH (10 minutes)." The code is made by
  the list service (Python `secrets`), never by the plugin. No relay within 10 s (the app is closed, the server isn't
  claimed): "This server can't make link codes. Type /link on an official Loner-Strike server."
- Code: 8 characters of Crockford base32 (40 bits) shown as XXXX-XXXX, case and hyphen ignored, 10 minutes, one use,
  stored as SHA-256 and compared with `hmac.compare_digest`. A newer code for the same SteamID replaces the old one.
- Discord: `/link code:<code>` (ephemeral answer) binds that Discord id ↔ SteamID; `/link` alone says what you're
  linked to; `/unlink`. One Discord id ↔ one SteamID, both ways. A SteamID linked to someone else: "That SteamID is
  linked to another Discord account. They can /unlink first." Relinking replaces your link, except while you're a
  server moderator through it ("You moderate <server> with this SteamID. Ask its owner to remove you first.").
- Not accepted: bots, `STEAM_ID_LAN`, `STEAM_ID_PENDING`, `VALVE_…`, `HLTV`. Every SteamID is kept and looked up in one
  form, `STEAM_0:Y:Z` (`STEAM_1:1:123` is the same account); older rows are made canonical at start (two links that turn
  out to be one account: the official one stays, else the older).
- **Trust (link squatting, security review 2026-11-14):** a host controls their own server's log, so a host could make a
  code for any SteamID and bind it to their own Discord. So:
  - every link keeps the server it came from; a link made on an official server is **verified**, others **unverified**
    (cards and the app's applications list say so: `link: "verified" | "unverified"`);
  - **an official-server link wins:** it replaces an unverified link of the same SteamID (the old one is deleted,
    logged as `evict`, and its Discord account gets a DM); two official links still need `/unlink`;
  - **presence:** before making a code the service asks the server itself for its player list (A2S_PLAYER) and refuses
    unless a player of that name is on it now ("You don't seem to be on this server right now…"). A2S gives names, never
    SteamIDs, so this proves someone of that name is playing there, not the SteamID;
  - at most **20 different SteamIDs per server per day** get codes;
  - moderators take a squatted link away: `lsmod.py roles unlink <steamid> [--reason ..]` (logged, audit line);
  - applications to moderate an **official** server need a verified link.
  On servers that let non-Steam clients in, SteamIDs can be faked anyway.
- Rate limits: codes 3 per SteamID and 60 per server per 10 min, 20 distinct SteamIDs per server a day; `/link` 5 failed tries per Discord id per hour.

## 3. Supporter VIP
- **Polar (the user's setup):**
  1. Polar dashboard › Benefits › New benefit › Discord: connect the Loner-Strike Discord, pick the role
     **Supporter** (create it first; the Polar bot's role must be above it in Server Settings › Roles).
  2. Attach the benefit to the monthly product and to the one-time donation product.
  3. Polar › Settings › Webhooks › Add endpoint: `https://list.lonerstrike.com/roles/polar/webhook`, format Raw,
     events `benefit_grant.created`, `benefit_grant.updated`, `benefit_grant.revoked`, `order.paid`. Copy the
     signing secret.
  4. Put the secret in a file on your computer, then
     `ssh you@box 'sudo /opt/lonerstrike-list/set-env.sh POLAR_WEBHOOK_SECRET' < that-file` (read from stdin, never on
     a command line; it goes in the service's 0600 env file), delete the file, restart the service. Polar › Webhooks › Send test event: the delivery shows 202.
- **The endpoint** `POST /roles/polar/webhook`: Standard Webhooks signature (`webhook-id`, `webhook-timestamp`,
  `webhook-signature: v1,<base64 HMAC-SHA256 of id.timestamp.body>`), checked in constant time, timestamp within 5 min,
  each `webhook-id` used once, body at most 64 KB. Without the secret set it answers 503 and keeps nothing. 202 on
  success; bad signature 401 (nothing logged but the fact).
- **Who supports:** a Discord benefit grant gives `customer_id` ↔ Discord id (`properties.account_id`).
  Monthly (the grant has a `subscription_id`): supporter while the grant is granted. One-time (`order.paid` without a
  subscription): **30 days per donation, stacking** (`SUPPORTER_DAYS`, default 30), from the later of now and the end
  of what they had; an order that arrives before its grant is kept and counted when the grant comes. Grant events can
  arrive out of order: each grant keeps when it changed (`modified_at`, else the webhook's timestamp); an older event is
  ignored, and a revoke is only undone by a newer grant.
- **To servers:** a supporter's VIP follows their current link. Official servers: ls-control (a timer, every 5 min)
  `GET /roles/vip` → every supporter's SteamID. Other servers that turned on **Honor Loner-Strike supporters** (off by
  default): `POST /roles/vip/check {steamids}` with the players on the server now (at most 64), answered with the ones
  who support; a host never gets the whole list. The relay writes them in their own block of users.ini
  (`; ---- Loner-Strike supporters ----`) with the new flag **"s"** (ADMIN_LEVEL_G), then `amx_reloadadmins`.
- **gm_vip** (new "supporter" role, bit 16 in the ability masks): defaults for supporters: the [VIP] tag, the scoreboard
  mark, a reserved slot and a join message; gear, money and abilities off (bit 16 isn't in any default mask). A host's
  own VIPs ("t") are unchanged. New cvars: `gm_vip_reserve` (roles mask with a reserved slot, default 1|16),
  `gm_vip_slots` (reserved slots, default 1 on official servers, 0 elsewhere), `gm_vip_joinmsg` (roles mask, default 16).
  Reserved slot: `sv_visiblemaxplayers` = maxplayers − slots; someone without a reservation who takes a hidden slot is
  kicked with "That slot is kept for supporters." unless a bot can make room (a bot is kicked first; bot_quota fills
  it again later).

## 4. Server moderator applications
- **Applying:** Apply › **Server moderator** (needs a link: "Link your SteamID first: type /link in game on any
  Loner-Strike server."). A select of claimed servers that accept applications (25 at most; official first), then a
  form: availability (days and hours, time zone), experience, why, "anything else". Cooldown 14 days after a no, one
  open application per server, 5 a day (as today).
- **The card** (signed button ids, as today) goes to the server's owner by **DM**, and to the private applications
  channel when the server lets Discord moderators decide (official servers always). Shows the Discord name and account
  age, the SteamID and where it was linked, the server, the answers.
- **Who decides:** the server's owner (their Discord id from `/server claim`), and members with the
  `DISCORD_SERVERMOD_DECIDERS` roles when the server has **Let Loner-Strike Discord moderators decide** on. Nobody
  decides on their own application. Button clicks from DMs are accepted only for these cards (no guild id there; the
  signed id and the decider check still apply).
- **Approved:** the applicant gets a DM ("approved; the server's team will send your invite"). The application shows in
  the owner's app (and its Admins' apps) under that server's shared control.
- **Invite:** "Create moderator invite" (the owner, or a shared-control **Admin**; Admins can only ever make Moderator
  invites, as today; only the owner can make Admin invites). The app makes a normal shared-control invite with
  role Moderator, bound to the applicant's SteamID, 24 h, one use, carrying the application id, then
  `POST /roles/applications/<id>/invite {invite, expires}`; the bot DMs it with how to accept. Only a hash of the
  invite is kept by the service. DMs closed: the app shows the invite to copy and send another way.
- **Redeeming:** in their app (Settings › Shared control › Accept an invite, as today). The owner's app (or ls-control)
  gives app access as Moderator and the in-game Moderator role for that SteamID (as today), then tells the service
  `POST /roles/applications/<id>/redeemed`; the bot gives the Discord role **Server Moderator**.
- **Revoke:** one click in the owner's (or an Admin's, for Moderators) app: removes app access and the in-game role
  (as today) and `POST /roles/applications/<id>/revoke`; the bot takes Server Moderator away (unless they moderate
  another server).
- **Log:** the service logs every claim, link, unlink, card decision, invite, redemption and revoke (table
  `roles_log`, `lsmod.py roles log`), with an audit line in the team channel. Moderator actions in game and in the app
  stay in the server's shared-control activity log (as today), now with the application id.

## List service API (list.lonerstrike.com)
Server endpoints take `Authorization: Bearer lsr1_…` (looked up by SHA-256, compared in constant time). JSON in and
out, bodies at most 16 KB; errors are `{"error": "<plain sentence>"}` with 4xx.

| Method, path | Auth | Body → answer |
|---|---|---|
| POST /roles/claim/start | none (10 per IP per hour) | `{address}` → `{claim, nonce, expires}` |
| POST /roles/claim/finish | none | `{claim}` → `{server, token, owner_code}`; "The server's ls_claim isn't set yet" |
| GET /roles/server | token | → `{server, name, address, official, owner_linked, settings}` |
| POST /roles/server | token | `{settings: {applications, discord_mods_decide, honor_supporters}}` → same as GET |
| POST /roles/server/owner-code | token | → `{owner_code, expires}` |
| DELETE /roles/server | token | → `{ok}` |
| POST /roles/link/code | token | `{steamid, name}` → `{code, expires}`; refused unless `name` is on the server now (A2S) |
| GET /roles/vip | token (official) | → `{steamids, ttl}` |
| POST /roles/vip/check | token (honor on) | `{steamids: [≤64]}` → `{steamids}` |
| GET /roles/applications | token | → `{applications: [{id, state, discord_name, steamid, linked_on, official_link, link: verified|unverified, availability, answers, decided_by, decided_at, invite: {state, expires}}]}` |
| POST /roles/applications/:id/invite | token | `{invite, expires}` → `{ok, dm}` (`dm: false`: DMs closed) |
| POST /roles/applications/:id/redeemed | token | `{member}` → `{ok}` |
| POST /roles/applications/:id/revoke | token | `{by}` → `{ok}` |
| POST /roles/polar/webhook | Standard Webhooks | → 202 |
| POST /discord/interactions | Discord signature | `/link`, `/unlink`, `/server claim`, the Apply flow, the cards |

Application states: `pending` → `approved` | `denied`; `approved` → `invited` → `active` → `revoked`; an unused invite
after 24 h → `approved` again (the app can make another).

## App API (for P5's UI; MCP builds these)
All under the app's own listener (from the window). The Admin rows also work for a shared-control Admin through the
sharing listener (rules.rs), pinned to their server.

| Method, path | Who | Answer |
|---|---|---|
| GET /api/roles/server/:id | owner | `{claimed, official, owner_linked, owner_code?, settings, error?}` |
| POST /api/roles/server/:id/claim | owner | `{ok, owner_code}`; slow (a few s): spinner "Claiming…". Needs the server running and reachable |
| POST /api/roles/server/:id/settings | owner | `{applications, discord_mods_decide, honor_supporters}` → `{ok, settings}` |
| POST /api/roles/server/:id/owner-code | owner | `{owner_code, expires}` |
| POST /api/roles/server/:id/unclaim | owner | `{ok}` (ask first) |
| GET /api/roles/server/:id/applications | owner, Admin | `{applications: [...]}` as above |
| POST /api/roles/server/:id/applications/:aid/invite | owner, Admin | `{ok, dm, invite?}` (`invite` only when the DM failed) |
| POST /api/roles/server/:id/applications/:aid/revoke | owner, Admin (Moderators only) | `{ok}` |
| GET/POST /api/admins/supporters | owner | the supporter perks (the ability grid's new row, reserved slots, join message) in the Admins page |

**UI (P5):** a "Loner-Strike roles" card on a server's Shared control page: claim (with the owner code and
"type /server claim … in the Loner-Strike Discord"), the three switches, the applications list (state chips, Create
moderator invite, Revoke with an ask, the "DMs closed: copy this invite" fallback). Admins see the list and the
buttons, not the claim or switches. The Admins page: a "Supporters" row in the ability grid and the reserved-slot and
join-message settings. Every slow action gets a spinner and its label ("Claiming…", "Sending invite…", "Revoking…").

## Security
- Untrusted: the Polar webhook, every code, every Discord interaction, every server request, every log line the relay
  reads (a player's name can hold anything: the relay takes only the SteamID and the uid from the `triggered "ls_link"`
  line the plugin wrote, and the plugin escapes quotes in names).
- Secrets come from the env file or are made by `secrets`; tokens, codes and invites are kept only as SHA-256;
  compares use `hmac.compare_digest` (Rust: `subtle`-style constant-time compare, as control/ does). Nothing secret is
  logged, printed or committed (tests use made-up values).
- Rate limits per IP, per server token, per Discord id and per SteamID (numbers above); the existing per-day limit for
  applications.
- Every write is idempotent where a retry can happen (webhook ids, interaction ids, invite/redeem/revoke by
  application id).
- Deploying (the box, registering the new commands, the Discord roles, Polar) waits for the user.

## QA4 rows (to add to docs/notes/QA4.md, area G "Roles")
Claim a test server; `/link` in game on a local and a Linux server; `/link` in Discord, a wrong and an expired code,
relink, unlink while moderating; Polar test events (monthly, one-time twice, revoke), a bad signature; VIP on an
official server (tag, scoreboard, join message, the reserved slot with bots), Honor supporters off/on on a host's
server; an application end to end (owner by DM, Discord moderators with the switch, own application refused, deny with
a reason), an Admin's invite, DMs closed, redeem, revoke (app, game, Discord role gone), the log.

## P5: the UI is in (2026-11-14), and what it expects from the routes
Files: `app/ui/src/views/Roles.tsx` (RolesCard, Applications, SupporterVip, LinkHelp), `communityRoles.ts` (rules and
words, tested by `scripts/roles.test.ts`: `node scripts/test-roles.mjs`), `roles.css`; mounted in Servers.tsx (a
server's Shared control tab: the card for the owner; Applications and the /link help for the owner and an Admin),
Admins.tsx (VIP & donations: supporters and the /link help), ShareControl.tsx (Join as moderator: the invite's server
and role and what that role can do, shown before it's used; the activity log's words for the roles routes).
Checked in a browser against mocked routes (claim card, owner code, switches, applications in every state, Create
moderator invite with "DMs closed" fallback, Revoke, supporter settings); the real routes aren't there yet.

**MCP, please match these (or tell me in P5.md and I'll change the UI):**
- Paths and methods as in "App API" above, with `:id` = the server's id as the page has it (for a shared-control Admin
  that's the shared entry's id, `PREFIX…`, sent through the usual X-Target: please map it to the owner's server there).
- `GET /api/roles/server/:id`: also `owner_code_expires` (unix s) when `owner_code` is there. Errors as `{error}`.
- `POST …/claim` answers `{ok, owner_code, expires}`; `POST …/owner-code` answers `{owner_code, expires}`.
- `POST …/settings` body is all three booleans; answer `{ok, settings}`.
- `POST …/applications/:aid/invite` and `…/revoke` bodies carry `{name: <discord_name>}` (only for the activity log's
  words); invite answers `{ok, dm, invite?}`.
- Applications: `answers` may be an object `{question: answer}` or `[{q, a}]`; `invite.state` "expired" on an approved
  one shows "Their last invite ran out unused".
- `GET /api/admins/supporters` → `{claimed, official, honor, tag, scoreboard, joinmsg, reserve, slots, error?}`;
  `POST` takes any subset (`honor` is the server's honor_supporters; `slots` 0-2, sent with `reserve: slots > 0`) and
  answers the same object. On an official server `honor` is ignored (always on). The gear/ability grid has no
  supporter column: supporters get the cosmetic defaults only.
- MCP policy (mcp/routes.rs, listed now; tests in `roles_policy_tests`): GETs read; claim, settings, unclaim,
  invite, revoke and POST supporters ask first; `owner-code` is the person's alone; `owner_code` values are hidden
  from agents (invites and SteamIDs already were).
