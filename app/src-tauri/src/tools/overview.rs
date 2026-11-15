//! The radar (overview) maker (Tools > Make, Beta): a top-down picture of a map in the
//! format Counter-Strike 1.6 and Condition Zero read for the spectator overview, drawn here
//! from the map file itself.
//!
//! - `GET  /api/tools/overview/:name`              what's installed now, the framing we'd use
//! - `GET  /api/tools/overview/:name/preview.png`  our picture (query: [`Options`])
//! - `GET  /api/tools/overview/:name/current.png`  the picture installed now, as PNG
//! - `POST /api/tools/overview/:name/install`      `{...Options, format: "bmp"|"tga", res}`:
//!   writes `overviews/<map>.txt` and the picture (old files kept as `.bak` the first time)
//!   and, for a downloaded map, brings its .res up to date so players get both
//! - `POST /api/tools/overview/:name/restore`      puts the `.bak` files back
//!
//! What the game reads (Valve's HLSDK, cl_dll/hud_spectator.cpp `ParseOverviewFile`,
//! `DrawOverviewLayer`; checked against the files the Steam game ships):
//! - `overviews/<map>.txt`: `global { ZOOM z  ORIGIN x y z  ROTATED 0|1 }` then
//!   `layer { IMAGE "overviews/<map>.bmp"  HEIGHT h }`. Only the first layer is drawn.
//! - The picture is cut into 128-pixel tiles (`LoadMapSprite`), `numframes / 12` must be a
//!   square: 1024x768 is what every stock file uses. 8-bit (256-colour) BMP, or 24-bit TGA
//!   (the Steam game ships 46 overviews as uncompressed 24-bit TGA, so both load).
//! - At zoom 1 the picture's long side covers 8192 units (8 units a pixel at 1024 wide),
//!   centred on ORIGIN's x and y. `ROTATED 0`: columns run along -Y, rows along -X (the map
//!   turned a quarter); `ROTATED 1`: columns along +X, rows along -Y (north up). HEIGHT is the
//!   z the picture is drawn at in the 3D overview; ORIGIN's z isn't used.
//! - The overview is what spectators see in overview mode (and the inset map), in a live
//!   game and when watching an HLTV demo. The in-game radar is a plain circle: it doesn't use it.
//!
//! The picture: every floor (faces facing up) seen from above, the highest one at each
//! pixel, lit with the map's own lightmaps through the game's light curve. Roofs are left out
//! with the bots' nav when the map has one (only floors a nav area stands on count), else
//! with a height above which nothing is drawn. Edges where the floor drops away are drawn
//! dark, like walls in the stock overviews.

use super::mapinfo::MapFile;
use super::{blocking, cache, find_map, json_answer, locate, Fail};
use crate::goldsrc::{self, bsp::Bsp, entities, entities::Entity, lightmap, mesh, nav, wad};
use crate::mapcopy::Finder;
use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 768;
/// A floor: its face points up at least this much (ramps up to ~70 degrees count).
const UP: f32 = 0.3;
/// A height drop bigger than this (a stair step is 18) is a wall edge.
const EDGE_STEP: f32 = 24.0;
/// A nav area this close above or below a face stands on it.
const NAV_REACH: f32 = 40.0;
/// Room around the map, as a share of its size.
const MARGIN: f32 = 0.04;
/// With the nav: other floors this close (world units) to a walked one are drawn too.
const NEAR: f32 = 96.0;

// ---- the overview file ------------------------------------------------------------------

/// Where the picture sits in the world, as the overview file says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub zoom: f32,
    pub origin: [f32; 3],
    pub rotated: bool,
    /// the layer's HEIGHT
    pub height: f32,
}

impl Frame {
    /// World units one pixel covers (at 1024 wide).
    pub fn unit(&self) -> f32 {
        8192.0 / (self.zoom * WIDTH as f32)
    }

    /// The picture position (pixels from the top left, continuous) of a world point.
    pub fn to_pixel(&self, x: f32, y: f32) -> (f32, f32) {
        let (u, z) = (self.unit(), self.zoom);
        let (ox, oy) = (self.origin[0], self.origin[1]);
        if self.rotated {
            ((x - ox + 4096.0 / z) / u, (oy + 3072.0 / z - y) / u)
        } else {
            ((oy + 4096.0 / z - y) / u, (ox + 3072.0 / z - x) / u)
        }
    }

    /// The world point (x, y) at a picture position.
    pub fn to_world(&self, col: f32, row: f32) -> (f32, f32) {
        let (u, z) = (self.unit(), self.zoom);
        let (ox, oy) = (self.origin[0], self.origin[1]);
        if self.rotated {
            (ox - 4096.0 / z + col * u, oy + 3072.0 / z - row * u)
        } else {
            (ox + 3072.0 / z - row * u, oy + 4096.0 / z - col * u)
        }
    }

    /// The framing that fits world box `lo..hi` (x, y) best. `rotated`: None picks the way
    /// round that makes the map biggest (the picture's long side along the map's long side).
    /// Zoom is rounded down to 2 decimals and the origin to whole units, as the file keeps them.
    pub fn fit(lo: [f32; 2], hi: [f32; 2], floor: f32, rotated: Option<bool>) -> Frame {
        let ex = (hi[0] - lo[0]).max(64.0) * (1.0 + 2.0 * MARGIN);
        let ey = (hi[1] - lo[1]).max(64.0) * (1.0 + 2.0 * MARGIN);
        let zoom_for = |rot: bool| if rot { (8192.0 / ex).min(6144.0 / ey) } else { (8192.0 / ey).min(6144.0 / ex) };
        let rotated = rotated.unwrap_or_else(|| zoom_for(true) >= zoom_for(false));
        let zoom = ((zoom_for(rotated) * 100.0).floor() / 100.0).clamp(0.05, 20.0);
        let origin = [((lo[0] + hi[0]) / 2.0).round(), ((lo[1] + hi[1]) / 2.0).round(), floor.round()];
        Frame { zoom, origin, rotated, height: floor.round() }
    }
}

/// The overview file for `map` with its picture at `image` ("overviews/de_x.bmp").
pub fn write_txt(map: &str, f: &Frame, image: &str) -> String {
    let num = |v: f32| if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v:.2}") };
    format!(
        "// overview description file for {map}.bsp\r\n// made by Loner-Strike's radar maker\r\n\r\nglobal\r\n{{\r\n\tZOOM\t{:.2}\r\n\tORIGIN\t{}\t{}\t{}\r\n\tROTATED\t{}\r\n}}\r\n\r\nlayer\r\n{{\r\n\tIMAGE\t\"{image}\"\r\n\tHEIGHT\t{}\r\n}}\r\n",
        f.zoom,
        num(f.origin[0]),
        num(f.origin[1]),
        num(f.origin[2]),
        f.rotated as u8,
        num(f.height)
    )
}

/// An overview file as the game reads it: its framing and the first layer's picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub frame: Frame,
    pub image: Option<String>,
}

