# Review of the legal pages (terms.html, privacy.html)

2026-11-13. A fresh read of `site/terms.html` and `site/privacy.html` against the code that
backs them (`services/serverlist/community.py`, `applications.py`, `modbot.py`, `serverlist.py`,
`backup.py`, the publisher Worker), `site/docs.html`, `docs/COMMUNITY.md`, the app's UI and
`docs/notes/Legal.md`. Review only; nothing on the pages was changed.

**I'm not a lawyer and this isn't legal advice.** It's a careful engineer's read of the pages
against the code and against the California and US rules I know of. Everything marked "lawyer"
needs a real one; the rest I'm reasonably confident about.

**Counts:** 8 must fix before launch · 15 should fix · 9 nice to have · 10 questions for a lawyer.

**The short list.** (1) The privacy page leaves out things we actually do: our own public game
server (play.lonerstrike.com), the Hetzner box and where the data lives, the copies of moderation
data that sit in Discord forever, and the model downloads from Hugging Face. (2) The app never
shows or links the terms, so under California law they're browsewrap and probably don't bind
anyone; one line on the Setup screen fixes most of that. (3) The repeat-infringer policy the terms
promise isn't what the code does (a block is a review hold, not a refusal, and doesn't pull what's
already listed). (4) There's a leftover `DRAFT` comment in the live privacy page and `Legal.md`'s
two "must fix before live" items are both already done.

**Overall:** the pages are unusually honest and readable for legal text, the disclaimers are set
apart properly, and the retention statements match the code line for line where they exist. The
gaps are mostly things the pages don't mention rather than things they get wrong.

---

## Must fix before launch

### M1. Stale `DRAFT` comment in the live privacy page
- **Where:** `site/privacy.html` line 97 (`<!-- DRAFT: the 180-day deletion of applications isn't
  in the code yet… -->`), and `docs/notes/Legal.md` lines 6–15.
- **Problem:** `Store.prune_applications` (community.py 1493) and the daily `housekeeping` (1455)
  already delete applications 180 days after the decision or after sending. The comment is wrong,
  and it ships in the page source where anyone can read "this isn't implemented". Legal.md's two
  "must fix before live" items (DMCA agent, application deletion) are both done; the DMCA agent is
  registered (DMCA-1070465) and in the page.
- **Fix:** delete line 97. In Legal.md, strike items 1 and 2 and the "DMCA interim path" question
  (Q8), note the security email now exists, and note the county question (Q2) is answered (Orange).

### M2. Our own public game server isn't in the privacy page at all
- **Where:** `site/privacy.html`, nothing between "The Loner-Strike app" (line 60) and "Community"
  (77). `site/index.html` line 1191 (`OUR_SERVER = { address: "play.lonerstrike.com" … }`);
  `terms.html` line 131 mentions "our game servers".
- **Problem:** We run a game server people join. HLDS logs every player's IP address, Steam ID,
  name, connect and disconnect times, and (with `log on`) chat; if voice commands are on, team
  voice is turned into text and written to that server's log; `gm_community` keeps a player list and
  in-game `/report`s. That is personal data we collect directly, and the page says "the app sends
  nothing about you to us", which a player on our server would read as covering this. The page
  also does not say how long those logs are kept or who sees them. Under CalOPPA (Bus. & Prof.
  Code 22575) and the CCPA's notice-at-collection idea, a category we collect and don't disclose is
  the single most likely thing a complaint would point at.
- **Fix:** add a section after "The Loner-Strike app". Fill in the real retention from the box:

  > ## Our game server
  > We run a public Counter-Strike server (play.lonerstrike.com). Like any game server it sees
  > your IP address, Steam ID and player name while you play, and its log records when you joined
  > and left, what you said in chat and, if voice commands are on, the lines of team voice it
  > turned into text (never the audio). We keep those logs for [N] days, read them only to deal
  > with cheating, abuse and reports, and don't share them. Players can be banned by Steam ID,
  > and a ban is kept for as long as it's in place. The website asks this server what map is
  > on and how many people are playing; that's all it shows.

  And add "game server logs (IP address, Steam ID, player name, chat)" to the CCPA categories in
  line 111.

