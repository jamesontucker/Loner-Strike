//! Every file Loner-Strike writes into a game folder, so it can all be undone.
//! Same format the old Python installer used, so its installs can still be undone.
//!
//! The record lives in <game>/addons/loner-strike/record.json. Before a file that
//! was already there (and not ours) is changed, a copy goes to
//! <game>/addons/loner-strike/backup/, and uninstalling puts it back.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub const RECORD_DIR: &str = "addons/loner-strike";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileInfo {
    #[serde(default)]
    pub backup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<String>>,
    /// Records written by early versions name a single owner
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
}

impl FileInfo {
    pub fn owners(&self) -> Vec<String> {
        match (&self.components, &self.component) {
            (Some(c), _) if !c.is_empty() => c.clone(),
            (_, Some(c)) => vec![c.clone()],
            (Some(c), None) => c.clone(),
            (None, None) => vec![],
        }
    }
    fn set_owners(&mut self, owners: Vec<String>) {
        self.components = Some(owners);
        self.component = None;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub files: BTreeMap<String, FileInfo>,
    #[serde(default)]
    pub components: Map<String, Value>,
    /// component id -> GameBanana mod ids its maps came from
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub maps: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub struct Record {
    pub game: PathBuf,
    pub dir: PathBuf,
    path: PathBuf,
    pub data: Data,
    /// record.json exists but couldn't be read: nothing may be installed or removed
    pub broken: bool,
    component: Option<String>,
    unsaved: usize,
}

/// A path inside the game folder from a '/'-separated relative path, or None if it
/// would leave the folder.
pub fn inside(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') {
        return None;
    }
    let mut out = root.to_path_buf();
    for part in Path::new(&rel).components() {
        match part {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (out != root).then_some(out)
}

/// The components installed in a game folder, for lists polled every few seconds:
/// record.json is only read again when it changes (it can be large).
pub fn component_ids_cached(game_dir: &Path) -> Vec<String> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    type Seen = (Option<std::time::SystemTime>, u64, Vec<String>);
    static CACHE: Mutex<Option<HashMap<PathBuf, Seen>>> = Mutex::new(None);
    let path = game_dir.join(RECORD_DIR).join("record.json");
    let meta = std::fs::metadata(&path).ok();
    let stamp = (meta.as_ref().and_then(|m| m.modified().ok()), meta.as_ref().map(|m| m.len()).unwrap_or(0));
    if let Some((t, len, ids)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|c| c.get(&path)) {
        if (*t, *len) == stamp {
            return ids.clone();
        }
    }
    let ids = Record::open(game_dir).component_ids();
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(path, (stamp.0, stamp.1, ids.clone()));
    ids
}

impl Record {
    pub fn open(game_dir: &Path) -> Record {
        let dir = game_dir.join(RECORD_DIR);
        let path = dir.join("record.json");
        let (data, broken) = match std::fs::read_to_string(&path) {
            Err(_) => (Data::default(), false),
            Ok(t) => match serde_json::from_str::<Data>(&t) {
                Ok(d) => (d, false),
                Err(_) => (Data::default(), true),
            },
        };
        Record { game: game_dir.to_path_buf(), dir, path, data, broken, component: None, unsaved: 0 }
    }

    /// Every component installed here ("foundation", "core", "mode-zp5"...).
    pub fn component_ids(&self) -> Vec<String> {
        self.data.components.keys().cloned().collect()
    }

    /// GameBanana mod ids of the maps that came with a component.
    pub fn map_mods(&self, component: &str) -> Vec<i64> {
        self.data.maps.get(component).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_i64()).collect()
    }

    pub fn all_map_mods(&self) -> Vec<i64> {
        let mut v: Vec<i64> = self.data.maps.keys().flat_map(|c| self.map_mods(c)).collect();
        v.dedup();
        v
    }

    /// Whether this file is one we wrote.
    pub fn ours(&self, rel: &str) -> bool {
        self.data.files.contains_key(&rel.replace('\\', "/"))
    }

    pub fn full(&self, rel: &str) -> Result<PathBuf, String> {
        inside(&self.game, rel).ok_or_else(|| format!("Refused to write outside the game folder ({rel})."))
    }

    // ---- writing files --------------------------------------------------
    /// Backs up a file we're about to change, the first time we touch it.
    pub fn prepare(&mut self, rel: &str) -> Result<PathBuf, String> {
        let rel = rel.replace('\\', "/");
        let full = self.full(&rel)?;
        let me = self.component.clone().unwrap_or_default();
        match self.data.files.get_mut(&rel) {
            None => {
                let mut backup = None;
                if full.exists() {
                    let b = format!("backup/{rel}");
                    let dst = self.dir.join(&b);
                    if let Some(d) = dst.parent() {
                        std::fs::create_dir_all(d).map_err(|e| couldnt("back up", &rel, e))?;
                    }
                    // a backup without a record entry is from a run that was cut short:
                    // it holds the original, so it's kept
                    if !dst.exists() {
                        copy2(&full, &dst).map_err(|e| couldnt("back up", &rel, e))?;
                    }
                    backup = Some(b);
                }
                self.data.files.insert(rel.clone(), FileInfo { backup, components: Some(vec![me]), component: None });
                // save now and then, so a crash halfway leaves little untracked
                self.unsaved += 1;
                if self.unsaved >= 200 {
                    self.save()?;
                }
            }
            Some(info) => {
                // several components can ship the same file (ZP 4.3 and ZP 5 share sounds)
                let mut owners = info.owners();
                if !owners.contains(&me) {
                    owners.push(me);
                    info.set_owners(owners);
                }
            }
        }
        if let Some(d) = full.parent() {
            std::fs::create_dir_all(d).map_err(|e| couldnt("write", &rel, e))?;
        }
        Ok(full)
    }

    pub fn copy(&mut self, src: &Path, rel: &str) -> Result<(), String> {
        let full = self.prepare(rel)?;
        copy2(src, &full).map_err(|e| couldnt("write", rel, e))
    }

    /// Writes text as Latin-1, exactly as given (no line ending changes).
    pub fn write(&mut self, rel: &str, text: &str) -> Result<(), String> {
        let full = self.prepare(rel)?;
        crate::sys::write_latin1(&full, text).map_err(|e| couldnt("write", rel, e))
    }

    pub fn read(&self, rel: &str) -> Option<String> {
        crate::sys::read_latin1(&inside(&self.game, rel)?).ok()
    }

    pub fn exists(&self, rel: &str) -> bool {
        inside(&self.game, rel).map(|p| p.exists()).unwrap_or(false)
    }

    // ---- components -----------------------------------------------------
    pub fn begin(&mut self, component: &str) {
        self.component = Some(component.to_string());
    }

    pub fn done(&mut self, component: &str, version: Value) -> Result<(), String> {
        let mut m = Map::new();
        m.insert("version".into(), version);
        m.insert("date".into(), Value::from(crate::sys::now() as i64));
        self.data.components.insert(component.into(), Value::Object(m));
        self.component = None;
        self.save()
    }

    pub fn installed(&self, component: &str) -> bool {
        self.data.components.contains_key(component)
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.unsaved = 0;
        std::fs::create_dir_all(&self.dir).map_err(|e| couldnt("save", "the install record", e))?;
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(&self.data).unwrap_or_default();
        std::fs::write(&tmp, text).map_err(|e| couldnt("save", "the install record", e))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| couldnt("save", "the install record", e))
    }