/// The game's tokenizer (COM_ParseFile): `//` comments, quoted strings, `{ } ( ) ' :` as
/// tokens of their own, else runs up to white space.
fn tokens(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let (mut i, mut out) = (0, Vec::new());
    while i < b.len() {
        let c = b[i];
        if c <= b' ' {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'"' {
            let s = i + 1;
            i = s;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
            out.push(String::from_utf8_lossy(&b[s..i]).to_string());
            i += 1;
        } else if b"{})(':".contains(&c) {
            out.push((c as char).to_string());
            i += 1;
        } else {
            let s = i;
            while i < b.len() && b[i] > b' ' && !b"{})(':\"".contains(&b[i]) {
                i += 1;
            }
            out.push(String::from_utf8_lossy(&b[s..i]).to_string());
        }
    }
    out
}

/// Reads an overview file like the game (unknown keys are an error there too). None when
/// the game would give up on it.
pub fn parse_txt(text: &str) -> Option<Parsed> {
    let t = tokens(text);
    let num = |s: Option<&String>| s.map(|s| atof(s));
    let mut f = Frame { zoom: 1.0, origin: [0.0; 3], rotated: false, height: 0.0 };
    let (mut image, mut layers, mut i) = (None, 0, 0);
    while i < t.len() {
        let block = t[i].to_ascii_lowercase();
        if (block != "global" && block != "layer") || t.get(i + 1).map(String::as_str) != Some("{") {
            return None;
        }
        i += 2;
        while i < t.len() && t[i] != "}" {
            match (block.as_str(), t[i].to_ascii_lowercase().as_str()) {
                ("global", "zoom") => {
                    f.zoom = num(t.get(i + 1))?;
                    i += 2;
                }
                ("global", "origin") => {
                    f.origin = [num(t.get(i + 1))?, num(t.get(i + 2))?, num(t.get(i + 3))?];
                    i += 4;
                }
                ("global", "rotated") => {
                    f.rotated = num(t.get(i + 1))? as i32 != 0;
                    i += 2;
                }
                ("global", "inset") => i += 5,
                ("layer", "image") => {
                    if layers == 0 {
                        image = t.get(i + 1).cloned();
                    }
                    i += 2;
                }
                ("layer", "height") => {
                    if layers == 0 {
                        f.height = num(t.get(i + 1))?;
                    }
                    i += 2;
                }
                _ => return None,
            }
        }
        if block == "layer" {
            layers += 1;
        }
        i += 1;
    }
    Some(Parsed { frame: f, image })
}

/// C's atof: the number at the start, 0 when there's none.
fn atof(s: &str) -> f32 {
    let end = s.char_indices().take_while(|(k, c)| c.is_ascii_digit() || *c == '.' || ((*c == '-' || *c == '+') && *k == 0)).map(|(k, c)| k + c.len_utf8()).last().unwrap_or(0);
    s[..end].parse().unwrap_or(0.0)
}

// ---- picture files ----------------------------------------------------------------------

/// An 8-bit (256-colour) Windows BMP of RGB pixels, as the stock overviews are: rows bottom
/// up, padded to 4 bytes, the palette made by median cut.
pub fn bmp8(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for p in rgb.chunks_exact(3) {
        *counts.entry([p[0], p[1], p[2]]).or_default() += 1;
    }
    // weights grow with the square root of the count, so small marks (a spawn dot, a site
    // letter) keep their own colours instead of melting into the big floors
    let mut colours: Vec<([u8; 3], u32)> = counts.into_iter().map(|(c, n)| (c, (n as f32).sqrt().ceil() as u32)).collect();
    colours.sort();
    let mut palette = goldsrc::wadwrite::quantise(&colours, 256);
    if palette.is_empty() {
        palette.push([0, 0, 0]);
    }
    let mut index: HashMap<[u8; 3], u8> = HashMap::new();
    let mut pick = |c: [u8; 3]| -> u8 {
        *index.entry(c).or_insert_with(|| {
            let d = |p: &[u8; 3]| (0..3).map(|k| (p[k] as i32 - c[k] as i32).pow(2)).sum::<i32>();
            palette.iter().enumerate().min_by_key(|(_, p)| d(p)).map(|(i, _)| i as u8).unwrap_or(0)
        })
    };
    let stride = (w as usize + 3) & !3;
    let data_at = 14 + 40 + 256 * 4;
    let size = data_at + stride * h as usize;
    let mut out = Vec::with_capacity(size);
    out.extend(b"BM");
    out.extend((size as u32).to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((data_at as u32).to_le_bytes());
    out.extend(40u32.to_le_bytes());
    out.extend((w as i32).to_le_bytes());
    out.extend((h as i32).to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(8u16.to_le_bytes());
    out.extend(0u32.to_le_bytes()); // no compression
    out.extend(((stride * h as usize) as u32).to_le_bytes());
    out.extend(2835u32.to_le_bytes());
    out.extend(2835u32.to_le_bytes());
    out.extend(256u32.to_le_bytes());
    out.extend(256u32.to_le_bytes());
    for k in 0..256 {
        let c = palette.get(k).copied().unwrap_or([0, 0, 0]);
        out.extend([c[2], c[1], c[0], 0]);
    }
    let mut row = vec![0u8; stride];
    for y in (0..h as usize).rev() {
        for x in 0..w as usize {
            let p = &rgb[(y * w as usize + x) * 3..][..3];
            row[x] = pick([p[0], p[1], p[2]]);
        }
        out.extend(&row);
    }
    out
}

/// A 24-bit uncompressed TGA, rows bottom up, like the TGA overviews the game ships.
pub fn tga24(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(18 + rgb.len());
    out.extend([0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend((w as u16).to_le_bytes());
    out.extend((h as u16).to_le_bytes());
    out.extend([24, 0]);
    for y in (0..h as usize).rev() {
        for p in rgb[y * w as usize * 3..(y + 1) * w as usize * 3].chunks_exact(3) {
            out.extend([p[2], p[1], p[0]]);
        }
    }
    out
}

// ---- the map, ready to draw -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Floor,
    Water,
}

/// One floor face: its outline (world x, y), plane, texture and light.
struct Poly {
    pts: Vec<[f32; 2]>,
    /// plane: n . p = d
    n: [f32; 3],
    d: f32,
    zlo: f32,
    zhi: f32,
    kind: Kind,
    face: usize,
    /// entity offset (brush models with an origin)
    offset: [f32; 3],
    tex: Option<usize>,
    on_nav: bool,
}

/// A texture's colours, in shrinking levels (full size first), texgamma applied.
struct Tex {
    levels: Vec<(u32, u32, Vec<[u8; 3]>)>,
    average: [u8; 3],
}

/// A zone or a spot to mark.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Mark {
    pub kind: &'static str,
    /// a box (zones) or a point (lo == hi)
    pub lo: [f32; 3],
    pub hi: [f32; 3],
    pub label: Option<String>,
}

pub struct Scene {
    bsp: Bsp,
    polys: Vec<Poly>,
    textures: Vec<Option<Tex>>,
    pub marks: Vec<Mark>,
    pub has_nav: bool,
    /// heights of the floors: lowest, highest
    pub zrange: [f32; 2],
    /// the default height cut without a nav: a bit above the highest spawn
    pub cut_default: f32,
}

const ZONES: [(&str, &str); 6] = [
    ("func_bomb_target", "bomb"),
    ("info_bomb_target", "bomb"),
    ("func_buyzone", "buy"),
    ("func_hostage_rescue", "rescue"),
    ("info_hostage_rescue", "rescue"),
    ("func_vip_safetyzone", "vip"),
];

fn skip_entity(e: &Entity) -> bool {
    let c = e.classname();
    let invisible = e.get("rendermode").is_some_and(|m| m.trim() != "0") && e.get("renderamt").map_or(true, |a| a.trim().parse::<f32>().unwrap_or(0.0) == 0.0);
    c.starts_with("trigger_") || c == "func_ladder" || c == "func_escapezone" || ZONES.iter().any(|z| z.0 == c) || invisible
}

/// The zones and spots worth marking: bomb sites (A, B... in the game's order), buy zones
/// (by team), rescue zones, VIP escape, spawns, hostages.
pub(crate) fn find_marks(bsp: &Bsp, ents: &[Entity]) -> Vec<Mark> {
    let mut out = Vec::new();
    let mut sites = 0u8;
    for e in ents {
        let c = e.classname();
        if let Some((_, kind)) = ZONES.iter().find(|z| z.0 == c) {
            let (lo, hi) = if let Some(m) = e.brush_model().and_then(|m| bsp.models.get(m)) {
                (m.mins, m.maxs)
            } else if let Some(o) = e.origin() {
                // point targets (info_bomb_target): the game's 256-unit reach around them
                ([o[0] - 128.0, o[1] - 128.0, o[2] - 64.0], [o[0] + 128.0, o[1] + 128.0, o[2] + 64.0])
            } else {
                continue;
            };
            let label = match *kind {
                "bomb" => {
                    sites += 1;
                    Some(((b'A' + sites - 1).min(b'Z') as char).to_string())
                }
                "buy" => match e.get("team").map(str::trim) {
                    Some("1") => Some("T".into()),
                    Some("2") => Some("CT".into()),
                    _ => None,
                },
                _ => None,
            };
            out.push(Mark { kind, lo, hi, label });
            continue;
        }
        let kind = match c {
            "info_player_start" => "ct",
            "info_player_deathmatch" => "t",
            "hostage_entity" | "monster_scientist" => "hostage",
            "info_vip_start" => "vipstart",
            _ => continue,
        };
        if let Some(o) = e.origin() {
            out.push(Mark { kind, lo: o, hi: o, label: None });
        }
    }
    out
}

fn texgamma(c: u8) -> u8 {
    (255.0 * (c as f32 / 255.0).powf(2.0 / 2.5)).floor() as u8
}

fn make_tex(t: &wad::Rgba) -> Tex {
    let mut levels = Vec::new();
    let mut cur: Vec<[u8; 3]> = t.pixels.chunks_exact(4).map(|p| [texgamma(p[0]), texgamma(p[1]), texgamma(p[2])]).collect();
    let (mut w, mut h) = (t.width.max(1), t.height.max(1));
    let total: [u64; 3] = cur.iter().fold([0; 3], |a, p| [a[0] + p[0] as u64, a[1] + p[1] as u64, a[2] + p[2] as u64]);
    let n = cur.len().max(1) as u64;
    let average = [(total[0] / n) as u8, (total[1] / n) as u8, (total[2] / n) as u8];
    loop {
        levels.push((w, h, cur.clone()));
        if (w == 1 && h == 1) || levels.len() >= 8 {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = Vec::with_capacity((nw * nh) as usize);
        for y in 0..nh {
            for x in 0..nw {
                let mut s = [0u32; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    let p = cur[(sy * w + sx) as usize];
                    for k in 0..3 {
                        s[k] += p[k] as u32;
                    }
                }
                next.push([(s[0] / 4) as u8, (s[1] / 4) as u8, (s[2] / 4) as u8]);
            }
        }
        cur = next;
        w = nw;
        h = nh;
    }
    Tex { levels, average }
}

/// A grid of nav areas by 256-unit cell, to find the areas near a face quickly.
struct NavGrid<'a> {
    areas: &'a [nav::Area],
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl<'a> NavGrid<'a> {
    fn new(areas: &'a [nav::Area]) -> NavGrid<'a> {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, a) in areas.iter().enumerate() {
            let (x0, x1) = ((a.lo[0].min(a.hi[0]) / 256.0).floor() as i32, (a.lo[0].max(a.hi[0]) / 256.0).floor() as i32);
            let (y0, y1) = ((a.lo[1].min(a.hi[1]) / 256.0).floor() as i32, (a.lo[1].max(a.hi[1]) / 256.0).floor() as i32);
            if (x1 - x0) > 64 || (y1 - y0) > 64 {
                continue;
            }
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    cells.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        NavGrid { areas, cells }
    }

    /// Whether some area overlaps box `lo..hi` (x, y) and stands within `below`..`above` of
    /// height `z(x, y)` there.
    fn stands_on(&self, lo: [f32; 2], hi: [f32; 2], z: impl Fn(f32, f32) -> f32, below: f32, above: f32) -> bool {
        let (x0, x1) = ((lo[0] / 256.0).floor() as i32, (hi[0] / 256.0).floor() as i32);
        let (y0, y1) = ((lo[1] / 256.0).floor() as i32, (hi[1] / 256.0).floor() as i32);
        if (x1 - x0) > 64 || (y1 - y0) > 64 {
            return false;
        }
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                for &i in self.cells.get(&(cx, cy)).into_iter().flatten() {
                    let a = &self.areas[i];
                    let (ax0, ax1) = (a.lo[0].min(a.hi[0]).max(lo[0]), a.lo[0].max(a.hi[0]).min(hi[0]));
                    let (ay0, ay1) = (a.lo[1].min(a.hi[1]).max(lo[1]), a.lo[1].max(a.hi[1]).min(hi[1]));
                    if ax0 > ax1 || ay0 > ay1 {
                        continue;
                    }
                    let (cx, cy) = ((ax0 + ax1) / 2.0, (ay0 + ay1) / 2.0);
                    let dz = a.z_at(cx, cy) - z(cx, cy);
                    if dz >= -below && dz <= above {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Scene {
    /// Everything needed to draw a map, read once (the BSP, its floors, textures, marks).
    pub fn build(bsp: Bsp, wads: &wad::WadSet, nav: Option<&nav::Nav>) -> Scene {
        let ents = entities::parse(&bsp.entities);
        let grid = nav.map(|n| NavGrid::new(&n.areas));
        // which entity owns each brush model; world is model 0
        let mut owner: HashMap<usize, &Entity> = HashMap::new();
        for e in &ents {
            if let Some(m) = e.brush_model().filter(|m| *m > 0) {
                owner.entry(m).or_insert(e);
            }
        }
        let mut polys = Vec::new();
        let mut tex_index: HashMap<usize, usize> = HashMap::new();
        let mut tex_list: Vec<usize> = Vec::new();
        for (mi, m) in bsp.models.iter().enumerate() {
            let ent = owner.get(&mi).copied();
            if mi > 0 && ent.map_or(true, skip_entity) {
                continue;
            }
            let classname = ent.map(|e| e.classname().to_string()).unwrap_or_else(|| "worldspawn".into());
            let offset = if mi > 0 { ent.and_then(|e| e.origin()).unwrap_or([0.0; 3]) } else { [0.0; 3] };
            if m.first_face < 0 || m.num_faces <= 0 {
                continue;
            }
            let (first, n) = (m.first_face as usize, m.num_faces as usize);
            let end = (first + n).min(bsp.faces.len());
            for fi in first..end {
                let f = &bsp.faces[fi];
                let Some((ti, t)) = bsp.face_texture(f) else { continue };
                let flags = mesh::flags_for(&bsp.textures[t].name, &classname);
                if flags & (mesh::FLAG_SKY | mesh::FLAG_TRIGGER) != 0 {
                    continue;
                }
                let water = flags & mesh::FLAG_WATER != 0 || classname == "func_water";
                // special faces (sky, tool textures) other than water have no picture
                if ti.flags & 1 != 0 && !water {
                    continue;
                }
                let (Some(nrm), Some(pts)) = (bsp.face_normal(f), bsp.face_points(f)) else { continue };
                if nrm[2] < UP {
                    continue;
                }
                let pts: Vec<[f32; 3]> = pts.iter().map(|p| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]]).collect();
                let d = nrm[0] * pts[0][0] + nrm[1] * pts[0][1] + nrm[2] * pts[0][2];
                let zlo = pts.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
                let zhi = pts.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
                let xy: Vec<[f32; 2]> = pts.iter().map(|p| [p[0], p[1]]).collect();
                let on_nav = match &grid {
                    None => true,
                    Some(g) => {
                        let lo = [xy.iter().map(|p| p[0]).fold(f32::MAX, f32::min), xy.iter().map(|p| p[1]).fold(f32::MAX, f32::min)];
                        let hi = [xy.iter().map(|p| p[0]).fold(f32::MIN, f32::max), xy.iter().map(|p| p[1]).fold(f32::MIN, f32::max)];
                        let z = |x: f32, y: f32| (d - nrm[0] * x - nrm[1] * y) / nrm[2];
                        // water: a nav area under its surface (the bottom of the pool) counts
                        if water {
                            g.stands_on(lo, hi, z, 512.0, NAV_REACH)
                        } else {
                            g.stands_on(lo, hi, z, NAV_REACH, NAV_REACH)
                        }
                    }
                };
                let tex = Some(*tex_index.entry(t).or_insert_with(|| {
                    tex_list.push(t);
                    tex_list.len() - 1
                }));
                polys.push(Poly { pts: xy, n: nrm, d, zlo, zhi, kind: if water { Kind::Water } else { Kind::Floor }, face: fi, offset, tex, on_nav });
            }
        }
        let textures = tex_list
            .iter()
            .map(|&t| {
                let mt = &bsp.textures[t];
                let rec = match &mt.embedded {
                    Some(r) => Some(r.clone()),
                    None => wads.read(&mt.name).ok().flatten(),
                };
                rec.and_then(|r| wad::decode_miptex(&r, &mt.name).ok()).map(|rgba| make_tex(&rgba))
            })
            .collect();
        let marks = find_marks(&bsp, &ents);
        let used: Vec<&Poly> = polys.iter().filter(|p| p.on_nav).collect();
        let zlo = used.iter().map(|p| p.zlo).fold(f32::MAX, f32::min);
        let zhi = used.iter().map(|p| p.zhi).fold(f32::MIN, f32::max);
        let zrange = if zlo <= zhi { [zlo, zhi] } else { [0.0, 0.0] };
        let spawn_top = marks.iter().filter(|m| m.kind == "ct" || m.kind == "t").map(|m| m.lo[2]).fold(f32::MIN, f32::max);
        let cut_default = if spawn_top > f32::MIN { (spawn_top + 320.0).min(zrange[1] + 1.0).round() } else { zrange[1] + 1.0 };
        Scene { bsp, polys, textures, marks, has_nav: grid.is_some(), zrange, cut_default }
    }

    /// The floors drawn with these options.
    fn kept<'s>(&'s self, o: &'s Options) -> impl Iterator<Item = &'s Poly> + 's {
        let use_nav = self.has_nav && o.roofs == Roofs::Nav;
        self.polys.iter().filter(move |p| if use_nav { p.on_nav } else { p.zlo <= o.cut.unwrap_or(self.cut_default) })
    }

    /// The framing for these options: the kept floors and the marks, fitted.
    pub fn frame(&self, o: &Options) -> Frame {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        let mut floor = f32::MAX;
        for p in self.kept(o) {
            for q in &p.pts {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            floor = floor.min(p.zlo);
        }
        for m in self.marks.iter().filter(|m| m.lo == m.hi) {
            lo = [lo[0].min(m.lo[0]), lo[1].min(m.lo[1])];
            hi = [hi[0].max(m.lo[0]), hi[1].max(m.lo[1])];
        }
        if lo[0] > hi[0] {
            return Frame { zoom: 1.0, origin: [0.0; 3], rotated: false, height: 0.0 };
        }
        let rotated = match o.orient {
            Orient::Auto => None,
            Orient::NorthUp => Some(true),
            Orient::Turned => Some(false),
        };
        Frame::fit(lo, hi, if floor == f32::MAX { 0.0 } else { floor }, rotated)
    }
}

// ---- drawing ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// textures and light, like the game draws the map (the stock overviews' look)
    #[default]
    Game,
    /// height colours, lit: a clean radar
    Clean,
    /// height colours only
    Simple,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Roofs {
    /// only floors the bots' nav stands on (falls back to the height cut without a nav)
    #[default]
    Nav,
    /// everything below the cut height
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orient {
    #[default]
    Auto,
    /// ROTATED 1: x across, north up
    #[serde(rename = "north")]
    NorthUp,
    /// ROTATED 0: y across
    Turned,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Background {
    /// pure green, as every stock overview has around the map
    #[default]
    Green,
    Black,
}

fn yes() -> bool {
    true
}

/// What to draw (the page's choices, as query or JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct Options {
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub roofs: Roofs,
    /// the cut height (Roofs::Cut, or no nav); None: a bit above the highest spawn
    #[serde(default)]
    pub cut: Option<f32>,
    #[serde(default)]
    pub orient: Orient,
    #[serde(default = "yes")]
    pub outline: bool,
    /// comma-separated: bomb, buy, rescue, vip, spawns, hostages
    #[serde(default)]
    pub marks: String,
    #[serde(default)]
    pub background: Background,
}

impl Default for Options {
    fn default() -> Options {
        Options { style: Style::Game, roofs: Roofs::Nav, cut: None, orient: Orient::Auto, outline: true, marks: String::new(), background: Background::Green }
    }
}

impl Options {
    fn mark(&self, k: &str) -> bool {
        self.marks.split(',').any(|m| m.trim() == k)
    }
}

/// The game's light curve (TOOLS-RENDER.md, gamma 2.5, brightness 1, lightgamma 2.5, no
/// overbright): a luxel sum (style-scaled) to the 0..255 the texture is multiplied by.
fn light_table() -> &'static [u8; 1024] {
    static T: OnceLock<[u8; 1024]> = OnceLock::new();
    T.get_or_init(|| {
        let (gamma, brightness, lightgamma) = (2.5f32, 1.0f32, 2.5f32);
        let g3 = 0.125 - 0.075 * brightness * brightness;
        std::array::from_fn(|i| {
            let mut f = (i as f32 / 1023.0).powf(lightgamma);
            f = if f <= g3 { f / g3 * 0.125 } else { (f - g3) / (1.0 - g3) * 0.875 + 0.125 };
            ((1023.0 * f.powf(1.0 / gamma)).floor() as u32 >> 2).min(255) as u8
        })
    })
}

/// A floor's light at world point p (0..255 per channel, through the game's curve), or None
/// when the face has no lightmap (drawn full bright, like the game).
fn light_at(bsp: &Bsp, face: usize, p: [f32; 3]) -> Option<[f32; 3]> {
    let f = bsp.faces.get(face)?;
    if f.styles[0] == 255 || f.light_ofs < 0 {
        return None;
    }
    let (ti, _) = bsp.face_texture(f)?;
    let pts = bsp.face_points(f)?;
    let st: Vec<(f32, f32)> = pts.iter().map(|q| ti.st(*q)).collect();
    let ext = lightmap::extents(&st)?;
    let maps = f.styles.iter().take_while(|&&s| s != 255).count();
    let one = (ext.w * ext.h * 3) as usize;
    let at = f.light_ofs as usize;
    let maps = (1..=maps).rev().find(|&n| at.checked_add(n * one).is_some_and(|e| e <= bsp.lighting.len()))?;
    let (s, t) = ti.st(p);
    let fx = (s / lightmap::LUXEL - ext.lo[0]).clamp(0.0, (ext.w - 1) as f32);
    let fy = (t / lightmap::LUXEL - ext.lo[1]).clamp(0.0, (ext.h - 1) as f32);
    let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(ext.w - 1), (y0 + 1).min(ext.h - 1));
    let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
    let mut sum = [0f32; 3];
    for m in 0..maps {
        let base = at + m * one;
        let px = |x: u32, y: u32, c: usize| bsp.lighting[base + ((y * ext.w + x) * 3) as usize + c] as f32;
        for c in 0..3 {
            let top = px(x0, y0, c) * (1.0 - ax) + px(x1, y0, c) * ax;
            let bot = px(x0, y1, c) * (1.0 - ax) + px(x1, y1, c) * ax;
            sum[c] += (top * (1.0 - ay) + bot * ay) * lightmap::STYLE_NORMAL as f32;
        }
    }
    let table = light_table();
    Some(sum.map(|acc| {
        let t = ((acc * 338.0) as u32 >> 14).min(1023);
        table[t as usize] as f32
    }))
}

/// Height colour for the clean styles: deep teal (low) to pale sand (high).
fn height_colour(t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    let lo = [62.0, 92.0, 104.0];
    let mid = [128.0, 140.0, 118.0];
    let hi = [222.0, 212.0, 170.0];
    let (a, b, k) = if t < 0.5 { (lo, mid, t * 2.0) } else { (mid, hi, (t - 0.5) * 2.0) };
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

/// Outside the map: the stock overviews' green, or black.
fn void_colour(o: &Options) -> [u8; 3] {
    if o.background == Background::Black {
        [0, 0, 0]
    } else {
        [0, 255, 0]
    }
}
const EDGE: [f32; 3] = [18.0, 18.0, 20.0];

/// A tiny 5x7 font for the bomb site letters and team words.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        _ => return None,
    })
}

struct Canvas {
    w: usize,
    h: usize,
    rgb: Vec<f32>,
}

impl Canvas {
    fn blend(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 3;
        for k in 0..3 {
            self.rgb[i + k] = self.rgb[i + k] * (1.0 - a) + c[k] * a;
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: [f32; 3], ring: [f32; 3]) {
        let ri = r.ceil() as i32 + 1;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt();
                let (x, y) = (cx.round() as i32 + dx, cy.round() as i32 + dy);
                if d <= r - 1.0 {
                    self.blend(x, y, c, 1.0);
                } else if d <= r + 0.5 {
                    self.blend(x, y, ring, 1.0);
                }
            }
        }
    }

    fn text(&mut self, cx: f32, cy: f32, s: &str, scale: i32, c: [f32; 3]) {
        let n = s.chars().count() as i32;
        let (w, h) = (n * 6 * scale - scale, 7 * scale);
        let (x0, y0) = (cx.round() as i32 - w / 2, cy.round() as i32 - h / 2);
        for (k, ch) in s.chars().enumerate() {
            let Some(g) = glyph(ch) else { continue };
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let (x, y) = (x0 + (k as i32 * 6 + col) * scale + sx, y0 + row as i32 * scale + sy);
                            // a dark outline under each letter pixel, then the letter
                            for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                                self.blend(x + ox, y + oy, [10.0, 10.0, 10.0], 0.6);
                            }
                        }
                    }
                }
            }
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.blend(x0 + (k as i32 * 6 + col) * scale + sx, y0 + row as i32 * scale + sy, c, 1.0);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Calls `put(pixel index, z)` for every pixel centre inside a floor (scanline fill).
fn raster(f: &Frame, p: &Poly, mut put: impl FnMut(usize, f32)) {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let px: Vec<(f32, f32)> = p.pts.iter().map(|q| f.to_pixel(q[0], q[1])).collect();
    let ymin = px.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
    let ymax = px.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil().min(h as f32 - 1.0) as i32;
    let mut xs: Vec<f32> = Vec::with_capacity(8);
    for row in ymin..=ymax {
        let yc = row as f32 + 0.5;
        xs.clear();
        for k in 0..px.len() {
            let (a, b) = (px[k], px[(k + 1) % px.len()]);
            if (a.1 <= yc && yc < b.1) || (b.1 <= yc && yc < a.1) {
                xs.push(a.0 + (yc - a.1) * (b.0 - a.0) / (b.1 - a.1));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            let c0 = (pair[0] - 0.5).ceil().max(0.0) as i32;
            let c1 = (pair[1] - 0.5).floor().min(w as f32 - 1.0) as i32;
            for col in c0..=c1 {
                let (x, y) = f.to_world(col as f32 + 0.5, yc);
                put(row as usize * w + col as usize, (p.d - p.n[0] * x - p.n[1] * y) / p.n[2]);
            }
        }
    }
}

/// Pixels within `r` pixels of a drawn one (chamfer distance, two passes).
fn near_mask(owner: &[u32], w: usize, h: usize, r: i32) -> Vec<bool> {
    let big = i32::MAX / 4;
    let mut d: Vec<i32> = owner.iter().map(|&o| if o == u32::MAX { big } else { 0 }).collect();
    // 3-4 chamfer: 3 per straight step, 4 per diagonal
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 {
                v = v.min(d[i - 1] + 3);
            }
            if y > 0 {
                v = v.min(d[i - w] + 3);
                if x > 0 {
                    v = v.min(d[i - w - 1] + 4);
                }
                if x + 1 < w {
                    v = v.min(d[i - w + 1] + 4);
                }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w {
                v = v.min(d[i + 1] + 3);
            }
            if y + 1 < h {
                v = v.min(d[i + w] + 3);
                if x + 1 < w {
                    v = v.min(d[i + w + 1] + 4);
                }
                if x > 0 {
                    v = v.min(d[i + w - 1] + 4);
                }
            }
            d[i] = v;
        }
    }
    d.into_iter().map(|v| v <= r * 3).collect()
}

/// Draws the map into a WIDTH x HEIGHT RGB picture with framing `f`.
pub fn render(s: &Scene, f: &Frame, o: &Options) -> Vec<u8> {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let mut zbuf = vec![f32::MIN; w * h];
    let mut owner = vec![u32::MAX; w * h];
    let cut = (!(s.has_nav && o.roofs == Roofs::Nav)).then(|| o.cut.unwrap_or(s.cut_default));
    let kept: Vec<(usize, &Poly)> = s.polys.iter().enumerate().filter(|(_, p)| if let Some(c) = cut { p.zlo <= c } else { p.on_nav }).collect();
    for &(pi, p) in &kept {
        raster(f, p, |i, z| {
            if cut.is_some_and(|c| z > c) {
                return;
            }
            if z > zbuf[i] {
                zbuf[i] = z;
                owner[i] = pi as u32;
            }
        });
    }
    if cut.is_none() {
        // with the nav: also what's next to the floors players walk (crate tops, rock slopes,
        // the tops of low walls), but never anything over a walked floor (a roof)
        let near = near_mask(&owner, w, h, (NEAR / f.unit()).round() as i32);
        let walked = zbuf.clone();
        let top = kept.iter().map(|(_, p)| p.zhi).fold(f32::MIN, f32::max) + 512.0;
        for (pi, p) in s.polys.iter().enumerate().filter(|(_, p)| !p.on_nav && p.zlo <= top) {
            raster(f, p, |i, z| {
                if !near[i] || (walked[i] > f32::MIN && z > walked[i] + NAV_REACH) {
                    return;
                }
                if z > zbuf[i] {
                    zbuf[i] = z;
                    owner[i] = pi as u32;
                }
            });
        }
    }

    // shading
    let (zlo, zhi) = {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for &(_, p) in &kept {
            lo = lo.min(p.zlo);
            hi = hi.max(p.zhi);
        }
        if lo > hi {
            (0.0, 1.0)
        } else {
            (lo, hi.max(lo + 1.0))
        }
    };
    let unit = f.unit();
    let mut cv = Canvas { w, h, rgb: vec![0.0; w * h * 3] };
    for i in 0..w * h {
        let pi = owner[i];
        if pi == u32::MAX {
            continue;
        }
        let p = &s.polys[pi as usize];
        let (col, row) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
        let (x, y) = f.to_world(col, row);
        let z = zbuf[i];
        // the face's own coordinates (brush entities with an origin are offset)
        let local = [x - p.offset[0], y - p.offset[1], z - p.offset[2]];
        let light = if p.kind == Kind::Water { None } else { light_at(&s.bsp, p.face, local) };
        let ht = (z - zlo) / (zhi - zlo);
        let c: [f32; 3] = match o.style {
            Style::Game => {
                let tex = p.tex.and_then(|t| s.textures.get(t)).and_then(|t| t.as_ref());
                let base = match tex {
                    Some(t) => sample(&s.bsp, p.face, t, local, unit),
                    None => [128.0, 128.0, 128.0],
                };
                let l = light.unwrap_or([255.0; 3]);
                // the game multiplies by light/255; overviews read better a step brighter
                let k = 1.0 / 255.0;
                [(base[0] * l[0] * k).min(255.0), (base[1] * l[1] * k).min(255.0), (base[2] * l[2] * k).min(255.0)]
            }
            Style::Clean => {
                let hc = height_colour(ht);
                let l = light.map(|l| (l[0] + l[1] + l[2]) / 3.0).unwrap_or(200.0);
                let k = 0.55 + 0.6 * (l / 255.0);
                [(hc[0] * k).min(255.0), (hc[1] * k).min(255.0), (hc[2] * k).min(255.0)]
            }
            Style::Simple => height_colour(ht),
        };
        let c = if p.kind == Kind::Water { [c[0] * 0.35 + 30.0, c[1] * 0.45 + 70.0, c[2] * 0.45 + 120.0] } else { c };
        cv.rgb[i * 3..i * 3 + 3].copy_from_slice(&c);
    }

    // walls: the top of every drop
    if o.outline {
        let mut edge = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if owner[i] == u32::MAX {
                    continue;
                }
                let z = zbuf[i];
                let drop = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        return true;
                    }
                    let j = ny as usize * w + nx as usize;
                    owner[j] == u32::MAX || zbuf[j] < z - EDGE_STEP
                });
                edge[i] = drop;
            }
        }
        for i in 0..w * h {
            if edge[i] {
                cv.rgb[i * 3..i * 3 + 3].copy_from_slice(&EDGE);
            }
        }
    }

    draw_marks(&mut cv, s, f, o);

    let mut out = vec![0u8; w * h * 3];
    for i in 0..w * h {
        if owner[i] == u32::MAX && cv.rgb[i * 3..i * 3 + 3] == [0.0, 0.0, 0.0] {
            out[i * 3..i * 3 + 3].copy_from_slice(&void_colour(o));
            continue;
        }
        for k in 0..3 {
            out[i * 3 + k] = cv.rgb[i * 3 + k].round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// A texture's colour at world point p of a face, from the mip level nearest one texel a pixel.
fn sample(bsp: &Bsp, face: usize, t: &Tex, p: [f32; 3], unit: f32) -> [f32; 3] {
    let Some((ti, _)) = bsp.faces.get(face).and_then(|f| bsp.face_texture(f)) else { return t.average.map(|c| c as f32) };
    let (s, tt) = ti.st(p);
    let scale = (ti.s[0] * ti.s[0] + ti.s[1] * ti.s[1] + ti.s[2] * ti.s[2]).sqrt().max(1e-3);
    let per_px = unit * scale;
    let level = (per_px.max(1.0).log2().floor() as usize).min(t.levels.len() - 1);
    let (w, h, px) = &t.levels[level];
    let k = (1u32 << level) as f32;
    let (base_w, base_h) = (t.levels[0].0 as f32, t.levels[0].1 as f32);
    let u = (s.rem_euclid(base_w) / k).floor() as u32 % w;
    let v = (tt.rem_euclid(base_h) / k).floor() as u32 % h;
    px[(v * w + u) as usize].map(|c| c as f32)
}

fn draw_marks(cv: &mut Canvas, s: &Scene, f: &Frame, o: &Options) {
    let zone = |kind: &str, label: &Option<String>| -> Option<([f32; 3], &'static str)> {
        Some(match kind {
            "bomb" if o.mark("bomb") => ([220.0, 40.0, 40.0], "bomb"),
            "buy" if o.mark("buy") => (if label.as_deref() == Some("T") { [230.0, 150.0, 40.0] } else { [60.0, 130.0, 240.0] }, "buy"),
            "rescue" if o.mark("rescue") => ([60.0, 200.0, 90.0], "rescue"),
            "vip" if o.mark("vip") => ([60.0, 200.0, 90.0], "vip"),
            _ => return None,
        })
    };
    for m in s.marks.iter().filter(|m| m.lo != m.hi) {
        let Some((c, kind)) = zone(m.kind, &m.label) else { continue };
        let (a, b) = (f.to_pixel(m.lo[0], m.lo[1]), f.to_pixel(m.hi[0], m.hi[1]));
        let (x0, x1) = (a.0.min(b.0).round() as i32, a.0.max(b.0).round() as i32);
        let (y0, y1) = (a.1.min(b.1).round() as i32, a.1.max(b.1).round() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let border = x - x0 < 2 || x1 - x < 2 || y - y0 < 2 || y1 - y < 2;
                cv.blend(x, y, c, if border { 0.9 } else { 0.28 });
            }
        }
        if kind == "bomb" {
            if let Some(l) = &m.label {
                let size = ((x1 - x0).min(y1 - y0) / 12).clamp(2, 6);
                cv.text((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0, l, size, [255.0, 235.0, 235.0]);
            }
        }
    }
    let r = (24.0 / f.unit()).clamp(2.5, 6.0);
    for m in s.marks.iter().filter(|m| m.lo == m.hi) {
        let (c, show) = match m.kind {
            "ct" => ([70.0, 140.0, 255.0], o.mark("spawns")),
            "t" => ([240.0, 120.0, 40.0], o.mark("spawns")),
            "vipstart" => ([255.0, 255.0, 255.0], o.mark("spawns")),
            "hostage" => ([250.0, 220.0, 60.0], o.mark("hostages")),
            _ => continue,
        };
        if show {
            let (x, y) = f.to_pixel(m.lo[0], m.lo[1]);
            cv.disc(x, y, r, c, [12.0, 12.0, 12.0]);
        }
    }
}

// ---- reading a map (cached a few minutes: dragging a choice redraws, it doesn't reread) ---

struct Memo {
    bsp: PathBuf,
    stamp: cache::Stamp,
    nav_stamp: Option<cache::Stamp>,
    scene: Arc<Scene>,
    at: Instant,
}

fn scene_of(m: &MapFile) -> Result<Arc<Scene>, Fail> {
    static MEMO: OnceLock<Mutex<Vec<Memo>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let stamp = cache::Stamp::of(&m.bsp).map_err(|_| Fail::no_map())?;
    let nav_path = m.sibling("nav");
    let nav_stamp = nav_path.as_deref().and_then(|p| cache::Stamp::of(p).ok());
    {
        let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|s| s.at.elapsed() < Duration::from_secs(300));
        if let Some(s) = list.iter_mut().find(|s| s.bsp == m.bsp && s.stamp == stamp && s.nav_stamp == nav_stamp) {
            s.at = Instant::now();
            return Ok(s.scene.clone());
        }
    }
    let bsp = Bsp::open(&m.bsp, true)?;
    let ents = entities::parse(&bsp.entities);
    let wads = m.wads(&Finder::default(), &entities::wad_list(&ents));
    // a nav for another version of the map would leave out the wrong floors
    let nav = nav_path.and_then(|p| crate::caps::read_file(&p, "nav", nav::MAX_NAV).ok()).and_then(|b| nav::Nav::parse(&b).ok()).filter(|n| n.bsp_size == 0 || n.bsp_size as u64 == stamp.size); // [Caps]
    let scene = Arc::new(Scene::build(bsp, &wads, nav.as_ref()));
    let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
    list.push(Memo { bsp: m.bsp.clone(), stamp, nav_stamp, scene: scene.clone(), at: Instant::now() });
    if list.len() > 2 {
        list.remove(0);
    }
    Ok(scene)
}

// ---- what's installed -------------------------------------------------------------------

/// The overview folder the game reads this map's files from first (the map's own game).
fn overview_dir(m: &MapFile) -> PathBuf {
    m.hl.join(&m.game).join("overviews")
}

/// The overview file the game would find for this map (its folders in order), if any.
fn installed(m: &MapFile) -> Option<(PathBuf, String)> {
    let f = Finder::default();
    super::doctor::game_path(&m.hl, &m.game).into_iter().find_map(|d| {
        let p = f.find(&d, &format!("overviews/{}.txt", m.name))?;
        let text = crate::sys::read_latin1(&p).ok()?;
        Some((p, text))
    })
}

/// The picture an overview file names, found where the game looks.
fn installed_image(m: &MapFile, image: &str) -> Option<PathBuf> {
    let rel = crate::mapcopy::clean_rel(image)?;
    let f = Finder::default();
    super::doctor::game_path(&m.hl, &m.game).into_iter().find_map(|d| f.find(&d, &rel))
}

fn is_ours(text: &str) -> bool {
    text.contains("Loner-Strike's radar maker")
}

fn info(m: &MapFile, game: &crate::game::Game) -> Result<Value, Fail> {
    let s = scene_of(m)?;
    let o = Options::default();
    let fr = s.frame(&o);
    let current = installed(m).map(|(p, text)| {
        let parsed = parse_txt(&text);
        let image = parsed.as_ref().and_then(|p| p.image.clone());
        let img_path = image.as_deref().and_then(|i| installed_image(m, i));
        json!({
            "file": p.strip_prefix(&m.hl).unwrap_or(&p).display().to_string().replace('\\', "/"),
            "readable": parsed.is_some(),
            "image": image,
            "imageFound": img_path.is_some(),
            "ours": is_ours(&text),
            "zoom": parsed.as_ref().map(|p| p.frame.zoom),
            "rotated": parsed.as_ref().map(|p| p.frame.rotated),
        })
    });
    let dir = overview_dir(m);
    let backups = ["txt", "bmp", "tga"].iter().any(|e| dir.join(format!("{}.{e}.bak", m.name)).is_file());
    let counts = |k: &str| s.marks.iter().filter(|x| x.kind == k).count();
    Ok(json!({
        "name": m.name,
        "game": m.game,
        "current": current,
        "hasNav": s.has_nav,
        "heights": {"low": s.zrange[0].floor(), "high": s.zrange[1].ceil(), "cut": s.cut_default},
        "frame": {"zoom": fr.zoom, "origin": fr.origin, "rotated": fr.rotated},
        "marks": {"bomb": counts("bomb"), "buy": counts("buy"), "rescue": counts("rescue"), "vip": counts("vip"),
                   "spawns": counts("ct") + counts("t"), "hostages": counts("hostage")},
        "stock": super::doctor::stock_map(&m.name, &m.game),
        "backups": backups,
        "inUse": super::doctor::map_in_use(game, &m.name),
    }))
}

fn png_answer(r: Result<Vec<u8>, Fail>) -> Response {
    match r {
        Ok(b) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-store")], b).into_response(),
        Err(f) => (if f.status == StatusCode::OK { StatusCode::UNPROCESSABLE_ENTITY } else { f.status }, Json(json!({"error": f.msg}))).into_response(),
    }
}

fn to_png(rgb: &[u8]) -> Result<Vec<u8>, Fail> {
    goldsrc::png(WIDTH, HEIGHT, rgb, false).map_err(Fail::from)
}

/// The installed picture as RGB at 1024x768 (other sizes are scaled to it).
fn current_rgb(m: &MapFile) -> Result<Vec<u8>, Fail> {
    let none = || Fail::new(StatusCode::NOT_FOUND, "This map has no overview yet.");
    let (_, text) = installed(m).ok_or_else(none)?;
    let image = parse_txt(&text).and_then(|p| p.image).ok_or_else(|| Fail::new(StatusCode::UNPROCESSABLE_ENTITY, "The overview file can't be read by the game."))?;
    let path = installed_image(m, &image).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "The overview's picture isn't on this computer."))?;
    let bytes = goldsrc::read_capped(&path, "picture", 32 << 20)?;
    // [Caps] by pixels too (the game's overviews are 1024x768)
    let fmt = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tga")).then_some(image::ImageFormat::Tga);
    let img = crate::caps::decode_picture(&bytes, fmt, crate::caps::MAX_PICTURE_SIDE).map_err(|e| Fail::new(StatusCode::UNPROCESSABLE_ENTITY, if e.contains("too big") { e.replacen("That picture", "The overview's picture", 1) } else { "The overview's picture can't be read.".to_string() }))?;
    let img = if img.width() != WIDTH || img.height() != HEIGHT { img.resize_exact(WIDTH, HEIGHT, image::imageops::FilterType::Triangle) } else { img };
    Ok(img.to_rgb8().into_raw())
}

