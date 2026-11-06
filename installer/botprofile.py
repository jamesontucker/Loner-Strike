"""An original bot profile (BotProfile.db) for Counter-Strike 1.6, which has
the same bots as Condition Zero but doesn't ship a profile file.

Each bot gets a short handle (the profile name), a skill tier, a temperament and
favourite weapons. The display names with clan tags come from gm_pubserver's
regulars list, so these only need to be unique, plain words.
"""

import random

WEAPONS = {
    "Rifle": ["m4a1", "ak47", "famas", "galil", "mp5"],
    "RifleT": ["ak47", "m4a1", "galil", "famas", "mp5"],
    "Scoped": ["aug", "sg552", "famas", "galil", "mp5"],
    "Sniper": ["awp", "scout", "sg550", "g3sg1", "famas", "galil"],
    "Heavy": ["m249", "xm1014", "m3", "famas", "galil"],
    "Shotgun": ["xm1014", "m3", "famas", "galil", "mp5"],
    "Spray": ["p90", "mp5", "tmp", "mac10", "ump45"],
}
# name: (skill, aggression, reaction time, buy cost tier, difficulty)
TIERS = {
    "Ace":     (95, 90, 0.20, 4, "EXPERT"),
    "Skilled": (80, 75, 0.25, 3, "HARD+EXPERT"),
    "Solid":   (65, 60, 0.30, 2, "NORMAL+HARD"),
    "Casual":  (45, 50, 0.40, 2, "EASY+NORMAL"),
    "Rookie":  (25, 40, 0.50, 1, "EASY"),
}
HANDLES = """Ash Axel Bandit Bishop Blaze Bolt Boone Brick Bronco Buck Bullet Cash Chase Chief Cinder Clutch Cobra
Colt Comet Cosmo Crash Crow Dash Delta Diesel Dingo Dodge Drake Duke Echo Ember Falcon Fang Finch Flint Fox Frost
Fury Ghost Gizmo Goose Grim Gunner Hawk Havoc Hex Hornet Hunter Iron Jinx Jolt Kilo Knox Lance Lobo Lynx Mako Mamba
Maverick Moose Nitro Nova Onyx Orbit Ozzy Patch Pike Pixel Puma Quill Radar Rafter Raven Razor Rebel Rex Rhino Ridge
Rocket Rogue Ronin Rook Rusty Sabre Scout Shade Shark Skid Slate Smoke Snipe Spike Spud Static Storm Stray Tank Thorn
Torch Tracer Trigger Tusk Viper Volt Wolf Wren Yeti Zed Ziggy Zulu""".split()


def generate(seed=2004):
    rnd = random.Random(seed)
    out = [
        "// BotProfile.db written by Loner-Strike (original content, GPL-3.0).",
        "// Bot personalities for Counter-Strike 1.6. Edit freely.",
        "",
        "Default",
        "\tSkill = 50", "\tAggression = 50", "\tReactionTime = 0.3", "\tAttackDelay = 0", "\tTeamwork = 75",
        "\tWeaponPreference = none", "\tCost = 0", "\tDifficulty = NORMAL", "\tVoicePitch = 100", "\tSkin = 0",
        "End", "",
    ]
    for name, guns in WEAPONS.items():
        out.append("Template %s" % name)
        out += ["\tWeaponPreference = %s" % g for g in guns]
        out += ["End", ""]
    for name, (skill, aggr, react, cost, diff) in TIERS.items():
        out += ["Template %s" % name, "\tSkill = %d" % skill, "\tAggression = %d" % aggr,
                "\tReactionTime = %.2f" % react, "\tCost = %d" % cost, "\tDifficulty = %s" % diff, "End", ""]
    tiers = list(TIERS)
    weights = [1, 2, 3, 3, 2]
    for handle in HANDLES:
        tier = rnd.choices(tiers, weights)[0]
        guns = rnd.choices(list(WEAPONS), [5, 4, 2, 2, 1, 1, 2])[0]
        out += ["%s+%s %s" % (tier, guns, handle),
                "\tAggression = %d" % max(0, min(100, TIERS[tier][1] + rnd.randint(-20, 20))),
                "\tTeamwork = %d" % rnd.randint(40, 100),
                "\tVoicePitch = %d" % rnd.choice([85, 90, 95, 100, 105, 110]),
                "End", ""]
    return "\r\n".join(out) + "\r\n"
