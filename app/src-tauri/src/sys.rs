//! Small cross-platform helpers: where the app keeps its files, HTTPS, finding
//! and running helper programs, and making sure child programs die with the app.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The app's own files (install records, caches, playit key): <config dir>/Loner-Strike.
pub fn app_dir() -> PathBuf {
    // test builds (the Tools dev server, agents' test servers, cargo test) never touch the
    // real app's files: everything goes under LS_APP_DIR, or a scratch folder (QA4-CORE F6)
    #[cfg(test)]
    {
        let root = test_root();
        check_isolated_once(&root);
        return root;
    }
    #[allow(unreachable_code)]
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Loner-Strike")
}

/// Test builds: where every app path goes (LS_APP_DIR, else a scratch folder in the temp folder).
#[cfg(test)]
pub fn test_root() -> PathBuf {
    std::env::var_os("LS_APP_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("loner-strike-test-app"))
}

/// Every folder and file the app keeps its own data in (settings, records, caches, downloads,
/// the window's data, the old panel's folder it adopts). In a test build all of them must be
/// under the test root, or the build refuses to go on.
#[cfg_attr(not(test), allow(dead_code))]
pub fn app_paths() -> Vec<(&'static str, PathBuf)> {
    let mut v = vec![
        ("the app folder", app_dir_unchecked()),
        ("settings.json", crate::app_api::settings_path()),
        ("the cache", cache_root()),
        ("setup's downloads", crate::setup::fetch::cache_dir()),
        ("the big folders", crate::localdata::local_root()),
    ];
    for rel in crate::localdata::BIG {
        v.push(("a big folder", crate::localdata::dir(rel)));
    }
    v.extend(window_data_dirs().into_iter().map(|d| ("the window's data", d)));
    if let Some(d) = old_panel_dir() {
        v.push(("the old panel's folder", d));
    }
    if let Some(d) = crate::setup::fetch::legacy_cache() {
        v.push(("the old installer's downloads", d));
    }
    v
}

#[cfg_attr(not(test), allow(dead_code))]
fn app_dir_unchecked() -> PathBuf {
    #[cfg(test)]
    return test_root();
    #[allow(unreachable_code)]
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Loner-Strike")
}

/// The paths of `app_paths` that aren't under `root` (a test build's check).
#[cfg_attr(not(test), allow(dead_code))]
pub fn outside(root: &Path, paths: &[(&'static str, PathBuf)]) -> Vec<String> {
    paths.iter().filter(|(_, p)| !p.starts_with(root)).map(|(what, p)| format!("{what}: {}", p.display())).collect()
}

/// A test build refuses to go on (panics) the first time an app path is asked for, if any of
/// them would be outside the test root: real app data is never written by a test.
#[cfg(test)]
fn check_isolated_once(root: &Path) {
    static CHECKED: OnceLock<()> = OnceLock::new();
    thread_local!(static CHECKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
    if CHECKED.get().is_some() || CHECKING.with(|c| c.get()) {
        return;
    }
    CHECKING.with(|c| c.set(true));
    let bad = outside(root, &app_paths());
    CHECKING.with(|c| c.set(false));
    if !bad.is_empty() {
        panic!("Test build: these app paths aren't under the test folder {} (LS_APP_DIR), so nothing runs:\n{}", root.display(), bad.join("\n"));
    }
    let _ = CHECKED.set(());
}

/// The window's own data (a Tauri plugin's and the web view's folders).
pub fn window_data_dirs() -> Vec<PathBuf> {
    #[cfg(test)]
    return vec![test_root().join("window-data")];
    #[allow(unreachable_code)]
    [dirs::data_dir(), dirs::config_dir(), dirs::cache_dir()].into_iter().flatten().map(|d| d.join("gg.lonerstrike.app")).collect()
}

/// The app's cache (setup's downloads and work folders). On Windows not the system cache
/// folder's Loner-Strike: that's %LOCALAPPDATA%\Loner-Strike, where the installer puts the
/// app itself (installMode currentUser), so clearing the cache there deleted the app. It
/// goes in the app's own %LOCALAPPDATA%\gg.lonerstrike.app (the window's data is there too).
pub fn cache_root() -> PathBuf {
    #[cfg(test)]
    return test_root().join("system-cache");
    #[allow(unreachable_code)]
    let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    if cfg!(windows) {
        base.join("gg.lonerstrike.app").join("cache")
    } else {
        base.join("Loner-Strike")
    }
}

/// The old Python panel's folder, to bring its records over the first time.
pub fn old_panel_dir() -> Option<PathBuf> {
    #[cfg(test)]
    return Some(test_root().join("CZ Control Panel"));
    #[allow(unreachable_code)]
    dirs::home_dir().map(|h| h.join("CZ Control Panel"))
}

/// Copies `from` to `to` if `to` doesn't exist yet (one-time move from the old panel).
/// Writes a file so readers only ever see the old or the new version: a temporary
/// file beside it (unique to this write), then renamed over it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}-{}.tmp", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&tmp, bytes)?;
    rename_over(&tmp, path)
}

/// write_atomic for a file holding keys: only this user can read it (on the Mac and Linux;
/// Windows keeps the app's folder to its user already). The file is never readable by
/// others, not even for a moment while it's written.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}-{}.tmp", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let written = o.open(&tmp).and_then(|mut f| f.write_all(bytes).and_then(|_| f.sync_all()));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    rename_over(&tmp, path)
}

/// A key file saved before write_private existed: made readable by this user only.
pub fn tighten(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0) {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Moves a finished temporary file over `path`. On Windows an antivirus scan or the game
/// reading the file can hold it for a moment, which fails the rename: tried a few times
/// before giving up. The temporary file is removed when it fails.
pub fn rename_over(tmp: &Path, path: &Path) -> std::io::Result<()> {
    // (Windows: a read-only target can't be replaced; players mark config.cfg read-only)
    if cfg!(windows) {
        make_writable(path);
    }
    let mut tries = 0;
    loop {
        match std::fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) if cfg!(windows) && tries < 5 && transient_lock(&e) => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(60 * tries));
            }
            Err(e) => {
                let _ = std::fs::remove_file(tmp);
                return Err(e);
            }
        }
    }
}

