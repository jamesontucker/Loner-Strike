//! The map editor for agents, the rest of it: every edit the editor page makes, with a typed
//! schema each (they all reach the same `/api/tools/editor/cmd` and `cmd2` routes the page
//! uses, so each is one undo step), the Checks panel as structured findings with fixes applied
//! by id, the Budget with every bar and its biggest contributors, face details for texture
//! work, the Content Browser, prefabs, mode kits and visgroups.
//!
//! Rules as everywhere in the connector: reading is free; edits to the open map need "Allow
//! changes" and are undoable; anything that writes into the game or over a file asks first
//! (copying the map's files into the game, replacing a prefab).

use super::catalog::{ask, err, get, obj, pick, post, s, vec3, Done, Fail, Kind, Spec};
use super::Ctx;
use serde_json::{json, Map, Value};

// ---- schemas -----------------------------------------------------------------------------

fn ids(what: &str) -> Value {
    json!({"type": "array", "items": {"type": "integer"}, "description": what})
}
fn faces(what: &str) -> Value {
    json!({"type": "array", "items": {"type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2}, "description": what})
}
fn tex() -> Value {
    json!({"type": "string", "description": "A texture name from editor_textures, e.g. CRATE01 (15 characters at most)"})
}
fn lock() -> Value {
    json!({"type": "boolean", "description": "Texture lock (default on): textures stay put on the faces as they move, turn or stretch"})
}
fn drag() -> Value {
    json!({"type": "string", "description": "Optional key: steps sent with the same key are one undo step (like one mouse drag)"})
}

pub fn specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        // ---- reading -----------------------------------------------------------------
        Spec { name: "editor_findings", title: "Editor: Checks findings", kind: Read,
            schema: || obj(json!({
                "severity": {"type": "string", "enum": ["error", "warning", "info"], "description": "Only this level"},
                "kind": {"type": "string", "description": "Only this kind (a finding's kind, e.g. z-fighting, stuck-in-solid, missing-texture, leak-risk)"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 500, "description": "Most findings listed (default 200; counts always cover all)"},
                "min_scale": {"type": "number", "description": "Checks settings: texture scales under this are 'stretched' (default 0.1)"},
                "max_scale": {"type": "number", "description": "Checks settings: texture scales over this are 'stretched' (default 4)"}}), &[]),
            description: "The Checks panel as data: every live finding on the open map (leaks risks, broken brushes, missing textures/models/sounds, spawns, z-fighting, things stuck in solid, micro-gaps, off-grid brushes, CS gameplay by map prefix, faces nobody sees, texture seams, stretched textures, ladders, lighting, heavy views, what the modes the map is made for still need). Each finding: id, kind, severity, message, location [x,y,z], brush/entity/face ids, hint, and fixes [{fixId, label}] to pass to editor_apply_fix. Counts by severity and kind." },
        Spec { name: "editor_budget", title: "Editor: Budget", kind: Read, schema: none,
            description: "How close the open map is to GoldSrc's limits, as the Budget panel shows it: every bar (last compile's BSP limits, brush entities, entities, edicts, textures, precache models and sounds with the game's baseline, the made-for modes' creatures and the creature guard's reserve) with used/max/percent and status green (under 75%), yellow or red (over 90%), the usual fix, and the biggest contributors with their brush and entity ids. warnings lists the red bars (Playtest and Publish ask first about those)." },
        Spec { name: "editor_faces", title: "Editor: faces of brushes", kind: Read,
            schema: || obj(json!({"brushes": ids("Brush ids (from editor_state); at most 200")}), &["brushes"]),
            description: "Each face of the given brushes: its index (for set_texture, align, move_face, extrude), texture, outward normal and which way it faces (up, down, north +Y, south, east +X, west), and its texture alignment (u/v axes, shift, scale, rotation)." },
        Spec { name: "editor_leak", title: "Editor: the leak", kind: Read,
            schema: || obj(json!({"points": {"type": "integer", "minimum": 0, "maximum": 4000, "description": "Most points of the line returned (default 200, thinned evenly; 0 for none)"}}), &[]),
            description: "Leak tracing: the leak line of the last compile that leaked (or a pointfile the person loaded), where the hole is and what's round it. leaked false when the last compile got past BSP without one. Else: pointfile, from (compile or file), pointCount, points [[x,y,z]…] from the entity that sees the void out to it, hole {at, index} (where the line leaves the world's brushes for good), nearest [{brush, distance}] (world brushes round the hole: close the gap there), brushEntitiesNear [{brush, entity, classname}] (they don't seal), start {entity, classname} (where the line begins), length and advice." },
        Spec { name: "editor_io", title: "Editor: what triggers what", kind: Read,
            schema: || obj(json!({
                "entity": {"type": "integer", "description": "An entity id: also its chain (what it sets off down the line, and what leads to it), and only the links in that chain"},
                "broken_only": {"type": "boolean", "description": "Only the broken links (default false)"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 5000, "description": "Most links listed (default 500; counts cover all)"}}), &[]),
            description: "The entity I/O graph (deathrun traps, jail cells, fun maps): nodes [{entity, classname, targetname, at [x,y,z], unused (named but nothing fires it)}], links [{from, to [entity ids], key, name, kind fires|kills|master|names, delay (a multi_manager's seconds)}] from target, killtarget, master, pathtarget, beams' ends, trigger_changetarget and every multi_manager name (name#2 fires name again), broken (links whose name no entity has: fix them by naming an entity so, or editor_findings kind dangling-target with its fixId to remove the key), shared (names several entities have: all of them fire) and counts. With entity: chain {down, up}." },
        Spec { name: "editor_history", title: "Editor: undo history", kind: Read, schema: none,
            description: "The undo and redo steps by name, newest first. editor_history_jump goes back or forward several at once." },
        Spec { name: "editor_find", title: "Editor: find things", kind: Read,
            schema: || obj(json!({
                "by": {"type": "string", "enum": ["text", "texture", "class", "value", "touching", "inside"], "description": "text: a texture name, classname, key or value (field); texture: brushes using textures; class: entities of the same classes as the given ones; value: entities with the same value of key; touching/inside: what touches or is inside the given brushes"},
                "field": {"type": "string", "enum": ["texture", "classname", "key", "value"], "description": "For by text"},
                "find": {"type": "string", "description": "For by text: what to look for"},
                "whole": {"type": "boolean", "description": "For by text: the whole name must match (default true)"},
                "key": {"type": "string", "description": "For by value (and text with field value): the key"},
                "textures": {"type": "array", "items": {"type": "string"}, "description": "For by texture"},
                "brushes": ids("The things to start from (similar, touching, inside) or to search in"),
                "entities": ids("The things to start from or to search in")}), &["by"]),
            description: "Finds brush and entity ids, like Select similar / touching / inside and Find in the editor. Nothing changes." },
        Spec { name: "editor_kits", title: "Editor: mode kits", kind: Read, schema: none,
            description: "The Gameplay panel's mode kits: each game mode's markers (id, label, point/area/path, how many it needs, keys), components and creatures; which modes the open map is made for; and what it still needs for them." },
        Spec { name: "content_list", title: "Content Browser: list", kind: Read,
            schema: || obj(json!({
                "kind": {"type": "string", "enum": ["model", "sprite", "sound"], "description": "Textures: editor_textures"},
                "search": {"type": "string", "description": "Words in the path"},
                "source": {"type": "string", "enum": ["game", "downloaded", "community", "mine", "other"]},
                "limit": {"type": "integer", "minimum": 1, "maximum": 500}}), &["kind"]),
            description: "The Content Browser: models, sprites or sounds in the game and the person's downloads, with where each came from (folder, path, size, source, page). Place one with editor_place_asset or set it on an entity key with editor_set_key." },
        Spec { name: "content_credits", title: "Content Browser: credits", kind: Read,
            schema: || obj(json!({"folder": {"type": "string", "description": "The item's folder from content_list (or project)"}, "path": {"type": "string", "description": "The item's path, e.g. models/crate.mdl"}}), &["folder", "path"]),
            description: "Who made a file and whether it may be shared inside a published map: source, page, authors, the license checklist, verdict (ok, ask, no, stock, unknown) and why. Recording permission or typing a credit is the person's alone." },
        Spec { name: "editor_assets", title: "Editor: files the map uses", kind: Read, schema: none,
            description: "The models, sprites, sounds and sky the open map's entities name, and whether each is found (game, download folder, beside the project) or missing." },
        Spec { name: "prefabs_list", title: "Prefabs: library", kind: Read,
            schema: || obj(json!({"search": {"type": "string"}}), &[]),
            description: "The prefab library (starter, community and the person's own): id, name, tags, size. Place one with editor_place_prefab." },
        // ---- edits (one undo step each) ----------------------------------------------
        Spec { name: "editor_add_shape", title: "Editor: add a shape", kind: Change,
            schema: || obj(json!({
                "shape": {"type": "string", "enum": ["box", "cylinder", "wedge", "spike", "cone", "sphere", "arch", "torus"]},
                "min": vec3("Lowest corner of the shape's box [x, y, z]"), "max": vec3("Highest corner [x, y, z]"),
                "texture": tex(),
                "sides": {"type": "integer", "minimum": 3, "maximum": 64, "description": "Sides (cylinder, spike, cone, sphere, arch, torus pieces); default 12"},
                "grid": {"type": "number", "description": "Snap corners to this grid (0: off)"},
                "arc": {"type": "number", "description": "Arch/torus: degrees round (default 180)"},
                "start": {"type": "number", "description": "Arch/torus: start angle"},
                "wall": {"type": "number", "description": "Arch: wall thickness; torus: tube width (default 16)"},
                "add_height": {"type": "number", "description": "Arch/torus: each piece climbs this much (spiral stairs and ramps)"},
                "top": {"type": "number", "description": "Cone: the top's size as a share of the base, 0 (a point) to 0.95"},
                "tube_sides": {"type": "integer", "description": "Torus: the tube's sides (default 8)"}}), &["shape", "min", "max"]),
            description: "Adds a primitive with all its settings, filling the box min..max (GoldSrc units, Z up). Returns the new brush ids." },
        Spec { name: "editor_transform", title: "Editor: move, turn, stretch", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes"), "entities": ids("Entities (with their brushes)"),
                "translate": vec3("Move by [x, y, z]"),
                "rotate": {"type": "object", "properties": {"axis": vec3("Axis, e.g. [0,0,1]"), "degrees": {"type": "number"}}, "required": ["axis", "degrees"]},
                "scale": vec3("Stretch factors [x, y, z]"), "pivot": vec3("Turn and stretch round this point (default: the middle)"),
                "lock": lock(), "keep_scale": {"type": "boolean", "description": "Scale lock: with texture lock, a stretch repeats the texture instead of stretching it"}}), &[]),
            description: "The Inspect Transform fields: moves, turns and/or stretches brushes and entities as one undo step." },
        Spec { name: "editor_move_face", title: "Editor: push/pull a face", kind: Change,
            schema: || obj(json!({"brush": {"type": "integer"}, "face": {"type": "integer", "description": "Face index (editor_faces)"}, "distance": {"type": "number", "description": "Units out (negative: in)"}, "lock": lock(), "drag": drag()}), &["brush", "face", "distance"]),
            description: "Moves one face of a brush along its normal (the brush stays convex)." },
        Spec { name: "editor_extrude_face", title: "Editor: extrude a face", kind: Change,
            schema: || obj(json!({"brush": {"type": "integer"}, "face": {"type": "integer"}, "distance": {"type": "number", "description": "How far out the new brush reaches"}}), &["brush", "face", "distance"]),
            description: "Pulls a new brush out of a face along its normal (textures carried on). Returns the new brush id." },
        Spec { name: "editor_vertices", title: "Editor: vertex edit", kind: Change,
            schema: || obj(json!({
                "action": {"type": "string", "enum": ["move", "split", "merge"], "description": "move: move corners; split: add a corner on an edge or face (at = its middle) and move it by delta; merge: join corners of one brush at their middle"},
                "picks": {"type": "array", "items": {"type": "array"}, "description": "move: [[brush, [x,y,z]], ...] corners; merge: [[x,y,z], ...] corners of brush"},
                "brush": {"type": "integer", "description": "split, merge: the brush"},
                "at": vec3("split: the edge's or face's middle"), "delta": vec3("move, split: by how much")}), &["action"]),
            description: "Vertex tool: move corners, split an edge or face with a new corner, or merge corners. Refused with a reason when the brush would stop being convex." },
        Spec { name: "editor_clip", title: "Editor: clip", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes to cut"), "points": {"type": "array", "items": vec3("point"), "minItems": 3, "maxItems": 3, "description": "Three points on the cutting plane"}, "keep": {"type": "string", "enum": ["front", "back", "both"]}, "texture": tex()}), &["brushes", "points", "keep"]),
            description: "Cuts brushes with a plane through three points; keeps one side or both (both: the new pieces' ids are returned)." },
        Spec { name: "editor_hollow", title: "Editor: hollow", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes"), "thickness": {"type": "number", "description": "Wall thickness (negative: walls grow outward)"}}), &["brushes", "thickness"]),
            description: "Makes each brush a room of walls (returns the wall brush ids)." },
        Spec { name: "editor_carve", title: "Editor: carve", kind: Change,
            schema: || obj(json!({"cutter": {"type": "integer"}, "targets": ids("Brushes to carve (empty: all it touches)")}), &["cutter"]),
            description: "Carves the cutter brush's shape out of the brushes it overlaps." },
        Spec { name: "editor_merge", title: "Editor: merge brushes", kind: Change,
            schema: || obj(json!({"brushes": ids("Two or more brushes")}), &["brushes"]),
            description: "Merges brushes into one convex brush (the note says when it had to fill gaps)." },
        Spec { name: "editor_snap", title: "Editor: snap to grid", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes"), "grid": {"type": "number", "description": "Grid size, e.g. 1, 8, 16"}}), &["brushes", "grid"]),
            description: "Snaps brush corners to the grid." },
        Spec { name: "editor_set_texture", title: "Editor: paint a texture", kind: Change,
            schema: || obj(json!({"faces": {"type": "array", "items": {"type": "array"}, "description": "[[brush, face], ...]; face null paints the whole brush"}, "texture": tex()}), &["faces", "texture"]),
            description: "Puts a texture on faces or whole brushes." },
        Spec { name: "editor_align_texture", title: "Editor: face edit sheet", kind: Change,
            schema: || obj(json!({"faces": faces("[[brush, face], ...]"),
                "action": {"type": "string", "enum": ["set", "shift", "scale", "rotate", "fit", "justify", "center", "world", "face", "view"], "description": "set: the sheet's numbers (any of shift_u, shift_v, scale_u, scale_v, rotation); shift: nudge by u, v; scale: set scale u, v; rotate: by degrees; fit: repeat u x v times over the face; justify: mode; center; world / face: align to the world or the face; view: to a camera (right, up)"},
                "u": {"type": "number"}, "v": {"type": "number"}, "degrees": {"type": "number"},
                "shift_u": {"type": "number"}, "shift_v": {"type": "number"}, "scale_u": {"type": "number"}, "scale_v": {"type": "number"}, "rotation": {"type": "number"},
                "mode": {"type": "string", "enum": ["left", "right", "top", "bottom", "center", "fit"], "description": "For justify"},
                "one": {"type": "boolean", "description": "Treat the faces as one surface (justify, fit, center, scale, rotate)"},
                "right": vec3("For view: the camera's right"), "up": vec3("For view: the camera's up"), "drag": drag()}), &["faces", "action"]),
            description: "Texture alignment like the face edit sheet: shift, scale, rotate, Fit, Justify, Center, to the world/face/view, or set exact numbers." },
        Spec { name: "editor_wrap_texture", title: "Editor: wrap a texture round", kind: Change,
            schema: || obj(json!({"from": json!({"type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2, "description": "[brush, face] to copy from"}), "faces": faces("[[brush, face], ...] to paint"), "texture": tex()}), &["from", "faces"]),
            description: "Alt+right-click: paints faces with the source face's texture and alignment, folded round the edge so it runs on." },
        Spec { name: "editor_delete", title: "Editor: delete", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes"), "entities": ids("Entities")}), &[]),
            description: "Deletes brushes and entities (undoable)." },
        Spec { name: "editor_duplicate", title: "Editor: duplicate", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes"), "entities": ids("Entities"), "offset": vec3("Where the copy goes, relative [x, y, z]"), "rename": {"type": "boolean", "description": "Give copies new target names (default true)"}}), &[]),
            description: "Copies brushes and entities by an offset. Returns the new ids." },
        Spec { name: "editor_array", title: "Editor: paste special (array)", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes"), "entities": ids("Entities"), "copies": {"type": "integer", "minimum": 1, "maximum": 256},
                "offset": vec3("Each copy moves by"), "rotate": vec3("Each copy turns by degrees about x, y, z"), "scale": vec3("Each copy stretches by"), "pivot": vec3("Turn round this point"), "lock": lock()}), &["copies"]),
            description: "Makes copies, each moved, turned and stretched from the last (stairs, fences, rings). Returns the new ids." },
        Spec { name: "editor_paste", title: "Editor: paste .map text", kind: Change,
            schema: || obj(json!({"text": {"type": "string", "description": "Valve 220 .map text (brushes and entities)"}, "offset": vec3("Move it by")}), &["text"]),
            description: "Adds brushes and entities written as .map text. Returns the new ids." },
        Spec { name: "editor_group", title: "Editor: groups", kind: Change,
            schema: || obj(json!({"action": {"type": "string", "enum": ["group", "ungroup", "hide", "show", "lock", "unlock", "rename", "move"], "description": "group: make a group of brushes/entities; move: put them (and groups) into group (null: out of every group)"},
                "group": {"type": ["integer", "null"]}, "name": {"type": "string"}, "brushes": ids("Brushes"), "entities": ids("Entities"), "groups": ids("Groups to move inside group")}), &["action"]),
            description: "Groups: make, ungroup, hide/show, lock/unlock, rename, or move things into a group. 'group' returns the new group id." },
        Spec { name: "editor_tie", title: "Editor: tie to entity / to world", kind: Change,
            schema: || obj(json!({"brushes": ids("Brushes to tie"), "classname": {"type": "string", "description": "e.g. func_wall, func_door, func_detail, func_buyzone, trigger_multiple"}, "to_world": {"type": "integer", "description": "Instead: an entity whose brushes go back to the world"}}), &[]),
            description: "Ties brushes to a new brush entity (returns its id) or moves an entity's brushes back to the world." },
        Spec { name: "editor_world", title: "Editor: map properties", kind: Change,
            schema: || obj(json!({"wads": {"type": "array", "items": {"type": "string"}, "description": "The whole WAD list"}, "set": {"type": "object", "additionalProperties": {"type": "string"}, "description": "worldspawn keys to set (empty value removes), e.g. skyname, light, ls_modes"}}), &[]),
            description: "Sets the WAD list and/or worldspawn keys (merged with the ones there)." },
        Spec { name: "editor_set_keys", title: "Editor: one key on many entities", kind: Change,
            schema: || obj(json!({"entities": ids("Entities"), "key": {"type": "string"}, "value": {"type": "string", "description": "Empty removes it"}}), &["entities", "key"]),
            description: "Sets one key on many entities at once (one undo step)." },
        Spec { name: "editor_replace", title: "Editor: find and replace", kind: Change,
            schema: || obj(json!({"field": {"type": "string", "enum": ["texture", "classname", "key", "value"]}, "find": {"type": "string"}, "replace": {"type": "string"}, "key": {"type": "string"}, "whole": {"type": "boolean"}, "brushes": ids("Only in these"), "entities": ids("Only in these")}), &["field", "find", "replace"]),
            description: "Replaces texture names, classnames, key names or values across the map (or in the given things)." },
        Spec { name: "editor_arrange", title: "Editor: align and distribute", kind: Change,
            schema: || obj(json!({"action": {"type": "string", "enum": ["align", "distribute"]}, "axis": {"type": "integer", "minimum": 0, "maximum": 2, "description": "0 x, 1 y, 2 z"}, "edge": {"type": "string", "enum": ["min", "centre", "max"], "description": "For align"}, "brushes": ids("Brushes"), "entities": ids("Entities"), "lock": lock()}), &["action", "axis"]),
            description: "Lines things up on an axis or spaces them evenly." },
        Spec { name: "editor_history_jump", title: "Editor: undo/redo several", kind: Change,
            schema: || obj(json!({"steps": {"type": "integer", "description": "Positive: undo that many; negative: redo"}}), &["steps"]),
            description: "Goes back or forward several steps in the undo history." },
        Spec { name: "editor_settings", title: "Editor: visgroups and cordon", kind: Change,
            schema: || obj(json!({"visgroups": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "brushes": ids("Brushes"), "entities": ids("Entities")}}, "description": "The whole visgroup list (editor_state shows it under editor.visSets)"},
                "cordon": {"type": "object", "description": "The cordon as editor_state shows it under editor.cordon"}}), &[]),
            description: "Sets the project's visgroups (named sets to hide and show) and the cordon." },
        Spec { name: "editor_apply_fix", title: "Editor: apply a Checks fix", kind: Change,
            schema: || obj(json!({"fixId": {"type": "string", "description": "A fixId from editor_findings"}, "fixIds": {"type": "array", "items": {"type": "string"}, "description": "Several, as one undo step"},
                "texture": tex(), "origin": vec3("Where to place it, for fixes that place an entity (default: the finding's location)"),
                "min_scale": {"type": "number"}, "max_scale": {"type": "number"}}), &[]),
            description: "Applies one-click fixes from editor_findings by id (one undo step), like the Checks panel's buttons. Replace-texture fixes need texture; place fixes use origin or the finding's location. Returns what's left." },
        Spec { name: "editor_place_asset", title: "Content Browser: place a file", kind: Change,
            schema: || obj(json!({"kind": {"type": "string", "enum": ["model", "sprite", "sound"]}, "path": {"type": "string", "description": "From content_list, e.g. models/crate.mdl"}, "origin": vec3("Where [x, y, z]"), "keys": {"type": "object", "additionalProperties": {"type": "string"}}}), &["kind", "path", "origin"]),
            description: "Places a Content Browser file like dragging it onto the map: a model as a cycler_sprite, a sprite as an env_sprite, a sound as an ambient_generic. Returns the entity id. To put a file on an existing entity's key use editor_set_key." },
        Spec { name: "editor_place_prefab", title: "Prefabs: place", kind: Change,
            schema: || obj(json!({"prefab": {"type": "string", "description": "A prefab id from prefabs_list"}, "point": vec3("Where on a surface"), "normal": vec3("The surface's normal (default up)"), "yaw": {"type": "number"}, "grid": {"type": "number"}}), &["prefab", "point"]),
            description: "Drops a prefab from the library at a point. Returns the new brush and entity ids." },
        Spec { name: "prefab_save", title: "Prefabs: save to library", kind: Change,
            schema: || obj(json!({"name": {"type": "string"}, "brushes": ids("Brushes"), "entities": ids("Entities"), "tags": {"type": "array", "items": {"type": "string"}}, "description": {"type": "string"}, "replace": {"type": "string", "description": "Your prefab's id to replace (asks the person first)"}}), &["name"]),
            description: "Saves brushes and entities as a prefab in the person's library." },
        Spec { name: "editor_place_marker", title: "Gameplay: place a marker", kind: Change,
            schema: || obj(json!({"mode": {"type": "string", "description": "A mode id from editor_kits"}, "marker": {"type": "string", "description": "A marker id (or a component classname) of that mode's kit"}, "origin": vec3("Where [x, y, z]")}), &["mode", "marker", "origin"]),
            description: "Places a mode kit's marker (an ls_marker with its kind, mode and keys; paths get the next spot) or component, like the Gameplay panel. Returns the entity id." },
        Spec { name: "editor_made_for", title: "Gameplay: made for modes", kind: Change,
            schema: || obj(json!({"modes": {"type": "array", "items": {"type": "string"}, "description": "Mode ids from editor_kits ([] for none)"}}), &["modes"]),
            description: "Says which game modes the map is made for (the worldspawn ls_modes key, with each mode's permanent uid). Checks then list what each still needs." },
        Spec { name: "editor_copy_assets", title: "Editor: copy the map's files", kind: Risky,
            schema: || obj(json!({"to": {"type": "string", "enum": ["project", "game"], "description": "project: beside the saved project; game: into the game's download folder (asks first)"}}), &["to"]),
            description: "Copies the models, sprites and sounds the map names (found elsewhere on this computer) beside the project or into the game." },
    ]
}

