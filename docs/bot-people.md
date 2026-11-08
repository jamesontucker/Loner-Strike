# Bot people: who the bots are and how they talk

The goal: a 2004-2007 public server full of individual people. Each bot is someone, the same
someone every time it shows up, who plays and talks the way that person would, remembers you,
and has history with the other regulars. Most of the time, like real players, it says nothing.

This replaces the single personality letter per bot (gm_botchat's PERSONAS) with traits,
archetypes and rules, in stages that each keep everything working.

## Plugins

| Plugin | Job |
|---|---|
| gm_people | Who everyone is: traits, archetype, typing style, relationships and memory. Natives for the rest. |
| gm_botchat | What gets said: the rules over lines, the director (who speaks, when), typing it out, Smarter chat. |
| gm_humanbots, gm_botfun, gm_botping | How they play, fidget and connect: read traits from gm_people. |

During the move every archetype also maps to the nearest old personality letter, so code
that still asks `botchat_persona()` keeps working.

## Traits

Ten axes, small integers, from a stable seed (the bot's name; a regular keeps them):

| Axis | Values |
|---|---|
| warmth | 0 cold, 1 neutral, 2 warm |
| ego | 0 humble, 1 normal, 2 show-off |
| temper | 0 chill, 1 normal, 2 tilty |
| talk | 0 silent, 1 quiet, 2 normal, 3 chatty, 4 spammer |
| humor | 0 none, 1 dry, 2 goofy, 3 troll |
| age | 0 kid, 1 teen, 2 adult, 3 old-timer |
| skill | 0 newbie, 1 average, 2 good, 3 ex-clan |
| region | us, uk, de, se, pl, ru, br, fr, nl, es, au, fi, dk |
| role | 0 regular, 1 drifter, 2 admin-wannabe, 3 clan member, 4 afk-er |
| hobby | two of: wc3, wow, halo, school, work, cars, music, lan, cz, source, awp, sleep, food, girlfriend, footy, metal |

Plus a playstyle (rusher, roamer, lurker, careful, camper, awper, knifer, nader, ninja,
objective, weirdgun) that gm_humanbots and the bot profiles act on.

Coherence: archetypes fix some axes; `[conflicts]` in people.ini rules out combinations
(kid + old-timer age, silent + spammer...); correlations nudge weights (show-off leans troll,
old-timer leans dry). Most values sit in the middle: only the extremes show.

## Archetypes

About 120 presets in configs/gamemodes/people.ini ("rager kid", "polite swede", "bitter
vet", "lurker sniper", "wannabe admin", "wow addict"...). Each has a weight, the axes it
fixes, a playstyle and the old letter it maps to. The rest of the axes are rolled.

## Typing style

Rolled separately and applied after a line is picked, so one written line comes out many
ways: capitals (lower / proper / CAPS when raging), abbreviations (u, ur, r, 2, plz),
leet (rare), laugh, emoticon, pet word, typo rate, punctuation habit (... !!! ?), typing
speed, the stray chat-key letter at the start ("yok", "ugo b"), "c1" for ok, a line cut off
when it dies mid-typing. Region adds a native word now and then (suka, kurwa, kkkk, vlw,
jajaja, fan, scheisse) and mild ESL grammar.

## Lines and rules

Lines stay in configs/gamemodes/botchat.ini, `[section]` then `conditions|text`. The
conditions are space-separated:

- `*` anyone; old personality letters (`ts`) still work: "has one of these"
- `axis=value`, `axis>=n`, `axis<=n` (`ego=2`, `temper>=1`, `age=kid`, `region=br`)
- `arch=awper`, `style=rusher`, `hobby=wow`
- facts about now: `kd>=1.5`, `deaths>=3` (in a row), `kills>=3`, `phase=dead|freeze|live|end`,
  `alive<=1`, `human` (the other one is a person), `latenight`
- the relationship with the other one: `rel>=2`, `rel<=-2`, `fam>=3` (sessions together),
  `days>=7` (since last seen), `owns` (kills it more than it dies to it), `owned`
- options after `!`: `!odds=40` (only that often), `!cd=60` (not again for a minute),
  `!once=map`, `!once=session`, `!set=key/ttl`, `!then=victim:section`
- mood marks (- + ^ =) as before

The most specific line that fits wins: lines are scored by how many conditions they meet;
the top score is picked from (one tier lower now and then, so it's not always the same).
`*` lines are the fallback, so general lines come first and specific ones are added on top.

Placeholders as before ({name} {other} {map} {place} {weapon} {us} {them}...) plus inline
choices `{a|b|c}` and {hobby}.

No repeats: the existing per-bot and server-wide memory of recent lines and themes stays.

## The director

Silence is the default. When something happens, the bots who might react make an offer
(the best line they have, how much they talk, how involved they are: killer, victim, the
one named, else a bystander) and the director picks one, now and then a second, almost
never a third. Budgets per phase: little mid-fight, more in freeze time, at round end and
among the dead. Replies come after reading and typing time; a thread ends on its own if
the moment has passed.

## Memory

Per bot and person (nVault): affinity (-5..5), familiarity (sessions together), last seen,
kills on and deaths to, a nickname, up to three memories ("knifed me on italy"), an inside
joke. Per pair of bots: affinity and rivalry. Affinity drifts back toward 0 over weeks;
memories fade. All of it is a condition lines can use.

## Stages

1. Rules over lines and the director, with the existing lines unchanged.
2. gm_people: traits, archetypes, typing style; the old letters map from archetypes.
3. Memory of people and between regulars.
4. Lines: about 1,700 to start, written per trait and situation, then more over time.
