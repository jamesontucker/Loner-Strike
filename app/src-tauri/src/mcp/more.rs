//! More tools over the app's own routes: Game Mode Studio (graphs, code, compile, playtest,
//! the debugger), Plugin Studio, map components, and the Make / Share tools. Same rules as
//! catalog.rs: in-process calls, read-only unless changes are allowed, asking first before
//! anything that overwrites, deletes, publishes or starts the game.

use super::catalog::{ask, err, get, id_arg, limit, map_arg, map_prop, none, obj, people_on, pick, plural, post, s, Done, Fail, Kind, Spec};
use super::Ctx;
use serde_json::{json, Value};

fn graph_id() -> Value {
    json!({"type": "string", "description": "Mode id: 2-24 characters, a-z, 0-9 and _, starting with a letter"})
}
fn folder() -> Value {
    json!({"type": "string", "enum": ["mine", "game", "include"], "description": "mine: My plugins; game: the game's scripting folder; include: .inc files"})
}
fn file_name() -> Value {
    json!({"type": "string", "description": "e.g. my_mode.sma"})
}
fn plugin_file() -> Value {
    obj(json!({"folder": folder(), "name": file_name()}), &["folder", "name"])
}

pub fn specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        // Game Mode Studio
        Spec { name: "studio_status", title: "Mode Studio: status", kind: Read, schema: none,
            description: "Whether the Game Mode Studio can compile (the game and its compiler are here) and where graphs are kept." },
        Spec { name: "studio_nodes", title: "Mode Studio: nodes", kind: Read,
            schema: || obj(json!({"search": {"type": "string", "description": "Words in a node's id, name or category"}, "category": {"type": "string"}}), &[]),
            description: "The Studio's node catalog (events, actions, values, flow) with pins and types: what a graph can be built from. The whole file is the resource loner://docs/studio-nodes." },
        Spec { name: "studio_graphs", title: "Mode Studio: graphs", kind: Read, schema: none,
            description: "The person's game mode graphs: id, name, description, node count, installed." },
        Spec { name: "studio_graph_read", title: "Mode Studio: read a graph", kind: Read, schema: || obj(json!({"id": graph_id()}), &["id"]),
            description: "One game mode graph (nodes, wires, variables, mode settings) as JSON." },
        Spec { name: "studio_graph_save", title: "Mode Studio: save a graph", kind: Risky,
            schema: || obj(json!({"id": graph_id(), "graph": {"type": "object", "description": "The whole graph; graph.mode.id must equal id"}}), &["id", "graph"]),
            description: "Saves a game mode graph. Asks the person first when it replaces one that's there." },
        Spec { name: "studio_graph_delete", title: "Mode Studio: delete a graph", kind: Risky, schema: || obj(json!({"id": graph_id()}), &["id"]),
            description: "Deletes a game mode graph file. Asks the person first." },
        Spec { name: "studio_validate", title: "Mode Studio: check a graph", kind: Read,
            schema: || obj(json!({"id": graph_id(), "graph": {"type": "object", "description": "A graph to check instead of the saved one"}}), &[]),
            description: "Problems in a graph (unwired pins, wrong types, missing events...) with the node and pin they're on." },
        Spec { name: "studio_code", title: "Mode Studio: show the code", kind: Read,
            schema: || obj(json!({"id": graph_id(), "graph": {"type": "object"}, "debug": {"type": "boolean"}, "offset": {"type": "integer", "minimum": 0, "description": "Character to start from (long plugins come in pages)"}}), &[]),
            description: "The AMX Mod X plugin source the Studio makes from a graph, with which lines came from which node." },
        Spec { name: "studio_compile", title: "Mode Studio: compile and install", kind: Change,
            schema: || obj(json!({"id": graph_id(), "debug": {"type": "boolean", "description": "Build with the debugger hooks"}}), &["id"]),
            description: "Compiles a saved graph into a plugin and installs it as a game mode in the person's game (its own plugin, plugins list and modes.ini line; only modes the Studio made)." },
        Spec { name: "studio_playtest", title: "Mode Studio: playtest", kind: Risky,
            schema: || obj(json!({"id": graph_id(), "map": map_prop(), "bots": {"type": "integer", "minimum": 0, "maximum": 31}, "breakpoints": {"type": "array", "items": {"type": "string"}, "description": "Node ids to stop at"}}), &["id", "map"]),
            description: "Compiles the graph with the debugger, then starts the game on a map in that mode with bots (asks the person first). Then read studio_debug_events and use studio_debug." },
        Spec { name: "studio_uninstall", title: "Mode Studio: take a mode out", kind: Risky, schema: || obj(json!({"id": graph_id()}), &["id"]),
            description: "Removes a Studio mode from the game (its plugin, plugins list and modes.ini line; the graph is kept). Asks the person first." },
        Spec { name: "studio_debug_events", title: "Mode Studio: debugger events", kind: Read,
            schema: || obj(json!({"mode": graph_id(), "from": {"type": "integer", "minimum": 0, "description": "The last answer's next (start at 0)"}, "kinds": {"type": "array", "items": {"type": "string"}, "description": "Only these: session, node_enter, node_exit, node_error, breakpoint_hit, world_paused, world_resumed, vars, var_player"}}), &["mode"]),
            description: "What a playtested mode did: nodes entered and left with their inputs and results, errors, breakpoint hits, paused/resumed, live variables (round, phase, time left, alive per team, players). Page with from/next; reset: true means a new map started the log again." },
        Spec { name: "studio_debug", title: "Mode Studio: debugger", kind: Change,
            schema: || obj(json!({"mode": graph_id(), "breakpoints": {"type": "array", "items": {"type": "string"}, "description": "The whole set of node ids to stop at ([] clears them all; leave out to keep)"}, "vars": {"type": "boolean", "description": "Ask for the live variables (they arrive as vars / var_player events)"}, "resume": {"type": "boolean", "description": "Carry on from a breakpoint"}}), &["mode"]),
            description: "Controls the running playtest: set or clear breakpoints, ask for live variables, resume from a breakpoint." },
        // Plugin Studio
        Spec { name: "plugins_list", title: "Plugin Studio: files", kind: Read, schema: none,
            description: "AMX Mod X sources: My plugins, the game's scripting folder and includes (name, size, read-only), whether the compiler is ready and which plugin is being tried." },
        Spec { name: "plugin_read", title: "Plugin Studio: read a file", kind: Read,
            schema: || obj(json!({"folder": folder(), "name": file_name(), "offset": {"type": "integer", "minimum": 0}}), &["folder", "name"]),
            description: "A .sma or .inc file's text (long files come in pages)." },
        Spec { name: "plugin_save", title: "Plugin Studio: save a file", kind: Risky,
            schema: || obj(json!({"folder": folder(), "name": file_name(), "text": {"type": "string"}}), &["folder", "name", "text"]),
            description: "Saves a .sma or .inc (new files in My plugins are made). Asks the person first when it replaces a file that's there. Loner-Strike's own files are read-only." },
        Spec { name: "plugin_delete", title: "Plugin Studio: delete a file", kind: Risky, schema: plugin_file,
            description: "Deletes a source file. Asks the person first." },
        Spec { name: "plugin_compile", title: "Plugin Studio: compile", kind: Change, schema: plugin_file,
            description: "Compiles a plugin with the game's amxxpc into the Studio's build folder: errors and warnings with line numbers, in plain words." },
        Spec { name: "plugin_try", title: "Plugin Studio: try it", kind: Risky, schema: plugin_file,
            description: "Compiles and loads the plugin in the person's game; a running map restarts so it's in (asks first if people might be playing). Read plugin_log for what it prints." },
        Spec { name: "plugin_untry", title: "Plugin Studio: take it out", kind: Risky,
            schema: || obj(json!({"name": file_name(), "restart": {"type": "boolean", "description": "Restart the map now so it's gone straight away"}}), &["name"]),
            description: "Takes the tried plugin out of the game (restarting the map asks first if people might be playing)." },
        Spec { name: "plugin_log", title: "Plugin Studio: live log", kind: Read,
            schema: || obj(json!({"name": file_name(), "cursor": {"type": "string", "description": "The last answer's cursor (leave out for the last lines)"}}), &["name"]),
            description: "Log lines from the plugin being tried (its messages and run-time errors), with a cursor to follow it." },
        // components
        Spec { name: "components_library", title: "Components: library", kind: Read,
            schema: || obj(json!({"search": {"type": "string"}}), &[]),
            description: "Game mode components that can be placed on a map (capture points, doors, traps, checkpoints...): class, keys, what bots do. Full docs: resource loner://docs/components." },
        Spec { name: "components_pack", title: "Components: the map's pack", kind: Read, schema: map_only_editor,
            description: "The component file the map editor would write for the open map (the ls_* entities as a component pack)." },
        Spec { name: "components_save_pack", title: "Components: write the file", kind: Risky,
            schema: || obj(json!({"map": map_prop(), "mode": {"type": "string", "description": "Only for this mode (map.mode.ini)"}}), &["map"]),
            description: "Writes the open map's components to the game's configs/components/<map>[.<mode>].ini (an old one is kept as .bak). Asks the person first." },
        // the safety net, radar, demos (everything else of theirs: app_call)
        Spec { name: "backups_list", title: "Safety net: changed files", kind: Read,
            schema: || obj(json!({"match": {"type": "string", "description": "Path patterns, comma-separated (e.g. *.nav,people.ini)"}}), &[]),
            description: "Every game file Loner-Strike's tools changed, with its kept versions (when, which tool, why). Compare with app_call GET /api/safety/version." },
        Spec { name: "backup_restore", title: "Safety net: put a version back", kind: Risky,
            schema: || obj(json!({"id": {"type": "string"}, "seq": {"type": "integer"}}), &["id", "seq"]),
            description: "Puts an earlier version of a game file back (the current one is kept first, so it can be undone). Asks the person first." },
        Spec { name: "radar_preview", title: "Radar: preview", kind: Read,
            schema: || obj(json!({"map": map_prop(), "style": {"type": "string", "enum": ["game", "clean", "simple"]}, "roofs": {"type": "string", "enum": ["nav", "cut"]}, "orient": {"type": "string", "enum": ["auto", "north", "turned"]}, "background": {"type": "string", "enum": ["green", "black"]}}), &["map"]),
            description: "A picture of the radar (overview) the radar maker would make for a map. Install with app_call POST /api/tools/overview/<map>/install (asks nothing; old files are kept as .bak)." },
        Spec { name: "demos_list", title: "Demos", kind: Read,
            schema: || obj(json!({"limit": {"type": "integer", "minimum": 1, "maximum": 500}}), &[]),
            description: "Recorded demos in the game and servers (map, length, playable). Details, watching (asks), turning one into a replay: app_call /api/tools/demos/*." },
        // Creature Studio
        Spec { name: "creature_types", title: "Creatures: types", kind: Read, schema: none,
            description: "The creature types the game has from creatures.ini (built-ins and the person's own): name, shown name, category, a one-line summary of how it plays, and problems. Format: resource loner://docs/creatures." },
        Spec { name: "creature_type_read", title: "Creatures: read a type", kind: Read, schema: || obj(json!({"name": creature_name()}), &["name"]),
            description: "One creature type with every value the game uses (hp, speeds, senses, attack, behaviour, model candidates, sounds...) and which keys the file sets." },
        Spec { name: "creature_type_save", title: "Creatures: save a type", kind: Change,
            schema: || obj(json!({"type": {"type": "object", "description": "name (required) and the values to set, keys as creature_type_read gives them; a new name starts from base (a type above, e.g. zombie) or the defaults"}, "remove": {"type": "boolean", "description": "Take this type's section out of the file (a built-in goes back to how the game has it)"}}), &["type"]),
            description: "Saves one creature type into the game's creatures.ini (checked against the game's limits first; the old file is kept in History). Then creature_apply to use it in the running map." },
        Spec { name: "creature_apply", title: "Creatures: apply to the game", kind: Change,
            schema: || obj(json!({"spawn": creature_name(), "calm": {"type": "boolean", "description": "The spawned one stands still (behaviour idle)"}}), &[]),
            description: "Makes the person's running game read creatures.ini again (ls_creature reload) and, with spawn, puts one of that type in front of them (or at a spawn) to try. Only types loaded with the map can be spawned." },
        Spec { name: "components_load_pack", title: "Components: load into the editor", kind: Change,
            schema: || obj(json!({"map": map_prop(), "mode": {"type": "string"}}), &["map"]),
            description: "Adds a map's component pack (the person's own, else the shipped one) to the open editor map as one undoable step." },
    ]
}