fn none() -> Value {
    super::catalog::none()
}

// ---- helpers -----------------------------------------------------------------------------

async fn cmd(ctx: &Ctx, body: Value) -> Done {
    post(ctx, "/api/tools/editor/cmd", body).await
}
async fn cmd2(ctx: &Ctx, body: Value) -> Done {
    post(ctx, "/api/tools/editor/cmd2", body).await
}

/// The listed fields that are there, plus op.
fn op(op: &str, a: &Value, keys: &[&str]) -> Value {
    let mut m = pick(a, keys);
    m.insert("op".into(), json!(op));
    Value::Object(m)
}

/// A short stable id (FNV-1a, 64 bits) for findings and fixes: the same problem keeps its id
/// across edits elsewhere, so an agent can read the findings, edit, and still fix by id.
fn short_id(prefix: &str, parts: &[&Value]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.to_string().bytes().chain([0u8]) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    format!("{prefix}{:012x}", h & 0xffff_ffff_ffff)
}

/// Plain words for a fix (the Checks panel's labels).
fn fix_label(fix: &str) -> String {
    if let Some(p) = fix.strip_prefix("place-marker:") {
        return format!("Place {p}");
    }
    crate::editor::checks::fix_label(fix).to_string()
}

fn num_q(a: &Value, k: &str) -> Option<f64> {
    a.get(k).and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0 && *x <= 1024.0)
}

