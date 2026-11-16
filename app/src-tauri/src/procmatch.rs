//! Ending a leftover process safely (agent MCP, Windows audit): a pid saved in a file can belong
//! to another program by now (Windows reuses pids quickly), so before ending one the app checks
//! that it's still the program it started: the same pid, the same executable (its full path) and
//! started when it was (within a few seconds). Used for servers the app lost track of (a crash).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// How far the start time the app noted may be from the one the system reports.
const START_SLACK: u64 = 15;

/// A pid file: "<pid> <start, seconds since 1970>" (older files: the pid alone).
pub fn pid_file_text(pid: u32) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{pid} {now}")
}

pub fn parse_pid_file(text: &str) -> Option<(u32, Option<u64>)> {
    let mut w = text.split_whitespace();
    let pid = w.next()?.parse::<u32>().ok().filter(|p| *p > 0)?;
    Some((pid, w.next().and_then(|t| t.parse::<u64>().ok())))
}

/// Started when the app noted (no note: an older pid file, the path check alone decides).
pub fn same_start(noted: Option<u64>, actual: u64) -> bool {
    noted.is_none_or(|n| n.abs_diff(actual) <= START_SLACK)
}

/// `ps -o etime` ("[[dd-]hh:]mm:ss") as seconds (Unix only: Windows reads the start time another way).
#[cfg(any(unix, test))]
pub fn parse_etime(s: &str) -> Option<u64> {
    let s = s.trim();
    let (days, rest) = match s.split_once('-') {
        Some((d, r)) => (d.parse::<u64>().ok()?, r),
        None => (0, s),
    };
    let parts: Vec<u64> = rest.split(':').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>()?;
    let secs = match parts.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(days * 86400 + secs)
}

/// The leftover is still the program the app started: then it's ended. `exe` is how the
/// command line names it (CrossOver: wine's command line holds the Windows path too, `also`).
pub fn end_if_ours(pid: u32, noted_start: Option<u64>, exe: &Path, also: Option<&str>) -> bool {
    #[cfg(unix)]
    {
        let mut c = crate::sys::command("ps");
        c.args(["-p", &pid.to_string(), "-o", "etime=", "-o", "command="]);
        let Some((_, out, _)) = crate::sys::run_output(c, std::time::Duration::from_secs(3)) else { return false };
        let line = out.trim();
        let Some((etime, cmdline)) = line.split_once(char::is_whitespace) else { return false };
        let Some(elapsed) = parse_etime(etime) else { return false };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path_ok = cmdline.contains(&*exe.to_string_lossy()) || exe.parent().is_some_and(|d| cmdline.contains(&*d.to_string_lossy())) || also.is_some_and(|a| !a.is_empty() && cmdline.contains(a));
        if path_ok && same_start(noted_start, now.saturating_sub(elapsed)) {
            // (SAFETY: kill only sends a signal to the pid just checked)
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            return true;
        }
        let _ = also;
        false
    }
    #[cfg(windows)]
    {
        let _ = also;
        win::end_if_ours(pid, noted_start, exe)
    }
}

/// Parity A (QA4-F F6): under CrossOver the program the app starts is wine's launcher
/// (winewrapper.exe); the server itself is another process, named by its Windows path
/// (`C:\Loner-Strike Servers\<id>\hlds.exe -console ...`). Ending the launcher leaves it running on
/// its port. These are the pids in `ps -axo pid=,command=` output that are that program: the
/// Windows-side process (its command line starts with the path) and any launcher running it.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn wine_side_pids(ps: &str, win_exe: &str, me: u32) -> Vec<u32> {
    let exe = win_exe.to_lowercase();
    if exe.len() < 8 || !exe.ends_with(".exe") {
        return Vec::new();
    }
    let named = |cmd: &str| cmd.starts_with(&exe) && cmd[exe.len()..].chars().next().is_none_or(char::is_whitespace);
    ps.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (pid, cmd) = l.split_once(char::is_whitespace)?;
            let pid = pid.parse::<u32>().ok().filter(|p| *p > 1 && *p != me)?;
            let cmd = cmd.trim_start().to_lowercase();
            let launcher = cmd.contains("winewrapper") && cmd.split(" -- ").skip(1).any(|rest| named(rest.trim_start()));
            (named(&cmd) || launcher).then_some(pid)
        })
        .collect()
}

/// Ends a CrossOver program by its Windows path (see wine_side_pids): asked first (SIGTERM),
/// then made to (SIGKILL) if it's still there after `grace`. True when none is left.
#[cfg(unix)]
pub fn end_wine_side(win_exe: &str, grace: std::time::Duration) -> bool {
    let list = || {
        let mut c = crate::sys::command("ps");
        c.args(["-axo", "pid=,command="]);
        crate::sys::run_output(c, std::time::Duration::from_secs(5)).map(|(_, out, _)| wine_side_pids(&out, win_exe, std::process::id()))
    };
    let Some(pids) = list() else { return false };
    if pids.is_empty() {
        return true;
    }
    for p in &pids {
        // (SAFETY: kill only sends a signal; the pids are this very program, by its full path)
        unsafe { libc::kill(*p as i32, libc::SIGTERM) };
    }
    let until = std::time::Instant::now() + grace;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let left = list().unwrap_or_default();
        if left.is_empty() {
            return true;
        }
        if std::time::Instant::now() > until {
            for p in &left {
                unsafe { libc::kill(*p as i32, libc::SIGKILL) };
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            return list().is_some_and(|l| l.is_empty());
        }
    }
}