fn creature_name() -> Value {
    json!({"type": "string", "description": "A creature type's name, e.g. zombie, runner, my_brute"})
}

fn map_only_editor() -> Value {
    obj(json!({"map": map_prop()}), &["map"])
}

/// Text in pages of about 40 000 characters.
fn page(text: &str, offset: usize) -> Value {
    const PAGE: usize = 40_000;
    let chars: Vec<char> = text.chars().collect();
    let start = offset.min(chars.len());
    let end = (start + PAGE).min(chars.len());
    json!({"text": chars[start..end].iter().collect::<String>(), "offset": start, "next": (end < chars.len()).then_some(end), "length": chars.len()})
}

/// The graph given, or the saved one.
async fn graph_of(ctx: &Ctx, a: &Value) -> Result<Value, Fail> {
    if let Some(g) = a.get("graph").filter(|g| g.is_object()) {
        return Ok(g.clone());
    }
    let id = id_arg(a, "id")?;
    let v = get(ctx, &format!("/api/tools/studio/graph/{id}")).await?;
    Ok(v["graph"].clone())
}

/// Asks unless the person's game is known to have nobody on (a map restart kicks nobody,
/// but it does restart the round for whoever's there).
async fn ask_if_playing(ctx: &Ctx, what: &str) -> Result<(), Fail> {
    match people_on(ctx, None).await {
        Some(0) => Ok(()),
        Some(n) => ask(ctx, format!("An AI agent wants to {what}. That restarts the map, and {} playing. Allow it?", plural(n))).await,
        None => ask(ctx, format!("An AI agent wants to {what}. If a map is running it restarts. Allow it?")).await,
    }
}