/// The raw Checks answer (asked again while the map changes under it).
async fn raw_checks(ctx: &Ctx, a: &Value) -> Done {
    let mut path = "/api/tools/editor/checks".to_string();
    let mut q = vec![];
    if let Some(x) = num_q(a, "min_scale") {
        q.push(format!("minScale={x}"));
    }
    if let Some(x) = num_q(a, "max_scale") {
        q.push(format!("maxScale={x}"));
    }
    if !q.is_empty() {
        path = format!("{path}?{}", q.join("&"));
    }
    for _ in 0..5 {
        let v = get(ctx, &path).await?;
        if v["stale"] != true {
            return Ok(v);
        }
    }
    Err(err("The map kept changing while it was checked. Try again."))
}

/// One finding in the shape agents get, and the fix requests behind its fix ids.
fn finding(i: &Value) -> (Value, Vec<(String, Value)>) {
    let kind = i["code"].clone();
    let key = [&kind, &i["brush"], &i["entity"], &i["face"], &i["texture"], &i["at"], &i["place"]];
    let id = short_id("f-", &key);
    let mut fixes = vec![];
    let mut reqs = vec![];
    let mut add = |fix: &str, brush: &Value, entity: &Value, data: &Value| {
        let fid = short_id("fx-", &[&json!(fix), &kind, brush, entity, &i["face"], &i["texture"], data, &i["at"]]);
        let mut needs = vec![];
        if fix == "replace-texture" {
            needs.push("texture");
        }
        if fix.starts_with("place") && i["at"].is_null() {
            needs.push("origin");
        }
        fixes.push(json!({"fixId": fid, "fix": fix, "label": fix_label(fix), "needs": needs}));
        let mut r = json!({"fix": fix, "brush": brush, "entity": entity, "texture": i["texture"], "face": i["face"], "fixData": data, "at": i["at"]});
        if let Some(o) = r.as_object_mut() {
            o.retain(|_, v| !v.is_null());
        }
        reqs.push((fid, r));
    };
    if let Some(f) = i["fix"].as_str() {
        add(f, &i["brush"], &i["entity"], &i["fixData"]);
    }
    for m in i["moreFixes"].as_array().into_iter().flatten() {
        if let Some(f) = m["fix"].as_str() {
            let b = if m["brush"].is_null() { &i["brush"] } else { &m["brush"] };
            let e = if m["entity"].is_null() { &i["entity"] } else { &m["entity"] };
            add(f, b, e, &m["fixData"]);
        }
    }
    if let Some(p) = i["place"].as_str() {
        add(&format!("place-marker:{p}"), &Value::Null, &Value::Null, &Value::Null);
    }
    let mut brushes: Vec<Value> = i["brush"].as_u64().map(|b| vec![json!(b)]).unwrap_or_default();
    let mut entities: Vec<Value> = i["entity"].as_u64().map(|e| vec![json!(e)]).unwrap_or_default();
    // (the other side of a z-fight or a gap)
    for d in std::iter::once(&i["fixData"]).chain(i["moreFixes"].as_array().into_iter().flatten().map(|m| &m["fixData"])) {
        if let Some(b) = d["otherBrush"].as_u64() {
            if !brushes.contains(&json!(b)) {
                brushes.push(json!(b));
            }
        }
    }
    for m in i["moreFixes"].as_array().into_iter().flatten() {
        if let Some(e) = m["entity"].as_u64() {
            if !entities.contains(&json!(e)) {
                entities.push(json!(e));
            }
        }
    }
    let mut out = json!({"id": id, "kind": kind, "severity": i["level"], "message": i["message"], "location": i["at"], "brushes": brushes, "entities": entities,
        "face": i["face"], "texture": i["texture"], "hint": i["hint"], "fixes": fixes});
    if let Some(o) = out.as_object_mut() {
        o.retain(|_, v| !v.is_null());
    }
    (out, reqs)
}