    // ---- undoing --------------------------------------------------------
    /// Puts one file back the way it was (restores the backup, or removes it).
    fn undo_file(&self, rel: &str, info: &FileInfo) -> Result<(), String> {
        let full = self.full(rel)?;
        if let Some(b) = &info.backup {
            if let Some(d) = full.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            copy2(&self.dir.join(b), &full).map_err(|e| couldnt("restore", rel, e))?;
            // restored: gone, so a later install backs up the file as it is then
            let _ = std::fs::remove_file(self.dir.join(b));
        } else if full.exists() {
            std::fs::remove_file(&full).map_err(|e| couldnt("remove", rel, e))?;
            if let Some(d) = full.parent() {
                prune(d, &self.game);
            }
        }
        Ok(())
    }

    /// The current component stops owning `rel`; the file is put back if nothing
    /// else needs it. Used when a mode's settings no longer belong in the game.
    pub fn release(&mut self, rel: &str) -> Result<(), String> {
        let me = self.component.clone().unwrap_or_default();
        let Some(info) = self.data.files.get(rel).cloned() else { return Ok(()) };
        let left: Vec<String> = info.owners().into_iter().filter(|c| *c != me).collect();
        if !left.is_empty() {
            self.data.files.get_mut(rel).unwrap().set_owners(left);
            return Ok(());
        }
        self.undo_file(rel, &info)?;
        self.data.files.remove(rel);
        Ok(())
    }

    /// Removes our files (all, or only those of the given components) and restores
    /// backups. A file that couldn't be put back stays in the record (with its
    /// backup) so trying again can finish the job.
    pub fn uninstall(&mut self, components: Option<&[String]>) -> Result<(), String> {
        let mut keep = BTreeMap::new();
        let mut failed = Vec::new();
        let mut files: Vec<(String, FileInfo)> = std::mem::take(&mut self.data.files).into_iter().collect();
        files.sort_by_key(|(rel, _)| std::cmp::Reverse(rel.len()));
        for (rel, mut info) in files {
            if let Some(comps) = components {
                let left: Vec<String> = info.owners().into_iter().filter(|c| !comps.contains(c)).collect();
                if !left.is_empty() {
                    // another component still needs it
                    info.set_owners(left);
                    keep.insert(rel, info);
                    continue;
                }
            }
            if let Err(e) = self.undo_file(&rel, &info) {
                failed.push(e);
                keep.insert(rel, info);
            }
        }
        self.data.files = keep;
        let ids: Vec<String> = self.data.components.keys().cloned().collect();
        for c in ids {
            if components.map(|cs| cs.contains(&c)).unwrap_or(true) {
                self.data.components.remove(&c);
            }
        }
        if components.is_none() && failed.is_empty() {
            let _ = std::fs::remove_dir_all(&self.dir);
            remove_empty(&self.game.join("addons"));
        } else {
            self.save()?;
        }
        if failed.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} file{} couldn't be put back (is the game running?): {}",
                failed.len(),
                if failed.len() == 1 { "" } else { "s" },
                failed.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
            ))
        }
    }
}

fn couldnt(what: &str, rel: &str, e: std::io::Error) -> String {
    format!("Couldn't {what} {rel} ({e}).")
}

/// shutil.copy2: copies the contents and keeps the modification time.
pub fn copy2(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst)?;
    if let Ok(t) = std::fs::metadata(src).and_then(|m| m.modified()) {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(dst) {
            let _ = f.set_modified(t);
        }
    }
    Ok(())
}

/// Removes empty folders under root (bottom up), and root itself if empty.
pub fn remove_empty(root: &Path) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                remove_empty(&p);
            }
        }
    }
    let _ = std::fs::remove_dir(root);
}

/// Removes folders we emptied, up to the game folder.
fn prune(d: &Path, stop: &Path) {
    let mut d = d.to_path_buf();
    while d != stop && d.starts_with(stop) {
        if std::fs::remove_dir(&d).is_err() {
            return;
        }
        match d.parent() {
            Some(p) => d = p.to_path_buf(),
            None => return,
        }
    }
}