fn q(v: &str) -> String {
    v.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn plugin_args(a: &Value) -> Result<(String, String), Fail> {
    let folder = s(a, "folder").filter(|f| ["mine", "game", "include"].contains(f)).ok_or_else(|| err("folder must be mine, game or include."))?;
    let name = s(a, "name").filter(|n| !n.is_empty() && n.len() <= 64 && !n.contains(['/', '\\'])).ok_or_else(|| err("Say the file name, e.g. my_mode.sma."))?;
    Ok((folder.to_string(), name.to_string()))
}

pub async fn run(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "studio_status" => get(ctx, "/api/tools/studio/status").await,
        "studio_nodes" => Ok(studio_nodes(s(a, "search").unwrap_or(""), s(a, "category"))),
        "studio_graphs" => get(ctx, "/api/tools/studio/graphs").await,
        "studio_graph_read" => get(ctx, &format!("/api/tools/studio/graph/{}", match id_arg(a, "id") { Ok(i) => i, Err(e) => return Some(Err(e)) })).await,
        "studio_graph_save" => studio_save(ctx, a).await,
        "studio_graph_delete" => async {
            let id = id_arg(a, "id")?;
            ask(ctx, format!("An AI agent wants to delete the game mode graph \"{id}\". Allow it?")).await?;
            post(ctx, &format!("/api/tools/studio/graph/{id}/delete"), json!({})).await
        }
        .await,
        "studio_validate" => async { post(ctx, "/api/tools/studio/validate", json!({"graph": graph_of(ctx, a).await?})).await }.await,
        "studio_code" => async {
            let v = post(ctx, "/api/tools/studio/generate", json!({"graph": graph_of(ctx, a).await?, "debug": a["debug"].as_bool().unwrap_or(false)})).await?;
            match v["source"].as_str() {
                Some(src) => {
                    let mut out = page(src, a["offset"].as_u64().unwrap_or(0) as usize);
                    out["plugin"] = v["plugin"].clone();
                    out["problems"] = v["problems"].clone();
                    out["spans"] = v["spans"].clone();
                    Ok(out)
                }
                None => Ok(v),
            }
        }
        .await,
        "studio_compile" => async {
            let g = graph_of(ctx, a).await?;
            post(ctx, "/api/tools/studio/compile", json!({"graph": g, "debug": a["debug"].as_bool().unwrap_or(false)})).await
        }
        .await,
        "studio_playtest" => async {
            let id = id_arg(a, "id")?;
            let map = map_arg(a)?;
            let bots = a.get("bots").and_then(Value::as_u64).map(|n| n.min(31));
            ask(ctx, format!("An AI agent wants to start Counter-Strike on {map} in the Studio mode \"{id}\"{} with the debugger. If the game is open it's closed first. Allow it?", bots.map(|n| format!(" with {n} bots")).unwrap_or_default())).await?;
            let g = graph_of(ctx, a).await?;
            let bps = a.get("breakpoints").cloned().unwrap_or_else(|| json!([]));
            let built = post(ctx, "/api/tools/studio/playtest", json!({"graph": g, "breakpoints": bps})).await?;
            if built["ok"] != true {
                return Ok(built);
            }
            let mut b = json!({"map": map, "mode": id, "restart": true});
            if let Some(n) = bots {
                b["bots"] = json!(n);
            }
            let launched = post(ctx, "/api/app/launch", b).await?;
            Ok(json!({"built": built, "launched": launched, "next": "Read studio_debug_events with mode and from: 0."}))
        }
        .await,
        "studio_uninstall" => async {
            let id = id_arg(a, "id")?;
            ask(ctx, format!("An AI agent wants to take the game mode \"{id}\" out of your game (the graph is kept). Allow it?")).await?;
            post(ctx, &format!("/api/tools/studio/uninstall/{id}"), json!({})).await
        }
        .await,
        "studio_debug_events" => async {
            let mode = id_arg(a, "mode")?;
            let from = a["from"].as_u64().unwrap_or(0);
            let mut v = get(ctx, &format!("/api/tools/studio/debug/events?mode={mode}&from={from}")).await?;
            if let Some(kinds) = a.get("kinds").and_then(Value::as_array) {
                let kinds: Vec<&str> = kinds.iter().filter_map(Value::as_str).collect();
                if let Some(ev) = v.get_mut("events").and_then(Value::as_array_mut) {
                    ev.retain(|e| e["e"].as_str().is_some_and(|k| kinds.contains(&k)));
                }
            }
            // (a page that's too big for one answer: the newest events, and say so)
            let n = v["events"].as_array().map(Vec::len).unwrap_or(0);
            if n > 400 {
                let ev: Vec<Value> = v["events"].as_array().unwrap()[n - 400..].to_vec();
                v["skipped"] = json!(n - 400);
                v["events"] = json!(ev);
            }
            Ok(v)
        }
        .await,
        "studio_debug" => async {
            let mode = id_arg(a, "mode")?;
            let mut b = pick(a, &["resume", "vars", "breakpoints"]);
            b.insert("mode".into(), json!(mode));
            post(ctx, "/api/tools/studio/debug/command", Value::Object(b)).await
        }
        .await,

        "plugins_list" => get(ctx, "/api/tools/plugins").await,
        "plugin_read" => async {
            let (f, n) = plugin_args(a)?;
            let v = get(ctx, &format!("/api/tools/plugins/file?folder={f}&name={}", q(&n))).await?;
            let mut out = page(v["text"].as_str().unwrap_or(""), a["offset"].as_u64().unwrap_or(0) as usize);
            out["readOnly"] = v["readOnly"].clone();
            out["name"] = v["name"].clone();
            Ok(out)
        }
        .await,
        "plugin_save" => async {
            let (f, n) = plugin_args(a)?;
            let text = s(a, "text").ok_or_else(|| err("text is the whole file."))?;
            let exists = get(ctx, &format!("/api/tools/plugins/file?folder={f}&name={}", q(&n))).await.is_ok();
            if exists {
                ask(ctx, format!("An AI agent wants to save over {n} in Plugin Studio. Allow it?")).await?;
            }
            post(ctx, "/api/tools/plugins/file", json!({"folder": f, "name": n, "text": text, "create": !exists})).await
        }
        .await,
        "plugin_delete" => async {
            let (f, n) = plugin_args(a)?;
            ask(ctx, format!("An AI agent wants to delete {n} from Plugin Studio. Allow it?")).await?;
            post(ctx, "/api/tools/plugins/delete", json!({"folder": f, "name": n})).await
        }
        .await,
        "plugin_compile" => async {
            let (f, n) = plugin_args(a)?;
            let mut v = post(ctx, "/api/tools/plugins/compile", json!({"folder": f, "name": n})).await?;
            // (the diagnostics say it all; the raw output only when there are none)
            if v["diagnostics"].as_array().is_some_and(|d| !d.is_empty()) {
                if let Some(o) = v.as_object_mut() {
                    o.remove("output");
                }
            }
            Ok(v)
        }
        .await,
        "plugin_try" => async {
            let (f, n) = plugin_args(a)?;
            ask_if_playing(ctx, &format!("load {n} in your game")).await?;
            post(ctx, "/api/tools/plugins/try", json!({"folder": f, "name": n})).await
        }
        .await,
        "plugin_untry" => async {
            let n = s(a, "name").ok_or_else(|| err("Say the plugin's name."))?.to_string();
            let restart = a["restart"].as_bool().unwrap_or(false);
            if restart {
                ask_if_playing(ctx, &format!("take {n} out of your game now")).await?;
            }
            post(ctx, "/api/tools/plugins/untry", json!({"name": n, "restart": restart})).await
        }
        .await,
        "plugin_log" => async {
            let n = s(a, "name").ok_or_else(|| err("Say the plugin's name."))?;
            let mut path = format!("/api/tools/plugins/log?name={}", q(n));
            if let Some(c) = s(a, "cursor") {
                path.push_str(&format!("&cursor={}", q(c)));
            }
            get(ctx, &path).await
        }
        .await,

        "components_library" => async {
            let v = get(ctx, "/api/tools/editor/components").await?;
            let words: Vec<String> = s(a, "search").unwrap_or("").to_lowercase().split_whitespace().map(String::from).collect();
            let list: Vec<Value> = v["components"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| {
                    let text = c.to_string().to_lowercase();
                    words.iter().all(|w| text.contains(w.as_str()))
                })
                .cloned()
                .collect();
            Ok(json!({"components": list}))
        }
        .await,
        "components_pack" => async { get(ctx, &format!("/api/tools/editor/components/pack?map={}", map_arg(a)?)).await }.await,
        "components_save_pack" => async {
            let m = map_arg(a)?;
            let mode = mode_arg(a)?;
            ask(ctx, format!("An AI agent wants to write the components file for {m}{} in your game (an old one is kept as .bak). Allow it?", mode.as_deref().map(|x| format!(" ({x})")).unwrap_or_default())).await?;
            post(ctx, "/api/tools/editor/components/save-pack", json!({"map": m, "mode": mode})).await
        }
        .await,
        "components_load_pack" => async {
            let m = map_arg(a)?;
            post(ctx, "/api/tools/editor/components/load-pack", json!({"map": m, "mode": mode_arg(a)?})).await
        }
        .await,
        _ => return extra(ctx, name, a).await,
    })
}

