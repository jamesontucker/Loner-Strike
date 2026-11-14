//! A coarse picture of the map's open space (a grid of cells, each blocked by world brushes or
//! open), for what needs to know "inside" from "outside": faces against the void, dark rooms,
//! and a sampled estimate of how much is in view (heavy views). The last compile's VIS data,
//! when there is one, gives the heavy views instead (`pvs_counts`).

use super::scan::{at, never_drawn, texlight, Scan};
use super::{issue, Issue, Level};
use crate::editor::geom::polygon_center;
use crate::editor::math::{v3, Bounds, Vec3};
use std::collections::{HashMap, VecDeque};

/// Cells at most (a few MB of flags).
const MAX_CELLS: usize = 3_000_000;

pub struct Field {
    pub o: Vec3,
    pub s: f64,
    pub n: [usize; 3],
    pub blocked: Vec<bool>,
    /// Reached from outside the map without crossing a world brush.
    pub outside: Vec<bool>,
    /// Something that should be inside (a spawn, a light) is outside: the map is open (or
    /// sealed only by gaps smaller than a cell), so void and room answers aren't trusted.
    pub leaks: bool,
}

impl Field {
    pub fn build(scan: &Scan) -> Option<Field> {
        let mut bb = Bounds::empty();
        for b in scan.brushes.iter().filter(|b| b.seals()) {
            bb = bb.union(&b.bounds);
        }
        if bb.is_empty() {
            return None;
        }
        let mut s = 32.0;
        let dims = |s: f64| {
            let sz = bb.size();
            [(sz.x / s).ceil() as usize + 4, (sz.y / s).ceil() as usize + 4, (sz.z / s).ceil() as usize + 4]
        };
        while dims(s).iter().product::<usize>() > MAX_CELLS {
            s *= 2.0;
        }
        let n = dims(s);
        let o = bb.min - v3(2.0 * s, 2.0 * s, 2.0 * s);
        let total = n[0] * n[1] * n[2];
        let mut blocked = vec![false; total];
        let idx = |x: usize, y: usize, z: usize| (z * n[1] + y) * n[0] + x;
        for b in scan.brushes.iter().filter(|b| b.seals()) {
            let lo = [((b.bounds.min.x - o.x) / s).floor() as usize, ((b.bounds.min.y - o.y) / s).floor() as usize, ((b.bounds.min.z - o.z) / s).floor() as usize];
            let hi = [
                (((b.bounds.max.x - o.x) / s).ceil() as usize).min(n[0]),
                (((b.bounds.max.y - o.y) / s).ceil() as usize).min(n[1]),
                (((b.bounds.max.z - o.z) / s).ceil() as usize).min(n[2]),
            ];
            let planes: Vec<_> = b.planes.iter().flatten().collect();
            for z in lo[2]..hi[2] {
                for y in lo[1]..hi[1] {
                    for x in lo[0]..hi[0] {
                        let c = idx(x, y, z);
                        if blocked[c] {
                            continue;
                        }
                        let min = o + v3(x as f64 * s, y as f64 * s, z as f64 * s);
                        let max = min + v3(s, s, s);
                        // overlap with positive depth (touching doesn't block)
                        let hit = planes.iter().all(|pl| {
                            let nn = pl.normal;
                            let sup = v3(if nn.x > 0.0 { min.x } else { max.x }, if nn.y > 0.0 { min.y } else { max.y }, if nn.z > 0.0 { min.z } else { max.z });
                            pl.distance(sup) < -0.01
                        }) && b.bounds.max.x > min.x + 0.01
                            && b.bounds.min.x < max.x - 0.01
                            && b.bounds.max.y > min.y + 0.01
                            && b.bounds.min.y < max.y - 0.01
                            && b.bounds.max.z > min.z + 0.01
                            && b.bounds.min.z < max.z - 0.01;
                        if hit {
                            blocked[c] = true;
                        }
                    }
                }
            }
        }
        let mut f = Field { o, s, n, blocked, outside: vec![false; total], leaks: false };
        // flood from the border
        let mut q = VecDeque::new();
        for z in 0..n[2] {
            for y in 0..n[1] {
                for x in 0..n[0] {
                    if x == 0 || y == 0 || z == 0 || x == n[0] - 1 || y == n[1] - 1 || z == n[2] - 1 {
                        let c = idx(x, y, z);
                        if !f.blocked[c] && !f.outside[c] {
                            f.outside[c] = true;
                            q.push_back(c);
                        }
                    }
                }
            }
        }
        f.flood(&mut q, |f, c| !f.blocked[c] && !f.outside[c], |f, c| f.outside[c] = true);
        // a spawn or a light outside means it leaks (at this coarse size, at least)
        f.leaks = scan.doc.entities.values().filter(|e| matches!(e.classname(), "info_player_start" | "info_player_deathmatch" | "light" | "light_spot")).filter_map(|e| e.origin()).any(|p| f.cell_index(p).is_some_and(|c| f.outside[c]));
        Some(f)
    }

