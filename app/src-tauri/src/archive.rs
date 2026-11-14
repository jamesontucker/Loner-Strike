//! Unpacking map downloads safely, and picking out the files that belong in the game.
//! The rules:
//!
//! - nothing may unpack to more than 3 GB (checked before, watched during, checked after);
//! - archives with symbolic or hard links aren't unpacked, and any link left over is removed;
//! - a partial unpack is a failure (a map missing its models crashes the game);
//! - only map content goes into the game, maps go straight into maps/, and map names
//!   must be plain (they end up in console commands).

use crate::sys;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const MAX_UNPACKED: u64 = 3 * 1024 * 1024 * 1024;
pub const ARCHIVES: &[&str] = &[".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2"];
const SKY_SIDES: &[&str] = &["up", "dn", "lf", "rt", "ft", "bk"];
const TOO_BIG: &str = "That download unpacks to more than 3 GB.";
const HAS_LINKS: &str = "That download contains links to other files, so it wasn't unpacked.";
// [Caps] zip bombs that stay under the byte budget: millions of tiny files, or a small
// download that unpacks to gigabytes. Real map/model/sound packs have a few thousand files
// at most and pack 1.5-10x (a WAD of flat colours ~50x); see docs/notes/Caps.md.
pub const MAX_ENTRIES: usize = 65_536;
pub const MAX_RATIO: u64 = 200;
const RATIO_FROM: u64 = 256 << 20;

/// [Caps] Refuses archives with too many files, or that unpack far bigger than they are.
pub fn bomb_check(entries: usize, unpacked: u64, packed: u64) -> Result<(), String> {
    if entries > MAX_ENTRIES {
        return Err(too_many(entries, MAX_ENTRIES));
    }
    if unpacked > RATIO_FROM && unpacked / packed.max(1) > MAX_RATIO {
        return Err(format!(
            "That download unpacks to {} from a {} file — far more than real game content packs to, so it wasn't unpacked.",
            crate::caps::size(unpacked),
            crate::caps::size(packed)
        ));
    }
    Ok(())
}

fn too_many(entries: usize, max: usize) -> String {
    format!("That download has too many files in it: {entries} — the most is {max}.")
}

fn packed_len(archive: &Path) -> u64 {
    std::fs::metadata(archive).map(|m| m.len()).unwrap_or(0)
}

/// Where each kind of file belongs inside cstrike/, and which extensions are allowed there.
fn content_exts(dir: &str) -> Option<&'static [&'static str]> {
    Some(match dir {
        "maps" => &[".bsp", ".nav", ".txt", ".res"],
        "gfx" => &[".tga", ".bmp"],
        "models" => &[".mdl"],
        "sound" => &[".wav", ".mp3"],
        "sprites" => &[".spr"],
        "overviews" => &[".txt", ".bmp", ".tga"],
        _ => return None,
    })
}

/// Map names end up in console commands (changelevel, votes), so only plain names are installed.
pub fn safe_map(name: &str) -> bool {
    !name.is_empty() && name.len() <= 31 && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-$".contains(&c))
}

pub fn is_archive(name: &str) -> bool {
    let n = name.to_lowercase();
    ARCHIVES.iter().any(|e| n.ends_with(e))
}

/// Python's os.path.splitext(name)[1]: leading dots don't start an extension.
pub fn ext_of(name: &str) -> &str {
    let lead = name.len() - name.trim_start_matches('.').len();
    match name[lead..].rfind('.') {
        Some(i) => &name[lead + i..],
        None => "",
    }
}

fn stem_of(name: &str) -> &str {
    &name[..name.len() - ext_of(name).len()]
}

// ---- walking the unpacked files ---------------------------------------------

/// Every regular file under root (links aren't followed or returned), in a stable order.
pub fn walk(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries.into_iter().rev() {
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => stack.push(p),
                Ok(m) if m.is_file() => out.push(p),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Total size of the files under root, not following links.
pub fn tree_size(root: &Path) -> u64 {
    tree_stats(root, usize::MAX).0
}

/// (total size, entries) under root, not following links; stops counting a little past
/// `stop` entries ([Caps]: a tree of millions of files isn't walked in full every poll).
fn tree_stats(root: &Path, stop: usize) -> (u64, usize) {
    let (mut total, mut count) = (0u64, 0usize);
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            count += 1;
            if count > stop {
                return (total, count);
            }
            // unpackers rename and remove temporary files as they go
            let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else {
                total += m.len();
            }
        }
    }
    (total, count)
}

/// Removes symbolic links an archive brought along, so nothing points outside the download.
pub fn strip_links(root: &Path) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
            if m.file_type().is_symlink() {
                // a link to a folder on Windows is removed like a folder
                if std::fs::remove_file(&p).is_err() {
                    let _ = std::fs::remove_dir(&p);
                }
            } else if m.is_dir() {
                stack.push(p);
            }
        }
    }
}

pub fn same_file(a: &Path, b: &Path) -> bool {
    use std::io::Read;
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else { return false };
    if ma.len() != mb.len() {
        return false;
    }
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let (mut ba, mut bb) = (vec![0u8; 256 * 1024], vec![0u8; 256 * 1024]);
    loop {
        let na = fa.read(&mut ba).unwrap_or(0);
        if na == 0 {
            return true;
        }
        if fb.read_exact(&mut bb[..na]).is_err() || ba[..na] != bb[..na] {
            return false;
        }
    }
}

fn clear_dir(d: &Path) {
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::create_dir_all(d);
}

// ---- unpacking ---------------------------------------------------------------

/// Unpacks archive into dest, refusing more than budget bytes.
pub fn extract(archive: &Path, dest: &Path, budget: u64) -> Result<(), String> {
    if archive.to_string_lossy().to_lowercase().ends_with(".zip") {
        match extract_zip(archive, dest, budget) {
            Ok(true) => return Ok(()),
            Ok(false) => clear_dir(dest), // some ".zip" files are really rar; let bsdtar try
            Err(e) => return Err(e),
        }
    }
    // bsdtar (built into macOS and Windows 10+) reads most rar, 7z and tarballs and refuses ../ paths.
    let tar = tar_path();
    let listing = tar.as_ref().and_then(|t| {
        let mut c = sys::command(t);
        c.arg("-tvf").arg(archive);
        sys::run_output(c, Duration::from_secs(300))
    });
    let (list_ok, list_out, list_err) = listing.unwrap_or((false, String::new(), "tar isn't available".into()));
    let (total, links, readable) = read_listing(list_ok, &list_out);
    let mut tar_err: Option<String> = None;
    if readable && !links {
        if total > budget {
            return Err(TOO_BIG.into());
        }
        bomb_check(list_out.lines().count(), total, packed_len(archive))?; // [Caps]
        let mut c = sys::command(tar.as_ref().unwrap());
        c.arg("-xf").arg(archive).arg("-C").arg(dest);
        let (ok, err) = run_limited(c, dest, budget)?;
        strip_links(dest);
        if ok {
            let files = list_out.lines().filter(|l| l.starts_with('-')).count();
            check_tree(dest, Some(files))?;
            return not_a_zip_package(dest);
        }
        tar_err = Some(err);
    }
    // tar's listing showed links: refused here, before another unpacker that may not see them
    // (7-Zip's listing doesn't show a zip's Unix links as links) gets a go at it
    if readable && links {
        return Err(HAS_LINKS.into());
    }
    // Windows 10's tar.exe (libarchive 3.5) can list but not unpack RAR files that use
    // filters ("Parsing filters is unsupported"), which many older maps do: 7-Zip can.
    #[cfg(windows)]
    if let Some(sz) = seven_zip() {
        match seven_zip_listing(&sz, archive) {
            Some((_, true, _)) => return Err(HAS_LINKS.into()),
            Some((size, false, _)) if size > budget => return Err(TOO_BIG.into()),
            Some((size, false, entries)) => {
                bomb_check(entries, size, packed_len(archive))?; // [Caps]
                clear_dir(dest);
                let mut c = sys::command(&sz);
                c.args(seven_zip_x_args(dest)).arg(archive);
                let (ok, err) = run_limited(c, dest, budget)?;
                strip_links(dest);
                if ok {
                    check_tree(dest, None)?;
                    return not_a_zip_package(dest);
                }
                tar_err = Some(seven_zip_error(&err));
            }
            None => {}
        }
    }
    let unar = sys::find_tool("unar");
    if let (Some(unar), Some(lsar)) = (unar.as_ref(), sys::find_tool("lsar")) {
        if let Some(entries) = lsar_entries(&lsar, archive) {
            let flag = |e: &serde_json::Value, k: &str| e.get(k).map(|v| v.as_bool() == Some(true) || v.as_i64() == Some(1)).unwrap_or(false);
            if entries.iter().any(|e| flag(e, "XADIsLink") || flag(e, "XADIsHardLink")) {
                return Err(HAS_LINKS.into());
            }
            let size: u64 = entries.iter().map(|e| e.get("XADFileSize").and_then(|v| v.as_u64()).unwrap_or(0)).sum();
            if size > budget {
                return Err(TOO_BIG.into());
            }
            bomb_check(entries.len(), size, packed_len(archive))?; // [Caps]
            clear_dir(dest);
            let mut c = sys::command(unar);
            c.args(["-q", "-f", "-D", "-o"]).arg(dest).arg(archive);
            let (ok, _) = run_limited(c, dest, budget)?;
            strip_links(dest);
            if ok {
                let files = entries.iter().filter(|e| !e.get("XADIsDirectory").is_some_and(|v| v.as_bool() == Some(true) || v.as_i64() == Some(1))).count();
                check_tree(dest, Some(files))?;
                return not_a_zip_package(dest);
            }
        }
    }
    let why_src = tar_err.unwrap_or(list_err);
    let why = why_src.trim().lines().last().unwrap_or("unknown error").to_string();
    let why = if why.is_empty() { "unknown error".to_string() } else { why };
    let hint = if unar.is_none() && cfg!(target_os = "macos") {
        " Installing unar (brew install unar) lets Loner-Strike open newer RAR files."
    } else {
        ""
    };
    Err(format!("Couldn't fully unpack the download ({why}).{hint}"))
}