/// A Windows error that usually clears by itself in a moment: access denied (5) while an
/// antivirus or the indexer has the file, the file in use by another process (32: Rust calls
/// it "uncategorized", not PermissionDenied) or a part of it locked (33).
pub fn transient_lock(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// Windows: no "has stopped working" or "can't find a file" boxes for this app and every
/// program it starts (they inherit the error mode): a crashed hlds.exe exits at once, so the
/// server watcher and the crash records see it, instead of sitting behind a dialog until
/// someone clicks. (Valve's own fatal-error message boxes still show.)
pub fn quiet_crash_dialogs() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn SetErrorMode(mode: u32) -> u32;
        }
        const SEM_FAILCRITICALERRORS: u32 = 0x0001;
        const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
        const SEM_NOOPENFILEERRORBOX: u32 = 0x8000;
        // (SAFETY: SetErrorMode only sets this process's error mode flags)
        unsafe {
            let old = SetErrorMode(0);
            SetErrorMode(old | SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        }
    }
}

/// A UDP port nothing holds: tried on every address and, when the server binds one, on that
/// address too (on Windows a bind to all addresses can succeed while another program holds the
/// port on one of them).
pub fn udp_port_free(ip: Option<&str>, port: u16) -> bool {
    let all = std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok();
    let one = match ip.filter(|i| !i.is_empty()) {
        Some(i) => std::net::UdpSocket::bind((i, port)).is_ok(),
        None => std::net::UdpSocket::bind(("127.0.0.1", port)).is_ok(),
    };
    all && one
}

/// Two Windows paths name the same folder: case-blind for every letter (not only ASCII:
/// D:\Игры and d:\игры), "/" and "\" alike, a trailing separator ignored, a "\\?\" prefix
/// dropped.
pub fn same_windows_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.replace('/', "\\");
        let s = s.strip_prefix("\\\\?\\").unwrap_or(&s).to_string();
        s.trim_end_matches('\\').to_lowercase()
    };
    norm(a) == norm(b)
}

/// A JSON file that's there but doesn't parse is kept aside (name.bad) rather than
/// silently replaced by an empty one on the next save.
pub fn set_aside(path: &Path) {
    if path.is_file() {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let _ = std::fs::rename(path, path.with_file_name(format!("{name}.bad")));
    }
}

