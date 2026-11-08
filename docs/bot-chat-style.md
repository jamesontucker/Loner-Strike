# Writing bot chat

How to write lines for the bots (plugins/configs/gamemodes/botchat*.ini) so they read like
people on a 2004-2007 Counter-Strike pub. The system itself is in docs/bot-people.md; check
your lines with `python3 scripts/botchat-lint.py`.

## What real pub chat was like

Measured in shooter chat logs of the time (BZFlag, CS studies) and what players remember:

- Tiny. 1.4 words a line on average. About 60% of lines are 1 to 3 words: "ns", "lol", "wtf",
  "gg", "nt", "rr", "owned". Full sentences are rare (under 15%).
- Mostly reactions to play (about 57%), then sorting out play (20%: "go b", "drop awp"),
  replies (10%), greetings (6%), technical moaning (6%: lag, reg, steam). Off-topic (school,
  work, WoW) about 1%, only when dead or between rounds.
- About half is emotional: happy, angry, mocking, sorry.
- Most of it happens dead, in freeze time and at round end. The dead commentate.
- Arguments are about cheating ("hax", "wallhack", "aimbot"), camping and AWPs. They pile
  up, then fizzle with a joke or a new round.
- Lowercase, no full stop, "u", "ur", "r", "y", "cuz", "dont", "im", "plz". Stretched letters
  (nooo, lolol), CAPS only when raging, "!!!" and "!!1". Emoticons :) :D :P xD ^^ -_- o_O.
- Typos from typing mid-fight, and lines cut off when the player dies ("nice sh").

## Era words (2003-2007)

Fine: gg, gl hf, ns, nt (2004 on), wp, gj, lol, lolz, rofl, lmao, omg, wtf, stfu, ffs, gtg,
brb, afk, cya, k, np, ty, thx, plz, owned, pwned, 0wned, pwnt, noob, n00b, newb, nub, hax,
h4x, haxor, wallhack, wh, aimbot, camper, awp whore, lagger, reg, choke, eco, ninja defuse,
c1 (= ok), zomg, teh, ftw, l33t (a few teens, half joking), "1.5 was better", "steam sucks"
(WON shut down July 2004, Steam was slow and fell over), "source beta" (Aug 2004),
"CZ bots", xfire, msn, vent, ts, IRC, wc3, WoW (2004 on), Halo 2, LAN, dial-up, 56k.

Don't: gg ez, ez as a taunt, rekt, rush b no stop, cyka blyat, huehue, kek, pog, yolo, sus,
ratio, noice, lit, u mad, epic fail, baited, based, cringe, simp, af, streamer talk, skins,
"ace" as the CS:GO term, "rtv" is fine but rarer before 2007.

Regions: Russians "blya", "suka", "davai", "bistro", "privet" (Latin letters, never Cyrillic);
Brazilians "kkkkk", "rsrs", "vlw" (thanks), "flw" (bye); Spanish "jajaja", "que lag"; Poles
"kurwa", "siema", "ja pierdole"; Swedes "fan", "hej"; Germans "scheisse", "hallo". Mostly
English with a word of their own when angry or happy. Mild non-native grammar, never mockery.

## How to write a line

- Write it plain and lowercase-ish. The bot adds its own style when it types: its laugh for
  "lol", text-speak, typos, emoticon, pet word, capitals. Don't bake those in.
- Short. If it's longer than 6 words, ask whether a real player would have typed it while
  dead with 3 seconds to spare.
- Not clever. Nobody on a pub is a comedy writer. A plain "lol" beats a punchline.
- Not uniformly nice or uniformly toxic. Mild, mixed, human. Insults are 2004-mild
  ("noob", "camper", "lol owned", "stfu"); nothing hateful, no slurs, nothing about race,
  sex, religion or disability.
- No facts that could contradict themselves later ("im 14", "i live in ohio"): keep them vague.
- Specific beats generic only when the condition makes it true: a line for `temper=tilty
  deaths>=3` can be bitter; a `*` line has to fit anyone.

## The format

    [section]
    conditions|text

Conditions, space separated (all must hold; more of them = more specific = chosen first):
- `*` anyone
- old personality letters (`ts` = tryhard or salty) and moods (`-` tilted/losing, `+` streak
  or winning, `^` just joined, `=` knows the person)
- traits: `warmth=cold|neutral|warm`, `ego=humble|normal|showoff`, `temper=chill|normal|tilty`,
  `talk=silent|quiet|normal|chatty|spammer`, `humor=none|dry|goofy|troll`,
  `age=kid|teen|adult|old`, `skill=newbie|average|good|pro`,
  `role=regular|drifter|admin|clan|afk`, `region=us|uk|de|se|pl|ru|br|fr|nl|es|au|fi|dk`,
  `style=rusher|roamer|lurker|careful|camper|awper|knifer|nader|ninja|objective|weirdgun`,
  `hobby=wc3|wow|halo|school|work|cars|music|lan|cz|source|awp|sleep|food|girlfriend|footy|metal`,
  `arch=<archetype name from people.ini>`; with >= <= != for the ordered ones
- what's going on: `kills>=2` (this round), `deaths>=3` (in a row), `frags`, `kd>=1.5`,
  `alive<=1` (its team), `diff<=-3` (score), `vs>=3` (it killed the other one this map),
  `vsby>=3` (the other one killed it), `joined<=120` (seconds), `phase=dead|freeze|live|end`,
  `other=human|bot|none`, `team=t|ct`, `time=night|morning|day|evening`, `map=dust2`, `flag=name`
- memory (once gm_people remembers): `rel>=2` / `rel<=-2` (how it feels about the other one,
  -5..5), `fam>=3` (sessions together), `days>=7` (since it last saw them), `rival=1`
- options: `!odds=30` (only that often, percent), `!cd=300` (not again for that many
  seconds), `!once=map`, `!set=name/120` (raise a flag for 120 s that `flag=name` can check)

Text: placeholders {name} {other} {map} {place} {weapon} {hs} {carrier} {us} {them} {diff}
{timeleft} {nextmap} {tag} {hobby}; inline choices {nice|ns|good one}; `@` at the start =
team chat; ` >tag` at the end asks the one it's about (or another bot) for a reply from
[r_tag]. Plain ASCII, no | outside {a|b}, under 100 characters (aim for under 40).
