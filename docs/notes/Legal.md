# Legal: terms and privacy update (draft for review)

2026-11-13. Drafts of `site/terms.html` and `site/privacy.html` for the user and a lawyer to review.
Nothing goes live until the user uploads the site. None of this is legal advice.

**Before going live, two things must be fixed:**
1. **DMCA agent.** terms.html shows `[DMCA agent: to be registered with the US Copyright Office]`.
   Register one (steps below) and replace the placeholder.
2. **Application deletion.** privacy.html says role applications are deleted 180 days after a
   decision (or after sending, if never decided). **The code doesn't do this yet** (there's an HTML
   comment beside the line). `community.py` keeps `applications` rows forever. If adopted, add to the
   store's housekeeping (next to the 90-day `DELETE FROM reports`):
   `DELETE FROM applications WHERE (decided_at > 0 AND decided_at < now-180d) OR (decided_at = 0 AND at < now-180d)`,
   plus a test in `test_applications.py`. Keep in mind the 14-day cooldown and per-day limits read
   old rows; 180 days is well past both. Otherwise change the privacy line.

## What changed, and why

### terms.html
| Section | Change | Why |
|---|---|---|
| Intro | Covers the online services by name; says Nanoshrine is a California LLC. | Terms now govern Community, the server list and Discord, not just the site and app. |
| Your game and your servers | Ports/third-party bullet moved to "Security and your accounts". | Avoid saying it twice. |
| **Third-party and community content** (new) | Others make it; we don't make, host or endorse it; no promise of safety, legality, accuracy or freedom from malware; our checks described as limited; your risk; keep antivirus and backups. | The app installs mods, GameBanana files and community creations. The checks named (format, size and archive-bomb limits, no code files, Studio modes as graphs, GameBanana's virus flag) are the ones the code does (Caps.md, COMMUNITY.md section 4), and nothing more is claimed. |
| **Publishing and your content** (new) | Only what you have rights to; banned content list; you're responsible for credits/permission records (and the app's ask/no behavior is described); a non-exclusive license to list, index, display, link and distribute listing details; you keep your rights; GameBanana's terms govern the files; remixes must credit; credit isn't ownership. | Matches Assets.md (ask warns, no refuses unless "Permission granted" recorded) and the remix credit (`remixOf`). The license is limited to what we do: we never host the files. |
| **Moderation** (new) | Hide/remove/refuse at our discretion; we settle permanent-id disputes; block creators or reporters; no duty to monitor; reports and applications create no obligation. | Matches the list service: Pull/Keep/Block creator, uid conflicts settled by moderators. |
| **Copyright and takedowns** (new) | DMCA notice elements (17 U.S.C. 512(c)(3)), counter-notice (512(g)), repeat infringers, false-notice warning, agent placeholder with an interim path (Discord DM or GitHub issue), trademark/other complaints. Notes the files are on GameBanana, so also report there. | Safe harbor needs a designated agent and a repeat-infringer policy. Community is a link/index service, which fits 512(d) (information location tools). |
| **Security and your accounts** (new) | You keep your passwords, SSH keys, RCON passwords and Steam account safe; public servers and ports at your risk; no responsibility for third-party breaches/outages/data loss; private disclosure by Discord DM. | The app stores those credentials locally. Discord DM is the only existing private path (GitHub private vulnerability reporting is **off** on the repo; no security email exists). |
| **Running a server** (new) | Operators are responsible for players and conduct, rules, chat/voice-to-text logs and player lists, applicable privacy/consumer laws, and Valve's terms. Listing doesn't make us the operator. | Voice-to-text lines go to the server's own log; player lists stay with the host. |
| **Discord and community spaces** (new) | Discord's terms apply; our rules and role applications are covered by these terms. | |
| No warranty | Extended to online services, availability, third-party/community content, malware, security. | As asked. |
| Limitation of liability | Explicitly covers third-party/community content (malware), servers you run, security incidents and data loss. **Cap changed from "what you paid, or $0" to "the greater of what you paid in 12 months or US$50"**, plus a carve-out for what can't legally be limited (fraud, willful misconduct). | A $0 cap can read as no remedy at all, which a court may refuse to enforce (California Civil Code 1668 already bars exempting fraud and willful injury). A small nonzero floor is common and more defensible. Lawyer to confirm the amount. |
| **Indemnity** (new) | Users cover claims from their content, servers, misuse or breach, "to the extent the law allows". | |
| **Suspension and termination** (new) | For Community, the server list and Discord; the app and GPL rights unaffected; survival clause. | GPL rights can't be taken away by these terms. |
| **Age** (new) | 13+; under 18 needs a parent's or guardian's OK. | |
| **Governing law** (new) | California law; state or federal courts located in California; consumer rights where you live preserved. **No arbitration, no class-action waiver** (lawyer to decide). | |
| **General** (new) | GPL governs the code, terms govern site and services; severability; entire agreement (with the privacy page); no waiver; assignment; export laws. | **Force majeure was left out on purpose**: we promise no performance (free, as-is, no uptime), so there's nothing to excuse. |
| Contact | Adds Discord. | |
| Date | November 13, 2026 (was November 6). | |

