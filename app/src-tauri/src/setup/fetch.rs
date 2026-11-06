//! Downloads (cached and checked by sha256) and unpacking (installer/fetch.py).
//!
//! The cache is <cache dir>/Loner-Strike/downloads; files the Python installer
//! already downloaded (~/.loner-strike/downloads) are reused once they check out.

use super::Progress;
use crate::archive;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const USER_AGENT: &str = "Loner-Strike-installer/0.1";
/// Nothing we install comes close; this only stops a runaway download or archive.
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn cache_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("LONER_STRIKE_DOWNLOADS") {
        return PathBuf::from(p);
    }
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("Loner-Strike").join("downloads")
}

/// The Python installer's cache (not used when LONER_STRIKE_DOWNLOADS picks a folder).
fn legacy_cache() -> Option<PathBuf> {
    if std::env::var_os("LONER_STRIKE_DOWNLOADS").is_some() {
        return None;
    }
    dirs::home_dir().map(|h| h.join(".loner-strike").join("downloads"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
pub fn hex_sha256(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub fn sha256(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn matches(path: &Path, sha: Option<&str>) -> bool {
    path.is_file() && sha.map(|s| sha256(path).map(|h| h.eq_ignore_ascii_case(s)).unwrap_or(false)).unwrap_or(true)
}

/// The cache file name, as fetch.py names it: sha1(url)[:16]-<last path part>.
pub fn cache_name(url: &str) -> String {
    let digest = hex(&Sha1::digest(url.as_bytes()));
    let base = url.split('?').next().unwrap_or(url).rsplit('/').next().unwrap_or("");
    format!("{}-{}", &digest[..16], base)
}

/// Path to a cached copy of url, downloading it if needed. With sha, the file must
/// match it exactly (a changed upstream file stops the install). `what` names the
/// download for the progress line ("AMX Mod X").
pub fn download(url: &str, sha: Option<&str>, what: &str, p: &dyn Progress) -> Result<PathBuf, String> {
    let cache = cache_dir();
    std::fs::create_dir_all(&cache).map_err(|e| format!("Couldn't make the download folder {} ({e}).", cache.display()))?;
    let name = cache_name(url);
    let path = cache.join(&name);
    if matches(&path, sha) {
        return Ok(path);
    }
    if let Some(old) = legacy_cache().map(|d| d.join(&name)) {
        if matches(&old, sha) && std::fs::copy(&old, &path).is_ok() {
            return Ok(path);
        }
    }
    p.detail(&format!("Downloading {what}"));
    let tmp = cache.join(format!("{name}.part"));
    let fail = |e: String| format!("Couldn't download {what} ({e}). Check your internet connection and try again.");
    let mut resp = crate::sys::http()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(900))
        .send()
        .map_err(|e| fail(short_err(&e)))?;
    if !resp.status().is_success() {
        return Err(fail(format!("the server answered {}", resp.status())));
    }
    let size = resp.content_length().unwrap_or(0);
    let mut out = std::fs::File::create(&tmp).map_err(|e| fail(e.to_string()))?;
    let mut got: u64 = 0;
    let mut last_shown = 0u64;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = match resp.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(fail(e.to_string()));
            }
        };
        got += n as u64;
        if got > MAX_BYTES {
            let _ = std::fs::remove_file(&tmp);
            return Err(fail("it's far bigger than expected".into()));
        }
        out.write_all(&buf[..n]).map_err(|e| fail(e.to_string()))?;
        if got - last_shown >= 512 * 1024 {
            last_shown = got;
            let mb = |b: u64| b as f64 / 1_048_576.0;
            if size > 0 {
                p.detail(&format!("Downloading {what} ({:.1} of {:.1} MB)", mb(got), mb(size)));
                p.fraction(got as f64 / size as f64);
            } else {
                p.detail(&format!("Downloading {what} ({:.1} MB)", mb(got)));
            }
        }
    }
    drop(out);
    if let Some(s) = sha {
        if !sha256(&tmp).map(|h| h.eq_ignore_ascii_case(s)).unwrap_or(false) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!(
                "The download of {what} isn't the file Loner-Strike was tested with (it changed upstream), so setup stopped to keep your game safe."
            ));
        }
    }
    std::fs::rename(&tmp, &path).map_err(|e| fail(e.to_string()))?;
    Ok(path)
}