/// (total size, has links, readable) from `tar -tvf` output. Anything we can't
/// read means "don't let tar unpack it".
pub fn read_listing(ok: bool, out: &str) -> (u64, bool, bool) {
    let (mut total, mut links, mut readable) = (0u64, false, ok);
    for line in out.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        match cols.get(4).and_then(|c| if c.bytes().all(|b| b.is_ascii_digit()) { c.parse::<u64>().ok() } else { None }) {
            Some(n) if cols.len() >= 5 => total += n,
            _ => {
                readable = false;
                break;
            }
        }
        links = links || line.starts_with('l') || line.starts_with('h');
    }
    (total, links, readable)
}

fn tar_path() -> Option<PathBuf> {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        let p = root.join("System32").join("tar.exe");
        return p.is_file().then_some(p).or_else(|| sys::find_tool("tar"));
    }
    let p = PathBuf::from("/usr/bin/tar");
    if cfg!(target_os = "macos") && p.is_file() {
        return Some(p);
    }
    sys::find_tool("bsdtar").or_else(|| p.is_file().then_some(p))
}

/// 7-Zip for Windows: the copy the Windows build ships next to the app first (bundle
/// resource 7zip/, see tauri.windows.conf.json; a known, current version), then the user's
/// own install (dev builds have no bundled copy; an old install may not take -bso0).
#[cfg(windows)]
fn seven_zip() -> Option<PathBuf> {
    let installed = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|d| PathBuf::from(d).join("7-Zip").join("7z.exe"));
    let bundled = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("7zip").join("7z.exe")));
    bundled.into_iter().chain(installed).find(|p| p.is_file())
}

/// `7z x` arguments (before the archive). 7-Zip does make symbolic and hard links on
/// extraction unless told not to (seen with 26.05: "Cannot create symbolic link" under Wine),
/// so -snl- -snh- skip link entries; -sns- writes no NTFS alternate streams (they'd be
/// invisible to the size watcher) and turns a ':' in a name into '_'. Without -spf it keeps
/// every path inside dest: "..", drive letters and \\server\share are dropped, CON/AUX/NUL
/// get a leading '_'. The output folder goes in as an OsString so no character is lost.
#[cfg_attr(not(windows), allow(dead_code))]
fn seven_zip_x_args(dest: &Path) -> Vec<std::ffi::OsString> {
    let mut out = std::ffi::OsString::from("-o");
    out.push(dest.as_os_str());
    let mut v: Vec<std::ffi::OsString> = ["x", "-y", "-bso0", "-bsp0", "-snl-", "-snh-", "-sns-"].iter().map(Into::into).collect();
    v.push(out);
    v
}

/// The line worth showing from 7-Zip's stderr: its last ERROR line ("Open ERROR: Cannot
/// open the file as [7z] archive", "ERROR: Data Error : maps\x.bsp"), else its last line.
#[cfg_attr(not(windows), allow(dead_code))]
fn seven_zip_error(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let line = lines.iter().rev().find(|l| l.contains("ERROR")).or(lines.last()).copied().unwrap_or("unknown error");
    format!("7-Zip: {line}")
}

/// (total unpacked size, has links, entries) from `7z l -slt`, or None if 7-Zip can't read it.
#[cfg(windows)]
fn seven_zip_listing(sz: &Path, archive: &Path) -> Option<(u64, bool, usize)> {
    let mut c = sys::command(sz);
    c.args(["l", "-slt"]).arg(archive);
    let (ok, out, _) = sys::run_output(c, Duration::from_secs(300))?;
    if !ok {
        return None;
    }
    read_7z_listing(&out)
}

/// The `7z l -slt` text: entries follow the "----------" line (the lines before it describe
/// the archive); each entry starts with "Path = ". [Caps] the entry count is for bomb_check.
/// Links: tar/RAR5 entries say "Symbolic Link = /target" (or Hard/Copy Link), but a zip's
/// Unix link only shows in its mode ("Attributes =  lrwxrwxrwx"; tar's "Mode = lrwx...").
#[cfg_attr(not(windows), allow(dead_code))]
fn read_7z_listing(out: &str) -> Option<(u64, bool, usize)> {
    let entries = out.split_once("\n----------")?.1;
    let (mut total, mut links, mut count) = (0u64, false, 0usize);
    let unix_link = |v: &str| {
        v.split(' ').any(|t| t.len() == 10 && t.starts_with('l') && t[1..].bytes().all(|b| b"rwxsStT-".contains(&b)))
    };
    for line in entries.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(n) = line.strip_prefix("Size = ") {
            total = total.saturating_add(n.trim().parse::<u64>().unwrap_or(0));
        } else if line.starts_with("Path = ") {
            count += 1;
        } else if let Some(v) = ["Symbolic Link = ", "Hard Link = ", "Copy Link = "].iter().find_map(|k| line.strip_prefix(k)) {
            links = links || !v.trim().is_empty();
        } else if let Some(v) = line.strip_prefix("Attributes = ").or_else(|| line.strip_prefix("Mode = ")) {
            links = links || unix_link(v);
        }
    }
    Some((total, links, count))
}

fn lsar_entries(lsar: &Path, archive: &Path) -> Option<Vec<serde_json::Value>> {
    let mut c = sys::command(lsar);
    c.arg("-j").arg(archive);
    let (ok, out, _) = sys::run_output(c, Duration::from_secs(300))?;
    if !ok {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&out).ok()?;
    v.get("lsarContents")?.as_array().cloned()
}

thread_local! {
    /// Entries the last unpack on this thread left out for safety (links, paths outside the
    /// download): plan() lists them with what it skipped, so they aren't dropped silently.
    static DROPPED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn note_dropped(what: String) {
    DROPPED.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() < 40 {
            d.push(what);
        }
    });
}

/// What the last unpack on this thread left out for safety, once.
pub fn take_dropped() -> Vec<String> {
    DROPPED.with(|d| std::mem::take(&mut *d.borrow_mut()))
}