/// Every finding, its fix requests by id, and the counts.
async fn findings(ctx: &Ctx, a: &Value) -> Result<(Value, Vec<Value>, Vec<(String, Value)>), Fail> {
    let v = raw_checks(ctx, a).await?;
    let mut all = vec![];
    let mut reqs = vec![];
    for i in v["issues"].as_array().into_iter().flatten() {
        let (f, r) = finding(i);
        all.push(f);
        reqs.extend(r);
    }
    Ok((v, all, reqs))
}

/// An ls_marker or component a kit places, with its keys (kits.ts markerKeys / kitPlacement).
fn kit_placement(kits: &Value, mode: &str, marker: &str, existing: &[Value]) -> Option<(String, Vec<(String, String)>)> {
    let kit = kits["kits"].as_array()?.iter().find(|k| k["mode"]["id"].as_str() == Some(mode))?;
    if let Some(m) = kit["markers"].as_array().into_iter().flatten().find(|m| m["id"].as_str() == Some(marker)) {
        let mut keys = vec![("kind".to_string(), marker.to_string()), ("mode".to_string(), mode.to_string())];
        for k in m["keys"].as_array().into_iter().flatten() {
            let name = k["name"].as_str().unwrap_or("");
            let v = match &k["default"] {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                Value::Bool(b) => if *b { "1".into() } else { "0".into() },
                Value::Array(a) => a.iter().map(|x| x.to_string().trim_matches('"').to_string()).collect::<Vec<_>>().join(" "),
                other => other.to_string(),
            };
            if !name.is_empty() && !v.is_empty() {
                keys.push((name.to_string(), v));
            }
        }
        match m["type"].as_str() {
            Some("area") => match m["draw"]["radius"].as_f64() {
                Some(r) if r > 0.0 => keys.push(("radius".into(), r.to_string())),
                _ => {
                    let size = m["draw"]["size"].as_array().map(|s| s.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")).unwrap_or_else(|| "256 256 128".into());
                    keys.push(("size".into(), size));
                }
            },
            Some("path") => {
                let kv = |e: &Value, k: &str| e["keys"].as_array().into_iter().flatten().find(|p| p[0].as_str() == Some(k)).and_then(|p| p[1].as_str().map(String::from));
                let spots: Vec<&Value> = existing.iter().filter(|e| e["classname"] == "ls_marker" && kv(e, "kind").as_deref() == Some(marker)).collect();
                let path_of = |e: &Value| kv(e, "path").and_then(|p| p.parse::<i64>().ok()).filter(|p| *p > 0).unwrap_or(1);
                let path = spots.iter().map(|e| path_of(e)).max().unwrap_or(1);
                let order = spots.iter().filter(|e| path_of(e) == path).filter_map(|e| kv(e, "order").and_then(|o| o.parse::<i64>().ok())).max().unwrap_or(0).max(0) + 1;
                keys.push(("path".into(), path.to_string()));
                keys.push(("order".into(), order.to_string()));
            }
            _ => {}
        }
        return Some(("ls_marker".into(), keys));
    }
    kit["components"].as_array().into_iter().flatten().find(|c| c["classname"].as_str() == Some(marker)).map(|c| (c["classname"].as_str().unwrap_or(marker).to_string(), vec![]))
}

/// Places a kit's marker or component (`place` = "mode:marker", or "kit:mode:marker").
async fn place_marker(ctx: &Ctx, mode: &str, marker: &str, origin: Value) -> Done {
    let kits = get(ctx, "/api/tools/editor/kits").await?;
    let doc = get(ctx, "/api/tools/editor/doc").await?;
    let existing = doc["entities"].as_array().cloned().unwrap_or_default();
    let (class, keys) = kit_placement(&kits, mode, marker, &existing).ok_or_else(|| err(format!("The {mode} kit has no marker or component called {marker} (editor_kits lists them).")))?;
    let keys: Vec<Value> = keys.into_iter().map(|(k, v)| json!([k, v])).collect();
    cmd(ctx, json!({"op": "add_entity", "classname": class, "origin": origin, "keys": keys})).await
}

// ---- the Budget ----------------------------------------------------------------------------
// The panel adds the game's own share and the made-for modes' creatures to the map's precache
// on the page (BudgetPanel.tsx); the same numbers here, so an agent sees what the person sees.

/// The game, a map and Loner-Strike's plugins before creatures (Y's measurement).
const BASELINE: (u64, u64) = (197, 315);
/// The slots the creature guard keeps free (`ls_creature_reserve "40 60"`).
const RESERVE: (u64, u64) = (40, 60);
/// Y's measured cost per creature type: models it adds, new sounds.
const Y_TYPES: &[(&str, &[&str], u64)] = &[
    ("zombie", &["zombie.mdl"], 12),
    ("headcrab", &["headcrab.mdl"], 11),
    ("runner", &["houndeye.mdl"], 10),
    ("babycrab", &["baby_headcrab.mdl"], 0),
    ("spitter", &["bullsquid.mdl"], 12),
    ("vortigaunt", &["islave.mdl", "sprites/lgtning.spr"], 10),
    ("brute", &["agrunt.mdl"], 10),
    ("snark", &["w_squeak.mdl"], 5),
    ("assassin", &["hassassin.mdl"], 2),
    ("soldier", &["hgrunt.mdl"], 6),
    ("babygarg", &["garg.mdl"], 10),
    ("gonarch", &["big_mom.mdl"], 10),
    ("boss", &["garg.mdl"], 1),
];

fn status(pct: f64) -> &'static str {
    if pct > 90.0 {
        "red"
    } else if pct > 75.0 {
        "yellow"
    } else {
        "green"
    }
}