### M3. Hetzner and the location of the data are missing; "Hosting" only covers the website
- **Where:** `privacy.html` line 49 (Hosting: Cloudflare Pages), line 80 ("nothing about you is
  sent"), line 88 ("runs on our own server"). `docs/COMMUNITY-SERVER.md` line 9 (index on R2);
  `serverlist.py` line 312 reads `CF-Connecting-IP`, so list.lonerstrike.com is behind Cloudflare;
  Legal.md line 118 names Hetzner.
- **Problem:** The community database, backups, reports, applications and moderation history live
  on a rented Hetzner machine (Hetzner is a processor and the data is most likely in Germany or
  Finland, so EU users' data is in the EU and Californians' data leaves the US; say which).
  dl.lonerstrike.com is Cloudflare R2 and list.lonerstrike.com is proxied by Cloudflare, so
  Cloudflare sees every app request's IP address, not only website visits. "Nothing about you is
  sent" is true of the request body, but our servers and Cloudflare do see the IP address, and the
  page should say so once, plainly, rather than let a reader discover it.
- **Fix:** in "This website" line 49 and a new sentence under "The Loner-Strike app" (after line
  72):

  > Whenever the app talks to our servers (dl.lonerstrike.com, list.lonerstrike.com,
  > status.lonerstrike.com), those servers and Cloudflare, which sits in front of them, see your
  > IP address for as long as it takes to answer, like any website. We don't store it (the server
  > list and Community count requests per address in memory for up to an hour, then forget them).

  And in "What our community server keeps" (line 88):

  > The Community and server list service runs on one server we rent from Hetzner in [Germany /
  > Finland]. See <a href="https://www.hetzner.com/legal/privacy-policy">Hetzner's privacy
  > policy</a>. Its database and backups stay on that machine.

  Also list Hetzner (and wherever analytics.nanoshrine.com runs, if that's different) wherever
  processors are named.

### M4. Copies of moderation data in Discord aren't covered by any retention statement
- **Where:** `privacy.html` lines 93–96 (2 years / 180 days / 90 days); code: `notify_report`
  (community.py 2055) posts the report reason, note and `Reporter #tag` to the mod channel;
  `applications.py` posts the application card (why, links, join date, account age) and audit
  lines ("Application #n denied by @actor"); `modbot.py` edits cards but nothing deletes them.
- **Problem:** Every report, application and moderation action is also a Discord message in a
  private channel, and Discord keeps messages until someone deletes them. So "we delete an
  application 180 days after it's decided" and "we keep reports for 90 days" are true of our
  database and false of the full record. A deletion request under "Your rights" would also need
  someone to go find and delete those messages by hand.
- **Fix (pick one):** (a) have `housekeeping` delete the Discord messages it prunes (cards keep
  `msg_id`; the bot has the token), or (b) say what happens. Text for (b), after line 96:

  > <li><b>Copies in our Discord.</b> Reports, applications and moderation actions are also
  > posted as messages in our private moderation channels on Discord, where only moderators can
  > see them. Those messages stay there under Discord's own retention until we delete them,
  > which we do when you ask us to delete your data.</li>

### M5. Model downloads from Hugging Face and GitHub aren't disclosed
- **Where:** `privacy.html` lines 61–72 (the list of what the app goes online for). Code:
  `app/src-tauri/src/voice.rs` line 40 (whisper model from huggingface.co), line 46 (intent model
  from dl.lonerstrike.com); `smartchat/catalog.rs` lines 48–160 (Llama and other chat models from
  huggingface.co, llama.cpp binaries from github.com).
- **Problem:** The list says "download the speech models for voice commands" without saying from
  where, and doesn't mention Smarter chat at all, which downloads language models from Hugging
  Face and a runtime from GitHub. Hugging Face sees the downloader's IP address and is a third
  party not named anywhere. The page's "it only goes online to:" list is presented as complete.
- **Fix:** replace line 67 with:

  > <li>download the speech model for voice commands from Hugging Face, and our own small
  > command model from dl.lonerstrike.com, when you turn voice commands on;</li>
  > <li>download the chat models and the llama.cpp runtime for Smarter chat from Hugging Face
  > and GitHub, if you turn that on. The models then run on your computer; no chat is sent
  > anywhere;</li>

  Line 63: "check GitHub for updates" is fine, but add "(GitHub sees that request)". Add Hugging
  Face to the processors wherever they're listed.

### M6. The app never shows or links the terms, so they're browsewrap
- **Where:** `app/ui/src/views/Setup.tsx` lines 151–165 (the first-run welcome pane links the
  docs only); `AppSettings.tsx` lines 495–537 (About: docs, issues, GitHub, no terms or privacy);
  `tools/community/Credits.tsx` and the Publish flow (no acknowledgement); `site/index.html`
  line 1154 (footer links only; the download button has no notice). No file in `app/ui/src`
  references `terms.html` or `privacy.html`.
- **Problem:** Under California and Ninth Circuit law a link in a footer that the user never has
  to see doesn't form a contract (Nguyen v. Barnes & Noble, 9th Cir. 2014). Terms only in the
  footer bind no one who installs the app, publishes to Community, reports, or runs a server. The
  GPL's own warranty disclaimer (sections 15–16) still protects the code, but the indemnity, the
  liability cap, the content rules, the creator license and the governing-law clause all depend on
  assent. The fix the courts accept is a conspicuous notice at the point of action with a button
  that manifests assent (Berman v. Freedom Financial, 9th Cir. 2022), not a modal wall.
- **Fix (code, not page text):**
  1. Setup welcome pane, under the "Set it up" button (Setup.tsx ~line 160): a muted line
     "By setting up Loner-Strike you agree to the <a>terms</a> and have read the <a>privacy
     page</a>." The button that follows is the assent. Show it again when Setup is re-run from
     Settings (cheap, and the first-run record may be gone).
  2. Publish, on the step that registers the page with the list (the "Paste the link" step in
     COMMUNITY.md section 6): a checkbox "I have the right to share this and I accept the
     <a>Community terms</a>", required before Register. This is where the creator license and the
     permission records matter most.
  3. About in Settings: add Terms and Privacy links next to Docs.
  4. The site's download button: "By downloading you agree to the terms." next to it, and a
     line in the GitHub release notes/README pointing at the terms.
  5. Keep a record: store the terms date the user accepted in the app's settings (one field), so
     a later dispute has a date.
  Also change `terms.html` line 46 from "By using them you agree to these terms" to "By
  downloading or using them, or by pressing a button that says you agree, you accept these
  terms" so the text matches the mechanism.

### M7. No Do Not Track statement (CalOPPA)
- **Where:** `privacy.html`, "This website" (lines 46–55).
- **Problem:** CalOPPA (Bus. & Prof. Code 22575(b)(5)–(6)) applies to any commercial website that
  collects personal information from California residents, with no revenue threshold, unlike the
  CCPA. It requires the policy to say how the site responds to browser Do Not Track signals and
  whether third parties collect personal information across sites. Cheap to add, and Legal.md's
  "the CCPA probably doesn't apply" reasoning doesn't cover CalOPPA.
- **Fix:** add a bullet after line 54:

  > <li><b>Do Not Track.</b> We don't track you across sites and no third party does so through
  > this site, so a "Do Not Track" signal from your browser changes nothing: you're already not
  > tracked.</li>

### M8. The repeat-infringer policy isn't what the code does
- **Where:** `terms.html` line 99 ("We block creators who repeatedly infringe") and line 80 ("We
  may block a creator from listing at once"). Code: `Community.block` (community.py 2076) only
  sets `blocked`; a blocked creator's submissions become `pending` and wait for a moderator
  (1902–1912), who can approve them. Nothing pulls the creator's existing listings, and nothing
  counts copyright notices per creator.
- **Problem:** DMCA safe harbor (17 U.S.C. 512(i)(1)(A)) needs a policy that terminates repeat
  infringers "in appropriate circumstances", that users are told about, and that is reasonably
  implemented. A review hold that a moderator may approve is not termination, "at once" doesn't
  describe it, and without a tally of notices nobody can show the policy was applied. Courts have
  denied safe harbor on exactly this (not tracking notices, not acting on them).
- **Fix:**
  1. Code: an `lsmod.py block --pull` (or a `ban` flag) that also pulls every listed entry by that
     `submitter_id` and makes future submissions refuse (409) rather than wait. Record copyright
     notices as log lines keyed `member:<id>` with action `dmca` so `lsmod.py` can show a count.
  2. Internal rule, written down (one paragraph in COMMUNITY.md or the runbook): e.g. "Two valid
     copyright notices against the same creator within 12 months, not answered by a counter-notice
     we restored on, and the creator is blocked and all their listings pulled."
  3. Terms, line 99:

     > <b>Repeat infringers.</b> If we get repeated valid copyright notices about the same
     > creator, we pull their listings and stop listing anything from them, in appropriate
     > circumstances. We keep count of the notices we receive.

  4. Terms, line 80: "We may put a creator's listings on hold for review, or stop listing them."

---

## Should fix

### S1. Limitation carve-out is narrower than California law requires
- **Where:** `terms.html` line 141, "such as for fraud or willful misconduct".
- **Problem:** Civil Code 1668 voids any term exempting "fraud, or willful injury to the person or
  property of another, or violation of law, whether willful or negligent", and City of Santa
  Barbara v. Superior Court (2007) adds gross negligence. A carve-out that names fewer things than
  the statute invites a reader (or judge) to say the drafter tried to exclude them. Naming them all
  costs nothing and makes the rest more likely to survive.
- **Fix:** "Nothing in these terms limits liability that the law doesn't allow to be limited, such
  as for fraud, gross negligence, willful misconduct, violation of law, or death or personal
  injury caused by our negligence."

### S2. The cap's wording: who was paid, and per claim or in total
- **Where:** `terms.html` line 141, "Our total liability for any claim is limited to the greater
  of the amount you paid us in the 12 months before it or US$50."
- **Problem:** With Polar as merchant of record the user pays Polar, not us, so "paid us" is
  contestable. "For any claim" can be read as US$50 per claim; the usual intent is all claims
  together.
- **Fix:** "Our total liability for all claims together is limited to the greater of what you
  contributed through Polar in the 12 months before the first claim, or US$50."

### S3. "Entire agreement" shouldn't pull the privacy page into the contract
- **Where:** `terms.html` line 162.
- **Problem:** Making the privacy page a contract term turns every retention statement in it into a
  promise whose breach is a contract claim, on top of the statutory exposure. Most policies are
  deliberately kept as notices, not terms.
- **Fix:** "These terms are the whole agreement between you and us about the website and online
  services. Our privacy page explains what we do with information about you; it isn't part of
  this agreement."

### S4. "Paying" calls it "not a purchase" while Polar sells it as a product with sales tax
- **Where:** `terms.html` line 125; `privacy.html` line 58 ("handles sales tax"); `index.html`
  lines 1040–1044 ("Name your price… pay what you like").
- **Problem:** Polar's merchant-of-record model is built around digital products (my recollection
  is that Polar dropped its standalone donations feature in 2025 and requires a product or
  service to be delivered; check their current acceptable-use terms). If the Polar product is
  "Loner-Strike, name your price", then it is a purchase of a download in Polar's eyes and the
  terms say the opposite. Consumer-law remedies (CLRA, Song-Beverly) attach to sales, and the
  mismatch is what a plaintiff would use. The fix is to make the three places say the same thing.
- **Fix:** decide one framing with Polar's rules in view. If it must be a product: "Name your
  price is a pay-what-you-want purchase of the same download everyone gets for free; it adds no
  features, support or promises. Polar is the seller of record, so Polar's terms and refund policy
  govern the payment itself." If Polar allows contributions: keep the current text but make the
  Polar product's name and description say "contribution, no benefits" too.

### S5. "Moderators never see it" and the report's app version
- **Where:** `privacy.html` line 82; `docs.html` line ~1694 (same sentence). Code: `notify_report`
  posts `Reporter #<tag>` to the mod channel; `add_report` stores the app version.
- **Problem:** Moderators do see the scrambled tag (that's its purpose); only the address is
  hidden. "It" is ambiguous. The stored app version isn't mentioned.
- **Fix:** "A report sends the page, the reason, your note and the app's version. We keep them for
  90 days. To spot repeated reports from one person, the server keeps a scrambled tag of your IP
  address, made with a key that only lives in memory and changes when the server restarts;
  moderators see the tag, never the address, and the address itself is never stored." Same change
  in docs.html.

### S6. "Stop accepting reports from someone who misuses them" has no mechanism
- **Where:** `terms.html` line 80. Code: reports are only rate-limited (`report_ip`, `report_day`);
  the reporter tag's salt resets on every restart, so a reporter can't be blocked durably.
- **Fix:** either implement a reporter block (would need a durable tag, which changes the privacy
  story) or soften: "or ignore reports from someone who misuses them."

### S7. Tell creators about takedowns, and keep legal notices longer than 90 days
- **Where:** `terms.html` lines 85–99; `privacy.html` "What our community server keeps".
- **Problem:** 512(g)(2)(A) asks the provider to take reasonable steps to notify the user whose
  material was removed. The app does show the reason on the Yours page (`GET
  /community/submission`, community.py 2015), but the terms don't say so, and a creator who never
  opens the app never learns. Separately, copyright notices arrive by email and the privacy page's
  only retention for anything report-like is 90 days; the notices and counter-notices are exactly
  what proves the policy was followed and must be kept for years.
- **Fix:** terms, after line 86: "When we remove a listing on a notice, the app shows the creator
  that it was pulled and why, on their Yours page, and we email them if we have an address."
  Privacy, after line 96: "<li><b>Legal notices.</b> Copyright notices, counter-notices and our
  replies are kept by email for as long as we may need to show how we handled them (at least
  three years).</li>"

### S8. CCPA categories are incomplete
- **Where:** `privacy.html` line 111.
- **Problem:** The page collects, in the law's categories, also "internet or other electronic
  network activity" (which pages were visited, in aggregate, via Plausible; the scrambled report
  tag; the in-memory rate counters), "geolocation" at country level (Plausible), and after M2,
  game server logs. It also doesn't name the sources per category, which the regulation asks for;
  "described above" is a fair pointer but a one-line source list is safer.
- **Fix:** extend the bullet: "…internet activity (which pages of this site were viewed, in
  totals only; the scrambled tag on a report); coarse location (the country a visit came from);
  and, if you play on our game server, its logs (IP address, Steam ID, name, chat). Sources: you,
  GameBanana's public pages, Discord, Polar and our own servers."

