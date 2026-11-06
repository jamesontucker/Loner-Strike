//! Small cross-platform helpers: where the app keeps its files, HTTPS, finding
//! and running helper programs, and making sure child programs die with the app.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The app's own files (install records, caches, playit key): <config dir>/Loner-Strike.
pub fn app_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Loner-Strike")
}

/// The old Python panel's folder, to bring its records over the first time.
pub fn old_panel_dir() -> Option<PathBuf> {
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
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
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

pub fn write_latin1(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect::<Vec<u8>>())
}

/// A program on PATH, or in one of the usual extra places (Homebrew on a Mac).
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
    #[cfg(not(any(target_os = "macos", windows)))]
    false
}
