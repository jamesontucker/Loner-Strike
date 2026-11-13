//! Half-Life's own monster models on our servers, kept the same as the game on this computer.
//!
//! Creatures (the fork's ls_creature, docs/CREATURES.md) and the Last Stand shopkeeper use
//! Half-Life's models from `valve/models`. The server picks a sequence by its number in the
//! server's copy of the file, and every player's game draws that number from its own copy. The
//! Half-Life 25th anniversary update changed some of them (zombie.mdl gained a sequence near the
//! start, so the server's "walk" was the game's "attack2" and its "attack1" the game's
//! "falling"). Servers made with SteamCMD before that update (and HLDS in general, which Valve
//! hasn't updated) still have the old files.
//!
//! So at each start, a server gets this computer's copy of those few files when its own differ.
//! Both copies are Valve's, on the player's own computers and servers; nothing is downloaded
//! from us or sent to anyone else. A server on another machine gets them over its SSH link.

use crate::servers::Entry;
use std::path::{Path, PathBuf};

/// The models creatures and the shopkeeper use (creatures.ini's built-in types, gm_laststand).
pub const MODELS: [&str; 16] = [
    "zombie", "bullsquid", "houndeye", "agrunt", "garg", "headcrab", "sentry", "barney", "gman", "islave", "hassassin", "hgrunt", "big_mom", "w_squeak",
    "controller", "barnacle",
];

/// This computer's game's copy of a model (the current Steam one), if it has it.
fn client_model(hl: &Path, name: &str) -> Option<PathBuf> {
    let p = hl.join("valve").join("models").join(format!("{name}.mdl"));
    p.is_file().then_some(p)
}

fn sha256(p: &Path) -> Option<String> {
    crate::setup::fetch::sha256(p).ok()
}

/// Puts this computer's copies of MODELS into the server where its own differ. Returns the
/// names replaced. Quiet: a model the game here doesn't have is left as the server has it.
pub fn copy_to_server(e: &Entry) -> Vec<String> {
    let Some(hl) = crate::navs::local_hl() else { return Vec::new() };
    let mut done = Vec::new();
    for name in MODELS {
        let Some(src) = client_model(&hl, name) else { continue };
        let Some(want) = sha256(&src) else { continue };
        let ok = match &e.remote {
            Some(r) => copy_remote(r, &e.id, name, &src, &want),
            None => copy_local(&e.dir, name, &src, &want),
        };
        match ok {
            Ok(true) => done.push(name.to_string()),
            Ok(false) => {}
            Err(why) => crate::diagnostics::log_error("half-life models", &why),
        }
    }
    done
}

fn copy_local(server: &Path, name: &str, src: &Path, want: &str) -> Result<bool, String> {
    let dest = server.join("valve").join("models").join(format!("{name}.mdl"));
    // (a server folder without valve/models isn't one HLDS made: left alone)
    if !dest.parent().is_some_and(|d| d.is_dir()) {
        return Ok(false);
    }
    if sha256(&dest).as_deref() == Some(want) {
        return Ok(false);
    }
    let tmp = dest.with_file_name(format!(".{name}.mdl.part"));
    std::fs::copy(src, &tmp).and_then(|_| std::fs::rename(&tmp, &dest)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't update {name}.mdl in {} ({e}).", server.display())
    })?;
    Ok(true)
}

fn copy_remote(r: &crate::remote::RemoteInfo, id: &str, name: &str, src: &Path, want: &str) -> Result<bool, String> {
    use crate::remote::{exec_quick, shell_quote, write_file};
    let rel = format!("{}/valve/models/{name}.mdl", crate::remote::server_dir(id));
    let q = shell_quote(&rel);
    let (_, out) = exec_quick(r, &format!("cd \"$HOME\" && if [ -f {q} ]; then sha256sum {q} | cut -c1-64; else echo none; fi"))?;
    let have = out.trim();
    // (no valve/models there: not an HLDS folder we know; left alone)
    if have == want || have == "none" {
        return Ok(false);
    }
    let data = std::fs::read(src).map_err(|e| format!("Couldn't read {name}.mdl ({e})."))?;
    let new = format!("{rel}.new");
    write_file(r, &new, &data)?;
    let (code, out) = exec_quick(r, &format!("cd \"$HOME\" && mv -f {} {q}", shell_quote(&new)))?;
    if code != 0 {
        return Err(format!("Couldn't put {name}.mdl in place ({}).", out.trim()));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_copy_only_when_different() {
        let base = std::env::temp_dir().join(format!("ls-hlmodels-{}", std::process::id()));
        let server = base.join("server");
        let models = server.join("valve/models");
        std::fs::create_dir_all(&models).unwrap();
        let src = base.join("zombie.mdl");
        std::fs::write(&src, b"new").unwrap();
        std::fs::write(models.join("zombie.mdl"), b"old").unwrap();
        let want = sha256(&src).unwrap();
        assert!(copy_local(&server, "zombie", &src, &want).unwrap());
        assert_eq!(std::fs::read(models.join("zombie.mdl")).unwrap(), b"new");
        // (the same now: nothing to do)
        assert!(!copy_local(&server, "zombie", &src, &want).unwrap());
        // (no valve/models: not touched)
        assert!(!copy_local(&base.join("nothing"), "zombie", &src, &want).unwrap());
        let _ = std::fs::remove_dir_all(&base);
    }
}