    fn neighbors(&self, c: usize) -> impl Iterator<Item = usize> + '_ {
        let n = self.n;
        let (x, y, z) = (c % n[0], (c / n[0]) % n[1], c / (n[0] * n[1]));
        let mut v = [usize::MAX; 6];
        if x > 0 {
            v[0] = c - 1;
        }
        if x + 1 < n[0] {
            v[1] = c + 1;
        }
        if y > 0 {
            v[2] = c - n[0];
        }
        if y + 1 < n[1] {
            v[3] = c + n[0];
        }
        if z > 0 {
            v[4] = c - n[0] * n[1];
        }
        if z + 1 < n[2] {
            v[5] = c + n[0] * n[1];
        }
        v.into_iter().filter(|c| *c != usize::MAX)
    }

    fn flood(&mut self, q: &mut VecDeque<usize>, ok: impl Fn(&Field, usize) -> bool, mut mark: impl FnMut(&mut Field, usize)) {
        while let Some(c) = q.pop_front() {
            let next: Vec<usize> = self.neighbors(c).filter(|d| ok(self, *d)).collect();
            for d in next {
                mark(self, d);
                q.push_back(d);
            }
        }
    }

    pub fn cell_index(&self, p: Vec3) -> Option<usize> {
        let r = (p - self.o) / self.s;
        if r.x < 0.0 || r.y < 0.0 || r.z < 0.0 {
            return None;
        }
        let (x, y, z) = (r.x as usize, r.y as usize, r.z as usize);
        (x < self.n[0] && y < self.n[1] && z < self.n[2]).then(|| (z * self.n[1] + y) * self.n[0] + x)
    }

    pub fn center(&self, c: usize) -> Vec3 {
        let n = self.n;
        let (x, y, z) = (c % n[0], (c / n[0]) % n[1], c / (n[0] * n[1]));
        self.o + v3((x as f64 + 0.5) * self.s, (y as f64 + 0.5) * self.s, (z as f64 + 0.5) * self.s)
    }

    /// The first open cell in front of a point on a face (None when it's all blocked).
    fn open_in_front(&self, p: Vec3, n: Vec3) -> Option<usize> {
        (0..4).find_map(|k| self.cell_index(p + n * (self.s * (0.6 + k as f64))).filter(|c| !self.blocked[*c]))
    }

    /// Every part of the face looks out at the void (the outside of a sealed map).
    pub fn face_on_void(&self, poly: &[Vec3], n: Vec3) -> bool {
        if self.leaks || poly.is_empty() {
            return false;
        }
        let c = polygon_center(poly);
        std::iter::once(c).chain(poly.iter().map(|p| c + (*p - c) * 0.75)).all(|p| self.open_in_front(p, n).is_some_and(|k| self.outside[k]))
    }
}