/// A JSON object file; missing is empty, and one that doesn't parse is set aside first.
/// A text file: None if it isn't there. Other errors (often another program, like an
/// antivirus, holding it for a moment on Windows) are retried briefly first.
pub fn read_text(path: &Path) -> std::io::Result<Option<String>> {
    let mut tries = 0;
    loop {
        match std::fs::read_to_string(path) {
            Ok(t) => return Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) if tries >= 5 => return Err(e),
            Err(_) => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
        }
    }
}

pub fn read_json_map(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    match read_text(path).ok().flatten().ok_or(()) {
        Ok(t) => serde_json::from_str::<serde_json::Value>(&t).ok().and_then(|v| v.as_object().cloned()).unwrap_or_else(|| {
            set_aside(path);
            Default::default()
        }),
        Err(_) => Default::default(),
    }
}

/// Waits for a program we started and don't need anything back from, on its own
/// thread, so it doesn't stay behind as a zombie until the app quits.
pub fn reap(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

pub fn adopt(from: &Path, to: &Path) {
    if !to.exists() && from.is_file() {
        if let Some(d) = to.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::copy(from, to);
    }
}

/// A blocking HTTPS client. Only use from std threads or spawn_blocking.
/// For big downloads: hours for the whole download (a slow connection still gets there; a
/// dropped one is resumed), and keepalives so a dead connection is noticed.
pub fn http_download() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .tcp_keepalive(Duration::from_secs(30))
            .timeout(Duration::from_secs(3 * 3600))
            .build()
            .expect("https client")
    })
}

pub fn http() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("https client")
    })
}

/// GoldSrc text is Latin-1: every byte is one character.
pub fn read_latin1(path: &Path) -> std::io::Result<String> {
    std::fs::read(path).map(|b| b.iter().map(|&c| c as char).collect())
}

/// Clears a file's read-only mark (some guides tell people to set it on liblist.gam), so a
/// repair can write it.
pub fn make_writable(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

pub fn write_latin1(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, latin1_bytes(text))
}

/// write_latin1, but written whole or not at all (write_atomic): for a config the game or
/// the plugin reads, which a crash part way mustn't leave cut short.
pub fn write_latin1_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    write_atomic(path, &latin1_bytes(text))
}

fn latin1_bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
}

/// A program the app ships for Linux servers, read whole: the file itself, or its .gz (the
/// Linux build ships them gzipped: the AppImage's packager, linuxdeploy, rewrites every Linux
/// program it bundles, which would break their checksums). The path is the plain file's.
pub fn read_shipped(bin: &Path) -> Option<Vec<u8>> {
    if bin.is_file() {
        return std::fs::read(bin).ok();
    }
    let gz = std::fs::File::open(PathBuf::from(format!("{}.gz", bin.display()))).ok()?;
    let mut out = Vec::new();
    // (a few MB at most; anything far bigger isn't it)
    std::io::Read::read_to_end(&mut std::io::Read::take(flate2::read::GzDecoder::new(gz), 256 << 20), &mut out).ok()?;
    Some(out)
}

/// Whether read_shipped finds `bin` (it or its .gz)
pub fn shipped_here(bin: &Path) -> bool {
    bin.is_file() || PathBuf::from(format!("{}.gz", bin.display())).is_file()
}

/// A program on PATH, or in one of the usual extra places (Homebrew on a Mac).
/// Where a program shipped with the app may be: next to the app's own program, or in its
/// resources (inside a Mac .app too).
pub fn bundled_places(file: &str) -> Vec<PathBuf> {
    let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) else { return Vec::new() };
    vec![d.join(file), d.join("resources").join(file), d.join("../Resources").join(file), d.join("../Resources/resources").join(file)]
}

pub fn find_tool(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(&exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    ["/opt/homebrew/bin", "/usr/local/bin"].iter().map(|d| Path::new(d).join(&exe)).find(|p| p.is_file())
}

/// A folder or file path as typed or pasted: no surrounding spaces or quotes (Windows
/// "Copy as path" adds them) and no trailing slash, except on a drive root like D:\.
pub fn clean_path(s: &str) -> String {
    let t = s.trim().trim_matches(['"', '\'']).trim();
    let bare = t.trim_end_matches(['/', '\\']);
    if bare.is_empty() {
        return t.chars().take(1).collect();
    }
    if bare.len() == 2 && bare.ends_with(':') {
        return format!("{bare}\\");
    }
    bare.to_string()
}

/// A canonical path without Windows' \\?\ prefix, which many programs (and people) can't read.
pub fn canonical(p: &Path) -> std::io::Result<PathBuf> {
    let c = p.canonicalize()?;
    let s = c.to_string_lossy();
    Ok(if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        c
    })
}