/// Zip files are read here directly. Ok(false): not a zip we can read, let tar try.
/// An archive entry's name as it unpacks: / for \\, no ./ or empty parts, lower case.
pub fn normalized_entry(name: &str) -> String {
    name.replace('\\', "/").split('/').filter(|p| !p.is_empty() && *p != ".").collect::<Vec<_>>().join("/").to_lowercase()
}

/// A creature maker's own folder (models/gb<id>/..., sound/gb<id>/...): only that maker's own
/// creature list puts files there (creaturepack::install_pack), never a download's file plan.
pub fn creature_folder(dest: &str) -> bool {
    let low = dest.replace('\\', "/").to_ascii_lowercase();
    ["models/", "sound/"].iter().any(|top| {
        low.strip_prefix(top).and_then(|r| r.split('/').next()).and_then(|ns| ns.strip_prefix("gb")).is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
    })
}

/// Files an installer acts on (a Loner-Strike package's manifest, its shared creatures, a map's
/// editable source): an archive carrying any of them is checked strictly (`strict_problem`) and
/// unpacked only from a zip.
pub fn has_marker(names: &[String]) -> bool {
    names.iter().any(|n| {
        let k = normalized_entry(n);
        k == "loner-strike.json" || k.ends_with("/loner-strike.json") || k == "creatures/creatures.ini" || k.ends_with("/creatures/creatures.ini") || k.starts_with("source/") || k == "source"
    })
}

/// A file tree that holds a marker (after another unpacker than ours made it).
fn tree_has_marker(dest: &Path) -> bool {
    let names: Vec<String> = walk(dest).iter().filter_map(|p| p.strip_prefix(dest).ok().map(|r| r.to_string_lossy().replace('\\', "/"))).collect();
    has_marker(&names)
}

/// One that came out of a rar, 7z or tarball carrying a marker is refused, its files cleared.
fn not_a_zip_package(dest: &Path) -> Result<(), String> {
    if tree_has_marker(dest) {
        clear_dir(dest);
        return Err("Loner-Strike packages come as .zip files; this one isn't, so it wasn't installed.".into());
    }
    Ok(())
}

/// Entry names, strictly (every Community install, and any archive with a marker): plain ASCII
/// letters, digits and `_ . / -` and spaces; no part that climbs out (..), is empty, ends in a dot
/// or a space, looks like a short name (`x~1`) or is a name Windows keeps for a device; and no
/// two that unpack to the same file (compared in lower case). The first problem, plainly.
pub fn strict_problem(names: &[String]) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    for n in names {
        let shown: String = n.chars().filter(|c| !c.is_control()).take(80).collect();
        let bad = |why: &str| Some(format!("This download has a file named so it can't be installed safely ({shown}: {why}), so it wasn't installed."));
        if !n.bytes().all(|c| c.is_ascii_alphanumeric() || b"_./ -".contains(&c)) {
            return bad("only letters, digits, spaces and _ . / - can be used");
        }
        let dir = n.ends_with('/');
        let body = n.strip_suffix('/').unwrap_or(n);
        if body.is_empty() || body.starts_with('/') {
            return bad("it starts at the root");
        }
        for part in body.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return bad("it points outside the download");
            }
            if part.ends_with('.') || part.ends_with(' ') || part.starts_with(' ') {
                return bad("a name ends in a dot or a space");
            }
            if let Some(t) = part.find('~') {
                if part[t + 1..].bytes().next().is_some_and(|c| c.is_ascii_digit()) {
                    return bad("it looks like a short name");
                }
            }
            if crate::setup::record::windows_unsafe(part) {
                return bad("Windows keeps that name for a device");
            }
        }
        if !dir && !seen.insert(body.to_ascii_lowercase()) {
            return Some(format!("This download has two files that unpack as {}, so it wasn't installed.", body.to_ascii_lowercase()));
        }
    }
    None
}

/// A part of a written path that some system would read as another file: a trailing dot or space,
/// a short name (`x~1`), a Windows device name, `..` or a drive.
pub fn alias_part(part: &str) -> Option<&'static str> {
    if part == ".." || part == "." {
        return Some("it points outside the download");
    }
    if part.contains(':') {
        return Some("it names a drive");
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return Some("a name ends in a dot or a space");
    }
    if let Some(t) = part.find('~') {
        if part[t + 1..].bytes().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some("it looks like a short name");
        }
    }
    if crate::setup::record::windows_unsafe(part) {
        return Some("Windows keeps that name for a device");
    }
    None
}

/// An entry whose stored name and the name it's written as differ in more than slash style (a
/// Unicode name field renaming it).
pub fn written_name_problem(raw: &[u8], resolved: &str) -> Option<&'static str> {
    let norm = |s: &str| s.replace('\\', "/");
    let same = match std::str::from_utf8(raw) {
        Ok(r) => norm(r) == norm(resolved),
        // (an old code page: its letters decode differently, but its folders, dots and plain
        // letters must stay where they are)
        Err(_) => {
            let a: String = raw.iter().filter(|b| b.is_ascii()).map(|b| if *b == b'\\' { '/' } else { *b as char }).collect();
            let b: String = resolved.chars().filter(|c| c.is_ascii()).map(|c| if c == '\\' { '/' } else { c }).collect();
            a == b
        }
    };
    (!same).then_some("its stored name and the name it would be written as differ")
}

/// A zip's entry names exactly as its central directory stores them, in order (None: a zip64
/// directory or one that can't be read here: the reader's raw names are used).
fn raw_names(path: &Path) -> Option<Vec<Vec<u8>>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let tail_len = len.min(66_000);
    f.seek(SeekFrom::Start(len - tail_len)).ok()?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail).ok()?;
    let at = (0..tail.len().saturating_sub(21)).rev().find(|i| tail[*i..*i + 4] == [0x50, 0x4b, 0x05, 0x06])?;
    let e = &tail[at..];
    let u16_at = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as u64;
    let u32_at = |b: &[u8], o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as u64;
    let (count, size, off) = (u16_at(e, 10), u32_at(e, 12), u32_at(e, 16));
    if count == 0xFFFF || size == 0xFFFF_FFFF || off == 0xFFFF_FFFF || size > 64 << 20 || off + size > len {
        return None;
    }
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut cd = vec![0u8; size as usize];
    f.read_exact(&mut cd).ok()?;
    let (mut out, mut p) = (Vec::new(), 0usize);
    for _ in 0..count {
        if cd.len() < p + 46 || cd[p..p + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return None;
        }
        let (n, x, c) = (u16_at(&cd, p + 28) as usize, u16_at(&cd, p + 30) as usize, u16_at(&cd, p + 32) as usize);
        out.push(cd.get(p + 46..p + 46 + n)?.to_vec());
        p += 46 + n + x + c;
    }
    Some(out)
}

/// An entry's name check: against its stored name when the zip's own directory could be read;
/// when it couldn't, an entry with a Unicode Path field (which can rename it) is refused, since
/// the reader's own "raw" name may already be that field's.
pub fn name_check(stored: Option<&[u8]>, unicode_field: bool, reader_raw: &[u8], resolved: &str) -> Option<&'static str> {
    match stored {
        Some(raw) => written_name_problem(raw, resolved),
        None if unicode_field => Some("it carries a Unicode name field this download's directory can't confirm"),
        None => written_name_problem(reader_raw, resolved),
    }
}

/// After another unpacker (tar, 7-Zip, unar): every written path checked as a zip's entries are,
/// and as many files as its listing had (a twin written over another leaves fewer).
fn check_tree(dest: &Path, listed_files: Option<usize>) -> Result<(), String> {
    let files = walk(dest);
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        let rel = f.strip_prefix(dest).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        let why = rel.split('/').find_map(alias_part);
        if why.is_some() || !seen.insert(rel.to_lowercase()) {
            clear_dir(dest);
            return Err(format!("This download has a file named so it could land somewhere else ({rel}), so it wasn't installed."));
        }
    }
    if listed_files.is_some_and(|n| files.len() < n) {
        clear_dir(dest);
        return Err("This download has two files that unpack as the same one, so it wasn't installed.".into());
    }
    Ok(())
}