/// A bar as agents get it: status, the fix, the biggest contributors with their ids.
fn bar_view(b: &Value, group: &str) -> Value {
    let pct = b["pct"].as_f64().unwrap_or(0.0);
    let mut v = json!({"id": b["id"], "group": group, "name": b["name"], "used": b["used"], "max": b["max"], "pct": pct, "status": status(pct), "what": b["what"]});
    if let Some(c) = b.get("contributors").filter(|c| c.is_object()) {
        let rows: Vec<Value> = c["rows"].as_array().into_iter().flatten().take(10).map(|r| {
            let mut r = r.clone();
            if let Some(o) = r.as_object_mut() {
                o.retain(|_, x| !x.is_null());
            }
            r
        }).collect();
        v["fix"] = c["fix"].clone();
        v["top"] = json!(rows);
        if let Some(basis) = c["basis"].as_str() {
            v["basis"] = json!(basis);
        }
    }
    v
}

/// What the modes the map is made for precache for their creatures (BudgetPanel.tsx modeLoad).
async fn mode_load(ctx: &Ctx) -> Option<Value> {
    let kits = get(ctx, "/api/tools/editor/kits").await.ok()?;
    let made: Vec<Value> = kits["madeFor"].as_array().cloned().unwrap_or_default();
    if made.is_empty() {
        return None;
    }
    let all = kits["kits"].as_array().cloned().unwrap_or_default();
    let kit_for = |m: &Value| -> Option<Value> {
        let uid = m["uid"].as_str().map(str::to_ascii_lowercase);
        all.iter()
            .find(|k| uid.is_some() && k["mode"]["uid"].as_str().map(str::to_ascii_lowercase) == uid)
            .or_else(|| all.iter().find(|k| k["mode"]["id"] == m["id"]))
            .cloned()
    };
    let used: Vec<Value> = made.iter().filter_map(kit_for).collect();
    let label = if used.is_empty() { made.iter().filter_map(|m| m["id"].as_str()).collect::<Vec<_>>().join(" + ") } else { used.iter().filter_map(|k| k["mode"]["name"].as_str()).collect::<Vec<_>>().join(" + ") };
    let mut names: Vec<String> = vec![];
    for k in &used {
        for c in k["creatures"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !names.iter().any(|n| n == c) {
                names.push(c.to_string());
            }
        }
    }
    let mut models: std::collections::BTreeSet<String> = Default::default();
    let mut sounds = 0u64;
    let mut others: std::collections::BTreeSet<String> = Default::default();
    let mut unknown = vec![];
    for n in &names {
        match Y_TYPES.iter().find(|(t, _, _)| t == n) {
            Some((_, ms, snd)) => {
                models.extend(ms.iter().map(|m| m.to_string()));
                sounds += snd;
            }
            None => unknown.push(n.clone()),
        }
    }
    if !unknown.is_empty() {
        if let Ok(c) = get(ctx, "/api/tools/creatures").await {
            let types: Vec<Value> = c["templates"].as_array().into_iter().flatten().chain(c["types"].as_array().into_iter().flatten()).cloned().collect();
            for n in &unknown {
                if let Some(t) = types.iter().rev().find(|t| t["name"].as_str() == Some(n)) {
                    if let Some(m) = t["model"].as_str().filter(|m| !m.is_empty()) {
                        models.insert(m.to_ascii_lowercase());
                    }
                    // the guard's default: 2 sounds per kind
                    for kind in t["snd"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        for w in kind.split(|c: char| c.is_whitespace() || c == '|').filter(|x| x.to_ascii_lowercase().ends_with(".wav")).take(2) {
                            others.insert(w.to_ascii_lowercase());
                        }
                    }
                }
            }
        }
    }
    Some(json!({"label": label, "roster": names, "models": models.len(), "sounds": sounds + others.len() as u64}))
}

async fn budget(ctx: &Ctx) -> Done {
    let r = get(ctx, "/api/tools/editor/budget").await?;
    let mut bars = vec![];
    for b in r["compile"]["rows"].as_array().into_iter().flatten() {
        bars.push(bar_view(b, "last compile"));
    }
    for b in r["live"].as_array().into_iter().flatten() {
        bars.push(bar_view(b, "as you build"));
    }
    let mode = mode_load(ctx).await;
    let p = &r["precache"];
    let (mm, ms) = (mode.as_ref().map(|m| m["models"].as_u64().unwrap_or(0)).unwrap_or(0), mode.as_ref().map(|m| m["sounds"].as_u64().unwrap_or(0)).unwrap_or(0));
    let pm = BASELINE.0 + p["models"].as_u64().unwrap_or(0) + mm;
    let ps = BASELINE.1 + p["sounds"].as_u64().unwrap_or(0) + ms;
    let max = p["max"].as_u64().unwrap_or(512);
    let pct = |u: u64| ((u as f64 / max as f64) * 1000.0).round() / 10.0;
    let ids_rows = |rows: &Value| -> Vec<Value> { rows.as_array().into_iter().flatten().take(15).cloned().collect() };
    bars.push(json!({"id": "precache_models", "group": "precache", "name": "Precached models", "used": pm, "max": max, "pct": pct(pm), "status": status(pct(pm)), "reserve": RESERVE.0,
        "parts": {"game": BASELINE.0, "map": p["models"], "inlineModels": p["inlineModels"], "modes": mm},
        "what": "the game, map and plugins before creatures (measured), this map's models, sprites and brush entities, and the made-for modes' creatures; the creature guard keeps the last slots free",
        "fix": "Reuse the same models and sprites; every func_wall, door and trigger is one model too.", "top": ids_rows(&p["modelRows"])}));
    bars.push(json!({"id": "precache_sounds", "group": "precache", "name": "Precached sounds", "used": ps, "max": max, "pct": pct(ps), "status": status(pct(ps)), "reserve": RESERVE.1,
        "parts": {"game": BASELINE.1, "map": p["sounds"], "modes": ms},
        "what": "the game, map and plugins before creatures (measured), this map's sounds, and the made-for modes' creatures (2 sounds per kind)",
        "fix": "Reuse the same sounds on doors, buttons and ambient_generics; a creature type brings all its sounds.", "top": ids_rows(&p["soundRows"])}));
    let warnings: Vec<Value> = bars.iter().filter(|b| b["status"] == "red").map(|b| json!(format!("{}: {} of {} ({}%)", b["name"].as_str().unwrap_or(""), b["used"], b["max"], b["pct"]))).collect();
    Ok(json!({
        "map": r["map"], "revision": r["revision"], "compiled": r["compile"].is_object(), "lastCompileAt": r["compile"]["when"],
        "bars": bars, "warnings": warnings,
        "modes": mode,
        "headroom": {"models": (max as i64 - RESERVE.0 as i64 - pm as i64).max(0), "sounds": (max as i64 - RESERVE.1 as i64 - ps as i64).max(0), "note": "after the creature guard's reserve"},
        "legend": "green under 75%, yellow over 75%, red over 90% (Playtest and Publish ask first)"
    }))
}

// ---- faces -----------------------------------------------------------------------------------

fn facing(n: &Value) -> &'static str {
    let c = |i: usize| n[i].as_f64().unwrap_or(0.0);
    let (x, y, z) = (c(0), c(1), c(2));
    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
    if az >= ax && az >= ay {
        if z >= 0.0 { "up" } else { "down" }
    } else if ay >= ax {
        if y >= 0.0 { "north" } else { "south" }
    } else if x >= 0.0 {
        "east"
    } else {
        "west"
    }
}

