//! The bot brain's map knowledge (`maps/<map>.lsv`, version 6), as cs_bot_ls.cpp saves it:
//! a 44-byte head, the sightline bit matrix (count x count bits, row-major), the visible pairs,
//! the wallbang spots, then one 315-byte record per nav area in the nav's file order.
//!
//! The head's `key` is the brain's LsNavKey over the nav it was built for; [`nav_key`] works it
//! out for a nav here, so the page can say when the knowledge is for a different nav.

use super::nav::Nav;
use super::{Error, Le, Result};

pub const LSV_MAGIC: u32 = 0x3256_534C; // "LSV2"
pub const LSV_VERSION: u32 = 6;
pub const HEAD_SIZE: usize = 44;
pub const PAIR_SIZE: usize = 6;
pub const BANG_SIZE: usize = 6;
pub const AREA_SIZE: usize = 315;
/// The brain gives maps with more areas no knowledge.
pub const MAX_AREAS: usize = 6000;
/// The biggest .lsv a 6000-area map could have, and some.
pub const MAX_LSV: u64 = 512 << 20;

pub const DIRS: usize = 16;
pub const CORNERS: usize = 8;
pub const ANGLES: usize = 8;
pub const OBJECTS: usize = 8;
pub const POINTS: usize = 9;

const WHAT: &str = "map knowledge file";

/// The stages the brain works through (a file saved part way says how far it got).
pub const STAGES: [&str; 5] = ["pairs", "areas", "angles", "exposure", "done"];

/// Object types (LSO_*), 1-based.
pub const OBJECT_TYPES: [&str; 6] = ["", "low cover", "door", "breakable", "box", "ladder"];