### privacy.html
| Section | Change | Why |
|---|---|---|
| Date | November 13, 2026. It said November 14, 2026, a day that hasn't happened. | Use the date the page actually changes. |
| Intro | Covers online services; California LLC. | |
| The app | Server list: says what's sent (the address; name, map and player count are read from the server, counts only, no player names: `serverlist.py`). Passwords and SSH keys stay local; server logs are the operator's. | Accuracy. |
| Community | Adds the GameBanana member number (the index publishes `ownerId`) and the package's credits. | Accuracy. |
| **What our community server keeps** (new) | Creators (GameBanana id and name), credits, permanent ids and disputes, moderation history (incl. moderators' Discord id/name), reports (90 days), role applications (Discord id and name, role, why, links, join date, decision, decider) with a 180-day retention (**needs code**), daily backups kept 14 days. Then the Cloudflare Worker. | From the schema in `community.py` (tables `entries`, `log`, `blocked`, `cards`, `reports`, `applications`, `uids`, `uid_conflicts`) and `backup.py` (`KEEP_DAYS = 14`). |
| **Our Discord bot** (new) | What Discord sends when someone presses a button or sends a form: Discord id, name, roles, join date, the button and message, form text. Interaction ids kept one day; who acted is kept in moderation history; a deny may be DM'd. | `modbot.py`, `applications.py` (`joined_at`), the `interactions` table (one-day prune). |
| Your rights | Now for everyone (access, correction, deletion), not just payers; mentions identity checks and safety holds. | Community and Discord hold data about non-payers too. |
| **California privacy rights** (new) | CCPA categories, no sale or sharing, no sensitive PI, right to know/correct/delete, authorized agents, no discrimination, how to ask, 45 days. | Offered regardless of whether the law applies (see below). |
| **Children** (new) | Not directed at under-13s; we delete such data when we learn of it. | |

**About "no personal data goes through the Worker".** Not quite: the Worker
(`services/community-publisher`) copies the public index, which contains each creator's GameBanana
name and member id (`author`, `ownerId`) and credited authors' names. That's data already public on
GameBanana, but it is personal data in the GDPR/CCPA sense. The page says exactly that, and that
reports, applications, moderation records and browsing never go through it.

**Moderation history has no end date.** The `log`, `cards`, `blocked`, `uids` and `uid_conflicts`
tables are never pruned. The page says it's kept "as a record". A lawyer may want a period
(say, 3 years after a listing is gone); that would need code too.

## Does the CCPA apply?
Almost certainly not at this scale. It applies to a for-profit business that does business in
California and meets one of: annual gross revenue over $25 million (inflation-adjusted, $26.625
million from 2025); buys, sells or shares the personal information of 100,000 or more consumers or
households a year; or gets 50% or more of its revenue from selling or sharing personal information.
Nanoshrine has voluntary contributions, sells and shares nothing, and holds data on far fewer people.
The section is offered anyway: it costs nothing, matches what we do, and is good faith toward users.
If it ever did apply, the law wants an email address (enough for an online-only business) as a
request method; set up a privacy email then (or now).

## Open questions for the lawyer
1. **Arbitration and class-action waiver.** Left out. Worth it for a free, open-source project, or
   does it just add friction (and mass-arbitration risk)?
2. **Which county** for the courts? The draft says "state or federal courts located in California".
   Name the county of the LLC's principal office?
3. **Age.** Is 13+ with parent or guardian consent under 18 enough? Consider COPPA (under 13), the
   GDPR's age of digital consent (13 to 16 by EU country, for Discord ids and GameBanana names of EU
   users), California's AADC, and whether minors can be bound by the indemnity at all.