### S9. A short GDPR section is worth having
- **Where:** `privacy.html`, "Your rights" line 106 only says "further rights".
- **Problem:** Whether the GDPR reaches a US LLC with no EU establishment is a lawyer question
  (L5), but the data for EU users of Community and the Discord is stored on a Hetzner box that is
  probably in the EU, the Discord has EU members, and the cost of a six-line section is nothing.
  What's needed is the controller, the legal bases, where data is stored, and the rights.
- **Fix:** add after "Your rights":

  > ## If you're in the EU or UK
  > Nanoshrine Interactive LLC is the controller. We process the data above to run Community,
  > the server list, our Discord and our game server and to keep them safe (our legitimate
  > interests; for role applications, because you asked us to), and to handle payments (contract,
  > through Polar). It's stored on a server in [Germany/Finland] and in the US (Cloudflare,
  > Discord, Polar and GitHub operate worldwide under their own safeguards). You have the right
  > to access, correct, delete, restrict or object, and to complain to your data protection
  > authority. We don't have an EU representative or a data protection officer; email
  > legal@nanoshrineinteractive.com.

### S10. "They agree to these terms for you" won't bind a parent who never saw them
- **Where:** `terms.html` line 150.
- **Fix:** "If you're under 18, use our online services only with a parent's or guardian's
  permission, and have them read these terms." (Whether that helps at all is lawyer question L2.)