/// A Command that doesn't flash a console window on Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Runs a command and returns (success, stdout, stderr), killing it after `timeout`.
pub fn run_output(mut cmd: Command, timeout: Duration) -> Option<(bool, String, String)> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let out = child.stdout.take();
    let err = child.stderr.take();
    let read = |s: Option<std::process::ChildStdout>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            if let Some(mut s) = s {
                let _ = std::io::Read::read_to_end(&mut s, &mut v);
            }
            v
        })
    };
    let t_out = read(out);
    let t_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(mut s) = err {
            let _ = std::io::Read::read_to_end(&mut s, &mut v);
        }
        v
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    let o = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).to_string();
    let e = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).to_string();
    Some((status.map(|s| s.success()).unwrap_or(false), o, e))
}

// ---- child programs that must not outlive the app -----------------------------

static CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Makes sure `child` is stopped when the app quits, even though Tauri ends the
/// process with exit() (so nothing gets dropped).
/// Unix: an atexit hook sends it SIGTERM. Windows: a job object that kills its
/// members when the app's handle to it closes (also on a crash).
pub fn tie_to_app(child: &Child) {
    #[cfg(unix)]
    {
        static HOOK: std::sync::Once = std::sync::Once::new();
        extern "C" fn on_exit() {
            if let Ok(list) = CHILDREN.try_lock() {
                for &pid in list.iter() {
                    unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
                }
            }
        }
        HOOK.call_once(|| unsafe {
            libc::atexit(on_exit);
        });
        CHILDREN.lock().unwrap().push(child.id());
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        static JOB: OnceLock<usize> = OnceLock::new();
        let job = *JOB.get_or_init(|| unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            job as usize
        });
        if job != 0 {
            unsafe { AssignProcessToJobObject(job as _, child.as_raw_handle() as _) };
        }
        CHILDREN.lock().unwrap().push(child.id());
    }
}

pub fn untie(child_id: u32) {
    CHILDREN.lock().unwrap().retain(|&p| p != child_id);
}