#[cfg(windows)]
mod win {
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE};

    /// FILETIME (100 ns since 1601) as seconds since 1970.
    fn unix_secs(f: &FILETIME) -> u64 {
        let t = ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        (t / 10_000_000).saturating_sub(11_644_473_600)
    }

    pub fn end_if_ours(pid: u32, noted_start: Option<u64>, exe: &Path) -> bool {
        // (SAFETY: plain Win32 calls on a handle opened here and closed before returning)
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE, 0, pid);
            if h.is_null() {
                return false;
            }
            let mut buf = [0u16; 1024];
            let mut n = buf.len() as u32;
            let named = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n) != 0;
            let path = String::from_utf16_lossy(&buf[..n as usize]);
            let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
            let timed = GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user) != 0;
            let ours = named && timed && crate::sys::same_windows_path(&path, &exe.to_string_lossy()) && super::same_start(noted_start, unix_secs(&created));
            let ended = ours && TerminateProcess(h, 1) != 0;
            CloseHandle(h);
            ended
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parity A (QA4-F F6), by hand on a Mac: a CrossOver server whose launcher was ended is
    /// still running (LS_WINE_EXE, its Windows path; LS_WINE_PORT, its port): end_wine_side ends
    /// it and its port is free after.
    #[test]
    #[ignore]
    #[cfg(unix)]
    fn wine_side_ends_a_leftover_server() {
        let exe = std::env::var("LS_WINE_EXE").unwrap();
        let port: u16 = std::env::var("LS_WINE_PORT").unwrap().parse().unwrap();
        let ps = |exe: &str| {
            let mut c = crate::sys::command("ps");
            c.args(["-axo", "pid=,command="]);
            wine_side_pids(&crate::sys::run_output(c, std::time::Duration::from_secs(5)).unwrap().1, exe, std::process::id())
        };
        let before = ps(&exe);
        assert!(!before.is_empty(), "the leftover server should be running");
        assert!(!crate::sys::udp_port_free(None, port), "it holds its port");
        let t = std::time::Instant::now();
        assert!(end_wine_side(&exe, std::time::Duration::from_secs(3)));
        assert!(ps(&exe).is_empty());
        assert!(crate::sys::udp_port_free(None, port), "its port is free");
        eprintln!("ended {before:?} in {:.1} s", t.elapsed().as_secs_f64());
    }

    #[test]
    fn wine_side_is_found_by_its_windows_path() {
        let ps = r#"  101 /Applications/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-windows/winewrapper.exe --workdir /Users/x/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Loner-Strike Servers/abc --run -- C:\Loner-Strike Servers\abc\hlds.exe -console -game cstrike -port 27015
  102 C:\Loner-Strike Servers\abc\hlds.exe -console -game cstrike -port 27015
  103 C:\Loner-Strike Servers\abc2\hlds.exe -console -game cstrike -port 27016
  104 C:\Loner-Strike Servers\abc\hlds.exe.bak
  105 /bin/zsh -c pgrep -f C:\Loner-Strike Servers\abc\hlds.exe
  106 c:\loner-strike servers\ABC\HLDS.EXE
    7 C:\Loner-Strike Servers\abc\hlds.exe
"#;
        assert_eq!(wine_side_pids(ps, r"C:\Loner-Strike Servers\abc\hlds.exe", 7), vec![101, 102, 106]);
        assert!(wine_side_pids(ps, "", 0).is_empty());
        assert!(wine_side_pids(ps, r"C:\x", 0).is_empty());
    }


    #[test]
    fn pid_files_old_and_new() {
        assert_eq!(parse_pid_file("4242 1794636723\n"), Some((4242, Some(1_794_636_723))));
        assert_eq!(parse_pid_file("4242"), Some((4242, None)));
        assert_eq!(parse_pid_file("0"), None);
        assert_eq!(parse_pid_file("hlds"), None);
        let (pid, start) = parse_pid_file(&pid_file_text(77)).unwrap();
        assert_eq!(pid, 77);
        assert!(start.is_some());
    }

    #[test]
    fn a_reused_pid_started_later_isnt_ours() {
        assert!(same_start(Some(1000), 1004));
        assert!(same_start(None, 5000), "an older pid file: the path decides");
        assert!(!same_start(Some(1000), 1000 + 3600), "started an hour later: another program with that pid");
    }

    #[test]
    fn ps_elapsed_times() {
        assert_eq!(parse_etime("05:07"), Some(307));
        assert_eq!(parse_etime("01:00:00"), Some(3600));
        assert_eq!(parse_etime("2-03:04:05"), Some(2 * 86400 + 3 * 3600 + 4 * 60 + 5));
        assert_eq!(parse_etime("x"), None);
    }

    /// A real child: ended when it's the one noted, left alone when the start time doesn't fit.
    #[cfg(unix)]
    #[test]
    fn ends_only_the_process_it_started() {
        let exe = Path::new("/bin/sleep");
        let mut child = std::process::Command::new(exe).arg("30").spawn().unwrap();
        let pid = child.id();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert!(!end_if_ours(pid, Some(now - 7200), exe, None), "a pid file from two hours ago");
        assert!(!end_if_ours(pid, Some(now), Path::new("/usr/bin/other"), None), "another program");
        assert!(end_if_ours(pid, Some(now), exe, None));
        let _ = child.wait();
    }
}
