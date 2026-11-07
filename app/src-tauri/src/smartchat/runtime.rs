//! Smarter chat's files and llama-server: downloading a model or the server (checked by
//! sha256), unpacking the server, and running it on 127.0.0.1 with a random free port.

use super::catalog::{self, Model};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// <app folder>/smarter-chat (LONER_STRIKE_AI_DIR picks another, for testing).
pub fn dir() -> PathBuf {
    if let Some(p) = std::env::var_os("LONER_STRIKE_AI_DIR") {
        return PathBuf::from(p);
    }
    crate::sys::app_dir().join("smarter-chat")
}

pub fn model_path(m: &Model) -> PathBuf {
    dir().join("models").join(format!("{}.gguf", m.id))
}

/// Downloaded and complete (the checksum is checked when it's downloaded; a file of the
/// right size is trusted after that, so the check doesn't hash gigabytes every time).
pub fn has_model(m: &Model) -> bool {
    std::fs::metadata(model_path(m)).map(|md| md.len() == m.size).unwrap_or(false)
}

fn runtime_dir() -> PathBuf {
    dir().join(format!("llama.cpp-{}", catalog::LLAMA_CPP_TAG))
}

/// llama-server, once it's been downloaded.
pub fn server_exe() -> Option<PathBuf> {
    let name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    let root = runtime_dir();
    // the archive puts it at the top or in one folder (llama-b11722/)
    let direct = root.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(&root).ok()?.flatten().map(|e| e.path().join(name)).find(|p| p.is_file())
}

/// Older llama.cpp builds and leftover partial downloads.
pub fn tidy_old() {
    let keep = runtime_dir();
    if let Ok(rd) = std::fs::read_dir(dir()) {
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() && name.starts_with("llama.cpp-") && p != keep {
                let _ = std::fs::remove_dir_all(&p);
            }
        }
    }
}

/// Downloads url to dest, checking sha256 as it goes. `progress(bytes so far)`.
pub fn download(url: &str, sha: &str, dest: &Path, what: &str, progress: &dyn Fn(u64)) -> Result<(), String> {
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't make the folder for {what} ({e})."))?;
    }
    let part = dest.with_extension("part");
    let fail = |e: String| format!("Couldn't download {what} ({e}). Check your internet connection and try again.");
    let mut resp = crate::sys::http()
        .get(url)
        .header("User-Agent", "Loner-Strike/0.1")
        .timeout(Duration::from_secs(4 * 3600))
        .send()
        .map_err(|e| fail(if e.is_connect() { "couldn't connect".into() } else { e.to_string() }))?;
    if !resp.status().is_success() {
        return Err(fail(format!("the server answered {}", resp.status())));
    }
    let mut out = std::fs::File::create(&part).map_err(|e| fail(e.to_string()))?;
    let mut h = Sha256::new();
    let mut got = 0u64;
    let mut buf = vec![0u8; 512 * 1024];
    let result = loop {
        let n = match resp.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => n,
            Err(e) => break Err(fail(e.to_string())),
        };
        got += n as u64;
        if got > 5 * 1024 * 1024 * 1024 {
            break Err(fail("it's far bigger than expected".into()));
        }
        h.update(&buf[..n]);
        if let Err(e) = out.write_all(&buf[..n]) {
            break Err(fail(format!("couldn't save it: {e}. Is the disk full?")));
        }
        progress(got);
    };
    drop(out);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let result = result.and_then(|_| {
        if hex.eq_ignore_ascii_case(sha) {
            Ok(())
        } else {
            Err(format!("The download of {what} isn't the file Loner-Strike expects (it changed upstream), so it wasn't kept."))
        }
    });
    match result {
        Ok(()) => std::fs::rename(&part, dest).map_err(|e| fail(e.to_string())),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// Downloads and unpacks llama-server for this computer, if it isn't there yet.
pub fn ensure_server(progress: &dyn Fn(u64)) -> Result<PathBuf, String> {
    if let Some(p) = server_exe() {
        return Ok(p);
    }
    let rt = catalog::runtime().ok_or("Smarter chat doesn't run on this kind of computer yet.")?;
    let tmp = dir().join("download");
    let archive = tmp.join(rt.url.rsplit('/').next().unwrap_or("llama.zip"));
    download(rt.url, rt.sha256, &archive, "llama.cpp", progress)?;
    let dest = runtime_dir();
    let _ = std::fs::remove_dir_all(&dest);
    let unpacked = if rt.url.ends_with(".zip") {
        crate::setup::fetch::unpack(&archive, &dest).map(|_| ())
    } else {
        untar_gz(&archive, &dest)
    };
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(format!("Couldn't unpack llama.cpp ({e})."));
    }
    tidy_old();
    server_exe().ok_or_else(|| "The llama.cpp download is missing llama-server.".into())
}