/// Unpacks a Community download: only a zip, every name checked strictly, every file new (a
/// twin can never write over an earlier one), no archives inside it unpacked.
pub fn extract_strict(archive: &Path, dest: &Path, budget: u64) -> Result<(), String> {
    match extract_zip_with(archive, dest, budget, true)? {
        true => Ok(()),
        false => {
            clear_dir(dest);
            Err("Loner-Strike creations come as .zip files; this one isn't, so it wasn't installed.".into())
        }
    }
}

fn extract_zip(archive: &Path, dest: &Path, budget: u64) -> Result<bool, String> {
    extract_zip_with(archive, dest, budget, false)
}

fn extract_zip_with(archive: &Path, dest: &Path, budget: u64, strict_always: bool) -> Result<bool, String> {
    let _ = take_dropped(); // (a fresh list for this archive)
    use std::io::{Read, Write};
    let Ok(f) = std::fs::File::open(archive) else { return Ok(false) };
    let Ok(mut z) = zip::ZipArchive::new(f) else { return Ok(false) };
    let mut listed: u64 = 0;
    for i in 0..z.len() {
        let Ok(m) = z.by_index_raw(i) else { return Ok(false) };
        listed = listed.saturating_add(m.size());
    }
    if listed > budget {
        return Err(TOO_BIG.into());
    }
    bomb_check(z.len(), listed, packed_len(archive))?; // [Caps]
    // a Community download, or any zip carrying a file an installer acts on, is checked strictly
    // (the community list checks each entry, so a later twin can't replace a checked one)
    let names: Vec<String> = (0..z.len()).filter_map(|i| z.by_index_raw(i).ok().map(|m| m.name_raw().iter().map(|b| *b as char).collect())).collect();
    // (Community installs, and any zip carrying a file an installer acts on: plain ASCII names only,
    // every one checked before anything unpacks)
    if strict_always || has_marker(&names) {
        if let Some(why) = strict_problem(&names) {
            return Err(why);
        }
    }
    let mut written: u64 = 0; // the sizes in a zip's index can lie, so count while writing too
    let mut buf = vec![0u8; 1024 * 1024];
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    // the names exactly as stored (the reader shows a Unicode name field's name instead)
    let raw = raw_names(archive).filter(|r| r.len() == z.len());
    for i in 0..z.len() {
        let Ok(mut m) = z.by_index(i) else { return Ok(false) };
        if m.is_dir() {
            continue;
        }
        // (left out, and said so in the install's "skipped": P5, QA4-G)
        let shown: String = m.name().map(|n| n.to_string()).unwrap_or_default().chars().filter(|c| !c.is_control()).take(120).collect();
        if m.is_symlink() {
            note_dropped(format!("{shown} (a link: never unpacked)"));
            continue; // links are never unpacked
        }
        // every archive: the path really written is checked (after any Unicode name field), and the
        // download is refused when it could land anywhere but where its listing says
        let stored = raw.as_ref().and_then(|r| r.get(i)).map(|v| v.as_slice());
        let renamed = m.extra_data_fields().any(|f| match f {
            zip::extra_fields::ExtraField::UnicodePath(_) => true,
            zip::extra_fields::ExtraField::Custom(c) => c.header_id == 0x7075,
            _ => false,
        });
        if let Some(why) = name_check(stored, renamed, m.name_raw(), &m.name().map(|n| n.to_string()).unwrap_or_default()) {
            return Err(format!("This download has a file whose name doesn't add up ({shown}: {why}), so it wasn't installed."));
        }
        let Some(rel) = m.enclosed_name() else {
            return Err(format!("This download has a file that points outside it ({shown}), so it wasn't installed."));
        };
        let target = dest.join(&rel);
        if !target.starts_with(dest) || target == dest {
            return Err(format!("This download has a file that points outside it ({shown}), so it wasn't installed."));
        }
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        if let Some(why) = rel_s.split('/').find_map(alias_part) {
            return Err(format!("This download has a file named so it could land somewhere else ({shown}: {why}), so it wasn't installed."));
        }
        if !seen.insert(rel_s.to_lowercase()) {
            return Err(format!("This download has two files that unpack as {}, so it wasn't installed.", rel_s.to_lowercase()));
        }
        if let Some(d) = target.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        // (each file new: nothing can write over one unpacked before it, or one already there)
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|_| format!("This download has two files that unpack as {shown}, so it wasn't installed."))?;
        loop {
            let n = match m.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => return Ok(false),
            };
            written += n as u64;
            if written > budget {
                return Err(TOO_BIG.into());
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
    }
    Ok(true)
}

/// Runs an unpacker, stopping it as soon as dest grows past budget bytes.
///
/// The listing checked beforehand can lie about sizes, so the folder is watched
/// while the unpacker works; on Unix no single file may exceed the budget either.
/// Returns (succeeded, stderr).
pub fn run_limited(cmd: Command, dest: &Path, budget: u64) -> Result<(bool, String), String> {
    run_limited_n(cmd, dest, budget, MAX_ENTRIES)
}

/// run_limited with a file-count limit too ([Caps]: the listing checked first can lie about
/// how many files there are, so they're counted while the unpacker works).
fn run_limited_n(mut cmd: Command, dest: &Path, budget: u64, max_entries: usize) -> Result<(bool, String), String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0); // its own group, so stopping it also stops anything it started
        let limit = budget as libc::rlim_t;
        unsafe {
            cmd.pre_exec(move || {
                let r = libc::rlimit { rlim_cur: limit, rlim_max: limit };
                libc::setrlimit(libc::RLIMIT_FSIZE, &r);
                Ok(())
            });
        }
    }
    let mut child = spawn_retrying(&mut cmd).map_err(|e| format!("Couldn't run the unpacker ({e})."))?;
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            let _ = std::io::Read::read_to_string(e, &mut s);
        }
        s
    });
    let kill = |child: &mut std::process::Child| {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(500));
                let (size, entries) = tree_stats(dest, max_entries);
                if size > budget {
                    kill(&mut child);
                    let _ = reader.join();
                    return Err(TOO_BIG.into());
                }
                if entries > max_entries {
                    kill(&mut child);
                    let _ = reader.join();
                    return Err(too_many(entries, max_entries));
                }
            }
            Err(e) => {
                kill(&mut child);
                return Err(e.to_string());
            }
        }
    };
    // anything it left running in its group goes too
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    let err = reader.join().unwrap_or_default();
    // an unpacker done between two looks is checked once more, for files as well as size
    let (size, entries) = tree_stats(dest, max_entries);
    if size > budget {
        return Err(TOO_BIG.into());
    }
    if entries > max_entries {
        return Err(too_many(entries, max_entries));
    }
    Ok((status.success(), err))
}

/// Starts a program, trying again for a moment when the system is briefly out of processes
/// or memory (fork's EAGAIN/ENOMEM, an interrupted call): a busy machine (the game, a build,
/// the full test suite) shouldn't fail an install that would work half a second later.
fn spawn_retrying(cmd: &mut Command) -> std::io::Result<std::process::Child> {
    let mut wait = Duration::from_millis(50);
    for _ in 0..6 {
        match cmd.spawn() {
            Err(e) if spawn_transient(&e) => {
                std::thread::sleep(wait);
                wait *= 2;
            }
            other => return other,
        }
    }
    cmd.spawn()
}

fn spawn_transient(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(e.kind(), WouldBlock | Interrupted | OutOfMemory)
}

/// Some uploads put the real archive inside the archive.
pub fn extract_nested(root: &Path) -> Result<(), String> {
    for path in walk(root) {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if is_archive(&name) {
            let inner = sys::make_temp_dir(root, "nested-").map_err(|e| e.to_string())?;
            let size = tree_size(root);
            if size >= MAX_UNPACKED {
                return Err(TOO_BIG.into());
            }
            // an inner archive that can't be unpacked (or is refused) is left out as before,
            // but never half of it: a stopped unpacker's files are removed too
            if extract(&path, &inner, MAX_UNPACKED - size).is_err() {
                let _ = std::fs::remove_dir_all(&inner);
            }
        }
    }
    Ok(())
}

