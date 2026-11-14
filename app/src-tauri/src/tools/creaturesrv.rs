//! Shared creatures on servers: what a server is given (a community mode, a map copied to it)
//! brings the custom creature types it uses, as blocks in the server's creatures.ini, the same
//! way the player's own game gets them (tools/creaturepack.rs). The server's own sections and
//! files are never touched. A Linux server gets them with the next upload: its folder here is
//! a copy of the machine's, and creatures.ini, models/ and sound/ go up with everything else.
//!
//! - A community mode or map from the library: its block (by package key) is copied from this
//!   computer's game, with its models and sounds (`carry_block`), and taken out with it.
//! - A map copied to a server (the player's own or a community one): one block per map
//!   (`map.<name>`) with the custom types its entities name, as they are on this computer
//!   (the player's own keep their names: it's their server), and the custom types behind them.
//!   A map's block goes when the map isn't on the server any more (`prune`, at each copy and
//!   when the server starts).

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};

use super::creaturepack as cp;

const START: &str = "; >>> from ";
const END: &str = "; <<< end ";
const TOOL: &str = "Shared creatures";
const INI: &str = "addons/amxmodx/configs/gamemodes/creatures.ini";

/// The block id for a map's creatures on a server.
pub fn map_block_id(map: &str) -> Option<String> {
    let id = format!("map.{}", map.to_ascii_lowercase());
    cp::valid_id(&id).then_some(id)
}

/// Copies a package's block (with its models and sounds) from this computer's game to a
/// server's game folder. None: this computer has no block for it (it brings no creatures).
pub fn carry_block(local_game: &Path, server_game: &Path, id: &str) -> Result<Option<cp::Done>, String> {
    let Some(text) = cp::block_text(local_game, id) else {
        // nothing here (any more): nothing there either
        cp::uninstall_pack(server_game, id)?;
        return Ok(None);
    };
    cp::install_trusted(server_game, id, &only_written(&text), Some(local_game)).map(Some)
}

/// A block as it goes to a server: its files are only the ones its package wrote into this
/// computer's game. A file the block names that the player already had (their own, private) is
/// never copied anywhere.
fn only_written(block: &str) -> String {
    let (_, mut blocks) = cp::split_blocks(block);
    for b in blocks.iter_mut() {
        let wrote = b.wrote.clone();
        b.files.retain(|f| wrote.iter().any(|w| w.eq_ignore_ascii_case(f)));
        b.wrote.clear();
    }
    cp::join_blocks("", &blocks)
}

/// Takes a package's block (and the files only it needed) off a server.
pub fn drop_block(server_game: &Path, id: &str) -> Result<Vec<String>, String> {
    cp::uninstall_pack(server_game, id)
}

/// `[name]` sections of `text` (the lines up to the next section), by lower-case name.
fn sections(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(n) = t.strip_prefix('[').and_then(|r| r.split(']').next()) {
            out.push((n.trim().to_ascii_lowercase(), format!("[{}]\n", n.trim().to_ascii_lowercase())));
            continue;
        }
        if t.starts_with(';') || t.starts_with('#') || t.starts_with("//") || t.is_empty() {
            continue;
        }
        if let Some((_, body)) = out.last_mut() {
            body.push_str(t);
            body.push('\n');
        }
    }
    out
}

/// A section's `base =` (the last one).
fn base_of(body: &str) -> Option<String> {
    body.lines().filter_map(|l| l.split_once('=')).filter(|(k, _)| k.trim().eq_ignore_ascii_case("base")).map(|(_, v)| v.trim().to_ascii_lowercase()).last()
}