4. **GameBanana's terms.** Does listing, indexing and linking GameBanana pages, reading its public
   API (apiv11, no key) and showing its pictures (hotlinked from images.gamebanana.com) impose
   anything on us? Their ToS licenses uploads to GameBanana only; we never rehost files, but the
   index republishes names, summaries and credits. Do we need their permission or attribution?
5. **The liability cap.** Is "greater of 12 months' payments or US$50" right? Should the
   disclaimers be visually conspicuous? (The `caps` class on those paragraphs has no CSS, so they
   render as normal text; UCC 2-316 wants a disclaimer of merchantability to be conspicuous.)
6. **The license from creators**: is "for as long as it's listed and for our records after" fine,
   and does it need to survive deletion requests?
7. **Retention**: confirm 180 days for applications and pick one for moderation history.
8. **DMCA interim path**: is a Discord DM or GitHub issue acceptable until the agent is registered?
   (Safe harbor needs the registered agent; until then it isn't available.)

## Registering a DMCA designated agent
1. Go to the US Copyright Office's DMCA Designated Agent Directory: https://dmca.copyright.gov
   (linked from copyright.gov > "DMCA Designated Agent Directory").
2. Create a registration account for Nanoshrine Interactive LLC. It asks for a primary and a
   secondary contact (name, email, phone); these are for the Office and aren't published.
3. Start a new designation for the service provider: legal name "Nanoshrine Interactive LLC", its
   address, and every name the service is known by: **Loner-Strike**, **lonerstrike.com**,
   **list.lonerstrike.com**, **dl.lonerstrike.com** (list all alternate names, or notices sent under
   another name may fall outside the safe harbor).
4. Name the agent: a person, a job title (for example "Copyright Agent") or a third-party service,
   with a mailing address, phone number and email address. These **are published**. A P.O. box or a
   registered-agent or virtual-mailbox address keeps a home address private; check the form's
   current rules. A dedicated email such as copyright@lonerstrike.com is a good idea.
5. Pay the fee: **$6** per designation (it was $6 when last checked; check the page), by card.
6. Save the confirmation. The designation **expires after 3 years** unless renewed (or amended,
   which resets the clock). The system emails reminders; put a calendar reminder too. Update it
   whenever the address or email changes.
7. In `site/terms.html`, section "Copyright and takedowns", replace
   `<b>[DMCA agent: to be registered with the US Copyright Office]</b>` with the agent's name (or
   title), mailing address, phone number and email, exactly as registered. Then drop the "Until
   that's filled in…" sentence (keep the trademark line).

## Steps outside the website
- **Keep the LLC separate.** Sign everything (Polar, Cloudflare, Hetzner, GitHub org if any,
  domain) in the LLC's name; don't mix personal and LLC money; keep the California Statement of
  Information current and pay the annual franchise tax ($800); keep simple records of decisions.
  This is what keeps the liability limit of the LLC meaningful.
- **A business bank account** for the LLC. Polar payouts go there, and project costs come from it.
- **Insurance.** General liability plus technology errors and omissions (tech E&O, often bundled
  with cyber liability). Ask a broker for a small-software-publisher policy; tell them about the
  community index, the Discord and the hosted services.
- **A one-time lawyer review** of these two pages (and the questions above), ideally a California
  lawyer who works with software or games. Bring this file.
- Optional but cheap: turn on GitHub private vulnerability reporting for the repo, or set up a
  security email, and point the "Security" bullet in the terms at it.
