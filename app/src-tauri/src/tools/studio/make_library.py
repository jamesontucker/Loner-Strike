# Writes the library items that ship with the app (the Studio's My library lists them as
# "built in"). Run from this folder:
#   python3 make_library.py ../../../../ui/src/views/tools/studio/library
# Each item is graph data checked like a player's own (mod.rs: check_library_item, and a
# compile test).
import json, os, sys

out = sys.argv[1] if len(sys.argv) > 1 else "../../../../ui/src/views/tools/studio/library"

# ---- Run Dialog: a conversation from the Dialog table -------------------------------------
# One row per line: id, speaker, the line, up to six replies and the row each one goes to
# ("end", or empty, stops). A Macro (Say and Choose waits for the pick, which a Function can't):
# it goes round Get Row -> Say and Choose -> the picked reply's next row until "end", a missing
# id (said in the server log; the table check names it in Problems), Timed out, Walked away, or
# 64 lines (a guard against a table that loops forever).
G = "rundlg"
COLS = [("id", "Id"), ("speaker", "Speaker"), ("text", "Line")] + [(f"reply{i}", f"Reply {i}") for i in range(1, 7)] + [(f"next{i}", f"Next {i}") for i in range(1, 7)]
ROWS = [
    ["start", "Mara", "Took you long enough. The gate's jammed again.", "I'll fix it.", "What happened?", "Not my problem.", "", "", "", "fix", "story", "end", "", "", ""],
    ["story", "Mara", "Raiders hit us at dawn and took the fuse.", "Where did they go?", "I'll get it back.", "", "", "", "", "where", "fix", "", "", "", ""],
    ["where", "Mara", "East, past the old silo. Watch for traps.", "Got it.", "", "", "", "", "", "fix", "", "", "", "", ""],
    ["fix", "Mara", "Bring the fuse to the box by the tower.", "On my way.", "Anything else?", "", "", "", "", "end", "bye", "", "", "", ""],
    ["bye", "Mara", "Stay sharp out there. Come back alive.", "Will do.", "", "", "", "", "", "end", "", "", "", "", ""],
]
LIMIT = 64

nodes, edges = [], []
def N(id, type, x, y, **params):
    nodes.append({"id": id, "type": type, "pos": {"x": x, "y": y}, "params": params, "graph": G})
    return id
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges) + 1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

# (each player's place in the talk, how many lines it has had, and who's talking: a later pick
# reads them back)
VARS = [
    {"id": "dlg_at", "name": "Dialog row", "type": "text", "scope": "player", "default": ""},
    {"id": "dlg_lines", "name": "Dialog lines", "type": "number", "scope": "player", "default": 0},
    {"id": "dlg_npc", "name": "Dialog NPC", "type": "entity", "scope": "player"},
]
def get(var, x, y, nid):
    N(nid, "var.get", x, y, var=var)
    W("in", "who", nid, "player")
    return nid

N("in", "macro.entry", 0, 0)
N("s_at", "var.set", 260, 0, var="dlg_at"); W("in", "go", "s_at", "exec"); W("in", "who", "s_at", "player"); W("in", "start", "s_at", "value")
N("s_n", "var.set", 520, 0, var="dlg_lines", value=0); W("s_at", "then", "s_n", "exec"); W("in", "who", "s_n", "player")
N("s_npc", "var.set", 780, 0, var="dlg_npc"); W("s_n", "then", "s_npc", "exec"); W("in", "who", "s_npc", "player"); W("in", "npc", "s_npc", "value")
# the row: found? (a missing id ends it, said in the log)
get("dlg_at", 1040, 200, "g_at")
N("row", "table.get_row", 1300, 200, table="dialog"); W("g_at", "out", "row", "key")
N("has", "flow.branch", 1300, 0); W("s_npc", "then", "has", "exec"); W("row", "found", "has", "cond")
N("lost", "debug.print", 1560, -200, text="Run Dialog: no row of Dialog has the id", chat=False); W("has", "false", "lost", "exec"); W("g_at", "out", "lost", "value")
# the guard: at most LIMIT lines in one talk
get("dlg_lines", 1560, 260, "g_n")
N("few", "math.compare", 1820, 260, op="lt", b=LIMIT); W("g_n", "out", "few", "a")
N("ok", "flow.branch", 1820, 0); W("has", "true", "ok", "exec"); W("few", "out", "ok", "cond")
N("long", "debug.print", 2080, -200, text=f"Run Dialog: stopped after {LIMIT} lines (does the table go round?) at", chat=False); W("ok", "false", "long", "exec"); W("g_at", "out", "long", "value")
N("count", "var.add", 2080, 0, var="dlg_lines", amount=1); W("ok", "true", "count", "exec"); W("in", "who", "count", "player")
# the line and the replies, from the row
get("dlg_npc", 2080, 300, "g_npc")
N("say", "npc.choose", 2340, 0, count=6, secs=30); W("count", "then", "say", "exec"); W("in", "who", "say", "player"); W("g_npc", "out", "say", "npc")
W("row", "speaker", "say", "speaker"); W("row", "text", "say", "text")
for i in range(1, 7):
    W("row", f"reply{i}", "say", f"o{i}")
