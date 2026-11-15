#!/usr/bin/env python3
"""Writes the TrenchBroom-style .map fixtures for tests/editor-coverage-trenchbroom.spec.ts.
Run it again after changing it: python3 make_fixtures.py (files land beside it).
Boxes use Hammer/TrenchBroom's face winding (normal points out of the brush)."""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
WAD = "C:\\\\Program Files (x86)\\\\Steam\\\\steamapps\\\\common\\\\Half-Life\\\\valve\\\\halflife.wad"


def n(x):
    return str(int(x)) if float(x).is_integer() else repr(x)


def box(mn, mx, tex, fmt="valve", extras=False):
    (x0, y0, z0), (x1, y1, z1) = mn, mx
    # (three points, valve u axis, valve v axis) per face: top, bottom, west, east, north, south
    faces = [
        (((x0, y1, z1), (x1, y1, z1), (x1, y0, z1)), (1, 0, 0), (0, -1, 0)),
        (((x0, y0, z0), (x1, y0, z0), (x1, y1, z0)), (1, 0, 0), (0, -1, 0)),
        (((x0, y1, z1), (x0, y0, z1), (x0, y0, z0)), (0, 1, 0), (0, 0, -1)),
        (((x1, y1, z0), (x1, y0, z0), (x1, y0, z1)), (0, 1, 0), (0, 0, -1)),
        (((x1, y1, z1), (x0, y1, z1), (x0, y1, z0)), (1, 0, 0), (0, 0, -1)),
        (((x1, y0, z0), (x0, y0, z0), (x0, y0, z1)), (1, 0, 0), (0, 0, -1)),
    ]
    out = ["{"]
    for pts, u, v in faces:
        p = " ".join("( " + " ".join(n(c) for c in q) + " )" for q in pts)
        if fmt == "valve":
            line = f"{p} {tex} [ {' '.join(map(n, u))} 0 ] [ {' '.join(map(n, v))} 0 ] 0 1 1"
        else:
            line = f"{p} {tex} 0 0 0 1 1"
        if extras:
            line += " 0 0 0"  # Quake 2: content flags, surface flags, value
        out.append(line)
    out.append("}")
    return out


def room(fmt, extras=False, tex="OUT_WALL5"):
    b = []
    t, lo, hi, top = 16, -256, 256, 192
    b.append(box((lo - t, lo - t, -t), (hi + t, hi + t, 0), tex, fmt, extras))  # floor
    b.append(box((lo - t, lo - t, top), (hi + t, hi + t, top + t), "SKY", fmt, extras))  # ceiling
    b.append(box((lo - t, lo - t, 0), (lo, hi + t, top), tex, fmt, extras))  # west
    b.append(box((hi, lo - t, 0), (hi + t, hi + t, top), tex, fmt, extras))  # east
    b.append(box((lo, lo - t, 0), (hi, lo, top), tex, fmt, extras))  # south
    b.append(box((lo, hi, 0), (hi, hi + t, top), tex, fmt, extras))  # north
    return b


def ent(keys, brushes=(), i=[0]):
    out = [f"// entity {i[0]}", "{"]
    i[0] += 1
    out += [f'"{k}" "{v}"' for k, v in keys]
    for bn, br in enumerate(brushes):
        out.append(f"// brush {bn}")
        out += br
    out.append("}")
    return out


def write(name, header, ents):
    with open(os.path.join(HERE, name), "w", newline="\n") as f:
        f.write("\n".join(header + [l for e in ents for l in e]) + "\n")