/// Unpacks a .tar.gz into dest: files keep their permissions, and the links between
/// libraries (libllama.dylib -> libllama.0.7.0.dylib) are kept, but only links to a
/// file in the same folder. Nothing may land outside dest.
fn untar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut t = tar::Archive::new(flate2::read::GzDecoder::new(std::io::BufReader::new(f)));
    let mut links: Vec<(PathBuf, String)> = Vec::new();
    let mut total = 0u64;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in t.entries().map_err(|e| e.to_string())? {
        let mut e = entry.map_err(|e| e.to_string())?;
        let name = e.path().map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        let name = name.trim_start_matches("./").to_string();
        if name.is_empty() {
            continue;
        }
        let target = crate::setup::record::inside(dest, &name).ok_or_else(|| format!("it contains an unsafe path: {name}"))?;
        let kind = e.header().entry_type();
        if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else if kind.is_file() {
            if let Some(d) = target.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let mode = e.header().mode().unwrap_or(0o644);
            let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
            total += std::io::copy(&mut e, &mut out).map_err(|e| e.to_string())?;
            if total > 1024 * 1024 * 1024 {
                return Err("it unpacks to far more than expected".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode & 0o755));
            }
            #[cfg(not(unix))]
            let _ = mode;
        } else if kind.is_symlink() {
            let to = e.link_name().map_err(|e| e.to_string())?.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            // a plain file name beside it, nothing else
            if !to.is_empty() && !to.contains('/') && !to.contains('\\') && to != "." && to != ".." {
                links.push((target, to));
            }
        }
    }
    for (at, to) in links {
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&at);
            std::os::unix::fs::symlink(&to, &at).map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        if let Some(d) = at.parent() {
            let _ = std::fs::copy(d.join(&to), &at);
        }
    }
    Ok(())
}

// ---- llama-server ------------------------------------------------------------------

pub struct Server {
    child: Child,
    pub port: u16,
    pub model: &'static Model,
    /// its last lines of output, for saying why it stopped
    log: Arc<Mutex<VecDeque<String>>>,
}

fn free_port() -> Option<u16> {
    std::net::TcpListener::bind(("127.0.0.1", 0)).ok()?.local_addr().ok().map(|a| a.port())
}

/// A few threads, leaving the rest for the game.
fn threads() -> usize {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (n / 2).clamp(2, 6)
}

impl Server {
    pub fn start(exe: &Path, m: &'static Model) -> Result<Server, String> {
        let port = free_port().ok_or("Couldn't find a free port on this computer.")?;
        let mut cmd = crate::sys::command(exe);
        cmd.args(["-m"])
            .arg(model_path(m))
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            // two slots of 1024 tokens each; the shared start of every prompt stays cached
            .args(["-c", "2048", "-np", "2", "--cache-ram", "256", "-t", &threads().to_string()])
            .args(["--no-webui", "--reasoning", "off", "--offline", "--no-slots"])
            .current_dir(exe.parent().unwrap_or(Path::new(".")))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // for measuring: LONER_STRIKE_AI_NGL=0 runs it on the CPU only, like a PC without a GPU build
        if let Ok(n) = std::env::var("LONER_STRIKE_AI_NGL") {
            cmd.args(["-ngl", &n]);
        }
        let mut child = cmd.spawn().map_err(|e| format!("Couldn't start llama-server ({e})."))?;
        crate::sys::tie_to_app(&child);
        let log = Arc::new(Mutex::new(VecDeque::new()));
        for pipe in [child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>), child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>)].into_iter().flatten() {
            let log = log.clone();
            std::thread::spawn(move || {
                for line in std::io::BufReader::new(pipe).lines().map_while(Result::ok) {
                    let mut l = log.lock().unwrap_or_else(|e| e.into_inner());
                    if l.len() >= 40 {
                        l.pop_front();
                    }
                    l.push_back(line);
                }
            });
        }
        Ok(Server { child, port, model: m, log })
    }

    /// Still running? Err(why) once it has stopped.
    pub fn alive(&mut self) -> Result<(), String> {
        match self.child.try_wait() {
            Ok(None) => Ok(()),
            _ => Err(self.why()),
        }
    }

    /// A short reason from its output: the last error line, if it printed one.
    pub fn why(&self) -> String {
        let log = self.log.lock().unwrap_or_else(|e| e.into_inner());
        let err = log.iter().rev().find(|l| {
            let l = l.to_lowercase();
            l.contains("error") || l.contains("failed") || l.contains("unknown model") || l.contains("not enough")
        });
        match err {
            Some(l) => {
                let l = l.trim();
                let l = l.split_once(": ").map(|(_, r)| r).filter(|r| r.len() > 8).unwrap_or(l);
                format!("llama-server stopped: {}", l.chars().take(140).collect::<String>())
            }
            None => "llama-server stopped unexpectedly.".into(),
        }
    }

    /// Loaded and answering.
    pub fn healthy(&self) -> bool {
        crate::sys::http()
            .get(format!("http://127.0.0.1:{}/health", self.port))
            .timeout(Duration::from_millis(800))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub fn stop(self) {
        crate::sys::stop_child(self.child, Duration::from_secs(2));
    }
}