### S11. "Don't use it to cheat" reads as a use restriction on GPL code
- **Where:** `terms.html` line 55.
- **Problem:** GPLv3 section 10 forbids further restrictions on the licensed rights; line 160
  already says the GPL wins, so it's harmless, but it's cleaner as a conduct rule tied to the
  services we can actually withdraw.
- **Fix:** "If you use it to cheat on other people's servers, that's on you, and we may bar you
  from Community, the server list and our Discord."

### S12. Changes clause: give notice for material changes
- **Where:** `terms.html` line 169.
- **Problem:** Silent changes to terms aren't binding on people who don't see them (Douglas v. US
  District Court, 9th Cir. 2007). One sentence turns it into something defensible.
- **Fix:** "We may update these terms. The date above shows when they last changed. For a change
  that matters we'll say so in the app's release notes and on our Discord; using Loner-Strike or
  our online services after that means you accept the new terms."

### S13. California Civil Code 1789.3 consumer notice
- **Where:** `terms.html`, "Contact" (line 171).
- **Problem:** California requires providers of online services to California consumers, where
  there's a fee, to give the provider's address and the Department of Consumer Affairs' complaint
  contact. Payments are voluntary, so applicability is arguable (L4), but the notice is three
  lines and widely included.
- **Fix:** add: "<p><b>California residents.</b> Under Civil Code 1789.3, you may reach the
  Complaint Assistance Unit of the Division of Consumer Services of the California Department of
  Consumer Affairs at 1625 North Market Blvd., Suite N 112, Sacramento, CA 95834, or (800)
  952-5210. Our address is above, under Copyright and takedowns.</p>"