/// Enclosed areas with no light in them (no light entity, light texture or sky).
pub fn dark_rooms(scan: &Scan, f: &Field, out: &mut Vec<Issue>) {
    if f.leaks || scan.doc.world.get("light").is_some_and(|l| l.trim().parse::<f64>().unwrap_or(0.0) > 0.0) {
        return;
    }
    let total = f.blocked.len();
    let mut comp = vec![u32::MAX; total];
    let mut sizes: Vec<usize> = Vec::new();
    let mut first: Vec<usize> = Vec::new();
    for c in 0..total {
        if f.blocked[c] || f.outside[c] || comp[c] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut q = VecDeque::from([c]);
        comp[c] = id;
        let mut size = 0;
        while let Some(k) = q.pop_front() {
            size += 1;
            for d in f.neighbors(k) {
                if !f.blocked[d] && !f.outside[d] && comp[d] == u32::MAX {
                    comp[d] = id;
                    q.push_back(d);
                }
            }
        }
        sizes.push(size);
        first.push(c);
    }
    if sizes.is_empty() {
        return;
    }
    let mut lit = vec![false; sizes.len()];
    let mut light_at = |p: Vec3| {
        if let Some(c) = f.cell_index(p) {
            for k in std::iter::once(c).chain(f.neighbors(c)) {
                if comp[k] != u32::MAX {
                    lit[comp[k] as usize] = true;
                }
            }
        }
    };
    for e in scan.doc.entities.values() {
        if matches!(e.classname(), "light" | "light_spot") || e.get("_light").is_some() && e.classname() != "light_environment" {
            if let Some(o) = e.origin() {
                light_at(o);
            }
        }
    }
    for b in &scan.brushes {
        if !b.drawn {
            continue;
        }
        for (i, face) in b.b.faces.iter().enumerate() {
            let t = &face.texture;
            if b.polys[i].is_empty() || !(texlight(t) || super::scan::is_sky(t)) || (never_drawn(t) && !super::scan::is_sky(t)) {
                continue;
            }
            let Some(pl) = b.planes[i] else { continue };
            if let Some(c) = f.open_in_front(polygon_center(&b.polys[i]), pl.normal) {
                if comp[c] != u32::MAX {
                    lit[comp[c] as usize] = true;
                }
            }
        }
    }
    let min_cells = ((128.0f64 / f.s).powi(3)).ceil().max(2.0) as usize;
    let mut n = 0;
    for (id, size) in sizes.iter().enumerate() {
        if lit[id] || *size < min_cells {
            continue;
        }
        n += 1;
        if n > 10 {
            continue;
        }
        // somewhere near the middle of it
        let cells: Vec<usize> = (0..total).filter(|c| comp[*c] == id as u32).collect();
        let mid = cells.iter().fold(Vec3::ZERO, |a, c| a + f.center(*c)) / cells.len() as f64;
        let best = cells.iter().min_by(|a, b| (f.center(**a) - mid).len().total_cmp(&(f.center(**b) - mid).len())).copied().unwrap_or(first[id]);
        out.push(Issue {
            at: Some(at(f.center(best))),
            hint: Some("Place a light (or use a light texture); without one the room is pitch black.".into()),
            ..issue(Level::Warning, "dark-room", format!("An enclosed space (about {} units across) has no light in it.", ((*size as f64) * f.s.powi(3)).cbrt().round()))
        });
    }
}