// ---- choosing what to install --------------------------------------------------

/// Python's os.path.normpath for a relative, '/'-separated path.
fn normpath(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if out.last().map(|l| *l != "..").unwrap_or(false) {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            c => out.push(c),
        }
    }
    out.join("/")
}

/// [(source file, path relative to cstrike/)] for every map file in the download,
/// and what was left out (at most 40).
pub fn plan(root: &Path) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let files = walk(root);
    let rels: Vec<Vec<String>> = files.iter().map(|p| rel_parts(root, p)).collect();
    let (picked, mut skipped) = plan_rels(&rels);
    // what the unpack left out for safety comes first, so it shows within the 40
    let mut dropped = take_dropped();
    dropped.append(&mut skipped);
    dropped.truncate(40);
    (picked.into_iter().map(|(i, d)| (files[i].clone(), d)).collect(), dropped)
}

/// The parts of path below root ("a/b/c.bsp" -> ["a", "b", "c.bsp"]).
pub fn rel_parts(root: &Path, path: &Path) -> Vec<String> {
    let Ok(relp) = path.strip_prefix(root) else { return vec![] };
    relp.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .map(String::from)
        .collect()
}

/// A path from an archive listing ("dir\\x/y.bsp") as parts, dropping "", "." and anything with "..".
/// A leading drive ("C:\\maps\\x.bsp") is dropped, as 7-Zip and tar do when they unpack.
pub fn listed_parts(rel: &str) -> Vec<String> {
    let mut parts: Vec<String> = rel.split(['/', '\\']).filter(|s| !s.is_empty() && *s != ".").map(String::from).collect();
    if parts.iter().any(|p| p == "..") {
        return vec![];
    }
    if parts.first().is_some_and(|d| d.len() == 2 && d.ends_with(':') && d.as_bytes()[0].is_ascii_alphabetic()) {
        parts.remove(0);
    }
    parts
}