/// The game's material letters (CHAR_TEX_*) and what they are.
pub const MATERIALS: [(char, &str); 13] = [
    ('C', "concrete"),
    ('M', "metal"),
    ('D', "dirt"),
    ('V', "vent"),
    ('G', "grate"),
    ('T', "tile"),
    ('S', "slosh"),
    ('W', "wood"),
    ('P', "computer"),
    ('Y', "glass"),
    ('F', "flesh"),
    ('N', "snow"),
    ('X', "unknown"),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Head {
    pub magic: u32,
    pub version: u32,
    pub key: u32,
    pub count: u32,
    pub stage: u32,
    pub ci: u32,
    pub cj: u32,
    pub pairs: u32,
    pub bangs: u32,
    pub traces: u32,
    pub spent_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    pub a: u16,
    pub b: u16,
    /// 0-255: how much of one is in view from the other.
    pub quality: u8,
    /// Bit 1: a crouched head is in view too.
    pub flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bang {
    pub a: u16,
    pub b: u16,
    /// Units of wall in between.
    pub thick: u8,
    /// A material letter (0 unknown).
    pub mat: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corner {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    /// Which way it opens, 0-255 round.
    pub yaw: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Angle {
    /// 0-255 round.
    pub yaw: u8,
    /// Degrees, + is up.
    pub pitch: i8,
    pub dist: u16,
    pub weight: u8,
    /// The area (file-order index) enemies appear from.
    pub area: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Object {
    pub kind: u8,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub top: i16,
    pub mat: u8,
}

/// LsAreaKnow: what the brain knows about one area.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AreaKnow {
    /// Open distance / 8 all round, at a crouched head and at standing eyes.
    pub cover_low: [u8; DIRS],
    pub cover_high: [u8; DIRS],
    /// What's there close by, per direction (a material letter, 0 none).
    pub mat: [u8; DIRS],
    /// Ceiling height / 4.
    pub ceiling: u8,
    /// Areas in sight.
    pub sees: u16,
    /// Share (0-255) of those that are lower: high ground.
    pub high: u8,
    pub corners: Vec<Corner>,
    pub angles: Vec<Angle>,
    pub objects: Vec<Object>,
    /// Per angle: which of the area's 9 points it sees, at a standing and a crouched head.
    pub exposed_stand: Vec<u16>,
    pub exposed_crouch: Vec<u16>,
    /// Per point: bit d = something solid within 64 units that way (16 directions).
    pub point_cover_low: [u16; POINTS],
    pub point_cover_high: [u16; POINTS],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lsv {
    pub head: Head,
    /// count x count bits, row-major: bit (i * count + j) = i sees j.
    pub sight: Vec<u8>,
    pub pairs: Vec<Pair>,
    pub bangs: Vec<Bang>,
    pub areas: Vec<AreaKnow>,
}

impl Lsv {
    /// Reads a whole .lsv, with the brain's own checks (the length must match the counts
    /// exactly; indices must be in range). Counts in records are clamped as the brain does.
    pub fn parse(b: &[u8]) -> Result<Lsv> {
        let r = Le::new(b, WHAT);
        if b.len() < HEAD_SIZE {
            return Err(Error::NotFormat { what: WHAT, detail: "it's too short".into() });
        }
        let u = |i: usize| r.u32(i * 4);
        let head = Head {
            magic: u(0)?,
            version: u(1)?,
            key: u(2)?,
            count: u(3)?,
            stage: u(4)?,
            ci: u(5)?,
            cj: u(6)?,
            pairs: u(7)?,
            bangs: u(8)?,
            traces: u(9)?,
            spent_ms: r.f32(40)?,
        };
        if head.magic != LSV_MAGIC {
            return Err(Error::NotFormat { what: WHAT, detail: "it doesn't start like Loner-Strike map knowledge".into() });
        }
        if head.version != LSV_VERSION {
            return Err(Error::NotFormat {
                what: WHAT,
                detail: format!("version {}; this Loner-Strike reads version {LSV_VERSION} (play the map again to rebuild it)", head.version),
            });
        }
        let n = head.count as usize;
        if n > MAX_AREAS {
            return Err(Error::damaged(WHAT, format!("it says {n} areas (the brain stops at {MAX_AREAS})")));
        }
        if head.stage as usize >= STAGES.len() {
            return Err(Error::damaged(WHAT, format!("stage {}", head.stage)));
        }
        let bits = (n * n + 7) / 8;
        // (n <= 6000 and pairs/bangs are u32: no overflow in u64)
        let want = HEAD_SIZE as u64 + bits as u64 + head.pairs as u64 * PAIR_SIZE as u64 + head.bangs as u64 * BANG_SIZE as u64 + (n * AREA_SIZE) as u64;
        if want != b.len() as u64 {
            return Err(Error::damaged(WHAT, format!("{} bytes where its counts say {want}", b.len())));
        }
        let mut at = HEAD_SIZE;
        let sight = r.bytes(at, bits)?.to_vec();
        at += bits;
        let mut pairs = Vec::with_capacity(head.pairs as usize);
        for _ in 0..head.pairs {
            let p = Pair { a: r.u16(at)?, b: r.u16(at + 2)?, quality: r.u8(at + 4)?, flags: r.u8(at + 5)? };
            if p.a as usize >= n || p.b as usize >= n {
                return Err(Error::damaged(WHAT, format!("a sightline to area {} of {n}", p.a.max(p.b))));
            }
            pairs.push(p);
            at += PAIR_SIZE;
        }
        let mut bangs = Vec::with_capacity(head.bangs as usize);
        for _ in 0..head.bangs {
            let g = Bang { a: r.u16(at)?, b: r.u16(at + 2)?, thick: r.u8(at + 4)?, mat: r.u8(at + 5)? };
            if g.a as usize >= n || g.b as usize >= n {
                return Err(Error::damaged(WHAT, format!("a wallbang spot at area {} of {n}", g.a.max(g.b))));
            }
            bangs.push(g);
            at += BANG_SIZE;
        }
        let mut areas = Vec::with_capacity(n);
        for _ in 0..n {
            areas.push(read_area(&Le::new(r.bytes(at, AREA_SIZE)?, WHAT), n)?);
            at += AREA_SIZE;
        }
        Ok(Lsv { head, sight, pairs, bangs, areas })
    }

    /// Whether area `i` sees area `j` (file-order indices).
    #[allow(dead_code)] // (the page decodes the bits itself; the health check will use this)
    pub fn sees(&self, i: usize, j: usize) -> bool {
        let n = self.head.count as usize;
        if i >= n || j >= n {
            return false;
        }
        let bit = i * n + j;
        self.sight[bit >> 3] >> (bit & 7) & 1 == 1
    }

    /// The stage's name ("done" when finished).
    pub fn stage_name(&self) -> &'static str {
        STAGES[(self.head.stage as usize).min(STAGES.len() - 1)]
    }

    /// Writes the file back as the brain does (tests, and the nav fixer later).
    #[allow(dead_code)]
    pub fn write(&self) -> Vec<u8> {
        let h = &self.head;
        let mut o = Vec::new();
        for v in [h.magic, h.version, h.key, h.count, h.stage, h.ci, h.cj, self.pairs.len() as u32, self.bangs.len() as u32, h.traces] {
            o.extend(v.to_le_bytes());
        }
        o.extend(h.spent_ms.to_le_bytes());
        o.extend(&self.sight);
        for p in &self.pairs {
            o.extend(p.a.to_le_bytes());
            o.extend(p.b.to_le_bytes());
            o.extend([p.quality, p.flags]);
        }
        for g in &self.bangs {
            o.extend(g.a.to_le_bytes());
            o.extend(g.b.to_le_bytes());
            o.extend([g.thick, g.mat]);
        }
        for a in &self.areas {
            write_area(&mut o, a);
        }
        o
    }
}

fn read_area(r: &Le, n: usize) -> Result<AreaKnow> {
    let mut k = AreaKnow::default();
    k.cover_low.copy_from_slice(r.bytes(0, DIRS)?);
    k.cover_high.copy_from_slice(r.bytes(16, DIRS)?);
    k.mat.copy_from_slice(r.bytes(32, DIRS)?);
    k.ceiling = r.u8(48)?;
    k.sees = r.u16(49)?;
    k.high = r.u8(51)?;
    let nc = (r.u8(52)? as usize).min(CORNERS);
    let mut na = (r.u8(53)? as usize).min(ANGLES);
    let no = (r.u8(54)? as usize).min(OBJECTS);
    let mut at = 55;
    for c in 0..CORNERS {
        if c < nc {
            k.corners.push(Corner { x: r.i16(at)?, y: r.i16(at + 2)?, z: r.i16(at + 4)?, yaw: r.u8(at + 6)? });
        }
        at += 7;
    }
    for a in 0..ANGLES {
        let v = Angle { yaw: r.u8(at)?, pitch: r.u8(at + 1)? as i8, dist: r.u16(at + 2)?, weight: r.u8(at + 4)?, area: r.u16(at + 5)? };
        // (as the brain does: an angle pointing past the areas ends the list)
        if a < na && v.area as usize >= n {
            na = a;
        }
        if a < na {
            k.angles.push(v);
        }
        at += 7;
    }
    for o in 0..OBJECTS {
        if o < no {
            k.objects.push(Object { kind: r.u8(at)?, x: r.i16(at + 1)?, y: r.i16(at + 3)?, z: r.i16(at + 5)?, top: r.i16(at + 7)?, mat: r.u8(at + 9)? });
        }
        at += 10;
    }
    for a in 0..ANGLES {
        if a < na {
            k.exposed_stand.push(r.u16(at + a * 2)?);
            k.exposed_crouch.push(r.u16(at + 16 + a * 2)?);
        }
    }
    at += 32;
    for p in 0..POINTS {
        k.point_cover_low[p] = r.u16(at + p * 2)?;
        k.point_cover_high[p] = r.u16(at + 18 + p * 2)?;
    }
    debug_assert_eq!(at + 36, AREA_SIZE);
    Ok(k)
}

#[allow(dead_code)]
fn write_area(o: &mut Vec<u8>, k: &AreaKnow) {
    let start = o.len();
    o.extend(k.cover_low);
    o.extend(k.cover_high);
    o.extend(k.mat);
    o.push(k.ceiling);
    o.extend(k.sees.to_le_bytes());
    o.push(k.high);
    o.extend([k.corners.len() as u8, k.angles.len() as u8, k.objects.len() as u8]);
    for c in 0..CORNERS {
        let v = k.corners.get(c).copied().unwrap_or_default();
        o.extend(v.x.to_le_bytes());
        o.extend(v.y.to_le_bytes());
        o.extend(v.z.to_le_bytes());
        o.push(v.yaw);
    }
    for a in 0..ANGLES {
        let v = k.angles.get(a).copied().unwrap_or_default();
        o.extend([v.yaw, v.pitch as u8]);
        o.extend(v.dist.to_le_bytes());
        o.push(v.weight);
        o.extend(v.area.to_le_bytes());
    }
    for i in 0..OBJECTS {
        let v = k.objects.get(i).copied().unwrap_or_default();
        o.push(v.kind);
        for s in [v.x, v.y, v.z, v.top] {
            o.extend(s.to_le_bytes());
        }
        o.push(v.mat);
    }
    for list in [&k.exposed_stand, &k.exposed_crouch] {
        for a in 0..ANGLES {
            o.extend(list.get(a).copied().unwrap_or(0).to_le_bytes());
        }
    }
    for list in [&k.point_cover_low, &k.point_cover_high] {
        for v in list {
            o.extend(v.to_le_bytes());
        }
    }
    debug_assert_eq!(o.len() - start, AREA_SIZE);
}

/// The brain's LsNavKey: FNV-1a over the area count, then each area's id and integer centre
/// (truncated towards zero, as a C cast), in the nav's file order.
pub fn nav_key(nav: &Nav) -> u32 {
    let mut h: u32 = 2_166_136_261;
    let mut mix = |v: u32| {
        h ^= v;
        h = h.wrapping_mul(16_777_619);
    };
    mix(nav.areas.len() as u32);
    for a in &nav.areas {
        let c = a.center();
        mix(a.id);
        for v in c {
            // (a C float-to-int cast; Rust's `as` saturates where C would be undefined)
            mix(v as i32 as u32);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::nav::{Area, Nav};

    // the brain's packed structs, field by field (cs_bot.h / cs_bot_ls.cpp)
    #[test]
    fn sizes_match_the_brain() {
        assert_eq!(HEAD_SIZE, 11 * 4);
        assert_eq!(PAIR_SIZE, 2 + 2 + 1 + 1);
        let corner = 2 * 3 + 1;
        let angle = 1 + 1 + 2 + 1 + 2;
        let object = 1 + 2 * 4 + 1;
        assert_eq!(AREA_SIZE, 16 * 3 + 1 + 2 + 1 + 3 + 8 * corner + 8 * angle + 8 * object + 8 * 2 * 2 + 9 * 2 * 2);
        let mut o = Vec::new();
        write_area(&mut o, &AreaKnow::default());
        assert_eq!(o.len(), 315);
    }

    fn sample() -> Lsv {
        let n = 3;
        let mut k = AreaKnow { ceiling: 32, sees: 2, high: 128, ..Default::default() };
        k.cover_low[3] = 8;
        k.mat[3] = b'M';
        k.corners = vec![Corner { x: -100, y: 200, z: -32, yaw: 64 }];
        k.angles = vec![Angle { yaw: 10, pitch: -12, dist: 900, weight: 5, area: 2 }, Angle { yaw: 200, pitch: 3, dist: 40, weight: 1, area: 1 }];
        k.objects = vec![Object { kind: 2, x: 1, y: 2, z: 3, top: 60, mat: b'W' }];
        k.exposed_stand = vec![0x1ff, 0x010];
        k.exposed_crouch = vec![0x0f0, 0];
        k.point_cover_low[4] = 0xffff;
        let mut sight = vec![0u8; (n * n + 7) / 8];
        for (i, j) in [(0, 2), (2, 0)] {
            sight[(i * n + j) >> 3] |= 1 << ((i * n + j) & 7);
        }
        Lsv {
            head: Head { magic: LSV_MAGIC, version: 6, key: 77, count: n as u32, stage: 4, ci: 3, cj: 3, pairs: 1, bangs: 1, traces: 1234, spent_ms: 56.5 },
            sight,
            pairs: vec![Pair { a: 0, b: 2, quality: 200, flags: 1 }],
            bangs: vec![Bang { a: 0, b: 1, thick: 8, mat: b'W' }],
            areas: vec![k, AreaKnow::default(), AreaKnow::default()],
        }
    }

    #[test]
    fn round_trip() {
        let l = sample();
        let b = l.write();
        assert_eq!(b.len(), 44 + 2 + 6 + 6 + 3 * 315);
        let back = Lsv::parse(&b).unwrap();
        assert_eq!(back, l);
        assert!(back.sees(0, 2) && back.sees(2, 0) && !back.sees(0, 1) && !back.sees(5, 0));
        assert_eq!(back.stage_name(), "done");
    }

    #[test]
    fn bad_files_are_errors() {
        let b = sample().write();
        assert!(Lsv::parse(&b[..b.len() - 1]).is_err());
        let mut longer = b.clone();
        longer.push(0);
        assert!(Lsv::parse(&longer).is_err());
        let mut v5 = b.clone();
        v5[4] = 5;
        assert!(Lsv::parse(&v5).unwrap_err().to_string().contains("version 5"));
        // a pair pointing past the areas
        let mut bad = b.clone();
        bad[44 + 2] = 9;
        assert!(Lsv::parse(&bad).is_err());
        // huge counts don't allocate or overflow
        let mut huge = b.clone();
        huge[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Lsv::parse(&huge).is_err());
        let mut many = b.clone();
        many[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Lsv::parse(&many).is_err());
        // an angle pointing past the areas ends that area's list, as in the brain
        let mut angle = b.clone();
        let at = 44 + 2 + 12 + 55 + 8 * 7 + 7 + 5; // area 0, angle 1's area
        angle[at..at + 2].copy_from_slice(&50u16.to_le_bytes());
        assert_eq!(Lsv::parse(&angle).unwrap().areas[0].angles.len(), 1);
    }

    #[test]
    fn key_is_the_brains() {
        let a = |id, x: f32| Area {
            id,
            attr: 0,
            lo: [x, -10.5, 0.0],
            hi: [x + 25.0, 10.0, 1.0],
            ne_z: 0.0,
            sw_z: 0.0,
            conns: Default::default(),
            hiding: vec![],
            approach: vec![],
            encounters: vec![],
            place: 0,
        };
        let nav = Nav { version: 5, bsp_size: 0, places: vec![], areas: vec![a(1, 0.0), a(2, -100.0)] };
        // worked by hand: count 2, then (1, 12, 0, 0), (2, -87 as u32, 0, 0)
        let mut h: u32 = 2_166_136_261;
        for v in [2u32, 1, 12, 0, 0, 2, (-87i32) as u32, 0, 0] {
            h ^= v;
            h = h.wrapping_mul(16_777_619);
        }
        assert_eq!(nav_key(&nav), h);
    }

    /// LS_TEST_SIM: the shipped and fixed knowledge files read, write back exactly, and their
    /// keys match their navs.
    #[test]
    fn real_files() {
        let Ok(sim) = std::env::var("LS_TEST_SIM") else { return };
        let sim = std::path::PathBuf::from(sim);
        for m in ["navpack/fixes/cs_rats2", "navpack/out/de_dust2", "navpack/out/de_nuke"] {
            let b = std::fs::read(sim.join(format!("{m}.lsv"))).unwrap();
            let l = Lsv::parse(&b).unwrap();
            assert_eq!(l.write(), b, "{m}");
            let nav = Nav::parse(&std::fs::read(sim.join(format!("{m}.nav"))).unwrap()).unwrap();
            assert_eq!(l.head.count as usize, nav.areas.len(), "{m}");
            assert_eq!(nav_key(&nav), l.head.key, "{m}");
        }
        // the cs_rats2 knowledge isn't for the unfixed nav
        let fixed = Lsv::parse(&std::fs::read(sim.join("navpack/fixes/cs_rats2.lsv")).unwrap()).unwrap();
        let old = Nav::parse(&std::fs::read(sim.join("navpack/out/cs_rats2.nav")).unwrap()).unwrap();
        assert_ne!(nav_key(&old), fixed.head.key);
    }
}