/// Asks a child to stop (SIGTERM on Unix), then kills it if it hasn't after `grace`.
pub fn stop_child(mut child: Child, grace: Duration) {
    untie(child.id());
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    let _ = grace;
    #[cfg(unix)]
    {
        let start = Instant::now();
        while start.elapsed() < grace {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Seconds since 1970, as Python's time.time().
pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// "YYYYmmdd-HHMMSS" (UTC), for backup file names.
pub fn stamp() -> String {
    let secs = now() as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// A new, empty, uniquely named folder inside `parent`.
pub fn make_temp_dir(parent: &Path, prefix: &str) -> std::io::Result<PathBuf> {
    use rand::Rng;
    std::fs::create_dir_all(parent)?;
    for _ in 0..20 {
        let n: u64 = rand::thread_rng().gen();
        let p = parent.join(format!("{prefix}{n:016x}"));
        match std::fs::create_dir(&p) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("couldn't make a temporary folder"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn stamp_shape() {
        let s = super::stamp();
        assert_eq!(s.len(), 15);
        assert!(s.starts_with("20"));
    }
}

/// Whether Counter-Strike or Condition Zero (hl.exe) is running. Setup won't change
/// the game's files while it is: Windows locks them, and the game would crash.
/// Closes the game (hl.exe) and waits up to 15 seconds for it to be gone.
pub fn close_game() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut c = command("/usr/bin/pkill");
        c.args(["-if", r"\\hl\.exe"]);
        let _ = run_output(c, Duration::from_secs(5));
    }
    #[cfg(windows)]
    {
        let mut c = command("taskkill");
        c.args(["/IM", "hl.exe", "/F"]);
        let _ = run_output(c, Duration::from_secs(5));
    }
    #[cfg(target_os = "linux")]
    for (pid, _) in linux_games() {
        // SAFETY: a plain signal to a process of this user's
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
    for _ in 0..30 {
        if !game_running() {
            // Steam takes a moment to see the game has gone before it starts it again
            std::thread::sleep(Duration::from_secs(3));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Counter-Strike didn't close. Quit it yourself and try again.".into())
}

pub fn game_running() -> bool {
    #[cfg(target_os = "macos")]
    {
        // under CrossOver the game shows up as ...\Half-Life\hl.exe (not hlds.exe, the server)
        let mut c = command("/usr/bin/pgrep");
        c.args(["-if", r"\\hl\.exe"]);
        matches!(run_output(c, Duration::from_secs(3)), Some((true, out, _)) if !out.trim().is_empty())
    }
    #[cfg(windows)]
    {
        let mut c = command("tasklist");
        c.args(["/FI", "IMAGENAME eq hl.exe", "/NH"]);
        matches!(run_output(c, Duration::from_secs(5)), Some((_, out, _)) if out.to_lowercase().contains("hl.exe"))
    }
    #[cfg(target_os = "linux")]
    {
        !linux_games().is_empty()
    }
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    false
}

/// The game's processes on Linux with their Half-Life folders, from /proc: the native game
/// (hl_linux), and the Windows one under Proton (hl.exe, whose command line names it the Wine
/// way: Z:\home\...\Half-Life\hl.exe, Z: being /, or under Proton 11 S:\steamapps\...).
#[cfg(target_os = "linux")]
pub fn linux_games() -> Vec<(i32, std::path::PathBuf)> {
    let Ok(rd) = std::fs::read_dir("/proc") else { return vec![] };
    rd.flatten()
        .filter_map(|e| {
            let pid: i32 = e.file_name().to_str()?.parse().ok()?;
            if let Ok(exe) = std::fs::read_link(e.path().join("exe")) {
                if exe.file_name()? == "hl_linux" {
                    return Some((pid, exe.parent().map(|p| p.to_path_buf()).unwrap_or_default()));
                }
            }
            let cmd = std::fs::read(e.path().join("cmdline")).ok()?;
            let first = String::from_utf8_lossy(cmd.split(|b| *b == 0).next()?).to_string();
            // (Proton maps the Steam library to its own drive, S: in Proton 11: the prefix's
            // dosdevices links say where each drive is)
            let drive = |letter: char| {
                let env = std::fs::read(e.path().join("environ")).ok()?;
                let prefix = env.split(|b| *b == 0).find_map(|v| v.strip_prefix(b"WINEPREFIX="))?;
                let devices = std::path::PathBuf::from(String::from_utf8_lossy(prefix).to_string()).join("dosdevices");
                let target = std::fs::read_link(devices.join(format!("{}:", letter.to_ascii_lowercase()))).ok()?;
                Some(devices.join(target))
            };
            proton_game_dir(&first, drive).map(|d| (pid, d))
        })
        .collect()
}

/// The Half-Life folder from a Proton game's first argument: a Wine path to hl.exe
/// ("Z:\\home\\me\\...\\Half-Life\\hl.exe", Z: being /; another drive through `drive`, which
/// gives where a drive letter points: "S:\\steamapps\\common\\Half-Life\\hl.exe" with S: the
/// Steam folder) or a plain Linux path to it; None for anything else.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn proton_game_dir(arg0: &str, drive: impl Fn(char) -> Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    let b = arg0.as_bytes();
    let p = if b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
        let rest = arg0[3..].replace('\\', "/");
        let letter = b[0] as char;
        if letter.eq_ignore_ascii_case(&'z') { std::path::PathBuf::from("/").join(rest) } else { drive(letter)?.join(rest) }
    } else if arg0.starts_with('/') {
        std::path::PathBuf::from(arg0)
    } else {
        return None;
    };
    p.file_name()?.to_str()?.eq_ignore_ascii_case("hl.exe").then(|| p.parent().map(|d| d.to_path_buf())).flatten()
}

/// The folders the running game (hl.exe) was started from, as the system names them: a full
/// Windows path on Windows; under CrossOver the Windows path from its command line.
pub fn running_game_dirs() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let mut c = command("/usr/bin/pgrep");
        c.args(["-ifl", r"\\hl\.exe"]);
        match run_output(c, Duration::from_secs(3)) {
            Some((true, out, _)) => out.lines().filter_map(game_dir_in).collect(),
            _ => Vec::new(),
        }
    }
    #[cfg(windows)]
    {
        let mut c = command("powershell");
        // (UTF-8 out: Windows PowerShell otherwise writes the old code page, and a folder like
        // D:\Игры would come back garbled and never match)
        c.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; (Get-Process hl -ErrorAction SilentlyContinue).Path",
        ]);
        match run_output(c, Duration::from_secs(8)) {
            // (a line that still came back garbled can't be compared: left out)
            Some((_, out, _)) => out.lines().filter(|l| !l.contains('\u{FFFD}')).filter_map(game_dir_in).collect(),
            _ => Vec::new(),
        }
    }
    #[cfg(target_os = "linux")]
    {
        linux_games().into_iter().map(|(_, d)| d.to_string_lossy().to_string()).collect()
    }
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    Vec::new()
}

/// The folder before \hl.exe in a process line or path ("C:\Games\Half-Life\hl.exe -steam"
/// -> "C:\Games\Half-Life").
fn game_dir_in(line: &str) -> Option<String> {
    // (ASCII lowercase keeps byte positions the same as the line's)
    let low = line.to_ascii_lowercase();
    let at = low.find("\\hl.exe")?;
    // the drive: a letter just before the first ":\" (not any earlier letter, like CrossOver's C)
    let b = line.as_bytes();
    let start = line[..at].find(":\\").filter(|&i| i > 0 && b[i - 1].is_ascii_alphabetic()).map(|i| i - 1).unwrap_or(0);
    Some(line[start..at].trim().to_string())
}

/// Whether a running game's folder (a Windows path) is the game folder Loner-Strike set up
/// (this computer's path to it: under CrossOver, inside the bottle). Compared by the folders'
/// own names and, on Windows, the whole path.
pub fn same_game_dir(running: &str, ours: &Path) -> bool {
    let name = |s: &str| s.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
    if cfg!(windows) {
        return same_windows_path(running, &ours.to_string_lossy());
    }
    if cfg!(target_os = "linux") {
        let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        return real(Path::new(running)) == real(ours);
    }
    name(running) == name(&ours.to_string_lossy())
}

#[cfg(test)]
mod windows_tests {
    use super::*;

    /// QA4-CORE F6: under test every app path (settings.json, the cache, setup's downloads, the
    /// big folders, the window's data, the old panel's folder, servers by default) is in the
    /// test folder, and one outside it is caught
    #[test]
    fn test_builds_keep_every_app_path_in_the_test_folder() {
        let root = super::test_root();
        let paths = super::app_paths();
        assert!(paths.len() >= 10, "{paths:?}");
        assert!(super::outside(&root, &paths).is_empty(), "{:?}", super::outside(&root, &paths));
        assert_eq!(crate::app_api::settings_path(), root.join("settings.json"));
        assert!(super::cache_root().starts_with(&root) && crate::setup::fetch::cache_dir().starts_with(&root));
        assert!(crate::setup::fetch::legacy_cache().is_none());
        let client = crate::steam::Install { hl: "/x/Half-Life".into(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec!["cstrike".into()], dedicated: false };
        assert!(crate::servers::default_root(&client).unwrap().starts_with(&root));
        let mut bad = paths.clone();
        bad.push(("settings.json", std::path::PathBuf::from("/Users/someone/Library/Application Support/Loner-Strike/settings.json")));
        assert_eq!(super::outside(&root, &bad).len(), 1);
    }


    #[test]
    fn windows_paths_compare_like_windows() {
        assert!(same_windows_path(r"D:\Игры\Steam\steamapps\common\Half-Life\", r"d:\игры\steam\steamapps\common\half-life"));
        assert!(same_windows_path(r"C:/Users/Zoë/Games/Half-Life", r"c:\users\ZOË\games\half-life"));
        assert!(same_windows_path(r"\\?\C:\Program Files (x86)\Steam", r"C:\Program Files (x86)\Steam\"));
        assert!(same_windows_path(r"\\server\share\Half-Life", r"\\SERVER\Share\half-life"));
        assert!(!same_windows_path(r"C:\Half-Life", r"D:\Half-Life"));
    }

    #[test]
    fn a_port_another_program_holds_isnt_free() {
        let held = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        let port = held.local_addr().unwrap().port();
        assert!(!udp_port_free(None, port), "held on 127.0.0.1");
        assert!(!udp_port_free(Some("127.0.0.1"), port));
        drop(held);
        assert!(udp_port_free(None, port));
    }

    #[test]
    fn lock_errors_that_clear_by_themselves() {
        for code in [5, 32, 33] {
            assert!(transient_lock(&std::io::Error::from_raw_os_error(code)), "{code}");
        }
        assert!(transient_lock(&std::io::Error::from(std::io::ErrorKind::PermissionDenied)));
        assert!(!transient_lock(&std::io::Error::from(std::io::ErrorKind::NotFound)));
    }

    #[test]
    fn read_only_files_can_be_made_writable() {
        let d = std::env::temp_dir().join(format!("ls-ro-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("config.cfg");
        std::fs::write(&f, "a").unwrap();
        let mut p = std::fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&f, p).unwrap();
        make_writable(&f);
        assert!(!std::fs::metadata(&f).unwrap().permissions().readonly());
        std::fs::write(&f, "b").unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod path_tests {
    use super::clean_path;
    #[test]
    fn cleans() {
        assert_eq!(clean_path(r#"  "C:\Games\Half-Life\"  "#), r"C:\Games\Half-Life");
        assert_eq!(clean_path("D:\\"), "D:\\");
        assert_eq!(clean_path("D:"), "D:\\");
        assert_eq!(clean_path("/"), "/");
        assert_eq!(clean_path("'/Users/me/key'"), "/Users/me/key");
        assert_eq!(clean_path(""), "");
    }

    #[test]
    fn proton_game_folder() {
        use super::proton_game_dir;
        use std::path::PathBuf;
        let none = |_: char| None;
        assert_eq!(proton_game_dir(r"Z:\home\me\.local\share\Steam\steamapps\common\Half-Life\hl.exe", none).unwrap(), PathBuf::from("/home/me/.local/share/Steam/steamapps/common/Half-Life"));
        assert_eq!(proton_game_dir("/mnt/lib/steamapps/common/Half-Life/HL.EXE", none).unwrap(), PathBuf::from("/mnt/lib/steamapps/common/Half-Life"));
        assert_eq!(proton_game_dir(r"C:\windows\system32\steam.exe", none), None);
        assert_eq!(proton_game_dir("/usr/bin/bash", none), None);
        // Proton 11 (seen 2026-11-24): the Steam library as S:
        let s = |l: char| (l == 'S').then(|| PathBuf::from("/home/me/.local/share/Steam"));
        assert_eq!(proton_game_dir(r"S:\steamapps\common\Half-Life\hl.exe", s).unwrap(), PathBuf::from("/home/me/.local/share/Steam/steamapps/common/Half-Life"));
        assert_eq!(proton_game_dir(r"S:\steamapps\common\Half-Life\hl.exe", none), None);
    }

    #[test]
    fn running_game_folder() {
        use super::{game_dir_in, same_game_dir};
        assert_eq!(game_dir_in(r"12345 C:\Program Files (x86)\Steam\steamapps\common\Half-Life\hl.exe -steam -game czero").as_deref(), Some(r"C:\Program Files (x86)\Steam\steamapps\common\Half-Life"));
        assert_eq!(game_dir_in(r"F:\Counter-Strike Condition Zero 1.2 build 2771\hl.exe").as_deref(), Some(r"F:\Counter-Strike Condition Zero 1.2 build 2771"));
        assert_eq!(game_dir_in("bash something"), None);
        assert_eq!(game_dir_in(r"812 /Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine C:\Games\Half-Life\hl.exe -steam").as_deref(), Some(r"C:\Games\Half-Life"));
        if cfg!(target_os = "linux") {
            // the native game's own folder, from /proc: compared as paths
            let ours = std::path::Path::new("/home/x/.local/share/Steam/steamapps/common/Half-Life");
            assert!(same_game_dir("/home/x/.local/share/Steam/steamapps/common/Half-Life", ours));
            assert!(!same_game_dir("/mnt/games/SteamLibrary/steamapps/common/Half-Life", ours));
        } else if !cfg!(windows) {
            let ours = std::path::Path::new("/Users/x/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life");
            assert!(same_game_dir(r"C:\Program Files (x86)\Steam\steamapps\common\Half-Life", ours));
            assert!(!same_game_dir(r"F:\Counter-Strike Condition Zero 1.2 build 2771", ours));
        }
    }
}