fn mode_arg(a: &Value) -> Result<Option<String>, Fail> {
    match s(a, "mode") {
        None | Some("") => Ok(None),
        Some(m) if crate::maps::safe_arg(m) => Ok(Some(m.to_string())),
        Some(_) => Err(err("That isn't a mode id.")),
    }
}

async fn studio_save(ctx: &Ctx, a: &Value) -> Done {
    let id = id_arg(a, "id")?;
    let graph = a.get("graph").filter(|g| g.is_object()).ok_or_else(|| err("graph is the whole graph object."))?;
    if get(ctx, &format!("/api/tools/studio/graph/{id}")).await.is_ok() {
        ask(ctx, format!("An AI agent wants to save over the game mode graph \"{id}\". Allow it?")).await?;
    }
    post(ctx, &format!("/api/tools/studio/graph/{id}"), json!({"graph": graph})).await
}

/// The Studio's node catalog, compiled in (the same file the Studio and its generator use).
pub const STUDIO_NODES: &str = include_str!("../../../ui/src/views/tools/studio/nodes.json");

fn studio_nodes(search: &str, category: Option<&str>) -> Value {
    let all: Value = serde_json::from_str(STUDIO_NODES).unwrap_or(Value::Null);
    let words: Vec<String> = search.to_lowercase().split_whitespace().map(String::from).collect();
    let nodes: Vec<Value> = all["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| category.is_none_or(|c| n["category"].as_str() == Some(c)))
        .filter(|n| {
            let text = format!("{} {} {}", n["id"], n["name"].as_str().or(n["label"].as_str()).unwrap_or(""), n["category"]).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .cloned()
        .collect();
    json!({"version": all["version"], "pinTypes": all["pinTypes"], "categories": all["categories"], "matched": nodes.len(), "nodes": nodes})
}

async fn extra(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "backups_list" => {
            let mut path = "/api/safety/files".to_string();
            if let Some(m) = s(a, "match") {
                path.push_str(&format!("?match={}", q(m)));
            }
            get(ctx, &path).await
        }
        "backup_restore" => async {
            let id = s(a, "id").filter(|i| !i.is_empty() && i.len() <= 128 && i.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))).ok_or_else(|| err("id is a file's id from backups_list."))?;
            let seq = a.get("seq").and_then(Value::as_u64).ok_or_else(|| err("seq is the version's number from backups_list."))?;
            ask(ctx, format!("An AI agent wants to put back an earlier version of a game file ({id}, version {seq}). The file as it is now is kept first. Allow it?")).await?;
            post(ctx, "/api/safety/restore", json!({"id": id, "seq": seq})).await
        }
        .await,
        "radar_preview" => async {
            let m = map_arg(a)?;
            let mut qs: Vec<String> = vec![];
            for k in ["style", "roofs", "orient", "background"] {
                if let Some(v) = s(a, k) {
                    qs.push(format!("{k}={}", q(v)));
                }
            }
            get(ctx, &format!("/api/tools/overview/{m}/preview.png?{}", qs.join("&"))).await
        }
        .await,
        "demos_list" => {
            let mut v = match get(ctx, "/api/tools/demos").await {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            if let Some(d) = v.get_mut("demos").and_then(Value::as_array_mut) {
                d.truncate(limit(a, 100, 500));
            }
            Ok(v)
        }
        "creature_types" => async {
            let v = get(ctx, "/api/tools/creatures").await?;
            let types: Vec<Value> = v["types"].as_array().into_iter().flatten().map(|t| json!({"name": t["name"], "display": t["display"], "category": t["category"], "builtin": t["builtin"], "inFile": t["inFile"], "summary": t["summary"]})).collect();
            Ok(json!({"file": v["file"], "types": types, "problems": v["problems"], "limits": v["limits"]}))
        }
        .await,
        "creature_type_read" => async {
            let n = s(a, "name").ok_or_else(|| err("Say the type's name."))?;
            let v = get(ctx, "/api/tools/creatures").await?;
            v["types"].as_array().into_iter().flatten().find(|t| t["name"].as_str() == Some(n)).cloned().ok_or_else(|| err(format!("There's no creature type called {n} (creature_types lists them).")))
        }
        .await,
        "creature_type_save" => creature_save(ctx, a).await,
        "creature_apply" => async {
            let reload = post(ctx, "/api/tools/creatures/reload", json!({})).await?;
            match s(a, "spawn") {
                Some(t) if reload["ok"] == true => {
                    let spawned = post(ctx, "/api/tools/creatures/spawn", json!({"type": t, "calm": a["calm"].as_bool().unwrap_or(false)})).await?;
                    Ok(json!({"reload": reload, "spawn": spawned}))
                }
                _ => Ok(reload),
            }
        }
        .await,
        _ => return None,
    })
}