### S14. The server list keeps listed addresses; say for how long
- **Where:** `privacy.html` line 69. Code: `serverlist.py` 209–218 saves `self.servers` to disk;
  entries are dropped when a server stops answering (`FRESH_SECS = 600`, lines 230–249); the
  registering client's IP is counted for an hour (280).
- **Fix:** "…tell it about a server you choose to list there (its address; the list then asks
  the server itself for its name, map and player count, and drops it once it stops answering for
  ten minutes). The address of the computer that listed it is counted for an hour, in memory, to
  stop floods, and not kept."

### S15. docs.html's Community privacy section drifted from the privacy page
- **Where:** `site/docs.html` lines 1685–1698: "The short version: nothing about you"; Publishing
  lists "the GameBanana name of who made it" (the privacy page adds the member number and the
  package's credits); the reports sentence (S5).
- **Fix:** align the three sentences with privacy.html, and change the short version to "nothing
  about you beyond what you publish, and your IP address only while a request is answered".

---

## Nice to have

### N1. Indemnity: add the procedure
- `terms.html` line 144. Add: "We'll tell you promptly about any such claim and let you run its
  defense with a lawyer we accept, as long as you don't settle anything that binds us without our
  OK." Makes it more likely to be enforced as written and reads as fair.

### N2. Feedback clause
- Suggestions arrive on GitHub and Discord. Add under "Publishing and your content": "If you send
  us ideas or feedback, we may use them without owing you anything."

### N3. Standard technical measures and 512(d)
- DMCA safe harbor also requires accommodating "standard technical measures" (512(i)(1)(B)). One
  sentence under "Copyright and takedowns": "We accommodate standard technical measures that
  copyright owners use to identify their works." Also consider naming the Community index for
  what it is ("a list of links; we don't host the files") in that section, which is already said
  at line 86; fine as is.

### N4. Processor list in one place
- The privacy page names Cloudflare, Polar, jsDelivr, Discord, GameBanana, GitHub, Plausible and
  the radio stations, scattered through the text. After M3 and M5 add Hetzner and Hugging Face. A
  short "Who else sees data" list at the end, one line each with a link, would make the page
  easier to audit against reality.

### N5. The cookie sentence
- `privacy.html` line 44 "we don't use cookies". The site sets none, but Polar's checkout (loaded
  from jsDelivr, opened over the page) runs Polar's code, which may set its own. "This site sets
  no cookies. When you open Polar's checkout, Polar's code runs under Polar's policy." Also line 48
  "stores no personal data" about Plausible: say "stores no IP addresses or identifiers" (what
  Plausible actually promises); "no personal data" is a legal conclusion.