def valve_layers():
    i = [0]
    E = lambda k, b=(): ent(k, b, i)
    ents = [
        E([("mapversion", "220"), ("wad", WAD), ("classname", "worldspawn"), ("_tb_def", "builtin:HalfLife.fgd"), ("skyname", "desert")], room("valve")),
        # a hidden, locked layer, its own group with a nested group inside
        E([("classname", "func_group"), ("_tb_type", "_tb_layer"), ("_tb_name", "Details"), ("_tb_id", "2"), ("_tb_layer_sort_index", "0"), ("_tb_layer_hidden", "1"), ("_tb_layer_locked", "1")], [box((-200, -200, 0), (-136, -136, 64), "CRATE01")]),
        E([("classname", "func_group"), ("_tb_type", "_tb_group"), ("_tb_name", "Crates"), ("_tb_id", "3"), ("_tb_layer", "2")], [box((100, 100, 0), (164, 164, 64), "CRATE01")]),
        E([("classname", "func_group"), ("_tb_type", "_tb_group"), ("_tb_name", "Inner"), ("_tb_id", "4"), ("_tb_group", "3")], [box((100, 100, 64), (132, 132, 96), "CRATE01")]),
        # a layer TrenchBroom leaves out when it exports
        E([("classname", "func_group"), ("_tb_type", "_tb_layer"), ("_tb_name", "Notes"), ("_tb_id", "5"), ("_tb_layer_sort_index", "1"), ("_tb_layer_omit_from_export", "1")], [box((-64, 160, 0), (-32, 192, 32), "AAATRIGGER")]),
        # linked groups (TrenchBroom 2023+): two copies, the second moved 128 on x
        E([("classname", "func_group"), ("_tb_type", "_tb_group"), ("_tb_name", "Pillar"), ("_tb_id", "6"), ("_tb_linked_group_id", "{3c1b6c8e-6f1e-4f43-9d7a-1a2b3c4d5e6f}"), ("_tb_transformation", "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1")], [box((-160, 0, 0), (-128, 32, 128), "C1A0_LABFLRD")]),
        E([("classname", "func_group"), ("_tb_type", "_tb_group"), ("_tb_name", "Pillar"), ("_tb_id", "7"), ("_tb_linked_group_id", "{3c1b6c8e-6f1e-4f43-9d7a-1a2b3c4d5e6f}"), ("_tb_transformation", "1 0 0 128 0 1 0 0 0 0 1 0 0 0 0 1")], [box((-32, 0, 0), (0, 32, 128), "C1A0_LABFLRD")]),
        # entities in the layer and in the nested group; a brush entity in a group
        E([("classname", "light"), ("origin", "0 0 160"), ("_light", "255 255 230 300"), ("_tb_layer", "2")]),
        E([("classname", "info_player_start"), ("origin", "0 -128 36"), ("angles", "0 90 0")]),
        E([("classname", "info_player_deathmatch"), ("origin", "64 -128 36"), ("angles", "0 90 0"), ("_tb_group", "4")]),
        E([("classname", "func_door"), ("angles", "0 0 0"), ("speed", "100"), ("targetname", "door1"), ("_tb_group", "3")], [box((180, -64, 0), (196, 64, 128), "GENERIC015V")]),
    ]
    write("tb_valve_layers.map", ["// Game: Half-Life", "// Format: Valve"], ents)


def standard(extras):
    i = [0]
    E = lambda k, b=(): ent(k, b, i)
    ents = [
        E([("classname", "worldspawn"), ("wad", WAD), ("_tb_def", "builtin:HalfLife.fgd")], room("standard", extras)),
        E([("classname", "func_group"), ("_tb_type", "_tb_group"), ("_tb_name", "Crate"), ("_tb_id", "2")], [box((0, 0, 0), (64, 64, 64), "CRATE01", "standard", extras)]),
        E([("classname", "light"), ("origin", "0 0 160"), ("_light", "255 255 230 300")]),
        E([("classname", "info_player_start"), ("origin", "-128 -128 36")]),
    ]
    write("tb_standard_q2.map" if extras else "tb_standard.map", ["// Game: Half-Life", "// Format: Standard"], ents)


def valve_q2():
    i = [0]
    E = lambda k, b=(): ent(k, b, i)
    ents = [
        E([("mapversion", "220"), ("classname", "worldspawn"), ("wad", WAD)], room("valve", True)),
        E([("classname", "light"), ("origin", "0 0 160"), ("_light", "255 255 230 300")]),
        E([("classname", "info_player_start"), ("origin", "-128 -128 36")]),
    ]
    write("tb_valve_q2extras.map", ["// Game: Half-Life", "// Format: Valve"], ents)


valve_layers()
standard(False)
standard(True)
valve_q2()
