//! Heavy views from the last compile (agent Checks): how many faces each VIS leaf can see,
//! read from the compiled .bsp in the compile work folder (or the game's maps folder), cached
//! until the file changes. The checks fall back to a sampled estimate without one.

use crate::editor::checks::views::pvs_counts;
use crate::editor::math::Vec3;
use crate::server::Shared;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

type Counts = (Vec<(Vec3, usize)>, String);

/// The newest compiled .bsp of the map.
fn bsp_of(st: &Shared, map: &str) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = super::budget::work_dirs(st, map).into_iter().map(|d| d.join(format!("{map}.bsp"))).collect();
    if let Some(g) = st.local_game() {
        v.push(g.hl.join("cstrike").join("maps").join(format!("{map}.bsp")));
    }
    v.into_iter().filter(|p| p.is_file()).max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH))
}

pub fn vis_for(st: &Shared, map: &str) -> Option<Counts> {
    if map.is_empty() || map.len() > 64 || !map.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')) || map.contains("..") {
        return None;
    }
    let path = bsp_of(st, map)?;
    let md = std::fs::metadata(&path).ok()?;
    if md.len() > 64 << 20 {
        return None;
    }
    let stamp = (path.clone(), md.len(), md.modified().ok()?);
    static CACHE: OnceLock<Mutex<Option<((PathBuf, u64, SystemTime), Option<Counts>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Some((k, v)) = cache.lock().ok()?.as_ref() {
        if *k == stamp {
            return v.clone();
        }
    }
    let bytes = std::fs::read(&path).ok()?;
    let when = md.modified().ok().map(|t| {
        let secs = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        day_time(secs)
    });
    let v = pvs_counts(&bytes).ok().filter(|c| !c.is_empty()).map(|c| (c, when.unwrap_or_default()));
    if let Ok(mut g) = cache.lock() {
        *g = Some((stamp, v.clone()));
    }
    v
}

/// "2026-11-13 15:04" (UTC) for a Unix time.
pub fn day_time(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let (h, m) = ((secs % 86400) / 3600, (secs % 3600) / 60);
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if mo <= 2 { 1 } else { 0 };
    format!("{y}-{mo:02}-{d:02} {h:02}:{m:02} UTC")
}

#[cfg(test)]
mod tests {
    #[test]
    fn day_time_reads() {
        assert_eq!(super::day_time(0), "1970-01-01 00:00 UTC");
        assert_eq!(super::day_time(1_763_046_000), "2025-11-13 15:00 UTC");
    }
}
