#!/usr/bin/env python3
"""Simulates bot chat on a full pub (docs/bot-people.md): who the bots are (people.ini, as
gm_people works it out), what they say (every botchat*.ini, conditions as gm_botchat reads
them) and when (the director, gaps, the rate cap, replies), over whole maps and sessions,
and measures it against what real pubs were like (docs/bot-chat-style.md).

    python3 scripts/botchat-sim.py [--maps 30] [--bots 20] [--personality 100] [--seed 1]

A model of the plugins, not the plugins: event chances and timings are copied from
gm_botchat.sma; game events (kills, rounds) are generated. Good for numbers and tuning.
"""

import argparse
import collections
import glob
import math
import os
import random
import re
import statistics as st

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CFG = os.path.join(ROOT, "plugins", "configs", "gamemodes")
SMA = os.path.join(ROOT, "plugins", "scripting", "gm_botchat.sma")

TRAITS = ["warmth", "ego", "temper", "talk", "humor", "age", "skill", "role", "region", "style"]
TRAIT_VALUES = {
    "warmth": "cold neutral warm", "ego": "humble normal showoff", "temper": "chill normal tilty",
    "talk": "silent quiet normal chatty spammer", "humor": "none dry goofy troll", "age": "kid teen adult old",
    "skill": "newbie average good pro", "role": "regular drifter admin clan afk",
    "region": "us uk de se pl ru br fr nl es au fi dk",
    "style": "rusher roamer lurker careful camper awper knifer nader ninja objective weirdgun",
}
HOBBIES = "wc3 wow halo school work cars music lan cz source awp sleep food girlfriend footy metal".split()
TALK = [0.12, 0.5, 1.0, 1.6, 2.4]
BIG = {"clutch", "ace", "teamkilled", "teamkill", "nemesis", "nemesis_won", "rage", "last_alive", "low_hp",
       "bomb_planted", "defuse_ninja"}


def u32(x):
    return x & 0xFFFFFFFF


def s32(x):
    x &= 0xFFFFFFFF
    return x - (1 << 32) if x & 0x80000000 else x


# ---- people.ini, as gm_people reads it -----------------------------------------------------

def value_index(lst, v):
    if v[:1].isdigit():
        return int(v)
    vals = lst.split()
    return vals.index(v) if v in vals else -1


def load_people():
    arch, conflicts, weights = [], [], {}
    section = None
    for line in open(os.path.join(CFG, "people.ini"), encoding="latin-1"):
        line = line.strip()
        if not line or line.startswith(";"):
            continue
        if line.startswith("["):
            section = line[1:line.index("]")]
            continue
        if section == "weights":
            name, rest = line.split("|", 1)
            weights[name.strip()] = [int(x) for x in rest.split()]
        elif section == "conflicts":
            a, b = line.split()[:2]
            conflicts.append((pair(a), pair(b)))
        elif section == "archetypes":
            f = [x.strip() for x in line.split("|")]
            fixed = {}
            hobby = None
            if f[3] and f[3] != "-":
                fixed["style"] = value_index(TRAIT_VALUES["style"], f[3])
            for tok in f[4].split():
                k, v = pair(tok)
                if k == "hobby":
                    hobby = v
                else:
                    fixed[k] = v
            arch.append(dict(name=f[0], weight=int(f[1]), letter=f[2][:1], fixed=fixed, hobby=hobby))
    return arch, conflicts, weights


def pair(tok):
    k, v = tok.split("=", 1)
    if k == "hobby":
        return k, HOBBIES.index(v)
    return k, value_index(TRAIT_VALUES[k], v)


def bare_name(name):
    out = name
    close = -1
    for i, c in enumerate(out):
        if c in "]})" or (c == "-" and i > 0 and out[i - 1] == "="):
            close = i
            break
    if 0 <= close < 12 and out[close + 1:]:
        out = out[close + 1:]
    bar = out.find("|")
    if bar > 2:
        out = out[:bar]
    return out.strip()