// ---- installing -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Bmp,
    Tga,
}

#[derive(Deserialize)]
struct InstallReq {
    #[serde(flatten)]
    opts: Options,
    #[serde(default)]
    format: Format,
    #[serde(default = "yes")]
    res: bool,
}

/// Writes the overview file and picture into the map's game folder; for a map that isn't
/// one of the game's own, its .res too (the map doctor's writer, which lists the overview
/// once it's there).
fn install(m: &MapFile, req: &InstallReq) -> Result<Value, String> {
    let s = scene_of(m).map_err(|f| f.msg)?;
    let fr = s.frame(&req.opts);
    let rgb = render(&s, &fr, &req.opts);
    let ext = if req.format == Format::Tga { "tga" } else { "bmp" };
    let image_rel = format!("overviews/{}.{ext}", m.name);
    let dir = overview_dir(m);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the overviews folder ({e})."))?;
    let bytes = if req.format == Format::Tga { tga24(WIDTH, HEIGHT, &rgb) } else { bmp8(WIDTH, HEIGHT, &rgb) };
    let save = super::doctor::save_file;
    let mut backed = save(&dir.join(format!("{}.{ext}", m.name)), &bytes).map_err(|e| format!("Couldn't save the picture ({e})."))?;
    backed |= save(&dir.join(format!("{}.txt", m.name)), write_txt(&m.name, &fr, &image_rel).as_bytes()).map_err(|e| format!("Couldn't save the overview file ({e})."))?;
    let mut msg = format!("Installed. Spectators see it in overview mode the next time {} loads.", m.name);
    if backed {
        msg.push_str(" The old files are kept next to them as .bak.");
    }
    let stock = super::doctor::stock_map(&m.name, &m.game);
    if req.res && !stock {
        match super::doctor::write_res(m) {
            Ok(_) => msg.push_str(&format!(" {}.res now lists it, so players joining your server get it too.", m.name)),
            Err(e) => msg.push_str(&format!(" The .res couldn't be updated: {e}")),
        }
    }
    Ok(json!({"ok": true, "message": msg, "frame": {"zoom": fr.zoom, "origin": fr.origin, "rotated": fr.rotated}}))
}

