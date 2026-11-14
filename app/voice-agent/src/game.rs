//! Talking to the game server without RCON (RCON's password would land in the server's console
//! log, which the console page and crash reports show): a folder next to the server's data,
//! `addons/amxmodx/data/gamemodes/lsvoice/`. ls-voice writes one file per request, `<n>.cmd`:
//! "feed <port>" (where the engine sends players' voice) or "ask <userid> <words>" (a line the
//! bots answer); gm_extras.amxx reads and removes them (only those two), and every 10 s writes
//! `status.txt` (the map, then the bots' names), which also says the server is up.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub struct Game {
    dir: PathBuf,
    n: AtomicU64,
}

impl Game {
    /// The folder beside the server's data folder (where panel_rcon.txt is).
    pub fn beside(data_file: &Path) -> Game {
        let dir = data_file.parent().unwrap_or(Path::new(".")).join("lsvoice");
        let _ = std::fs::create_dir_all(&dir);
        Game { dir, n: AtomicU64::new(0) }
    }

    /// One request for the plugin (written whole, then renamed, so it's never read half done).
    pub fn send(&self, line: &str) -> bool {
        let line: String = line.chars().filter(|c| !c.is_control()).collect();
        let n = self.n.fetch_add(1, Ordering::SeqCst);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let name = format!("{stamp}-{}-{n}", std::process::id());
        let tmp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, format!("{line}\n")).and_then(|_| std::fs::rename(&tmp, self.dir.join(format!("{name}.cmd")))).is_ok()
    }

    /// The bots' names and the map, from the plugin's status.txt; None when it's older than a
    /// minute (the server, or Loner-Strike's plugin, isn't running).
    pub fn status(&self) -> Option<(Vec<String>, String)> {
        let p = self.dir.join("status.txt");
        let age = std::fs::metadata(&p).and_then(|m| m.modified()).ok()?.elapsed().ok()?;
        if age > Duration::from_secs(60) {
            return None;
        }
        Some(parse_status(&std::fs::read_to_string(&p).ok()?))
    }
}

pub fn parse_status(text: &str) -> (Vec<String>, String) {
    let mut bots = Vec::new();
    let mut map = String::new();
    for l in text.lines() {
        if let Some(m) = l.strip_prefix("map ") {
            map = m.trim().to_string();
        } else if let Some(b) = l.strip_prefix("bot ") {
            if !b.trim().is_empty() {
                bots.push(b.trim().to_string());
            }
        }
    }
    (bots, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_status_through_the_folder() {
        let d = std::env::temp_dir().join(format!("ls-voice-game-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let g = Game::beside(&d.join("panel_rcon.txt"));
        assert!(g.status().is_none());
        assert!(g.send("feed 27615") && g.send("ask 3 \"rush b\nquit"));
        let mut cmds: Vec<String> = std::fs::read_dir(d.join("lsvoice")).unwrap().flatten().map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "cmd")).map(|p| std::fs::read_to_string(p).unwrap()).collect();
        cmds.sort();
        assert_eq!(cmds, vec!["ask 3 \"rush bquit\n".to_string(), "feed 27615\n".to_string()]);
        std::fs::write(d.join("lsvoice/status.txt"), "map de_dust2\nbot Kyle\nbot  \nbot Zoë [LS]\n").unwrap();
        assert_eq!(g.status(), Some((vec!["Kyle".to_string(), "Zoë [LS]".to_string()], "de_dust2".to_string())));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