/// One type into the whole list the save route takes: merged over the type that's there, or
/// a new one from its base (or the defaults); remove takes its section out.
async fn creature_save(ctx: &Ctx, a: &Value) -> Done {
    let given = a.get("type").filter(|t| t.is_object()).ok_or_else(|| err("type is an object with at least name."))?;
    let name = given["name"].as_str().filter(|n| !n.is_empty()).ok_or_else(|| err("type.name is the type's name."))?.to_string();
    let v = get(ctx, "/api/tools/creatures").await?;
    let mut types: Vec<Value> = v["types"].as_array().cloned().unwrap_or_default().into_iter().filter(|t| t["skipped"] != true).collect();
    let builtin = types.iter().any(|t| t["name"] == name && t["builtin"] == true);
    if a["remove"].as_bool() == Some(true) {
        if builtin {
            let fresh = v["templates"].as_array().into_iter().flatten().find(|t| t["name"] == name).cloned().unwrap_or(json!({}));
            for t in types.iter_mut().filter(|t| t["name"] == name) {
                *t = fresh.clone();
            }
        } else {
            types.retain(|t| t["name"] != name);
        }
    } else {
        let start = match types.iter().find(|t| t["name"] == name) {
            Some(t) => t.clone(),
            None => {
                let base = given["base"].as_str().unwrap_or("");
                let mut t = types.iter().find(|t| !base.is_empty() && t["name"] == base).cloned().unwrap_or(json!({}));
                t["name"] = json!(name);
                t["base"] = json!(base);
                if t.get("display").is_none() {
                    t["display"] = json!(name);
                }
                types.push(t.clone());
                t
            }
        };
        let mut merged = start;
        for (k, val) in given.as_object().into_iter().flatten() {
            merged[k] = val.clone();
        }
        for t in types.iter_mut().filter(|t| t["name"] == name) {
            *t = merged.clone();
        }
    }
    post(ctx, "/api/tools/creatures/save", json!({"types": types})).await
}