/// Puts the `.bak` files back (the overview from before our first install).
fn restore(m: &MapFile) -> Result<Value, String> {
    let dir = overview_dir(m);
    let mut put_back = 0;
    for ext in ["txt", "bmp", "tga"] {
        let file = dir.join(format!("{}.{ext}", m.name));
        let bak = dir.join(format!("{}.{ext}.bak", m.name));
        if bak.is_file() {
            crate::sys::rename_over(&bak, &file).map_err(|e| format!("Couldn't put {}.{ext} back ({e}).", m.name))?;
            put_back += 1;
        }
    }
    if put_back == 0 {
        return Err("There's no older overview to put back.".into());
    }
    Ok(json!({"ok": true, "message": "Put the old overview back."}))
}

// ---- routes -----------------------------------------------------------------------------

async fn api_info(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || info(&locate(&game, &name)?, &game)).await,
    };
    json_answer(r)
}

/// Bad picture settings, in words (QA4-T2 L5: not serde's "unknown variant").
const BAD_SETTINGS: &str = "Those overview settings aren't ones the Radar maker knows.";

async fn api_preview(State(st): State<Shared>, UrlPath(name): UrlPath<String>, q: Result<Query<Options>, axum::extract::rejection::QueryRejection>) -> Response {
    let Ok(Query(o)) = q else { return png_answer(Err(Fail::new(StatusCode::BAD_REQUEST, BAD_SETTINGS))) };
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let s = scene_of(&locate(&game, &name)?)?;
                to_png(&render(&s, &s.frame(&o), &o))
            })
            .await
        }
    };
    png_answer(r)
}