/// plan() on paths alone (from the unpacked folder, or from GameBanana's listing of an
/// archive): [(index into rels, destination)], and what was left out.
pub fn plan_rels(rels: &[Vec<String>]) -> (Vec<(usize, String)>, Vec<String>) {
    let (mut plan, mut seen, mut skipped) = (Vec::new(), std::collections::HashSet::new(), Vec::new());
    let bsps: std::collections::HashSet<String> = rels
        .iter()
        .filter_map(|p| p.last().map(|n| n.to_lowercase()))
        .filter(|n| n.ends_with(".bsp"))
        .map(|n| n[..n.len() - 4].to_string())
        .collect();
    // P5: a Loner-Strike package's source/ (the editable source: its project, .map and texture
    // file) is for Remix, never installed into the game
    let ls_package = rels.iter().any(|p| p.len() == 1 && p[0].eq_ignore_ascii_case("loner-strike.json"));
    for (index, parts) in rels.iter().enumerate() {
        if parts.is_empty() || (ls_package && parts.len() > 1 && parts[0].eq_ignore_ascii_case("source")) {
            continue;
        }
        let rel = parts.join("/");
        // ':' is a drive ("C:x.wad") or an NTFS stream ("x.bsp:s") on Windows, never a game file
        if parts.iter().any(|p| p.contains(':')) {
            skipped.push(rel);
            continue;
        }
        let low: Vec<String> = parts.iter().map(|p| p.to_lowercase()).collect();
        let name = low.last().unwrap().clone();
        let ext = ext_of(&name).to_string();
        let file = parts.last().unwrap().clone();
        let dirs = &low[..low.len() - 1];
        // the outermost folder that takes this kind of file, so models/dx/maps/x.mdl
        // stays a model rather than counting as something under maps/
        let idx = dirs
            .iter()
            .position(|p| content_exts(p).map(|e| e.contains(&ext.as_str())).unwrap_or(false))
            .or_else(|| dirs.iter().rposition(|p| content_exts(p).is_some()));
        let at = idx.map(|i| low[i].as_str());
        let fits = |d: &str| content_exts(d).map(|e| e.contains(&ext.as_str())).unwrap_or(false);
        let dest: Option<String> = if at == Some("maps") && fits("maps") {
            Some(format!("maps/{file}")) // the game only loads maps straight from maps/
        } else if let (Some(i), true) = (idx, at.map(fits).unwrap_or(false)) {
            Some(std::iter::once(low[i].clone()).chain(parts[i + 1..].iter().cloned()).collect::<Vec<_>>().join("/"))
        } else if ext == ".wad" {
            Some(file.clone())
        } else if ext == ".bsp" || ext == ".nav" || ext == ".res" {
            Some(format!("maps/{file}"))
        } else if ext == ".txt" && bsps.contains(&name[..name.len() - 4]) {
            Some(format!("maps/{file}"))
        } else if ext == ".spr" {
            Some(format!("sprites/{file}"))
        } else if (ext == ".tga" || ext == ".bmp") && SKY_SIDES.iter().any(|s| name[..name.len() - 4].ends_with(s)) {
            Some(format!("gfx/env/{file}"))
        } else {
            None
        };
        let Some(dest) = dest else {
            if !is_archive(&name) {
                skipped.push(rel);
            }
            continue;
        };
        let dest = normpath(&dest);
        if dest.is_empty() || dest.starts_with("..") || seen.contains(&dest.to_lowercase()) {
            continue;
        }
        // a creature maker's own folder only comes through that maker's own creature list
        if creature_folder(&dest) {
            skipped.push(format!("{rel} (a creature maker's own folder)"));
            continue;
        }
        if dest.starts_with("maps/") && !safe_map(stem_of(&file)) {
            skipped.push(format!("{rel} (name has characters the game console can't take)"));
            continue;
        }
        seen.insert(dest.to_lowercase());
        plan.push((index, dest));
    }
    skipped.truncate(40);
    (plan, skipped)
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_names_with_dollar() {
        assert!(super::safe_map("$2000$") && super::safe_map("aim_ak-colt") && crate::maps::safe_arg("$2000$"));
        assert!(!super::safe_map("de_dust2;quit") && !super::safe_map("a b") && !super::safe_map("x\"y"));
    }

    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join("loner-strike-tests");
        sys::make_temp_dir(&base, tag).unwrap()
    }

    fn put(root: &Path, rel: &str, data: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }

    #[test]
    fn map_names() {
        assert!(safe_map("de_dust2"));
        assert!(safe_map("aim_map-v2.1"));
        assert!(!safe_map(""));
        assert!(!safe_map("de dust"));
        assert!(!safe_map("de_dust;quit"));
        assert!(!safe_map("de_\"x"));
        assert!(!safe_map(&"a".repeat(32)));
        assert!(safe_map(&"a".repeat(31)));
        assert!(!safe_map("dé_dust"));
    }

    #[test]
    fn splitext_like_python() {
        assert_eq!(ext_of("a.bsp"), ".bsp");
        assert_eq!(ext_of(".bashrc"), "");
        assert_eq!(ext_of("a.tar.gz"), ".gz");
        assert_eq!(ext_of("noext"), "");
        assert_eq!(normpath("maps/./x/../y.bsp"), "maps/y.bsp");
        assert_eq!(normpath("../x"), "../x");
    }

    #[test]
    fn listing() {
        let ok = "-rw-r--r--  0 user staff    1234 Jan  1 12:00 maps/a.bsp\n-rw-r--r--  0 user staff 10 Jan  1 12:00 b.wad\n";
        assert_eq!(read_listing(true, ok), (1244, false, true));
        let link = "lrwxr-xr-x  0 user staff       0 Jan  1 12:00 x -> /etc/passwd\n";
        assert_eq!(read_listing(true, link), (0, true, true));
        let hard = "hrw-r--r--  0 user staff       0 Jan  1 12:00 y link to x\n";
        assert!(read_listing(true, hard).1);
        assert!(!read_listing(true, "garbage line\n").2);
        assert!(!read_listing(false, "").2);
    }

    #[test]
    fn plan_flattens_and_filters() {
        let r = tmp("plan-");
        put(&r, "MyPack/cstrike/maps/sub/de_test.bsp", b"bsp");
        put(&r, "MyPack/cstrike/maps/sub/de_test.txt", b"briefing");
        put(&r, "MyPack/cstrike/maps/de_test.nav", b"nav");
        put(&r, "MyPack/de_test.wad", b"wad");
        put(&r, "MyPack/models/dx/maps/thing.mdl", b"mdl");
        put(&r, "MyPack/sound/ambience/wind.wav", b"wav");
        put(&r, "MyPack/gfx/env/skyup.tga", b"tga");
        put(&r, "loose/coolskyft.bmp", b"bmp");
        put(&r, "loose/glow.spr", b"spr");
        put(&r, "loose/readme.txt", b"readme");
        put(&r, "loose/evil.dll", b"dll");
        put(&r, "loose/server.cfg", b"cfg");
        put(&r, "loose/bad name.bsp", b"bsp");
        put(&r, "loose/inner.zip", b"zip");
        put(&r, "other/maps/de_test.bsp", b"dupe");
        let (plan, skipped) = plan(&r);
        let dests: Vec<&str> = plan.iter().map(|(_, d)| d.as_str()).collect();
        for want in [
            "maps/de_test.bsp",
            "maps/de_test.txt",
            "maps/de_test.nav",
            "de_test.wad",
            "models/dx/maps/thing.mdl",
            "sound/ambience/wind.wav",
            "gfx/env/skyup.tga",
            "gfx/env/coolskyft.bmp",
            "sprites/glow.spr",
        ] {
            assert!(dests.contains(&want), "missing {want} in {dests:?}");
        }
        assert_eq!(dests.iter().filter(|d| **d == "maps/de_test.bsp").count(), 1);
        assert!(!dests.iter().any(|d| d.contains("dll") || d.contains("cfg") || d.contains("readme")));
        assert!(skipped.iter().any(|s| s == "loose/evil.dll"));
        assert!(skipped.iter().any(|s| s == "loose/server.cfg"));
        assert!(skipped.iter().any(|s| s.starts_with("loose/bad name.bsp (name has")));
        assert!(!skipped.iter().any(|s| s.contains("inner.zip")));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_never_followed_or_kept() {
        let r = tmp("links-");
        put(&r, "maps/de_ok.bsp", b"x");
        std::os::unix::fs::symlink("/etc/passwd", r.join("maps/de_evil.bsp")).unwrap();
        std::os::unix::fs::symlink("/etc", r.join("models")).unwrap();
        let (p, _) = plan(&r);
        assert_eq!(p.len(), 1, "{p:?}");
        strip_links(&r);
        assert!(std::fs::symlink_metadata(r.join("maps/de_evil.bsp")).is_err());
        assert!(std::fs::symlink_metadata(r.join("models")).is_err());
        assert!(r.join("maps/de_ok.bsp").exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn tar_with_link_is_refused() {
        let r = tmp("tarlink-");
        let src = r.join("src");
        put(&src, "maps/de_a.bsp", b"bsp");
        std::os::unix::fs::symlink("/etc/passwd", src.join("maps/de_a.txt")).unwrap();
        let archive = r.join("x.tar");
        let st = Command::new(tar_path().unwrap()).arg("-cf").arg(&archive).arg("-C").arg(&src).arg("maps").status().unwrap();
        assert!(st.success());
        let dest = r.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let e = extract(&archive, &dest, MAX_UNPACKED);
        // tar's listing shows the link, and lsar's does too when unar is installed
        assert!(e.unwrap_err().contains("links"));
        assert!(std::fs::symlink_metadata(dest.join("maps/de_a.txt")).map(|m| !m.file_type().is_symlink()).unwrap_or(true));
        let _ = std::fs::remove_dir_all(&r);
    }

    // [Caps]
    #[test]
    fn zip_with_too_many_files_is_refused() {
        use std::io::Write;
        let r = tmp("zipmany-");
        let archive = r.join("many.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for i in 0..=MAX_ENTRIES {
                z.start_file(format!("f{i}"), o).unwrap();
                z.write_all(b"x").unwrap();
            }
            z.finish().unwrap();
        }
        let dest = r.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        assert_eq!(extract(&archive, &dest, MAX_UNPACKED).unwrap_err(), "That download has too many files in it: 65537 — the most is 65536.");
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0, "nothing unpacked");
        let _ = std::fs::remove_dir_all(&r);
    }

    // [Caps] the 7-Zip listing (Windows' unpacker for RAR/7z) gives the real file count
    #[test]
    fn seven_zip_listing_counts_entries() {
        let mut out = String::from("7-Zip 23.01\n\nListing archive: x.7z\n\n--\nPath = x.7z\nType = 7z\nPhysical Size = 900\n\n----------\n");
        for i in 0..=MAX_ENTRIES {
            out.push_str(&format!("Path = f{i}\r\nSize = 1\r\nAttributes = A\r\n\r\n"));
        }
        let (size, links, n) = read_7z_listing(&out).unwrap();
        assert_eq!((size, links, n), (MAX_ENTRIES as u64 + 1, false, MAX_ENTRIES + 1), "the archive's own Path line (before the separator) isn't counted");
        assert_eq!(bomb_check(n, size, 900).unwrap_err(), "That download has too many files in it: 65537 — the most is 65536.");
        let link = "--\nPath = a\n\n----------\nPath = maps/x.txt\nSize = 3\nSymbolic Link = /etc/passwd\n";
        assert_eq!(read_7z_listing(link), Some((3, true, 1)));
    }

    // Real `7z l -slt` output from the bundled 7-Zip 26.05 (x64), run under CrossOver, with
    // Windows line endings. Names are printed in the OEM code page ("ü" is byte 0x81, which
    // run_output's lossy decoding turns into U+FFFD); only the ASCII keys are read.
    fn crlf(s: &str) -> String {
        s.replace('\n', "\r\n")
    }

    const SZ_7Z: &str = "\n7-Zip 26.05 (x64) : Copyright (c) 1999-2026 Igor Pavlov : 2026-10-21\n\nScanning the drive for archives:\n1 file, 262 bytes (1 KiB)\n\nListing archive: C:\\Users\\Jo Smith\\Downloads\\my pack \u{FFFD}.7z\n\n--\nPath = C:\\Users\\Jo Smith\\Downloads\\my pack \u{FFFD}.7z\nType = 7z\nPhysical Size = 262\nHeaders Size = 251\nMethod = LZMA2:12\nSolid = +\nBlocks = 1\n\n----------\nPath = maps\nSize = 0\nPacked Size = 0\nModified = 2026-11-13 01:15:33.2128504\nAttributes = D\nCRC = \nEncrypted = -\nMethod = \nBlock = \n\nPath = sound\\my folder\nSize = 0\nPacked Size = 0\nModified = 2026-11-13 01:15:33.2127187\nAttributes = D\nCRC = \nEncrypted = -\nMethod = \nBlock = \n\nPath = maps\\de_test.bsp\nSize = 3\nPacked Size = 11\nModified = 2026-11-13 01:15:33.2126648\nAttributes = A\nCRC = E0059D55\nEncrypted = -\nMethod = LZMA2:12\nBlock = 0\n\nPath = maps\\__.txt\nSize = 1\nPacked Size = \nModified = 2026-11-13 01:15:33.2128959\nAttributes = A\nCRC = 8CDC1683\nEncrypted = -\nMethod = LZMA2:12\nBlock = 0\n\nPath = sound\\my folder\\h\u{FFFD} llo \u{FFFD}.wav\nSize = 3\nPacked Size = \nModified = 2026-11-13 01:15:33.2127990\nAttributes = A\nCRC = 6B7F2928\nEncrypted = -\nMethod = LZMA2:12\nBlock = 0\n\n";

    const SZ_RAR: &str = "\n7-Zip 26.05 (x64) : Copyright (c) 1999-2026 Igor Pavlov : 2026-10-21\n\nScanning the drive for archives:\n1 file, 2565319 bytes (2506 KiB)\n\nListing archive: C:\\Users\\Jo Smith\\AppData\\Local\\Temp\\download.rar\n\n--\nPath = C:\\Users\\Jo Smith\\AppData\\Local\\Temp\\download.rar\nType = Rar\nPhysical Size = 2565319\nSolid = -\nBlocks = 9\nMultivolume = -\nVolumes = 1\n\n----------\nPath = My Map v3\\sound\\loadermusic.wav\nFolder = -\nSize = 2377772\nPacked Size = 1674447\nModified = 2006-08-17 18:32:12.0000000\nCreated = \nAccessed = \nAttributes = A\nEncrypted = -\nSolid = -\nCommented = -\nSplit Before = -\nSplit After = -\nCRC = 8EF63135\nHost OS = Win32\nMethod = m3:22\nVersion = 29\nVolume Index = \n\nPath = My Map v3\\README.txt\nFolder = -\nSize = 221\nPacked Size = 178\nModified = 2009-04-03 10:40:56.5120000\nCreated = \nAccessed = \nAttributes = A\nEncrypted = -\nSolid = -\nCommented = -\nSplit Before = -\nSplit After = -\nCRC = CDDD0BE0\nHost OS = Win32\nMethod = m3:22\nVersion = 29\nVolume Index = \n\n";

    // a zip made by Info-ZIP on Unix, and one with a trailing-garbage warning in the header block
    const SZ_ZIP: &str = "\n7-Zip 26.05 (x64) : Copyright (c) 1999-2026 Igor Pavlov : 2026-10-21\n\nScanning the drive for archives:\n1 file, 965 bytes (1 KiB)\n\nListing archive: C:\\dl\\tail.zip\n\nWARNINGS:\nThere are data after the end of archive\n\n--\nPath = C:\\dl\\tail.zip\nType = zip\nWARNINGS:\nThere are data after the end of archive\nPhysical Size = 951\nTail Size = 14\n\n----------\nPath = maps\nFolder = +\nSize = 0\nPacked Size = 0\nModified = 2026-11-13 01:15:33.2128504\nCreated = \nAccessed = \nAttributes = D\nEncrypted = -\nComment = \nCRC = \nMethod = Store\nCharacteristics = NTFS\nHost OS = FAT\nVersion = 20\nVolume Index = 0\nOffset = 0\n\nPath = maps\\de_a.bsp\nFolder = -\nSize = 3\nPacked Size = 3\nModified = 2026-11-13 01:18:48\nCreated = \nAccessed = \nAttributes = V 01800000 0rw-------\nEncrypted = -\nComment = \nCRC = E0059D55\nMethod = Store\nCharacteristics = \nHost OS = Unix\nVersion = 20\nVolume Index = 0\nOffset = 54\n\n";

    #[test]
    fn seven_zip_listing_real_samples() {
        // the archive's own block (before "----------") is never counted, even with spaces and
        // OEM-mangled names in its path; directories are entries too, like tar's and lsar's lists
        assert_eq!(read_7z_listing(&crlf(SZ_7Z)), Some((7, false, 5)));
        assert_eq!(read_7z_listing(&crlf(SZ_RAR)), Some((2377772 + 221, false, 2)));
        assert_eq!(read_7z_listing(&crlf(SZ_ZIP)), Some((3, false, 2)), "warnings and \"V 01800000 0rw-------\" aren't links");
        assert_eq!(read_7z_listing(SZ_7Z), Some((7, false, 5)), "plain \\n too");
        // no separator: 7-Zip couldn't read it
        assert_eq!(read_7z_listing("\r\n7-Zip 26.05\r\n\r\nOpen ERROR: Cannot open the file as archive\r\n"), None);
    }

    #[test]
    fn seven_zip_listing_sees_every_kind_of_link() {
        let head = "--\r\nPath = C:\\x.bin\r\nType = zip\r\n\r\n----------\r\n";
        // real: a zip with a Unix link has no "Symbolic Link" line, only its mode
        let zip_link = format!("{head}Path = maps\\de_a.txt\r\nFolder = -\r\nSize = 11\r\nAttributes =  lrwxrwxrwx\r\nHost OS = Unix\r\n\r\n");
        assert_eq!(read_7z_listing(&zip_link), Some((11, true, 1)));
        // real: tar
        let tar_link = format!("{head}Path = evil\r\nSize = 4\r\nMode = lrwxr-xr-x\r\nSymbolic Link = /etc\r\nHard Link = \r\n\r\n");
        assert!(read_7z_listing(&tar_link).unwrap().1);
        let tar_mode_only = format!("{head}Path = evil\r\nMode = lrwxr-xr-x\r\n\r\n");
        assert!(read_7z_listing(&tar_mode_only).unwrap().1);
        for k in ["Hard Link = maps\\a.bsp", "Copy Link = maps\\a.bsp", "Symbolic Link = ..\\..\\Windows"] {
            assert!(read_7z_listing(&format!("{head}Path = b\r\nSize = 0\r\n{k}\r\n\r\n")).unwrap().1, "{k}");
        }
        // empty link fields (every tar entry has them) and plain modes are not links
        let plain = format!("{head}Path = a\r\nMode = -rw-r--r--\r\nSymbolic Link = \r\nHard Link = \r\nAttributes = A -rw-r--r--\r\n\r\nPath = d\r\nMode = drwxr-xr-x\r\n\r\n");
        assert_eq!(read_7z_listing(&plain), Some((0, false, 2)));
        // a link-looking name or comment isn't a mode
        let name = format!("{head}Path = lrwxrwxrwx\r\nComment = lrwxrwxrwx\r\n\r\n");
        assert!(!read_7z_listing(&name).unwrap().1);
    }

    #[test]
    fn seven_zip_extract_arguments() {
        let dest = Path::new("C:\\Users\\Jo Smith\\AppData\\Local\\loner strike\\tmp\\dl-ü 日本\\x");
        let a: Vec<String> = seven_zip_x_args(dest).iter().map(|s| s.to_string_lossy().to_string()).collect();
        assert_eq!(a[..7], ["x", "-y", "-bso0", "-bsp0", "-snl-", "-snh-", "-sns-"]);
        // one argument, no space after -o (Command quotes it as a whole on Windows)
        assert_eq!(a[7], format!("-o{}", dest.to_str().unwrap()));
        assert_eq!(a.len(), 8, "the archive goes last, added by the caller");
        assert!(!a.iter().any(|s| s == "-spf" || s == "-snl" || s == "-snld"));
    }

    #[test]
    fn seven_zip_error_lines() {
        // real stderr from 7z x on a broken .7z and on a damaged one
        let open = "ERROR: C:\\dl\\bad.7z\r\nC:\\dl\\bad.7z\r\nOpen ERROR: Cannot open the file as [7z] archive\r\n\r\n\r\n";
        assert_eq!(seven_zip_error(open), "7-Zip: Open ERROR: Cannot open the file as [7z] archive");
        assert_eq!(seven_zip_error("ERROR: Data Error : maps\\de_x.bsp\r\n"), "7-Zip: ERROR: Data Error : maps\\de_x.bsp");
        assert_eq!(seven_zip_error("something odd\n"), "7-Zip: something odd");
        assert_eq!(seven_zip_error(""), "7-Zip: unknown error");
    }

    // what 7-Zip and bsdtar leave on disk is planned from its parts; listing paths (GameBanana's
    // preview) can hold anything, and neither may put a file outside cstrike/'s content folders
    #[test]
    fn traversal_rules() {
        for bad in ["../x.bsp", "maps/../../x.bsp", "maps\\..\\..\\x.bsp", "..\\x.bsp", "a/./../b.bsp"] {
            assert!(listed_parts(bad).is_empty(), "{bad}");
        }
        assert_eq!(listed_parts("./maps//de_a.bsp"), ["maps", "de_a.bsp"]);
        let rels: Vec<Vec<String>> = [
            "C:\\maps\\de_drive.bsp",
            "\\\\server\\share\\maps\\de_unc.bsp",
            "/abs/models/player/x/x.mdl",
            "maps/de_ads.bsp:evil",
            "sound/x.wav::$DATA",
            "maps/CON.bsp",
            "C:evil.wad",
        ]
        .iter()
        .map(|r| listed_parts(r))
        .collect();
        let (plan, skipped) = plan_rels(&rels);
        let dests: Vec<&str> = plan.iter().map(|(_, d)| d.as_str()).collect();
        assert!(dests.contains(&"maps/de_drive.bsp") && dests.contains(&"maps/de_unc.bsp") && dests.contains(&"models/player/x/x.mdl"), "{dests:?}");
        assert!(skipped.iter().any(|s| s == "C:evil.wad"), "{skipped:?}");
        for d in &dests {
            assert!(!d.contains(':'), "{d}");
            assert!(!d.starts_with('/') && !d.starts_with('\\') && !d.split('/').any(|p| p == ".."), "{d}");
        }
        // a stream name isn't a map extension, so it's left out rather than installed
        assert!(skipped.iter().any(|s| s.ends_with("de_ads.bsp:evil")) && skipped.iter().any(|s| s.ends_with("x.wav::$DATA")), "{skipped:?}");
    }

    // an inner archive that fails leaves nothing behind
    #[test]
    fn nested_failure_leaves_nothing() {
        let r = tmp("nested-fail-");
        put(&r, "maps/de_a.bsp", b"bsp");
        put(&r, "extra/broken.rar", b"Rar!\x1a\x07\x00 not really");
        extract_nested(&r).unwrap();
        let left: Vec<String> = std::fs::read_dir(&r).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert!(!left.iter().any(|n| n.starts_with("nested-")), "{left:?}");
        let _ = std::fs::remove_dir_all(&r);
    }

    // an unpacker that makes too many files and is done before the first look is still caught
    #[cfg(unix)]
    #[test]
    fn quick_unpacker_counted_at_the_end() {
        let r = tmp("count-quick-");
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg("i=0; while [ $i -lt 300 ]; do : > f$i; i=$((i+1)); done").current_dir(&r);
        let e = run_limited_n(c, &r, MAX_UNPACKED, 200).unwrap_err();
        assert!(e.ends_with("the most is 200."), "{e}");
        let _ = std::fs::remove_dir_all(&r);
    }

    // the only ways run_limited fails with something other than its size/count messages are
    // a program that can't be started or waited for; a briefly full process table is retried
    #[cfg(unix)]
    #[test]
    fn spawn_retries_only_brief_shortages() {
        let e = |n: i32| std::io::Error::from_raw_os_error(n);
        assert!(spawn_transient(&e(libc::EAGAIN)) && spawn_transient(&e(libc::EINTR)) && spawn_transient(&e(libc::ENOMEM)));
        assert!(!spawn_transient(&e(libc::ENOENT)) && !spawn_transient(&e(libc::EACCES)));
        let mut missing = Command::new("/nonexistent/unpacker");
        let started = std::time::Instant::now();
        assert!(spawn_retrying(&mut missing).is_err());
        assert!(started.elapsed() < Duration::from_millis(500), "a missing program isn't retried");
    }

    // [Caps] an unpacker that makes more files than its listing said is stopped while it works
    #[cfg(unix)]
    #[test]
    fn unpacker_stopped_at_the_file_count() {
        let r = tmp("count-");
        let dest = r.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg("i=0; while [ $i -lt 5000 ]; do : > f$i; i=$((i+1)); done; sleep 30").current_dir(&dest);
        let started = std::time::Instant::now();
        let e = run_limited_n(c, &dest, MAX_UNPACKED, 200).unwrap_err();
        assert!(e.starts_with("That download has too many files in it:") && e.ends_with("the most is 200."), "{e}");
        assert!(started.elapsed() < Duration::from_secs(20), "stopped while it works, not at the end");
        let _ = std::fs::remove_dir_all(&r);
    }

    // [Caps]
    #[test]
    fn bomb_ratio_only_stops_bombs() {
        // a 1 MB zip that says 2.9 GB: refused, with both sizes
        let e = bomb_check(3, 2900 << 20, 1 << 20).unwrap_err();
        assert!(e.starts_with("That download unpacks to 2900 MB from a 1.0 MB file"), "{e}");
        // real packs: a 1.4 GB map pack of 3 GB; a WAD of flat colours (50x) under 256 MB; a big model pack
        assert!(bomb_check(4000, 3 << 30, 1400 << 20).is_ok());
        assert!(bomb_check(1, 200 << 20, 4 << 20).is_ok());
        assert!(bomb_check(MAX_ENTRIES, 900 << 20, 300 << 20).is_ok());
    }

    #[test]
    fn zip_budget_and_traversal() {
        use std::io::Write;
        let r = tmp("zip-");
        let archive = r.join("a.zip");
        {
            let f = std::fs::File::create(&archive).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("maps/de_a.bsp", o).unwrap();
            z.write_all(&[7u8; 5000]).unwrap();
            z.add_symlink("maps/link.bsp", "/etc/hosts", o).unwrap();
            z.finish().unwrap();
        }
        let dest = r.join("ok");
        std::fs::create_dir_all(&dest).unwrap();
        extract(&archive, &dest, MAX_UNPACKED).unwrap();
        assert_eq!(std::fs::read(dest.join("maps/de_a.bsp")).unwrap().len(), 5000);
        // P5 (QA4-G): the left-out link is listed with what plan() skipped
        let (picked, skipped) = plan(&dest);
        assert!(picked.iter().any(|(_, d)| d == "maps/de_a.bsp"));
        assert!(skipped.iter().any(|x| x.contains("link.bsp") && x.contains("a link")), "{skipped:?}");
        assert!(take_dropped().is_empty(), "listed once");
        // an entry that points outside: the whole download is refused, nothing escapes
        let bad = r.join("bad.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("maps/de_b.bsp", o).unwrap();
            z.write_all(b"x").unwrap();
            z.start_file("../../escape.txt", o).unwrap();
            z.write_all(b"nope").unwrap();
            z.finish().unwrap();
        }
        let d2 = r.join("bad");
        std::fs::create_dir_all(&d2).unwrap();
        assert!(extract(&bad, &d2, MAX_UNPACKED).unwrap_err().contains("points outside"));
        // a zip whose own directory can't be read here: an entry with a Unicode name field is refused
        assert!(name_check(None, true, b"maps/a.bsp", "maps/a.bsp").is_some());
        assert!(name_check(None, false, b"maps/a.bsp", "maps/a.bsp").is_none());
        assert!(name_check(Some(b"maps/a.bsp"), true, b"x", "creatures/creatures.ini").is_some());
        assert!(name_check(Some(b"maps/a.bsp"), true, b"x", "maps/a.bsp").is_none());
        assert!(!r.join("escape.txt").exists() && !r.parent().unwrap().join("escape.txt").exists());

        let small = r.join("small");
        std::fs::create_dir_all(&small).unwrap();
        assert_eq!(extract(&archive, &small, 1000).unwrap_err(), TOO_BIG);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn watchdog_stops_runaway_unpacker() {
        let r = tmp("budget-");
        // an "unpacker" that keeps writing far past what its listing said
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(format!("i=0; while [ $i -lt 400 ]; do head -c 65536 /dev/zero > {}/f$i; i=$((i+1)); sleep 0.01; done", r.display()));
        let res = run_limited(c, &r, 1024 * 1024);
        assert_eq!(res.unwrap_err(), TOO_BIG);
        let after = tree_size(&r);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(tree_size(&r), after, "unpacker still running after the budget stop");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn rlimit_caps_single_file() {
        let r = tmp("rlimit-");
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(format!("head -c 3000000 /dev/zero > {}/big", r.display()));
        let res = run_limited(c, &r, 1024 * 1024);
        // either the file-size limit killed it or the watchdog caught it; never more than the budget on disk
        match res {
            Ok((ok, _)) => assert!(!ok),
            Err(e) => assert_eq!(e, TOO_BIG),
        }
        assert!(tree_size(&r) <= 1024 * 1024);
        let _ = std::fs::remove_dir_all(&r);
    }
}