def name_seed(name):
    h = 2166136261
    for c in name.encode("latin-1", "replace"):
        h = s32(u32((s32(h) ^ c) * 16777619))
    return h & 0x7FFFFFFF


class Rand:
    def __init__(self, seed):
        self.state = seed

    def next(self):
        self.state = (self.state * 1103515245 + 12345) & 0x7FFFFFFF
        return self.state >> 8


def person(name, people):
    arch, conflicts, weights = people
    r = Rand(name_seed(bare_name(name)))
    total = sum(a["weight"] for a in arch)
    x = r.next() % max(total, 1)
    a = arch[-1]
    for cand in arch[:-1]:
        if x < cand["weight"]:
            a = cand
            break
        x -= cand["weight"]

    def roll(t):
        w = weights.get(t, [1])
        x = r.next() % max(sum(w), 1)
        for i, wi in enumerate(w):
            if x < wi:
                return i
            x -= wi
        return len(w) - 1

    traits = {t: (a["fixed"][t] if t in a["fixed"] else roll(t)) for t in TRAITS}
    hobby = [a["hobby"] if a["hobby"] is not None else r.next() % 16, r.next() % 16]
    if hobby[1] == hobby[0]:
        hobby[1] = (hobby[1] + 1 + r.next() % 15) % 16
    for _ in range(12):
        hit = None
        for (ka, va), (kb, vb) in conflicts:
            if has(traits, hobby, ka, va) and has(traits, hobby, kb, vb):
                hit = (ka, kb)
                break
        if not hit:
            break
        t = hit[1] if hit[1] != "hobby" and hit[1] not in a["fixed"] else hit[0]
        if t == "hobby" or t in a["fixed"]:
            hobby[1] = (hobby[1] + 1) % 16
            continue
        traits[t] = roll(t)
    return dict(arch=a["name"], letter=a["letter"], traits=traits, hobby=hobby)


def has(traits, hobby, k, v):
    return v in hobby if k == "hobby" else traits[k] == v


# ---- gm_botchat's tables -------------------------------------------------------------------

def botchat_tables():
    src = open(SMA, encoding="latin-1").read()
    personas = re.search(r'new const PERSONAS\[\] = "([^"]+)"', src).group(1)
    chatty = [float(x) for x in re.search(r"new const Float:CHATTY\[\] = \{([^}]+)\}", src).group(1).replace("\n", " ").split(",")]
    return dict(zip(personas, chatty))