/// The custom types `used` names and the custom types behind them, with their sections, as
/// this computer's creatures.ini has them (the player's own part first, then the blocks), bases
/// first. Types it doesn't have are left out (the server plays what it has, or a stand-in).
fn collect(local_ini: &str, used: &BTreeSet<String>) -> Vec<(String, String, Vec<String>)> {
    let (user, blocks) = cp::split_blocks(local_ini);
    let own = sections(&user);
    let find = |n: &str| -> Option<(String, Vec<String>)> {
        // (several sections of one name: the game reads them in order, so they all go)
        let mine: Vec<&(String, String)> = own.iter().filter(|(x, _)| x == n).collect();
        if !mine.is_empty() {
            return Some((mine.iter().map(|(_, b)| b.as_str()).collect(), vec![]));
        }
        let b = blocks.iter().find(|b| b.types.iter().any(|t| t.eq_ignore_ascii_case(n)))?;
        let s: Vec<String> = sections(&b.body).into_iter().filter(|(x, _)| x == n).map(|(_, b)| b).collect();
        // (an installed type's files: only those its package wrote here, never a player's own)
        (!s.is_empty()).then(|| (s.concat(), b.wrote.clone()))
    };
    let mut order: Vec<(String, String, Vec<String>)> = Vec::new();
    for n in used {
        let n = n.to_ascii_lowercase();
        if cp::is_builtin(&n) || n == "all" {
            continue;
        }
        let mut chain: Vec<(String, String, Vec<String>)> = Vec::new();
        let mut at = n.clone();
        while chain.len() < 16 && !cp::is_builtin(&at) {
            let Some((body, files)) = find(&at) else { break };
            let next = base_of(&body);
            chain.push((at.clone(), body, files));
            match next {
                Some(b) if !chain.iter().any(|(x, _, _)| *x == b) => at = b,
                _ => break,
            }
        }
        for c in chain.into_iter().rev() {
            if !order.iter().any(|(x, _, _)| *x == c.0) {
                order.push(c);
            }
        }
    }
    order
}

/// Model and sound files a section names that aren't the game's own (game paths).
fn files_of(body: &str, game: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in body.lines() {
        let Some((k, v)) = l.split_once('=') else { continue };
        let k = k.trim().to_ascii_lowercase();
        let model = k == "model";
        if !model && !k.starts_with("snd_") {
            continue;
        }
        for w in v.split([' ', '|']).map(str::trim).filter(|w| !w.is_empty()) {
            let w = w.replace('\\', "/");
            let rel = if model { w } else { format!("sound/{w}") };
            if rel.contains("..") || rel.contains(':') || super::doctor::is_stock(&rel, game) {
                continue;
            }
            if !out.iter().any(|x| x.eq_ignore_ascii_case(&rel)) {
                out.push(rel);
            }
        }
    }
    out
}