/// Faces in view from sampled standing spots (rays through the open cells), for when there's
/// no compile to read: [(spot, faces seen)].
pub fn sampled_views(scan: &Scan, f: &Field, hidden: &std::collections::HashSet<(usize, usize)>) -> Vec<(Vec3, usize)> {
    if f.leaks {
        return Vec::new();
    }
    // faces by the cells they touch
    let mut faces_in: HashMap<usize, Vec<u32>> = HashMap::new();
    let mut nfaces = 0u32;
    let mut budget = 6_000_000usize;
    for (i, b) in scan.brushes.iter().enumerate() {
        for (k, poly) in b.polys.iter().enumerate() {
            if !b.face_drawn(k) || hidden.contains(&(i, k)) {
                continue;
            }
            let id = nfaces;
            nfaces += 1;
            let mut bb = Bounds::empty();
            poly.iter().for_each(|p| bb.add(*p));
            let lo = (bb.min - f.o) / f.s;
            let hi = (bb.max - f.o) / f.s;
            let r = |a: f64, m: usize| (a.max(0.0) as usize).min(m - 1);
            for z in r(lo.z - 0.01, f.n[2])..=r(hi.z + 0.01, f.n[2]) {
                for y in r(lo.y - 0.01, f.n[1])..=r(hi.y + 0.01, f.n[1]) {
                    for x in r(lo.x - 0.01, f.n[0])..=r(hi.x + 0.01, f.n[0]) {
                        if budget == 0 {
                            break;
                        }
                        budget -= 1;
                        faces_in.entry((z * f.n[1] + y) * f.n[0] + x).or_default().push(id);
                    }
                }
            }
        }
    }
    // standing spots: open inside cells with something solid under them
    let layer = f.n[0] * f.n[1];
    let spots: Vec<usize> = (layer..f.blocked.len()).filter(|c| !f.blocked[*c] && !f.outside[*c] && f.blocked[c - layer]).collect();
    if spots.is_empty() {
        return Vec::new();
    }
    let step = (spots.len() / 400).max(1);
    let dirs: Vec<Vec3> = (0..160)
        .map(|i| {
            // a Fibonacci sphere, a little flattened toward the horizon (where players look)
            let y = 1.0 - (i as f64 + 0.5) / 160.0 * 2.0;
            let r = (1.0 - y * y).sqrt();
            let t = i as f64 * 2.399_963;
            v3(r * t.cos(), r * t.sin(), y * 0.6).normalized()
        })
        .collect();
    let mut seen = vec![0u32; nfaces as usize];
    let mut stamp = 0u32;
    let max_steps = (4096.0 / f.s) as usize * 2;
    let mut out = Vec::new();
    for c in spots.iter().step_by(step) {
        stamp += 1;
        let start = f.center(*c);
        let mut count = 0usize;
        for d in &dirs {
            let mut p = start;
            for _ in 0..max_steps {
                p = p + *d * (f.s * 0.5);
                let Some(k) = f.cell_index(p) else { break };
                if let Some(list) = faces_in.get(&k) {
                    for id in list {
                        if seen[*id as usize] != stamp {
                            seen[*id as usize] = stamp;
                            count += 1;
                        }
                    }
                }
                if f.blocked[k] {
                    break;
                }
            }
        }
        out.push((start, count));
    }
    out
}