fn short_err(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "it took too long".into()
    } else if e.is_connect() {
        "couldn't connect".into()
    } else {
        e.to_string()
    }
}

// ---- unpacking -------------------------------------------------------------------

/// Unpacks zip and tar (.gz/.xz/.bz2) archives into dest; refuses paths that leave
/// dest and skips links. Anything else (rar, 7z) goes to the map store's unpacker.
pub fn unpack(archive_path: &Path, dest: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 6];
    let n = std::fs::File::open(archive_path).and_then(|mut f| f.read(&mut magic)).map_err(|e| e.to_string())?;
    let magic = &magic[..n];
    let bad = |e: String| format!("Couldn't unpack a download ({e}). Try again; if it keeps happening, clear Loner-Strike's download cache.");
    let open = || std::fs::File::open(archive_path).map(std::io::BufReader::new).map_err(|e| bad(e.to_string()));
    if magic.starts_with(b"PK\x03\x04") || magic.starts_with(b"PK\x05\x06") {
        unzip(archive_path, dest).map_err(bad)?;
    } else if magic.starts_with(&[0x1f, 0x8b]) {
        untar(flate2::read::MultiGzDecoder::new(open()?), dest).map_err(bad)?;
    } else if magic.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
        untar(lzma_rust2::XzReader::new(open()?, true), dest).map_err(bad)?;
    } else if magic.starts_with(b"BZh") {
        untar(bzip2::read::MultiBzDecoder::new(open()?), dest).map_err(bad)?;
    } else if is_plain_tar(archive_path) {
        untar(open()?, dest).map_err(bad)?;
    } else {
        archive::extract(archive_path, dest, archive::MAX_UNPACKED).map_err(bad)?;
        archive::strip_links(dest);
    }
    Ok(dest.to_path_buf())
}

fn is_plain_tar(path: &Path) -> bool {
    let mut b = [0u8; 262];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut b)).is_ok() && &b[257..262] == b"ustar"
}

/// dest/name, or an error if name would leave dest.
fn safe_target(dest: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let name = name.replace('\\', "/");
    let trimmed = name.trim_start_matches("./");
    if trimmed.is_empty() || trimmed == "." {
        return Ok(None);
    }
    super::record::inside(dest, trimmed).map(Some).ok_or_else(|| format!("it contains an unsafe path: {name}"))
}

fn write_limited(mut src: impl Read, target: &Path, total: &mut u64) -> Result<(), String> {
    if let Some(d) = target.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let mut out = std::fs::File::create(target).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = src.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        *total += n as u64;
        if *total > MAX_BYTES {
            return Err("it unpacks to far more than expected".into());
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
}

fn untar(r: impl Read, dest: &Path) -> Result<(), String> {
    let mut t = tar::Archive::new(r);
    let mut total = 0u64;
    for entry in t.entries().map_err(|e| e.to_string())? {
        let mut e = entry.map_err(|e| e.to_string())?;
        let kind = e.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            continue; // no links or devices
        }
        let name = e.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
        let Some(target) = safe_target(dest, &name)? else { continue };
        if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            write_limited(&mut e, &target, &mut total)?;
        }
    }
    Ok(())
}

fn unzip(path: &Path, dest: &Path) -> Result<(), String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut m = z.by_index(i).map_err(|e| e.to_string())?;
        if m.is_symlink() {
            continue;
        }
        let name = m.name().map_err(|e| e.to_string())?.to_string();
        let Some(target) = safe_target(dest, &name)? else { continue };
        if m.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            write_limited(&mut m, &target, &mut total)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_names_match_python() {
        // the Python installer's names, so its cache can be reused
        assert_eq!(
            cache_name("https://github.com/rehlds/ReGameDLL_CS/releases/download/5.30.0.814/regamedll-bin-5.30.0.814.zip"),
            "1b489a21eb2b1398-regamedll-bin-5.30.0.814.zip"
        );
    }

    #[test]
    fn refuses_escaping_paths() {
        let d = Path::new("/tmp/x");
        assert!(safe_target(d, "../evil").is_err());
        assert!(safe_target(d, "/etc/passwd").is_err());
        assert!(safe_target(d, "a/../../b").is_err());
        assert_eq!(safe_target(d, "./a/b").unwrap(), Some(d.join("a/b")));
        assert_eq!(safe_target(d, "./").unwrap(), None);
    }
}