# a pick: on to its row (end or empty: the talk is over), round to the row's check again
N("done", "macro.exit", 3380, -300)
get("dlg_at", 3120, 300, "g_at2")
N("is_end", "text.equals", 3120, 420, b="end"); W("g_at2", "out", "is_end", "a")
N("is_none", "text.equals", 3120, 540, b=""); W("g_at2", "out", "is_none", "a")
N("over", "logic.and_or", 3380, 420, op="or"); W("is_end", "out", "over", "a"); W("is_none", "out", "over", "b")
N("again", "flow.branch", 3380, 0); W("over", "out", "again", "cond")
for i in range(1, 7):
    nid = N(f"to{i}", "var.set", 2860, -150 + 100 * i, var="dlg_at")
    W("say", f"r{i}", nid, "exec"); W("in", "who", nid, "player"); W("row", f"next{i}", nid, "value"); W(nid, "then", "again", "exec")
W("again", "false", "has", "exec")
for a, ap in [("again", "true"), ("say", "timeout"), ("say", "away"), ("lost", "then"), ("long", "then")]:
    W(a, ap, "done", "end")
get("dlg_at", 3120, -300, "g_last"); W("g_last", "out", "done", "last")

item = {
    "schema": "loner-strike/graph-library",
    "version": 1,
    "name": "Run Dialog",
    "description": "A conversation from the Dialog table: line after line, each reply going to its Next row, until end. Fill the table's rows; wire Talked To NPC in.",
    "category": "NPCs",
    "kind": "macro",
    "root": G,
    "enums": [], "structs": [], "functions": [],
    "macros": [{"id": G, "name": "Run Dialog",
                "inputs": [{"id": "go", "name": "In", "type": "exec"}, {"id": "npc", "name": "NPC", "type": "entity"}, {"id": "who", "name": "Player", "type": "player"}, {"id": "start", "name": "Start row", "type": "text"}],
                "outputs": [{"id": "end", "name": "Ended", "type": "exec"}, {"id": "last", "name": "Last row", "type": "text"}]}],
    "variables": VARS,
    "nodes": nodes,
    "edges": edges,
    "comments": [{"id": "c1", "graph": G, "pos": {"x": -40, "y": -380}, "size": {"w": 1400, "h": 120},
                  "text": "Run Dialog: Start row is a row's id in Dialog (start). Each reply goes to its Next row; end (or empty) stops. A missing id or 64 lines stop it too (the server log says which). Ended runs once, with the row it stopped at."}],
    "dataTables": [{"id": "dialog", "name": "Dialog", "key": "id", "columns": [{"id": c, "name": n, "type": "text"} for c, n in COLS], "rows": ROWS}],
}
for r in ROWS:
    assert len(r) == len(COLS) and all(len(c) <= 48 for c in r), r
os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "run_dialog.lib.json"), "w") as f:
    json.dump(item, f, indent=1)
    f.write("\n")
print("run_dialog:", len(nodes), "nodes,", len(edges), "wires,", len(ROWS), "rows")