/// Faces each VIS leaf can see, from a compiled map: [(leaf center, faces)]. Reads the leaves,
/// their face lists and the compressed visibility.
pub fn pvs_counts(b: &[u8]) -> Result<Vec<(Vec3, usize)>, String> {
    use crate::goldsrc::bsp;
    let l = bsp::header(b).map_err(|e| e.to_string())?;
    let lump = |i: usize| &b[l[i].offset as usize..(l[i].offset + l[i].length) as usize];
    let (vis, leaves, marks, models) = (lump(4), lump(10), lump(11), lump(bsp::LUMP_MODELS));
    if models.len() < 64 || leaves.len() % 28 != 0 || marks.len() % 2 != 0 {
        return Err("The compiled map's leaves can't be read.".into());
    }
    let u16_at = |s: &[u8], o: usize| u16::from_le_bytes([s[o], s[o + 1]]) as usize;
    let i16_at = |s: &[u8], o: usize| i16::from_le_bytes([s[o], s[o + 1]]) as f64;
    let i32_at = |s: &[u8], o: usize| i32::from_le_bytes([s[o], s[o + 1], s[o + 2], s[o + 3]]);
    let nleaves = leaves.len() / 28;
    let visleafs = (i32_at(models, 52).max(0) as usize).min(nleaves.saturating_sub(1));
    let nmarks = marks.len() / 2;
    let faces_of = |leaf: usize| -> Vec<usize> {
        let o = leaf * 28;
        let (first, num) = (u16_at(leaves, o + 20), u16_at(leaves, o + 22));
        (first..(first + num).min(nmarks)).map(|m| u16_at(marks, m * 2)).collect()
    };
    let nfaces = l[bsp::LUMP_FACES].length as usize / 20;
    let mut seen = vec![0u32; nfaces.max(1)];
    let mut out = Vec::new();
    let row = visleafs.div_ceil(8);
    for leaf in 1..=visleafs {
        let o = leaf * 28;
        if i32_at(leaves, o) != -1 {
            continue; // only open (empty) leaves
        }
        let visofs = i32_at(leaves, o + 4);
        let stamp = leaf as u32;
        let mut count = 0;
        let mut add = |lf: usize, count: &mut usize| {
            for fc in faces_of(lf) {
                if fc < seen.len() && seen[fc] != stamp {
                    seen[fc] = stamp;
                    *count += 1;
                }
            }
        };
        add(leaf, &mut count);
        if visofs >= 0 && (visofs as usize) < vis.len() {
            // run-length: a zero byte is followed by how many zero bytes it stands for
            let mut p = visofs as usize;
            let mut bit = 0usize;
            while bit < row * 8 && p < vis.len() {
                let byte = vis[p];
                p += 1;
                if byte == 0 {
                    let run = vis.get(p).copied().unwrap_or(0) as usize;
                    p += 1;
                    bit += run * 8;
                    continue;
                }
                for k in 0..8 {
                    if byte & (1 << k) != 0 && bit + k < visleafs {
                        add(bit + k + 1, &mut count);
                    }
                }
                bit += 8;
            }
        }
        let c = v3((i16_at(leaves, o + 8) + i16_at(leaves, o + 14)) / 2.0, (i16_at(leaves, o + 10) + i16_at(leaves, o + 16)) / 2.0, (i16_at(leaves, o + 12) + i16_at(leaves, o + 18)) / 2.0);
        out.push((c, count));
    }
    Ok(out)
}

/// The heaviest views, a few spots far apart, over `limit` faces.
pub fn heavy_views(points: &[(Vec3, usize)], limit: usize, source: &str, out: &mut Vec<Issue>) {
    let mut v: Vec<&(Vec3, usize)> = points.iter().filter(|(_, n)| *n > limit).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    // most of the map over the limit: it's open everywhere, one line rather than random spots
    let mut all: Vec<usize> = points.iter().map(|p| p.1).collect();
    all.sort_unstable();
    let median = all.get(all.len() / 2).copied().unwrap_or(0);
    if median > limit {
        if let Some((p, n)) = v.first() {
            out.push(Issue {
                at: Some(at(*p)),
                hint: Some("Most places see most of the map, so VIS can't cut it down. Block long sight lines (walls, hills, buildings), add HINT brushes at the openings, make small detail func_detail and NULL faces nobody sees.".into()),
                ..issue(Level::Info, "heavy-view", format!("The whole map is heavy to draw: half of it sees {median}+ faces, up to {n} ({source})."))
            });
        }
        return;
    }
    let mut picked: Vec<Vec3> = Vec::new();
    for (p, n) in v {
        if picked.len() >= 5 {
            break;
        }
        if picked.iter().any(|q| (*q - *p).len() < 768.0) {
            continue;
        }
        picked.push(*p);
        out.push(Issue {
            at: Some(at(*p)),
            hint: Some("Split long views with HINT brushes, turn small detail into func_detail so it doesn't cut the world, and NULL faces nobody sees.".into()),
            ..issue(Level::Info, "heavy-view", format!("Heavy view: about {n} faces can be in sight here ({source})."))
        });
    }
}