### N6. Llama license attribution
- Smarter chat downloads Llama 3.2 models whose community license wants "Built with Llama" where
  the model is used and the license passed along. Not a terms matter, but add it to CREDITS.md and
  the Smarter chat panel's credit line (the catalog has a `credit` field; check it says it).

### N7. Security disclosure: promise not to sue researchers
- `terms.html` line 107. "We won't take legal action against anyone who reports a problem in good
  faith and gives us a fair chance to fix it." Cheap, and it's what researchers look for. Also turn
  on GitHub private vulnerability reporting (Legal.md line 128 still open).

### N8. Readability nits
- `terms.html` line 46: "They're offered by" → "They're from". Line 72: the creator license
  sentence is long; split at "for Community". Line 98 (counter-notice) is one 90-word sentence;
  a bulleted list like the notice above it. Line 141: the limitation paragraph is dense; a line
  break before "Our total liability". Class name `.caps` is fine (it's CSS), but the heading
  "No warranty" could be "No warranty (please read)" to nudge the eye.
- `privacy.html` line 44 "Short version: we don't use cookies, we don't track you, and we don't
  sell anything about you." Keep; it's the best line on either page.

### N9. Font and trademark housekeeping
- The "Counter-Strike" display font (fonts/counter-strike.ttf) is a dafont fan font credited in
  CREDITS.md line 117 as "100% free", which is not a license. Low risk, but if the Valve
  non-affiliation line (terms line 152) ever matters, a logo-style font is the kind of thing
  that gets pointed at. Consider Big Shoulders Stencil (already a fallback) for the site.

---

## Questions only a lawyer can settle

- **L1. Arbitration and class-action waiver.** Left out on purpose (Legal.md Q1). For a free
  project with a US$50 cap the main value would be the class waiver; the main cost is
  mass-arbitration fee exposure and the friction. A lawyer should weigh it; I'd lean no.
- **L2. Minors.** Family Code 6710 lets a minor disaffirm a contract, which would take the
  indemnity and the cap with it. Is 13+ with parental permission the right line for an M-rated
  game's companion app, or should online services (Community publishing, applications) be 18+?
  Also the GDPR age of consent (13–16 by country) for Discord ids of EU users.
- **L3. Does Song-Beverly or the CLRA attach** when the software is free and the only money is a
  pay-what-you-want payment to Polar as merchant of record? If yes, "as is" needs Song-Beverly's
  1792.4 phrasing ("…the entire risk as to the quality and performance is with the buyer") and the
  limitation must not touch implied-warranty rights.
- **L4. Civil Code 1789.3** (S13): required, or only prudent, given voluntary payments.
- **L5. GDPR reach** (Art. 3(2)): does an English-language free app and a Discord with EU members
  count as "offering services" to the EU? If yes, is the Art. 27 representative exemption
  (occasional, low-risk processing) available? Where exactly is the Hetzner box?
- **L6. 512(c) or 512(d).** Community is a link index (512(d)); the counter-notice and put-back
  machinery is written for 512(c) storage. Keeping it is harmless and generous, but the lawyer
  should confirm the agent registration lists every alternate name (Loner-Strike, lonerstrike.com,
  list./dl.lonerstrike.com) and that a 512(d) provider gets the same benefit from it. Also whether
  Section 230 covers the moderation discretion in "Moderation" (it should; worth one sentence
  from someone who can say so).
- **L7. The cap amount** (US$50 or 12 months' payments) and whether a floor is needed at all for a
  free product; and whether the bold-on-panel presentation satisfies "conspicuous" under UCC
  1-201(b)(10) (contrasting type or colour qualifies; capitals aren't required; I think it does).
- **L8. The creator license** ("for as long as it's listed and for our records after"): does it
  need to survive a deletion request, and does it need an explicit right to keep the listing's
  facts in moderation history for the 2-year retention the privacy page promises?
- **L9. GameBanana's terms** (Legal.md Q4): reading apiv11 without a key, hotlinking
  images.gamebanana.com, republishing names, summaries and credits in a public index. Permission
  or attribution needed?
- **L10. Polar** (S4): what Polar's current acceptable-use and merchant terms require of us
  (product vs donation, refund policy, who the consumer's contract is with), and whether our
  "not refundable" line can stand next to Polar's own refund rules and EU withdrawal rights.

---

## What checks out (so nobody re-audits it)

- Reports 90 days: `add_report` and `housekeeping` (`REPORT_DAYS = 90`). ✓
- Applications 180 days after decision or sending: `prune_applications`. ✓
- Moderation history, cards, uid disputes 2 years; block history kept while blocked, then 2 years
  from its last line: `housekeeping` (`HISTORY_DAYS = 730`). ✓
- Delisted entries deleted 2 years after their last change, with their uid rows. ✓
- Interaction ids 1 day. ✓ Backups 14 days on the same box: `backup.py KEEP_DAYS = 14`. ✓
- Housekeeping runs with the re-check, every 24 h (`--recheck-hours 24`). ✓
- Submit body is page + file id + app version only (`POST /community/submit`). ✓
- Reporter tag: HMAC of the visitor key with an in-memory salt. ✓
- Publisher Worker every 10 min copies the public index only (`wrangler.toml crons`). ✓
- Package checks named in the terms (format, size, archive bombs, no code files, GameBanana's AV
  flag) match `read_package`, `CODE_EXT`, `_sAvResult`. ✓
- DMCA notice elements match 512(c)(3)(A); counter-notice elements match 512(g)(3); 10–14
  business days put-back matches 512(g)(2)(C); agent registration number present. ✓
- Takedown and put-back exist in code (`moderate` pull/hide/restore) and reach the app via the
  index's `revoked` list within minutes. ✓
- Age line, non-affiliation, governing law (Orange County), severability, no-waiver, assignment,
  export: present and ordinary. ✓
- Disclaimer presentation: bold, brighter text, panel with a coloured edge; distinct from body
  text, which is what "conspicuous" means. ✓