async fn api_current(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || to_png(&current_rgb(&locate(&game, &name)?)?)).await,
    };
    png_answer(r)
}

async fn write_route(st: Shared, h: HeaderMap, name: String, act: impl FnOnce(&MapFile) -> Result<Value, String> + Send + 'static) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || act(&locate(&game, &name)?).map_err(|e| Fail::new(StatusCode::OK, e))).await,
    };
    json_answer(r)
}

async fn api_install(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>, req: Result<Json<InstallReq>, axum::extract::rejection::JsonRejection>) -> Response {
    let Ok(Json(req)) = req else { return (StatusCode::BAD_REQUEST, Json(json!({"error": BAD_SETTINGS}))).into_response() };
    write_route(st, h, name, move |m| install(m, &req)).await
}

async fn api_restore(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>) -> Response {
    write_route(st, h, name, restore).await
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/overview/:name", get(api_info))
        .route("/api/tools/overview/:name/preview.png", get(api_preview))
        .route("/api/tools/overview/:name/current.png", get(api_current))
        .route("/api/tools/overview/:name/install", post(api_install))
        .route("/api/tools/overview/:name/restore", post(api_restore))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_file_round_trips_and_reads_stock_files() {
        let f = Frame { zoom: 1.5, origin: [-223.0, 1097.0, -192.0], rotated: false, height: -192.0 };
        let text = write_txt("de_x", &f, "overviews/de_x.bmp");
        assert!(text.contains("ZOOM\t1.50") && text.contains("ORIGIN\t-223\t1097\t-192") && text.contains("ROTATED\t0") && text.contains("IMAGE\t\"overviews/de_x.bmp\"") && text.contains("HEIGHT\t-192"), "{text}");
        let p = parse_txt(&text).unwrap();
        assert_eq!(p.frame, f);
        assert_eq!(p.image.as_deref(), Some("overviews/de_x.bmp"));
        // a stock file (spaces, tabs, a trailing space after "global", comments)
        let stock = "// overview description file for de_dust2.bsp\r\n\r\nglobal \r\n{\r\n\tZOOM\t1.50\r\n\tORIGIN\t-223 \t1097\t-192\r\n\tROTATED\t0\r\n}\r\n\r\nlayer \r\n{\r\n\tIMAGE\t\"overviews/de_dust2.bmp\"\r\n\tHEIGHT\t-192\r\n}\r\n";
        let p = parse_txt(stock).unwrap();
        assert_eq!(p.frame.zoom, 1.5);
        assert_eq!(p.frame.origin, [-223.0, 1097.0, -192.0]);
        assert_eq!(p.image.as_deref(), Some("overviews/de_dust2.bmp"));
        // an unquoted image and two layers: the first one counts
        let p = parse_txt("global { ROTATED 1 } layer { IMAGE overviews/a.tga HEIGHT 10 } layer { IMAGE \"b.bmp\" HEIGHT 99 }").unwrap();
        assert!(p.frame.rotated);
        assert_eq!((p.image.as_deref(), p.frame.height), (Some("overviews/a.tga"), 10.0));
        // what the game rejects
        assert!(parse_txt("global { SCALE 2 }").is_none());
        assert!(parse_txt("global ZOOM 2").is_none());
        assert_eq!(atof("1.25abc"), 1.25);
        assert_eq!(atof("-3"), -3.0);
        assert_eq!(atof("x"), 0.0);
    }

    #[test]
    fn framing_follows_the_game() {
        // ROTATED 0 (de_dust2's stock file): the top left pixel is +X, +Y; columns go -Y
        let f = Frame { zoom: 1.5, origin: [-223.0, 1097.0, 0.0], rotated: false, height: 0.0 };
        let (x, y) = f.to_world(0.0, 0.0);
        assert!((x - (-223.0 + 2048.0)).abs() < 0.01 && (y - (1097.0 + 4096.0 / 1.5)).abs() < 0.01, "{x} {y}");
        let (x, y) = f.to_world(512.0, 384.0);
        assert!((x + 223.0).abs() < 0.01 && (y - 1097.0).abs() < 0.01);
        let (c, r) = f.to_pixel(100.0, -500.0);
        let (x, y) = f.to_world(c, r);
        assert!((x - 100.0).abs() < 0.01 && (y + 500.0).abs() < 0.01);
        // ROTATED 1: north up, x across
        let f = Frame { zoom: 1.0, origin: [0.0, 0.0, 0.0], rotated: true, height: 0.0 };
        assert_eq!(f.to_world(0.0, 0.0), (-4096.0, 3072.0));
        assert_eq!(f.to_pixel(4096.0, -3072.0), (1024.0, 768.0));
        assert_eq!(f.unit(), 8.0);
        // fitting: a map long in x goes north up; one long in y turns; both fit inside
        let f = Frame::fit([-3000.0, -1000.0], [3000.0, 1000.0], -64.0, None);
        assert!(f.rotated);
        assert!(f.zoom <= 8192.0 / 6000.0 && f.zoom > 1.2, "{}", f.zoom);
        let f = Frame::fit([-1000.0, -3000.0], [1000.0, 3000.0], 0.0, None);
        assert!(!f.rotated);
        for (x, y) in [(-1000.0, -3000.0), (1000.0, 3000.0)] {
            let (c, r) = f.to_pixel(x, y);
            assert!((0.0..=1024.0).contains(&c) && (0.0..=768.0).contains(&r), "{c} {r}");
        }
        assert_eq!(f.height, 0.0);
    }

    #[test]
    fn pictures_are_what_the_game_reads() {
        let (w, h) = (WIDTH, HEIGHT);
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 3) as usize;
                rgb[i] = (x % 256) as u8;
                rgb[i + 1] = (y % 256) as u8;
                rgb[i + 2] = if x < 10 && y < 10 { 255 } else { 0 };
            }
        }
        let b = bmp8(w, h, &rgb);
        assert_eq!(&b[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(b[2..6].try_into().unwrap()) as usize, b.len());
        assert_eq!(u32::from_le_bytes(b[10..14].try_into().unwrap()), 1078);
        assert_eq!(u16::from_le_bytes(b[28..30].try_into().unwrap()), 8);
        assert_eq!(b.len(), 1078 + 1024 * 768);
        let img = image::load_from_memory(&b).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (1024, 768));
        // the corner square stays blue-ish, the far corner stays dark blue-less
        assert!(img.get_pixel(2, 2).0[2] > 150, "{:?}", img.get_pixel(2, 2));
        assert!(img.get_pixel(1000, 700).0[2] < 60);
        let t = tga24(w, h, &rgb);
        assert_eq!(&t[0..18], &[0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 3, 24, 0]);
        let img = image::load_from_memory_with_format(&t, image::ImageFormat::Tga).unwrap().to_rgb8();
        assert_eq!(img.get_pixel(5, 5).0, [5, 5, 255]);
        assert_eq!(img.get_pixel(300, 700).0, [44, 188, 0]);
    }

    #[test]
    fn draws_the_cube_room_from_above() {
        let bsp = Bsp::parse(&crate::goldsrc::bsp::fixture::build(), true).unwrap();
        let s = Scene::build(bsp, &wad::WadSet { wads: Vec::new() }, None);
        assert!(!s.polys.is_empty(), "no floors found");
        assert!(s.marks.iter().any(|m| m.kind == "ct") && s.marks.iter().any(|m| m.kind == "t"), "{:?}", s.marks);
        let o = Options { marks: "spawns".into(), background: Background::Black, ..Options::default() };
        let f = s.frame(&o);
        let rgb = render(&s, &f, &o);
        assert_eq!(rgb.len(), (WIDTH * HEIGHT * 3) as usize);
        let lit = rgb.chunks_exact(3).filter(|p| p != &[0, 0, 0]).count();
        assert!(lit > 10_000, "only {lit} pixels drawn");
        // the middle is floor, the corners are outside the room
        let px = |x: u32, y: u32| &rgb[((y * WIDTH + x) * 3) as usize..][..3];
        assert_ne!(px(512, 384), &[0, 0, 0]);
        assert_eq!(px(0, 0), &[0, 0, 0]);
        for style in [Style::Clean, Style::Simple] {
            let o = Options { style, background: Background::Black, ..Options::default() };
            assert_ne!(render(&s, &f, &o)[((384 * WIDTH + 512) * 3) as usize..][..3], [0, 0, 0]);
        }
    }

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> draws de_dust2 with the stock overview's framing
    /// and checks our floors land where the stock picture has map (not black). LS_TEST_DUMP=<dir>
    /// saves both pictures to look at.
    #[test]
    fn real_dust2_matches_the_stock_overview() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = PathBuf::from(hl);
        let bsp_path = hl.join("cstrike/maps/de_dust2.bsp");
        let Ok(text) = std::fs::read_to_string(hl.join("cstrike/overviews/de_dust2.txt")) else { return };
        let stock = image::open(hl.join("cstrike/overviews/de_dust2.bmp")).unwrap().to_rgb8();
        let p = parse_txt(&text).unwrap();
        let m = MapFile { name: "de_dust2".into(), bsp: bsp_path, game: "cstrike".into(), hl: hl.clone(), czero: false };
        let t0 = Instant::now();
        let s = scene_of(&m).unwrap();
        let built = t0.elapsed();
        let o = Options { marks: "bomb,buy,spawns".into(), background: Background::Black, ..Options::default() };
        let rgb = render(&s, &p.frame, &o);
        println!("de_dust2: nav {} floors {} built {:?} drawn {:?}", s.has_nav, s.polys.len(), built, t0.elapsed() - built);
        // agreement: of our drawn pixels, the share where the stock picture isn't background
        let bg = stock.get_pixel(0, 0).0;
        let (mut ours, mut agree) = (0, 0);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let i = ((y * WIDTH + x) * 3) as usize;
                if rgb[i..i + 3] != [0, 0, 0] {
                    ours += 1;
                    let q = stock.get_pixel(x, y).0;
                    if (0..3).any(|k| (q[k] as i32 - bg[k] as i32).abs() > 12) {
                        agree += 1;
                    }
                }
            }
        }
        println!("drawn {ours}, on the stock map {agree} ({:.1}%)", 100.0 * agree as f32 / ours.max(1) as f32);
        if let Ok(dir) = std::env::var("LS_TEST_DUMP") {
            let dir = PathBuf::from(dir);
            // red: map in the stock picture, nothing in ours; blue: the other way round
            let mut diff = rgb.clone();
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let i = ((y * WIDTH + x) * 3) as usize;
                    let q = stock.get_pixel(x, y).0;
                    let theirs = (0..3).any(|k| (q[k] as i32 - bg[k] as i32).abs() > 12);
                    let ours = rgb[i..i + 3] != [0, 0, 0];
                    if theirs && !ours {
                        diff[i..i + 3].copy_from_slice(&[255, 0, 0]);
                    } else if ours && !theirs {
                        diff[i..i + 3].copy_from_slice(&[0, 80, 255]);
                    }
                }
            }
            std::fs::write(dir.join("ov-dust2-diff.png"), to_png(&diff).unwrap()).unwrap();
            std::fs::write(dir.join("ov-dust2-ours.png"), to_png(&rgb).unwrap()).unwrap();
            for (style, roofs) in [("game", "nav"), ("clean", "nav"), ("simple", "nav"), ("game", "cut")] {
                let o: Options = serde_json::from_value(json!({"style": style, "roofs": roofs, "marks": "bomb,buy,spawns,hostages"})).unwrap();
                let f = s.frame(&o);
                std::fs::write(dir.join(format!("ov-dust2-fit-{style}-{roofs}.png")), to_png(&render(&s, &f, &o)).unwrap()).unwrap();
            }
        }
        assert!(agree as f32 > 0.85 * ours as f32, "our floors don't line up with the stock overview");
    }
}
#[cfg(test)]
mod look {
    use super::*;

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> LS_TEST_DUMP=<dir> LS_TEST_OV_MAPS=a,b draws
    /// those maps (each style) into <dir> to look at.
    #[test]
    fn real_maps_dump() {
        let (Ok(hl), Ok(dir), Ok(maps)) = (std::env::var("LS_TEST_GAME"), std::env::var("LS_TEST_DUMP"), std::env::var("LS_TEST_OV_MAPS")) else { return };
        let hl = PathBuf::from(hl);
        for name in maps.split(',') {
            let bsp = hl.join("cstrike/maps").join(format!("{name}.bsp"));
            let m = MapFile { name: name.into(), bsp, game: "cstrike".into(), hl: hl.clone(), czero: false };
            let t = Instant::now();
            let s = match scene_of(&m) {
                Ok(s) => s,
                Err(e) => {
                    println!("{name}: {}", e.msg);
                    continue;
                }
            };
            for style in ["game", "clean"] {
                let o: Options = serde_json::from_value(json!({"style": style, "marks": "bomb,buy,rescue,spawns,hostages"})).unwrap();
                let f = s.frame(&o);
                std::fs::write(PathBuf::from(&dir).join(format!("ov-{name}-{style}.png")), to_png(&render(&s, &f, &o)).unwrap()).unwrap();
            }
            println!("{name}: nav {} floors {} heights {:?} cut {} ({:?})", s.has_nav, s.polys.len(), s.zrange, s.cut_default, t.elapsed());
        }
    }
}