async fn faces_of(ctx: &Ctx, a: &Value) -> Done {
    let want: Vec<u64> = a["brushes"].as_array().into_iter().flatten().filter_map(Value::as_u64).take(200).collect();
    if want.is_empty() {
        return Err(err("brushes is a list of brush ids (editor_state lists them)."));
    }
    let v = get(ctx, "/api/tools/editor/meshes").await?;
    let mut out = vec![];
    for b in v["brushes"].as_array().into_iter().flatten().filter(|b| b["id"].as_u64().is_some_and(|i| want.contains(&i))) {
        let faces: Vec<Value> = b["faces"].as_array().into_iter().flatten().map(|f| {
            let al = &f["align"];
            let n = |i: usize| al[i].clone();
            json!({"face": f["face"], "texture": f["texture"], "normal": f["normal"], "facing": facing(&f["normal"]),
                "align": {"uAxis": [n(0), n(1), n(2)], "shiftU": n(3), "vAxis": [n(4), n(5), n(6)], "shiftV": n(7), "rotation": n(8), "scaleU": n(9), "scaleV": n(10)}})
        }).collect();
        out.push(json!({"brush": b["id"], "entity": b["entity"], "group": b["group"], "faces": faces}));
    }
    let missing: Vec<u64> = want.iter().filter(|i| !out.iter().any(|b| b["brush"].as_u64() == Some(**i))).copied().collect();
    Ok(json!({"brushes": out, "notFound": missing}))
}

// ---- running them ----------------------------------------------------------------------------