/// Puts the creature types a map uses on a server, as the map's block: copied from this
/// computer's game (`local_game`), with the models and sounds the server doesn't have (a file
/// the server has, different, stays the server's). A type the server's own part defines is the
/// server's and isn't added. Returns what went: {"types": [...], "kept": [...], "skipped": [...]}.
pub fn carry_map(local_game: &Path, game: &str, server_game: &Path, map: &str, entities: &str) -> Result<Value, String> {
    let Some(id) = map_block_id(map) else { return Ok(json!({"types": []})) };
    let used = cp::types_in_entity_text(entities);
    let local_ini = std::fs::read(local_game.join(INI)).map(|b| b.iter().map(|c| *c as char).collect::<String>()).unwrap_or_default();
    let found = collect(&local_ini, &used);
    let own_names: BTreeSet<String> = sections(&cp::split_blocks(&local_ini).0).into_iter().map(|(n, _)| n).collect();
    let server_ini_path = server_game.join(INI);
    let old = std::fs::read(&server_ini_path).map(|b| b.iter().map(|c| *c as char).collect::<String>()).unwrap_or_default();
    let (server_user, _) = cp::split_blocks(&old);
    let theirs: BTreeSet<String> = sections(&server_user).into_iter().map(|(n, _)| n).collect();
    // the map's previous block goes first (a map copied again brings what it uses now)
    cp::uninstall_pack(server_game, &id)?;
    let mut types: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut files: Vec<String> = Vec::new();
    for (n, s, wrote_files) in &found {
        if theirs.contains(n) {
            skipped.push(n.clone());
            continue;
        }
        types.push(n.clone());
        body.push_str(s);
        // the player's own type: the files it names; an installed one: what its package wrote
        let installed = !own_names.contains(n);
        let mut fs: Vec<String> = if installed { wrote_files.iter().filter(|f| files_of(s, game).iter().any(|x| x.eq_ignore_ascii_case(f))).cloned().collect() } else { files_of(s, game) };
        fs.retain(|f| !files.iter().any(|x| x.eq_ignore_ascii_case(f)));
        files.extend(fs);
    }
    if types.is_empty() {
        return Ok(json!({"types": [], "skipped": skipped}));
    }
    // the files: copied where the server has none; the same file there is fine; a different
    // one is the server's (kept)
    let mut wrote: Vec<String> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    for rel in &files {
        let src = local_game.join(rel);
        let dest = server_game.join(rel);
        if !src.is_file() || std::fs::symlink_metadata(&src).map(|m| !m.is_file()).unwrap_or(true) {
            continue;
        }
        if dest.exists() {
            if !crate::archive::same_file(&src, &dest) {
                kept.push(rel.clone());
            }
            continue;
        }
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
        }
        std::fs::copy(&src, &dest).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
        wrote.push(rel.clone());
    }
    // the block, written the way creaturepack reads it back
    let mut b = format!("{START}{id} (shared creatures: Loner-Strike adds and removes this block)\n; types: {}\n", types.join(" "));
    if !files.is_empty() {
        b.push_str(&format!("; files: {}\n", files.join(" ")));
    }
    if !wrote.is_empty() {
        b.push_str(&format!("; wrote: {}\n", wrote.join(" ")));
    }
    b.push_str(&body);
    b.push_str(&format!("{END}{id}\n"));
    let now = std::fs::read(&server_ini_path).map(|b| b.iter().map(|c| *c as char).collect::<String>()).unwrap_or_default();
    let (user, mut blocks) = cp::split_blocks(&now);
    let (_, mut new) = cp::split_blocks(&b);
    blocks.append(&mut new);
    let text = cp::join_blocks(&user, &blocks);
    crate::safety::before(TOOL, &format!("Add {map}'s creatures to a server"), &server_ini_path);
    if let Some(d) = server_ini_path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save creatures.ini ({e})."))?;
    }
    crate::sys::make_writable(&server_ini_path);
    crate::sys::write_latin1_atomic(&server_ini_path, &text).map_err(|e| format!("Couldn't save creatures.ini ({e})."))?;
    Ok(json!({"types": types, "kept": kept, "skipped": skipped}))
}

/// Takes the blocks of maps that aren't on the server any more off it. True when something went.
pub fn prune(server_game: &Path) -> Result<bool, String> {
    let p = server_game.join(INI);
    let Ok(raw) = std::fs::read(&p) else { return Ok(false) };
    let text: String = raw.iter().map(|c| *c as char).collect();
    let (_, blocks) = cp::split_blocks(&text);
    let mut any = false;
    for b in blocks {
        let Some(map) = b.id.strip_prefix("map.") else { continue };
        if !server_game.join("maps").join(format!("{map}.bsp")).is_file() && !has_map_any_case(server_game, map) {
            cp::uninstall_pack(server_game, &b.id)?;
            any = true;
        }
    }
    Ok(any)
}

fn has_map_any_case(server_game: &Path, map: &str) -> bool {
    let want = format!("{map}.bsp");
    std::fs::read_dir(server_game.join("maps")).map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&want))).unwrap_or(false)
}

