#!/usr/bin/env python3
"""The commit list for docs/notes/Recheck.md: every fix and feature commit since QA round 4 began,
grouped by area, with the tests each added or touched (Rust test fns, Playwright specs, list-service
tests, node test scripts), or the area's usual check when it has none of its own.

    python3 scripts/recheck-list.py [BASE]     # BASE: the first QA4 commit (6eb8e379)
"""
import json, re, subprocess, sys, os
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = sys.argv[1] if len(sys.argv) > 1 else "6eb8e379"
rows = []
log = subprocess.run(["git", "-C", REPO, "log", "--format=%h%x09%s", f"{BASE}^..HEAD"], capture_output=True, text=True).stdout.splitlines()
for line in log:
    h, subj = line.split("\t", 1)
    diff = subprocess.run(["git", "-C", REPO, "show", "--format=", "-U0", h], capture_output=True, text=True, errors="replace").stdout
    files = re.findall(r"^\+\+\+ b/(.+)$", diff, re.M)
    rust, cur, prev_test = [], None, False
    for l in diff.splitlines():
        if l.startswith("+++ b/"): cur = l[6:]
        if cur and cur.endswith(".rs") and l.startswith("+"):
            m = re.match(r"\+\s*(?:async )?fn (\w+)\(\)", l)
            if m and (prev_test or "/tests" in cur or cur.endswith("tests.rs")): rust.append(m.group(1))
            prev_test = bool(re.match(r"\+\s*#\[(tokio::)?test", l)) or (prev_test and l.strip().startswith("+    ///")) or (prev_test and re.match(r"\+\s*#\[", l) is not None)
    specs = sorted({f.split("/")[-1] for f in files if f.endswith(".spec.ts")})
    py = sorted({f.split("/")[-1] for f in files if re.search(r"(^|/)test_[^/]+\.py$", f)})
    node = sorted({f.split("/")[-1] for f in files if re.search(r"scripts/test-[^/]+\.mjs$", f)})
    notes = all(f.startswith("docs/") or f.endswith(".md") for f in files)
    rows.append({"h": h, "s": subj, "rust": sorted(set(rust)), "specs": specs, "py": py, "node": node, "notes": notes, "files": len(files)})

AREAS=[
 ("GameBanana publish", r"GameBanana's rules"),
 ("QA4-CORE", r"QA4-CORE|QA4 core"),
 ("Editor coverage", r"Editor coverage|editor coverage"),
 ("Node coverage", r"node coverage"),
 ("Plugin import and export", r"Plugin Studio: Community doesn't take plugins|Plugin import|Plugin Studio.*Export"),
 ("Contrast", r"[Cc]ontrast"),
 ("Base Builder", r"Base Builder"),
 ("Gun Game", r"^Gun Game"),
 ("Share and remix", r"[Rr]emix|editable source"),
 ("Creature sharing", r"[Ss]hared creature|creature kind|creatures in packages|creature security|creature review|Creature Studio: the running|creaturepack|namespace|Community downloads unpack|package entry names|custom creature types"),
 ("QA4-T2", r"QA4-T2|QA4 T2|T2 low|Spray maker: Put back|Map tools find maps|Tools UX from QA4-T2|Stock files"),
 ("QA4-T1", r"QA4-T1|QA4 T1|T1-|Last Stand: the first map after|Last Stand round table|Last Stand: creature names"),
 ("QA4-A (map editor)", r"QA4-A|area A|Map editor"),
 ("QA4-B (Game Mode Studio)", r"QA4-B|area B|Studio|One in the Chamber|Juggernaut|Node help|Timeline|Requirements and the community page"),
 ("QA4-C / W1 (Last Stand, creatures)", r"QA4-C|area C|Last Stand|turret|Components: Creatures|W1"),
 ("QA4-D / W2 (Battle Royale, downed)", r"Battle Royale|BR |Downed|revive|W2|ls_mode down"),
 ("QA4-E (Deathrun, mode vote)", r"Deathrun|[Mm]ode vote|area E"),
 ("QA4-F / Y (crashes, servers, setup, updates)", r"QA4-F|Crash|crash|Servers|servers|Update path|ReHLDS|relocalize|RCON|Ham Sandwich|Windows|Linux|ls-control|voice|Voice|Foundation|Game Mode Menu|gm_vip"),
 ("QA4-G (Community share and import)", r"QA4-G|area G|Community|List service|list service|Site: Made by players|Open in Loner-Strike"),
 ("Community roles and Support Pack", r"[Rr]oles|Polar|Support|Supporter|Terms|Thanks page|lsmod"),
 ("Site and docs", r"^Site|^Docs|docs"),
]
DEFAULT={
 "Base Builder":"mod-files-check.py; sims basebuilder, bb-precache",
 "Gun Game":"mod-files-check.py; sim gungame",
 "QA4-C / W1 (Last Stand, creatures)":"sim laststand; the client part by the user",
 "QA4-D / W2 (Battle Royale, downed)":"sim br; the client part by the user",
 "QA4-E (Deathrun, mode vote)":"sims deathrun, modevote",
 "QA4-F / Y (crashes, servers, setup, updates)":"the Rust suite; Windows and Linux-server parts by the user (QA4.md H)",
 "QA4-G (Community share and import)":"list-service tests; the live list by the user",
 "Community roles and Support Pack":"list-service test_roles.py; live purchase and /link rows by the user",
 "Site and docs":"by eye (site not deployed)",
 "QA4-A (map editor)":"the editor Playwright specs",
 "QA4-B (Game Mode Studio)":"the studio Playwright specs; a playtest by the user",
 "Creature sharing":"Rust creaturepack and community tests; list-service test_creatures.py",
 "Contrast":"the full Playwright run (screenshots by eye)",
 "QA4-T1":"the T1 specs (qa4-t1-plugins-demos.spec.ts, creatures-qa4.spec.ts, sim-replay-qa4.spec.ts)",
 "QA4-T2":"qa4-t2-fixes.spec.ts",
 "Share and remix":"Rust mapsource tests; community-remix.spec.ts",
}
def area(s):
    for a,p in AREAS:
        if re.search(p,s): return a
    return "Other"
code=[r for r in rows if not r["notes"]]
by={}
for r in code: by.setdefault(area(r["s"]),[]).append(r)
order=[a for a,_ in AREAS]+["Other"]
out=[]
for a in order:
    if a not in by: continue
    out.append(f"\n### {a}\n\n| Commit | What | Verified by |\n|---|---|---|")
    for r in by[a]:
        ev=[]
        if r["rust"]: ev.append("Rust " + ", ".join(f"`{t}`" for t in r["rust"][:3]) + (f" (+{len(r['rust'])-3})" if len(r["rust"])>3 else ""))
        if r["specs"]: ev.append("Playwright " + ", ".join(f"`{t}`" for t in r["specs"][:3]) + (f" (+{len(r['specs'])-3})" if len(r["specs"])>3 else ""))
        if r["py"]: ev.append("list service " + ", ".join(f"`{t}`" for t in r["py"]))
        if r["node"]: ev.append("node " + ", ".join(f"`{t}`" for t in r["node"]))
        subj=r["s"].replace("|","/")
        subj=subj if len(subj)<=150 else subj[:147].rsplit(" ",1)[0]+"…"
        out.append(f"| {r['h']} | {subj} | {'; '.join(ev) if ev else '(none of its own) ' + DEFAULT.get(a, 'the full suites')} |")
print("\n".join(out))
print(f"\n<!-- {len(rows)} commits, {len(code)} with code, {len(rows)-len(code)} notes only -->", file=sys.stderr)