pub async fn run(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "editor_findings" => async {
            let (v, all, _) = findings(ctx, a).await?;
            let mut counts = json!({"error": 0, "warning": 0, "info": 0});
            let mut by_kind = Map::new();
            for f in &all {
                if let Some(l) = f["severity"].as_str() {
                    counts[l] = json!(counts[l].as_u64().unwrap_or(0) + 1);
                }
                let k = f["kind"].as_str().unwrap_or("").to_string();
                let n = by_kind.get(&k).and_then(Value::as_u64).unwrap_or(0) + 1;
                by_kind.insert(k, json!(n));
            }
            let sev = s(a, "severity");
            let kind = s(a, "kind");
            let n = super::catalog::limit(a, 200, 500);
            let shown: Vec<Value> = all.into_iter().filter(|f| sev.is_none_or(|x| f["severity"] == x) && kind.is_none_or(|x| f["kind"] == x)).collect();
            let total = shown.len();
            Ok(json!({"docId": v["docId"], "revision": v["revision"], "counts": counts, "byKind": by_kind, "matched": total, "findings": shown.into_iter().take(n).collect::<Vec<_>>(),
                "texturesChecked": v["texturesChecked"], "stats": v["stats"], "next": "Apply a fix with editor_apply_fix {fixId}."}))
        }
        .await,
        "editor_io" => async {
            let q = match a["entity"].as_u64() {
                Some(e) => format!("/api/tools/editor/io?entity={e}"),
                None => "/api/tools/editor/io".to_string(),
            };
            let mut v = get(ctx, &q).await?;
            let n = super::catalog::limit(a, 500, 5000);
            let chain: Option<Vec<u64>> = v["chain"].is_object().then(|| {
                let mut c: Vec<u64> = ["down", "up"].iter().flat_map(|k| v["chain"][k].as_array().cloned().unwrap_or_default()).filter_map(|x| x.as_u64()).collect();
                c.extend(a["entity"].as_u64());
                c
            });
            let keep = |l: &Value| {
                let from = l["from"].as_u64().unwrap_or(0);
                let to: Vec<u64> = l["to"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
                chain.as_ref().is_none_or(|c| c.contains(&from) && (to.is_empty() || to.iter().any(|t| c.contains(t))))
            };
            let src = if a["broken_only"] == true { "broken" } else { "links" };
            let links: Vec<Value> = v[src].as_array().into_iter().flatten().filter(|l| keep(l)).cloned().collect();
            let matched = links.len();
            v["links"] = json!(links.into_iter().take(n).collect::<Vec<_>>());
            v["matched"] = json!(matched);
            if let Some(c) = &chain {
                let nodes: Vec<Value> = v["nodes"].as_array().into_iter().flatten().filter(|x| x["entity"].as_u64().is_some_and(|e| c.contains(&e))).cloned().collect();
                v["nodes"] = json!(nodes);
            }
            Ok(v)
        }
        .await,
        "editor_leak" => async {
            let mut v = get(ctx, "/api/tools/editor/leak").await?;
            if v["leaked"] == true {
                let all: Vec<Value> = v["points"].as_array().cloned().unwrap_or_default();
                let n = a["points"].as_u64().unwrap_or(200).min(4000) as usize;
                // thinned evenly, the first and last (and the hole) always kept
                let hole = v["hole"]["index"].as_u64().map(|i| i as usize);
                let shown: Vec<Value> = if n == 0 {
                    vec![]
                } else if all.len() <= n {
                    all.clone()
                } else {
                    let step = all.len().div_ceil(n.max(2) - 1);
                    all.iter().enumerate().filter(|(i, _)| i % step == 0 || *i == all.len() - 1 || Some(*i) == hole).map(|(_, p)| p.clone()).collect()
                };
                v["pointCount"] = json!(all.len());
                v["points"] = json!(shown);
                v["next"] = json!("Look at the brushes in nearest (editor_faces, editor_find touching) and close the gap with a world brush; compile_start checks it again.");
            }
            Ok(v)
        }
        .await,
        "editor_apply_fix" => async {
            let mut want: Vec<String> = a["fixIds"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
            if let Some(f) = s(a, "fixId") {
                want.push(f.to_string());
            }
            if want.is_empty() {
                return Err(err("Say which fix: fixId (or fixIds) from editor_findings."));
            }
            let (_, _, reqs) = findings(ctx, a).await?;
            let mut batch = vec![];
            let mut placed = vec![];
            let mut unknown = vec![];
            for id in &want {
                let Some((_, r)) = reqs.iter().find(|(f, _)| f == id) else {
                    unknown.push(id.clone());
                    continue;
                };
                let fix = r["fix"].as_str().unwrap_or("");
                if let Some(p) = fix.strip_prefix("place-marker:").or_else(|| fix.strip_prefix("place:")) {
                    let origin = match a.get("origin") {
                        Some(_) => super::catalog::vec3_arg(a, "origin")?,
                        None => r.get("at").cloned().filter(Value::is_array).ok_or_else(|| err("This fix places something: say where with origin [x, y, z]."))?,
                    };
                    let done = if fix.starts_with("place-marker:") {
                        let p = p.strip_prefix("kit:").unwrap_or(p);
                        let (mode, marker) = p.split_once(':').ok_or_else(|| err("That fix can't be placed."))?;
                        place_marker(ctx, mode, marker, origin).await?
                    } else {
                        cmd(ctx, json!({"op": "add_entity", "classname": p, "origin": origin})).await?
                    };
                    placed.push(json!({"fixId": id, "placed": p, "created": done["created"]}));
                    continue;
                }
                let mut r = r.clone();
                if let Some(o) = r.as_object_mut() {
                    o.remove("at");
                }
                batch.push(r);
            }
            if !unknown.is_empty() && batch.is_empty() && placed.is_empty() {
                return Err(err(format!("No such fix now: {}. The map may have changed; read editor_findings again.", unknown.join(", "))));
            }
            let mut out = json!({"placed": placed, "unknown": unknown});
            if !batch.is_empty() {
                let mut body = json!({"op": "fix", "fixes": batch});
                if let Some(t) = s(a, "texture") {
                    body["with"] = json!(t);
                }
                let v = cmd(ctx, body).await?;
                out["fixed"] = v["note"].clone();
                out["changed"] = v["changed"].clone();
            }
            let (_, left, _) = findings(ctx, a).await?;
            out["left"] = json!(left.len());
            out["leftErrors"] = json!(left.iter().filter(|f| f["severity"] == "error").count());
            Ok(out)
        }
        .await,
        "editor_budget" => budget(ctx).await,
        "editor_faces" => faces_of(ctx, a).await,
        "editor_history" => get(ctx, "/api/tools/editor/history").await,
        "editor_history_jump" => async {
            let n = a["steps"].as_i64().filter(|n| *n != 0).ok_or_else(|| err("steps is a whole number, not 0."))?;
            post(ctx, "/api/tools/editor/history/jump", json!({"steps": n})).await
        }
        .await,
        "editor_find" => async {
            let by = s(a, "by").unwrap_or("");
            let mut sel = pick(a, &["brushes", "entities"]);
            let v = match by {
                "text" => {
                    let mut spec = pick(a, &["field", "find", "key", "whole"]);
                    spec.extend(sel);
                    cmd2(ctx, json!({"op": "find", "spec": spec})).await?
                }
                "texture" => cmd2(ctx, json!({"op": "similar", "brushes": a.get("brushes").cloned().unwrap_or(json!([])), "entities": [], "similar": {"by": "texture", "textures": a.get("textures").cloned().unwrap_or(json!([]))}})).await?,
                "class" => {
                    sel.insert("op".into(), json!("similar"));
                    sel.insert("similar".into(), json!({"by": "class"}));
                    cmd2(ctx, Value::Object(sel)).await?
                }
                "value" => {
                    let k = s(a, "key").ok_or_else(|| err("by value needs key."))?;
                    sel.insert("op".into(), json!("similar"));
                    sel.insert("similar".into(), json!({"by": "value", "key": k}));
                    cmd2(ctx, Value::Object(sel)).await?
                }
                "touching" | "inside" => {
                    sel.insert("op".into(), json!("touching"));
                    sel.insert("inside".into(), json!(by == "inside"));
                    cmd2(ctx, Value::Object(sel)).await?
                }
                _ => return Err(err("by is text, texture, class, value, touching or inside.")),
            };
            Ok(if v.get("found").is_some() { v["found"].clone() } else { v["select"].clone() })
        }
        .await,
        "editor_kits" => async {
            let v = get(ctx, "/api/tools/editor/kits").await?;
            let kits: Vec<Value> = v["kits"].as_array().into_iter().flatten().map(|k| json!({"mode": k["mode"], "markers": k["markers"].as_array().into_iter().flatten().map(|m| json!({"id": m["id"], "label": m["label"], "description": m["description"], "type": m["type"], "required": m["required"], "keys": m["keys"]})).collect::<Vec<_>>(), "components": k["components"], "creatures": k["creatures"], "map": k["map"]})).collect();
            Ok(json!({"kits": kits, "madeFor": v["madeFor"], "needs": v["needs"], "bad": v["bad"], "key": v["key"]}))
        }
        .await,
        "content_list" => async {
            let kind = s(a, "kind").filter(|k| ["model", "sprite", "sound"].contains(k)).ok_or_else(|| err("kind is model, sprite or sound (textures: editor_textures)."))?;
            let v = get(ctx, &format!("/api/tools/editor/content/list?kind={kind}")).await?;
            let words: Vec<String> = s(a, "search").unwrap_or("").to_lowercase().split_whitespace().map(String::from).collect();
            let src = s(a, "source");
            let all: Vec<Value> = v["items"].as_array().into_iter().flatten().filter(|i| {
                let p = i["path"].as_str().unwrap_or("").to_lowercase();
                words.iter().all(|w| p.contains(w.as_str())) && src.is_none_or(|x| i["source"].as_str() == Some(x))
            }).cloned().collect();
            let total = all.len();
            Ok(json!({"game": v["game"], "matched": total, "items": all.into_iter().take(super::catalog::limit(a, 100, 500)).collect::<Vec<_>>()}))
        }
        .await,
        "content_credits" => async {
            let f = s(a, "folder").filter(|f| !f.is_empty() && f.len() <= 64 && f.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))).ok_or_else(|| err("folder is the item's folder from content_list."))?;
            let p = s(a, "path").filter(|p| !p.is_empty() && p.len() <= 260 && !p.contains("..") && !p.starts_with('/') && !p.contains('\\')).ok_or_else(|| err("path is the item's path from content_list."))?;
            get(ctx, &format!("/api/tools/credits/asset?folder={}&path={}", qs(f), qs(p))).await
        }
        .await,
        "editor_assets" => get(ctx, "/api/tools/editor/assets").await,
        "prefabs_list" => async {
            let v = get(ctx, "/api/tools/editor/prefabs").await?;
            let words: Vec<String> = s(a, "search").unwrap_or("").to_lowercase().split_whitespace().map(String::from).collect();
            let keep = |p: &Value| {
                let t = p.to_string().to_lowercase();
                words.iter().all(|w| t.contains(w.as_str()))
            };
            let mut out = v.clone();
            for k in ["prefabs", "items", "starter", "mine", "community"] {
                if let Some(list) = out.get_mut(k).and_then(Value::as_array_mut) {
                    list.retain(&keep);
                    for p in list.iter_mut() {
                        if let Some(o) = p.as_object_mut() {
                            o.remove("thumb");
                            o.remove("text");
                        }
                    }
                }
            }
            Ok(out)
        }
        .await,

        "editor_add_shape" => async {
            let shape = s(a, "shape").unwrap_or("box");
            let (min, max) = (super::catalog::vec3_arg(a, "min")?, super::catalog::vec3_arg(a, "max")?);
            let tex = s(a, "texture").unwrap_or("OUT_WALL5");
            match shape {
                "box" => cmd(ctx, json!({"op": "add_box", "min": min, "max": max, "texture": tex})).await,
                "cylinder" => {
                    let (lo, hi) = (|i: usize| min[i].as_f64().unwrap_or(0.0).min(max[i].as_f64().unwrap_or(0.0)), |i: usize| min[i].as_f64().unwrap_or(0.0).max(max[i].as_f64().unwrap_or(0.0)));
                    let r = ((hi(0) - lo(0)).min(hi(1) - lo(1))) / 2.0;
                    let mut b = json!({"op": "add_cylinder", "base": [(lo(0) + hi(0)) / 2.0, (lo(1) + hi(1)) / 2.0, lo(2)], "radius": r, "height": hi(2) - lo(2), "sides": a["sides"].as_u64().unwrap_or(12), "texture": tex});
                    if let Some(g) = a["grid"].as_f64() {
                        b["grid"] = json!(g);
                    }
                    cmd(ctx, b).await
                }
                _ => {
                    let mut b = op("add_shape", a, &["shape", "sides", "grid", "arc", "start", "wall", "add_height", "top", "tube_sides"]);
                    b["min"] = min;
                    b["max"] = max;
                    b["texture"] = json!(tex);
                    cmd(ctx, b).await
                }
            }
        }
        .await,
        "editor_transform" => cmd(ctx, op("transform", a, &["brushes", "entities", "translate", "rotate", "scale", "pivot", "lock", "keep_scale"])).await,
        "editor_move_face" => cmd(ctx, op("move_face", a, &["brush", "face", "distance", "lock", "drag"])).await,
        "editor_extrude_face" => cmd(ctx, op("extrude_face", a, &["brush", "face", "distance"])).await,
        "editor_vertices" => async {
            match s(a, "action") {
                Some("move") => cmd(ctx, op("move_vertices", a, &["picks", "delta"])).await,
                Some("split") => {
                    let mut b = op("add_vertex", a, &["brush", "at", "delta"]);
                    if b.get("delta").is_none() {
                        b["delta"] = json!([0, 0, 0]);
                    }
                    cmd(ctx, b).await
                }
                Some("merge") => cmd(ctx, op("merge_vertices", a, &["brush", "picks"])).await,
                _ => Err(err("action is move, split or merge.")),
            }
        }
        .await,
        "editor_clip" => cmd(ctx, op("clip", a, &["brushes", "points", "keep", "texture"])).await,
        "editor_hollow" => cmd(ctx, op("hollow", a, &["brushes", "thickness"])).await,
        "editor_carve" => async {
            let mut b = op("carve", a, &["cutter", "targets"]);
            if b.get("targets").is_none() {
                b["targets"] = json!([]);
            }
            cmd(ctx, b).await
        }
        .await,
        "editor_merge" => cmd(ctx, op("merge", a, &["brushes"])).await,
        "editor_snap" => cmd(ctx, op("snap", a, &["brushes", "grid"])).await,
        "editor_set_texture" => cmd(ctx, op("set_texture", a, &["faces", "texture"])).await,
        "editor_align_texture" => async {
            let mut b = op("align", a, &["faces", "action", "u", "v", "degrees", "mode", "one", "right", "up", "drag"]);
            for (from, to) in [("shift_u", "ou"), ("shift_v", "ov"), ("scale_u", "su"), ("scale_v", "sv"), ("rotation", "rot")] {
                if let Some(x) = a.get(from) {
                    b[to] = x.clone();
                }
            }
            cmd(ctx, b).await
        }
        .await,
        "editor_wrap_texture" => cmd(ctx, op("wrap_texture", a, &["from", "faces", "texture"])).await,
        "editor_delete" => cmd(ctx, op("delete", a, &["brushes", "entities"])).await,
        "editor_duplicate" => async {
            let mut b = op("duplicate", a, &["brushes", "entities", "offset", "rename"]);
            if b.get("offset").is_none() {
                b["offset"] = json!([16, 16, 0]);
            }
            cmd2(ctx, b).await
        }
        .await,
        "editor_array" => cmd(ctx, op("array", a, &["brushes", "entities", "copies", "offset", "rotate", "scale", "pivot", "lock"])).await,
        "editor_paste" => cmd(ctx, op("paste", a, &["text", "offset"])).await,
        "editor_group" => async {
            let g = a["group"].as_u64();
            let need = || g.ok_or_else(|| err("Say which group (its id from editor_state)."));
            match s(a, "action") {
                Some("group") => {
                    let mut b = op("group", a, &["brushes", "entities"]);
                    b["name"] = json!(s(a, "name").unwrap_or("Group"));
                    let mut v = cmd(ctx, b).await?;
                    v["group"] = v["note"].as_str().and_then(|n| n.parse::<u64>().ok()).map(Value::from).unwrap_or(Value::Null);
                    Ok(v)
                }
                Some("ungroup") => cmd(ctx, json!({"op": "ungroup", "group": need()?})).await,
                Some(x @ ("hide" | "show")) => cmd(ctx, json!({"op": "group_flags", "group": need()?, "hidden": x == "hide"})).await,
                Some(x @ ("lock" | "unlock")) => cmd(ctx, json!({"op": "group_flags", "group": need()?, "locked": x == "lock"})).await,
                Some("rename") => cmd2(ctx, json!({"op": "rename_group", "group": need()?, "name": s(a, "name").unwrap_or("Group")})).await,
                Some("move") => {
                    let mut b = op("move_to_group", a, &["brushes", "entities", "groups"]);
                    b["group"] = json!(g);
                    cmd2(ctx, b).await
                }
                _ => Err(err("action is group, ungroup, hide, show, lock, unlock, rename or move.")),
            }
        }
        .await,
        "editor_tie" => async {
            match a["to_world"].as_u64() {
                Some(e) => cmd(ctx, json!({"op": "to_world", "entity": e})).await,
                None => cmd(ctx, op("tie", a, &["brushes", "classname"])).await,
            }
        }
        .await,
        "editor_world" => async {
            let mut b = json!({"op": "set_world"});
            if let Some(w) = a.get("wads").filter(|w| w.is_array()) {
                b["wads"] = w.clone();
            }
            match a.get("set").and_then(Value::as_object) {
                Some(set) => set_world_keys(ctx, b, set).await,
                None => cmd(ctx, b).await,
            }
        }
        .await,
        "editor_set_keys" => cmd2(ctx, op("set_keys", a, &["entities", "key", "value"])).await,
        "editor_replace" => async {
            let spec = pick(a, &["field", "find", "replace", "key", "whole", "brushes", "entities"]);
            cmd2(ctx, json!({"op": "replace", "spec": spec})).await
        }
        .await,
        "editor_arrange" => async {
            match s(a, "action") {
                Some("align") => {
                    let mut b = op("align", a, &["brushes", "entities", "axis", "lock"]);
                    b["edge"] = json!(s(a, "edge").unwrap_or("min"));
                    cmd2(ctx, b).await
                }
                Some("distribute") => cmd2(ctx, op("distribute", a, &["brushes", "entities", "axis", "lock"])).await,
                _ => Err(err("action is align or distribute.")),
            }
        }
        .await,
        "editor_settings" => async {
            let mut b = json!({});
            if let Some(v) = a.get("visgroups").filter(|v| v.is_array()) {
                b["visSets"] = v.clone();
            }
            if let Some(c) = a.get("cordon").filter(|c| c.is_object()) {
                b["cordon"] = c.clone();
            }
            if b.as_object().is_some_and(Map::is_empty) {
                return Err(err("Give visgroups and/or cordon."));
            }
            let v = post(ctx, "/api/tools/editor/settings", b).await?;
            Ok(json!({"editor": v["editor"], "state": v["state"]}))
        }
        .await,
        "editor_place_asset" => async {
            let kind = s(a, "kind").unwrap_or("");
            let path = s(a, "path").filter(|p| !p.is_empty() && p.len() <= 260 && !p.contains(['"', '\n', '\\'])).ok_or_else(|| err("path is the file's path from content_list."))?;
            let origin = super::catalog::vec3_arg(a, "origin")?;
            let (class, mut keys): (&str, Vec<(String, String)>) = match kind {
                "model" => ("cycler_sprite", vec![("model".into(), path.into()), ("framerate".into(), "1".into()), ("renderamt".into(), "255".into())]),
                "sprite" => ("env_sprite", vec![("model".into(), path.into()), ("rendermode".into(), "5".into()), ("renderamt".into(), "255".into()), ("scale".into(), "1".into()), ("framerate".into(), "10".into()), ("spawnflags".into(), "1".into())]),
                "sound" => {
                    let v = if path.len() > 6 && path[..6].eq_ignore_ascii_case("sound/") { &path[6..] } else { path };
                    ("ambient_generic", vec![("message".into(), v.into()), ("health".into(), "10".into()), ("spawnflags".into(), "4".into())])
                }
                _ => return Err(err("kind is model, sprite or sound.")),
            };
            for (k, v) in a["keys"].as_object().into_iter().flatten() {
                let v = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                keys.retain(|(x, _)| x != k);
                keys.push((k.clone(), v));
            }
            let keys: Vec<Value> = keys.into_iter().map(|(k, v)| json!([k, v])).collect();
            cmd(ctx, json!({"op": "add_entity", "classname": class, "origin": origin, "keys": keys})).await
        }
        .await,
        "editor_place_prefab" => cmd(ctx, op("place_prefab", a, &["prefab", "point", "normal", "yaw", "grid"])).await,
        "prefab_save" => async {
            if a.get("replace").is_some_and(|r| !r.is_null()) {
                ask(ctx, format!("An AI agent wants to save a prefab over your prefab \"{}\" in the library. Allow it?", s(a, "replace").unwrap_or(""))).await?;
            }
            post(ctx, "/api/tools/editor/prefabs/save", Value::Object(pick(a, &["name", "brushes", "entities", "tags", "description", "replace"]))).await
        }
        .await,
        "editor_place_marker" => async {
            let mode = s(a, "mode").ok_or_else(|| err("Say the mode (editor_kits)."))?;
            let marker = s(a, "marker").ok_or_else(|| err("Say the marker."))?;
            place_marker(ctx, mode, marker, super::catalog::vec3_arg(a, "origin")?).await
        }
        .await,
        "editor_made_for" => async {
            let kits = get(ctx, "/api/tools/editor/kits").await?;
            let key = kits["key"].as_str().unwrap_or("ls_modes").to_string();
            let mut parts = vec![];
            for m in a["modes"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                if m.is_empty() || m.len() > 32 || !m.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
                    return Err(err(format!("{m} isn't a mode id.")));
                }
                let uid = kits["kits"].as_array().into_iter().flatten().find(|k| k["mode"]["id"].as_str() == Some(m)).and_then(|k| k["mode"]["uid"].as_str());
                parts.push(match uid {
                    Some(u) => format!("{u}:{m}"),
                    None => m.to_string(),
                });
            }
            let mut set = Map::new();
            set.insert(key, json!(parts.join(",")));
            set_world_keys(ctx, json!({"op": "set_world"}), &set).await
        }
        .await,
        "editor_copy_assets" => async {
            let to = s(a, "to").filter(|t| ["project", "game"].contains(t)).ok_or_else(|| err("to is project or game."))?;
            if to == "game" {
                ask(ctx, "An AI agent wants to copy the map's models, sprites and sounds into your game's download folder. Allow it?".into()).await?;
            }
            post(ctx, "/api/tools/editor/assets/copy", json!({"to": to})).await
        }
        .await,
        _ => return None,
    })
}

/// worldspawn keys merged into the ones there (an empty value removes one), with `b`'s WADs.
async fn set_world_keys(ctx: &Ctx, mut b: Value, set: &Map<String, Value>) -> Done {
    let doc = get(ctx, "/api/tools/editor/doc").await?;
    let mut keys: Vec<(String, String)> = doc["world"]["keys"].as_array().into_iter().flatten().filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_str()?.to_string()))).collect();
    for (k, v) in set {
        let v = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
        keys.retain(|(x, _)| x != k);
        if !v.is_empty() {
            keys.push((k.clone(), v));
        }
    }
    b["keys"] = json!(keys.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>());
    cmd(ctx, b).await
}

/// A query-string value.
fn qs(v: &str) -> String {
    v.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_./".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