def chatty_tier(name):
    h = 2166136261
    for c in name.encode("latin-1", "replace"):
        h = s32(u32((s32(h) ^ c) * 16777619))
    h &= 0x7FFFFFFF
    tier = (h // 977) % 100
    # with gm_people the talk trait does most of the spreading: gentler steps
    return 0.5 if tier < 45 else 0.8 if tier < 75 else 1.2 if tier < 93 else 1.7


# ---- the lines ---------------------------------------------------------------------------

def load_lines():
    sections = collections.defaultdict(list)
    files = [os.path.join(CFG, "botchat.ini")] + sorted(glob.glob(os.path.join(CFG, "botchat-*.ini")))
    for path in files:
        sec = None
        for line in open(path, encoding="latin-1"):
            s = line.strip()
            if not s or s.startswith(";"):
                continue
            if s.startswith("["):
                sec = s[1:s.index("]")].lower()
                continue
            if "|" in s and sec:
                cond, text = s.split("|", 1)
                sections[sec].append((cond.strip(), text.strip()))
    return sections


OP = re.compile(r"([a-z0-9]+)(>=|<=|!=|=|<|>)(.+)")


def cmp(have, op, want):
    return {">=": have >= want, "<=": have <= want, "!=": have != want, ">": have > want, "<": have < want}.get(op, have == want)


# ---- the server ----------------------------------------------------------------------------

class Bot:
    def __init__(self, name, people, chatty):
        self.name = name
        p = person(name, people)
        self.arch, self.letter, self.traits, self.hobby = p["arch"], p["letter"], p["traits"], p["hobby"]
        self.chatty = chatty_tier(name)
        self.base_chatty = chatty.get(self.letter, 1.0)
        self.team = 0
        self.alive = True
        self.next_bot = 0.0
        self.deathrow = 0
        self.round_kills = 0
        self.frags = self.deaths = 0
        self.lines = 0
        self.joined = 0.0
        self.recent = collections.deque(maxlen=24)


class Sim:
    def __init__(self, args):
        self.args = args
        self.people = load_people()
        self.chatty = botchat_tables()
        self.sections = load_lines()
        names = [l.split(";")[0].strip() for l in open(os.path.join(CFG, "regular_names.ini"), encoding="latin-1")
                 if l.strip() and not l.startswith(";") and not l.startswith("[")]
        names = [n for n in names if n and "|" not in n]
        random.shuffle(names)
        self.pool = names
        self.pers = args.personality / 100.0
        self.level_mult = 1.8
        self.gap_mult = 1.5 - 0.5 * self.pers
        self.global_gap = 4.0 * self.gap_mult
        self.bot_gap = 20.0 * self.gap_mult
        self.now = 0.0
        self.next_any = 0.0
        self.cap = collections.deque(maxlen=6)
        self.recent = collections.deque(maxlen=160)
        self.line_at, self.once, self.flags = {}, set(), {}
        self.offers = []
        self.pending = []
        self.said = []        # (time, bot, section, text, phase, chain)
        self.kills = collections.Counter()
        self.memory = {}       # (bot, other) -> dict(rel, fam, last)
        self.phase = "freeze"
        self.day = 0
        self.score = [0, 0]

    # facts and conditions, as gm_botchat's line_weight and fact_match
    def fact(self, bot, other, k, op, v):
        human = other == "HUMAN"
        if k in TRAITS:
            want = value_index(TRAIT_VALUES[k], v)
            return cmp(bot.traits[k], op, want) if want >= 0 else False
        if k == "hobby":
            hit = HOBBIES.index(v) in bot.hobby if v in HOBBIES else False
            return hit != (op == "!=")
        if k == "arch":
            return (bot.arch == v) != (op == "!=")
        if k in ("rel", "fam", "days", "rival"):
            m = self.memory.get((bot.name, other if human else getattr(other, "name", None)))
            if m is None:
                m = dict(aff=0, fam=0, last=None)
            have = {"rel": int(m["aff"] / 10), "fam": m["fam"], "days": (self.day - m["last"]) if m["last"] is not None else 9999,
                    "rival": 1 if m["aff"] <= -20 and m.get("kills", 0) >= 10 else 0}[k]
            return cmp(have, op, float(v))
        have = None
        if k == "kills":
            have = bot.round_kills
        elif k == "deaths":
            have = bot.deathrow
        elif k == "frags":
            have = bot.frags
        elif k == "kd":
            have = bot.frags / max(bot.deaths, 1)
        elif k == "alive":
            have = sum(1 for b in self.bots if b.alive and b.team == bot.team)
        elif k == "diff":
            have = self.score[bot.team] - self.score[1 - bot.team]
        elif k == "vs":
            have = self.kills[(bot.name, other if human else getattr(other, "name", ""))]
        elif k == "vsby":
            have = self.kills[(other if human else getattr(other, "name", ""), bot.name)]
        elif k == "joined":
            have = self.now - bot.joined
        if have is not None:
            return cmp(have, op, float(v))
        if k == "phase":
            have = "dead" if not bot.alive else self.phase
        elif k == "other":
            have = "human" if human else ("bot" if other else "none")
        elif k == "team":
            have = "t" if bot.team == 0 else "ct"
        elif k == "time":
            have = "evening"
        elif k == "map":
            return (v in self.map) != (op == "!=")
        elif k == "flag":
            return (self.flags.get(v, -1) > self.now) != (op == "!=")
        else:
            return False
        return (have == v) != (op == "!=")

    def weight(self, bot, other, cond, mood, key):
        score, w, trait, moodset = 0, 1.0, False, False
        for tok in cond.split():
            if tok.startswith("!"):
                if tok.startswith("!odds=") and random.randint(1, 100) > int(tok[6:]):
                    return 0, 0
                if tok.startswith("!cd=") and key in self.line_at and self.now - self.line_at[key] < float(tok[4:]):
                    return 0, 0
                if tok.startswith("!once=") and key in self.once:
                    return 0, 0
                continue
            m = OP.fullmatch(tok)
            if m:
                k, op, v = m.groups()
                if not self.fact(bot, other, k, op, v):
                    return 0, 0
                score += 1
                trait = trait or k in TRAITS or k in ("hobby", "arch")
                continue
            if any(c in tok for c in mood):
                w, moodset = 1.5 + 0.5 * self.pers, True
            elif tok == "*":
                continue
            elif bot.letter in tok:
                if not moodset:
                    w *= self.pers
                score += 1
            else:
                return 0, 0
        return (w * self.pers if trait else w), score

    def pick(self, bot, section, other):
        lines = self.sections.get(section)
        if not lines:
            return None
        mood = ""
        if bot.deathrow >= 3:
            mood += "-"
        elif bot.round_kills >= 2:
            mood += "+"
        if self.now - bot.joined < 150:
            mood += "^"
        cands = []
        for cond, text in lines:
            key = text
            w, score = self.weight(bot, other, cond, mood, key)
            if w <= 0:
                continue
            body = text.split(">")[0].strip()
            if len(body) > 6 and (body in self.recent or body in bot.recent):
                continue
            cands.append((w, score, cond, text))
        if not cands:
            return None
        best = max(c[1] for c in cands)
        top = sum(1 for c in cands if c[1] == best)
        tier = best - 1 if best > 0 and (top < 3 or random.randint(1, 5) == 1) else best
        pool = [c for c in cands if c[1] >= tier]
        topw = sum(c[0] for c in pool if c[1] == best)
        restw = sum(c[0] for c in pool if c[1] != best)
        boost = max(1.0, restw / topw) if tier < best and top < 3 and topw > 0 else 1.0
        ws = [c[0] * boost if c[1] == best else c[0] for c in pool]
        r = random.uniform(0, sum(ws))
        for c, w in zip(pool, ws):
            r -= w
            if r <= 0:
                return c
        return pool[-1]

    def want(self, bot, section, other, base, chain=0):
        if bot is None:
            return
        if not chain and (self.now < self.next_any or self.now < bot.next_bot or (len(self.cap) == 6 and self.now - self.cap[0] < 30)):
            return
        phase = 1.4 if (not bot.alive or self.phase in ("freeze", "end")) else (1.0 if section in BIG else 0.35)
        p = base * self.level_mult * self.pers * bot.base_chatty * bot.chatty * TALK[bot.traits["talk"]] * phase
        if random.random() > p:
            return
        pick = self.pick(bot, section, other)
        if not pick:
            return
        if chain:
            self.say(bot, section, pick, other, chain)
            return
        bid = (1 + 0.5 * pick[1]) * bot.base_chatty * math.sqrt(max(bot.chatty, 0.15)) * random.uniform(0.5, 1.5)
        self.offers.append((bid, bot, section, pick, other))

    def director(self):
        if not self.offers:
            return
        offers = sorted(self.offers, key=lambda o: -o[0])
        self.offers = []
        spoke = set()
        for turn, more in enumerate((1.0, 0.25, 0.05)):
            if turn and random.random() >= more:
                break
            cand = [o for o in offers if o[1].name not in spoke]
            if not cand:
                break
            o = cand[0]
            spoke.add(o[1].name)
            self.say(o[1], o[2], o[3], o[4], 0, extra=turn * random.uniform(1, 3))

    def say(self, bot, section, pick, other, chain, extra=0.0):
        w, score, cond, text = pick
        body = text.split(">")[0].strip()
        self.recent.append(body)
        bot.recent.append(body)
        for tok in cond.split():
            if tok.startswith("!cd="):
                self.line_at[text] = self.now
            elif tok.startswith("!once="):
                self.once.add(text)
            elif tok.startswith("!set="):
                name, _, ttl = tok[5:].partition("/")
                self.flags[name] = self.now + float(ttl or 120)
        delay = random.uniform(0.8, 2.2) + len(body) * random.uniform(0.1, 0.17) + extra
        bot.next_bot = self.now + delay + self.bot_gap * random.uniform(0.6, 1.4) / math.sqrt(max(bot.chatty, 0.15))
        if not chain:
            self.next_any = self.now + delay + self.global_gap * random.uniform(0.6, 1.4)
            self.cap.append(self.now)
        bot.lines += 1
        phase = "dead" if not bot.alive else self.phase
        self.said.append((self.now + delay, bot, section, choose(body), phase, chain, score))
        # no answer asked for: now and then someone answers anyway
        if ">" not in text and not chain and section in FOLLOW:
            to, who, odds = FOLLOW[section]
            if random.random() < odds:
                if who == 1:
                    target = other if isinstance(other, Bot) and other is not bot else None
                elif who == 2:
                    mates = [b for b in self.bots if b.team == bot.team and b is not bot]
                    target = random.choice(mates) if mates else None
                else:
                    target = random.choice([b for b in self.bots if b is not bot])
                if target:
                    self.pending.append((self.now + delay + random.uniform(0.3, 1.2),
                                         lambda t=target, s=to, b=bot: self.want(t, s, b, 1.0, 1)))
        # a reply asked for
        if ">" in text and chain < 3:
            tag = text.split(">", 1)[1].strip()
            who = other if isinstance(other, Bot) and other is not bot else random.choice([b for b in self.bots if b is not bot])
            self.pending.append((self.now + delay, lambda who=who, tag=tag, bot=bot, chain=chain: self.want(who, "r_" + tag, bot, 0.5, chain + 1)))

    def tick_pending(self):
        due = [p for p in self.pending if p[0] <= self.now]
        self.pending = [p for p in self.pending if p[0] > self.now]
        for _, f in due:
            f()

    # ---- a map -----------------------------------------------------------------------------
    def run_map(self, bots, mapname):
        self.bots, self.map = bots, mapname
        self.kills.clear()
        self.score = [0, 0]
        for i, b in enumerate(bots):
            b.team = i % 2
            b.frags = b.deaths = b.deathrow = 0
            b.joined = self.now
        # greetings as people join
        for b in random.sample(bots, k=min(4, len(bots))):
            self.step(random.uniform(2, 10))
            self.want(b, "greet", None, 0.5)
            self.director()
        end = self.now + 20 * 60
        while self.now < end:
            self.round()
        # map end
        self.phase = "end"
        for b in bots:
            self.want(b, "map_end", "HUMAN", 0.25)
        self.director()
        self.step(8)

    def step(self, secs):
        t = self.now + secs
        while self.now < t:
            self.now += 0.5
            self.tick_pending()
            self.director()

    def round(self):
        for b in self.bots:
            b.alive, b.round_kills = True, 0
        self.phase = "freeze"
        if random.random() < 0.22:
            self.want(random.choice(self.bots), "idle", None, 0.6)
        self.step(6)
        self.phase = "live"
        length = random.uniform(40, 110)
        kills = random.randint(6, 10)
        times = sorted(random.uniform(3, length) for _ in range(kills))
        last = 0
        for t in times:
            self.step(t - last)
            last = t
            self.kill()
        self.step(max(0, length - last))
        alive = [sum(1 for b in self.bots if b.alive and b.team == x) for x in (0, 1)]
        winner = 0 if alive[0] >= alive[1] else 1
        self.score[winner] += 1
        self.phase = "end"
        win = [b for b in self.bots if b.team == winner]
        lose = [b for b in self.bots if b.team != winner]
        self.want(random.choice(win), "round_win", None, 0.12)
        self.want(random.choice(lose), "round_lose", None, 0.12)
        self.step(5)

    def kill(self):
        alive = [b for b in self.bots if b.alive]
        if len(alive) < 2:
            return
        killer = random.choice(alive)
        victims = [b for b in alive if b.team != killer.team]
        if not victims:
            return
        victim = random.choice(victims)
        victim.alive = False
        killer.round_kills += 1
        killer.frags += 1
        killer.deathrow = 0
        victim.deaths += 1
        victim.deathrow += 1
        self.kills[(killer.name, victim.name)] += 1
        r = random.random()
        if r < 0.02:
            self.want(killer, "kill_knife", victim, 0.35)
            self.want(victim, "death_knife", killer, 0.45)
        elif r < 0.05:
            self.want(killer, "kill_nade", victim, 0.2)
            self.want(victim, "death_nade", killer, 0.3)
        elif r < 0.35:
            self.want(killer, "kill_headshot", victim, 0.15)
            self.want(victim, "death_headshot", killer, 0.18)
        else:
            self.want(killer, "kill_awp" if random.random() < 0.1 else "kill", victim, 0.06)
            self.want(victim, "death", killer, 0.1)
        if killer.round_kills == 3:
            self.want(killer, "streak", victim, 0.35)
        mates = [b for b in self.bots if b.team == killer.team and b is not killer]
        if mates:
            self.want(random.choice(mates), "nice_shot", killer, 0.1)
        if sum(1 for b in self.bots if b.alive and b.team == victim.team) == 1:
            last = next(b for b in self.bots if b.alive and b.team == victim.team)
            self.want(last, "last_alive", None, 0.45)


FOLLOW = {}
for _s in ("kill", "kill_headshot", "kill_nade", "kill_awp", "noscope"):
    FOLLOW[_s] = ("r_kill", 1, 0.45)
FOLLOW["kill_knife"] = ("r_kill", 1, 0.6)
for _s in ("death", "death_headshot", "death_nade"):
    FOLLOW[_s] = ("r_death", 1, 0.45)
FOLLOW["death_knife"] = ("r_death", 1, 0.55)
FOLLOW.update({"nice_shot": ("r_ns_thanks", 1, 0.65), "round_win": ("r_round", 2, 0.5), "round_lose": ("r_round", 2, 0.5),
               "greet": ("r_hi", 3, 0.55), "idle": ("r_idle", 3, 0.5), "late_night": ("r_idle", 3, 0.5),
               "map_end": ("r_gg", 3, 0.65), "teamkilled": ("r_sorry_tk", 1, 0.75)})


def choose(text):
    def one(m):
        opts = m.group(1).split("|")
        return random.choice(opts) if len(opts) > 1 else m.group(0)
    for _ in range(4):
        text = re.sub(r"\{([^{}]*\|[^{}]*)\}", one, text)
    return text


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--maps", type=int, default=30)
    ap.add_argument("--bots", type=int, default=20)
    ap.add_argument("--personality", type=int, default=100)
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    random.seed(args.seed)
    sim = Sim(args)
    nlines = sum(len(v) for v in sim.sections.values())
    maps = ["de_dust2", "de_inferno", "cs_office", "de_aztec", "de_nuke", "cs_italy", "de_train"]
    pool = sim.pool[:60]
    all_bots = {}
    per_map_bots = []
    for m in range(args.maps):
        names = random.sample(pool, args.bots)
        bots = [all_bots.setdefault(n, Bot(n, sim.people, sim.chatty)) for n in names]
        start = len(sim.said)
        for b in bots:
            b.lines = 0
        sim.run_map(bots, maps[m % len(maps)])
        per_map_bots.append([b.lines for b in bots])
        sim.day += 1 if m % 3 == 2 else 0

    said = sim.said
    minutes = args.maps * 20
    print(f"{nlines} lines in {len(sim.sections)} sections; {args.maps} maps x 20 min, {args.bots} bots, personality {args.personality}%\n")
    print(f"Bot lines per minute (server):   {len(said) / minutes:.2f}   (research: busy pub ~4-6 all players, target 1.5-4 for bots)")
    per_bot_min = [l / 20 for m in per_map_bots for l in m]
    print(f"Per bot per minute:              {st.mean(per_bot_min):.3f}  (target 0.15-0.35 for talkers)")
    silent = sum(1 for m in per_map_bots for l in m if l == 0) / sum(len(m) for m in per_map_bots)
    near = sum(1 for m in per_map_bots for l in m if l <= 1) / sum(len(m) for m in per_map_bots)
    print(f"Bots silent for a whole map:     {silent:.0%}   (0 or 1 line: {near:.0%}; target 30-40% near silent)")
    top = sorted((l for m in per_map_bots for l in m), reverse=True)
    share = sum(top[:len(top) // 5]) / max(sum(top), 1)
    print(f"Lines from the chattiest 20%:    {share:.0%}  (target about half)")
    phases = collections.Counter(p for _, _, _, _, p, _, _ in said)
    quiet = (phases["dead"] + phases["freeze"] + phases["end"]) / max(len(said), 1)
    print(f"Said dead / freeze / round end:  {quiet:.0%}  (target ~70%)   {dict(phases)}")
    words = [len(t.split()) for _, _, _, t, _, _, _ in said]
    print(f"Words per line:                  mean {st.mean(words):.1f}, 1-3 words {sum(w <= 3 for w in words) / len(words):.0%}  (target ~2, 60%)")
    replies = sum(1 for s in said if s[5] > 0) / max(len(said), 1)
    print(f"Replies to another line:         {replies:.0%}  (real pubs 40-47% of all messages answer something)")
    specific = collections.Counter(min(s[6], 3) for s in said)
    print(f"Specificity of lines said:       " + ", ".join(f"{k}{'+' if k == 3 else ''} conditions {v / len(said):.0%}" for k, v in sorted(specific.items())))
    # repetition: the same text from the same bot, and anyone, within a 2-hour session
    texts = [(t, b.name) for _, b, _, t, _, _, _ in said]
    session = len(said) * 120 // minutes
    rep_any = rep_bot = 0
    for i in range(0, len(texts) - session, session):
        chunk = texts[i:i + session]
        c = collections.Counter(t for t, _ in chunk if len(t) > 12)
        rep_any += sum(v - 1 for v in c.values() if v > 1)
        cb = collections.Counter(chunk)
        rep_bot += sum(v - 1 for (t, _), v in cb.items() if v > 1 and len(t) > 12)
    chunks = max(1, (len(texts) - session) // max(session, 1))
    print(f"In a 2-hour session:             {session} bot lines; lines over 12 characters repeated by anyone {rep_any / chunks:.1f}, by the same bot {rep_bot / chunks:.1f}")
    distinct = len({t for t, _ in texts})
    print(f"Distinct texts said overall:     {distinct} of {len(texts)} lines ({distinct / max(len(texts), 1):.0%})")
    # who the bots were
    archs = collections.Counter(b.arch for b in all_bots.values())
    print(f"\n{len(all_bots)} different bots; archetypes among them: {len(archs)}; most common: {archs.most_common(5)}")
    print("\nA stretch of chat:")
    for t, b, sec, text, phase, chain, score in said[200:230]:
        print(f"  {int(t // 60):3d}:{int(t % 60):02d}  {b.name[:16]:16} ({b.arch[:14]:14}) [{sec}] {'  ' * chain}{text}")


if __name__ == "__main__":
    main()