/// A map copied to a server: its creatures go with it (the map's .bsp entities say which).
/// Problems are logged, not raised: the map itself is there either way.
pub fn after_map_copy(local_game: &Path, game: &str, server_game: &Path, map: &str, bsp: &Path) {
    let r = (|| -> Result<Value, String> {
        let bytes = crate::caps::read_file(bsp, "map", 512 << 20)?;
        let ents = cp::bsp_entities(&bytes).unwrap_or_default();
        let _ = prune(server_game);
        carry_map(local_game, game, server_game, map, &ents)
    })();
    if let Err(e) = r {
        crate::diagnostics::log_error("creatures to a server", &format!("{map}: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("ls-creaturesrv-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (local, server, pkg) = (root.join("local/cstrike"), root.join("srv/cstrike"), root.join("pkg"));
        for d in [&local, &server, &pkg] {
            std::fs::create_dir_all(d.join("addons/amxmodx/configs/gamemodes")).unwrap();
        }
        // (Half-Life folders, as the safety net wants before it removes a file)
        for d in [&local, &server] {
            std::fs::create_dir_all(d.parent().unwrap().join("valve")).unwrap();
        }
        (root, local, server)
    }

    const PACK: &str = "; types: gb7.runner_x\n; files: models/gb7/runner_x.mdl sound/gb7/x.wav\n[gb7.runner_x]\nbase = runner\nhp = 77\nmodel = models/gb7/runner_x.mdl\nsnd_pain = gb7/x.wav\n";
    const SERVER_OWN: &str = "; the server's own\n[srv_type]\nbase = zombie\nhp = 5\n";

    fn ini(dir: &Path) -> String {
        std::fs::read_to_string(dir.join(INI)).unwrap_or_default()
    }

    #[test]
    fn a_mode_to_a_server_carries_its_block_and_files_and_takes_them_back() {
        let (root, local, server) = dirs("mode");
        let pkg = root.join("pkg");
        std::fs::create_dir_all(pkg.join("models/gb7")).unwrap();
        std::fs::create_dir_all(pkg.join("sound/gb7")).unwrap();
        std::fs::write(pkg.join("models/gb7/runner_x.mdl"), b"IDST model").unwrap();
        std::fs::write(pkg.join("sound/gb7/x.wav"), b"RIFF wav").unwrap();
        // installed on this computer (as CP-core's install does), then put on the server
        cp::install_pack(&local, "mod-1", PACK, Some(&pkg), "gb7").unwrap();
        std::fs::write(server.join(INI), SERVER_OWN).unwrap();
        let done = carry_block(&local, &server, "mod-1").unwrap().unwrap();
        assert_eq!(done.types, vec!["gb7.runner_x"]);
        assert!(server.join("models/gb7/runner_x.mdl").is_file() && server.join("sound/gb7/x.wav").is_file());
        let text = ini(&server);
        assert!(text.starts_with(SERVER_OWN), "the server's own part is byte-identical: {text}");
        assert!(text.contains("; >>> from mod-1") && text.contains("[gb7.runner_x]"));
        // a second item (a map's block) uses the same type: dropping the mode keeps it
        std::fs::create_dir_all(server.join("maps")).unwrap();
        std::fs::write(server.join("maps/qa_map.bsp"), b"x").unwrap();
        let r = carry_map(&local, "cstrike", &server, "qa_map", "{\n\"classname\" \"ls_creature\"\n\"type\" \"gb7.runner_x\"\n}\n").unwrap();
        assert_eq!(r["types"], json!(["gb7.runner_x"]));
        drop_block(&server, "mod-1").unwrap();
        let text = ini(&server);
        assert!(text.starts_with(SERVER_OWN));
        assert!(!text.contains("from mod-1") && text.contains("from map.qa_map") && text.contains("[gb7.runner_x]"));
        assert!(server.join("models/gb7/runner_x.mdl").is_file(), "the map's block still needs the model");
        // the map goes from the server: its block (and the files only it needed) go too
        std::fs::remove_file(server.join("maps/qa_map.bsp")).unwrap();
        assert!(prune(&server).unwrap());
        assert_eq!(ini(&server), SERVER_OWN);
        assert!(!server.join("models/gb7/runner_x.mdl").exists() && !server.join("sound/gb7/x.wav").exists());
        assert!(!prune(&server).unwrap());
        // nothing for a package with no block here
        assert!(carry_block(&local, &server, "mod-2").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A file the block names that the player already had (their own copy, not written by the
    /// package) never leaves their computer, for a mode or a map.
    #[test]
    fn a_players_private_file_is_never_carried() {
        let (root, local, server) = dirs("private");
        let pkg = root.join("pkg");
        for d in [&pkg, &local] {
            std::fs::create_dir_all(d.join("models/gb7")).unwrap();
            std::fs::create_dir_all(d.join("sound/gb7")).unwrap();
        }
        std::fs::write(pkg.join("models/gb7/runner_x.mdl"), b"IDST pkg").unwrap();
        std::fs::write(pkg.join("sound/gb7/x.wav"), b"RIFF pkg").unwrap();
        // the player already had their own sound at that path: kept, not written by the package
        std::fs::write(local.join("sound/gb7/x.wav"), b"RIFF private").unwrap();
        let d = cp::install_pack(&local, "mod-1", PACK, Some(&pkg), "gb7").unwrap();
        assert_eq!(d.kept, vec!["sound/gb7/x.wav"]);
        carry_block(&local, &server, "mod-1").unwrap().unwrap();
        assert!(server.join("models/gb7/runner_x.mdl").is_file());
        assert!(!server.join("sound/gb7/x.wav").exists(), "the player's own file stays on their computer");
        std::fs::create_dir_all(server.join("maps")).unwrap();
        std::fs::write(server.join("maps/m.bsp"), b"x").unwrap();
        let server2 = root.join("srv2/cstrike");
        std::fs::create_dir_all(server2.parent().unwrap().join("valve")).unwrap();
        carry_map(&local, "cstrike", &server2, "m", "{\n\"classname\" \"ls_creature\"\n\"type\" \"gb7.runner_x\"\n}\n").unwrap();
        assert!(server2.join("models/gb7/runner_x.mdl").is_file() && !server2.join("sound/gb7/x.wav").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_map_brings_the_players_own_types_by_their_names_with_their_bases() {
        let (root, local, server) = dirs("own");
        let own = "[my_base]\nbase = brute\nhp = 900\nmodel = models/mine/big.mdl\n[my_boss]\nbase = my_base\nscale = 2\n[unused]\nbase = zombie\n";
        std::fs::write(local.join(INI), own).unwrap();
        std::fs::create_dir_all(local.join("models/mine")).unwrap();
        std::fs::write(local.join("models/mine/big.mdl"), b"IDST mine").unwrap();
        // the server has its own different model at that path, and its own my_other type
        std::fs::create_dir_all(server.join("models/mine")).unwrap();
        std::fs::write(server.join("models/mine/big.mdl"), b"IDST theirs").unwrap();
        let theirs = "[my_other]\nbase = zombie\n";
        std::fs::write(server.join(INI), theirs).unwrap();
        let ents = "{\n\"classname\" \"ls_creature\"\n\"type\" \"my_boss\"\n}\n{\n\"classname\" \"ls_creature\"\n\"type\" \"zombie\"\n}\n{\n\"classname\" \"ls_creature\"\n\"type\" \"my_other\"\n}\n";
        let r = carry_map(&local, "cstrike", &server, "my_map", ents).unwrap();
        assert_eq!(r["types"], json!(["my_base", "my_boss"]), "bases first, built-ins never, names kept");
        assert_eq!(r["kept"], json!(["models/mine/big.mdl"]));
        assert_eq!(std::fs::read(server.join("models/mine/big.mdl")).unwrap(), b"IDST theirs");
        let text = ini(&server);
        assert!(text.starts_with(theirs) && !text.contains("[unused]") && text.contains("base = my_base"));
        // read back as blocks: the game and the app see the same thing
        let (user, blocks) = cp::split_blocks(&text);
        assert_eq!(user, theirs);
        assert_eq!(blocks[0].id, "map.my_map");
        assert_eq!(blocks[0].types, vec!["my_base", "my_boss"]);
        assert!(blocks[0].wrote.is_empty(), "the server's file wasn't ours to write");
        // copied again: one block, not two
        carry_map(&local, "cstrike", &server, "my_map", ents).unwrap();
        assert_eq!(cp::split_blocks(&ini(&server)).1.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
