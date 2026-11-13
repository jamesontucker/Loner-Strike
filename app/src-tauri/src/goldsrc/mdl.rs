//! GoldSrc studio models (`.mdl`, version 10): bones, sequences (the pose of any frame),
//! textures and skins, body parts, and the triangles of every mesh. Clean-room: written from
//! the public descriptions of the format, not from Valve's SDK.
//!
//! Layout facts used (all little-endian, offsets from the start of the file):
//! - header (244 bytes): "IDST", i32 version (10), name[64], i32 length, five vec3 (eye
//!   position, min, max, bbmin, bbmax), i32 flags, then count/offset pairs: bones (136+4),
//!   bone controllers, hitboxes, sequences, sequence groups, textures (count, offset, data
//!   offset), skins (refs, families, offset), body parts, attachments, sounds, transitions.
//! - bone (112): name[32], i32 parent, i32 flags, i32 controller[6], f32 value[6] (position,
//!   then rotation as x/y/z angles in radians), f32 scale[6] (what one animation step means).
//! - sequence (176): label[32], f32 fps, i32 flags, i32 activity, i32 weight, events (count,
//!   offset), i32 frames, pivots (count, offset), i32 motion type, i32 motion bone, vec3 linear
//!   movement, two offsets, vec3 bbmin/bbmax, i32 blends, i32 anim offset, blend data,
//!   i32 sequence group, nodes, i32 next.
//! - animation: per blend, per bone, six u16 offsets (from that record) to run-length
//!   encoded values: a {valid u8, total u8} header, then `valid` i16 values; a frame past
//!   `valid` repeats the last value. Value = bone value + step × bone scale.
//! - texture (80): name[64], i32 flags, i32 width, i32 height, i32 offset; the pixels
//!   (palette indices) are followed by a 256-colour RGB palette.
//! - body part (76): name[64], i32 model count, i32 base, i32 offset; model (112): name[64],
//!   type, radius, meshes (count, offset), vertices (count, bone-per-vertex offset, offset),
//!   normals (count, bone offset, offset), groups; mesh (20): i32 triangle-command count,
//!   offset, skin ref, normals (count, offset).
//! - triangle commands: i16 n; n > 0 a strip, n < 0 a fan of |n| vertices, 0 ends; each vertex
//!   is four i16: vertex index, normal index, s, t (in texels).
//!
//! Models whose textures live in a companion file (`<name>T.mdl`) have no textures of their
//! own: `Model::take_textures` brings them over. Sequences stored in group files
//! (`<name>01.mdl`, "IDSQ") are posed with `pose_with`.
//!
//! Everything read from the file is bounds-checked and capped: a damaged file is an Error,
//! never a panic.

use super::{cname, Error, Le, Result};

const WHAT: &str = "model file";
/// The biggest model file we open (the biggest stock player models are ~3 MB).
pub const MAX_MDL: u64 = 64 << 20;
const MAX_BONES: usize = 256;
const MAX_CONTROLLERS: usize = 64;
const MAX_SEQUENCES: usize = 4096;
const MAX_TEXTURES: usize = 512;
const MAX_SKIN_REFS: usize = 512;
const MAX_SKIN_FAMILIES: usize = 128;
const MAX_BODYPARTS: usize = 64;
const MAX_SUBMODELS: usize = 64;
const MAX_MESHES: usize = 1024;
const MAX_VERTS: usize = 1 << 16;
/// Triangle corners over the whole model (the biggest stock ones have ~10k).
const MAX_CORNERS: usize = 3_000_000;
/// Texture pixels over the whole model.
const MAX_TEX_BYTES: usize = 64 << 20;
const MAX_TEX_SIDE: u32 = 2048;
/// Frames we pose for one sequence (longer ones are cut, for the page).
pub const MAX_FRAMES: usize = 1024;

/// Motion flags: a sequence that moves the model along x/y/z keeps it in place when drawn.
const STUDIO_X: i32 = 1;
const STUDIO_Y: i32 = 2;
const STUDIO_Z: i32 = 4;
/// Sequence flag: the sequence loops.
pub const SEQ_LOOPING: i32 = 1;

/// Texture flags.
pub const TEX_CHROME: i32 = 2;
pub const TEX_FULLBRIGHT: i32 = 4;
pub const TEX_ADDITIVE: i32 = 32;
pub const TEX_MASKED: i32 = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: i32,
    pub value: [f32; 6],
    pub scale: [f32; 6],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: String,
    pub fps: f32,
    pub flags: i32,
    pub activity: i32,
    pub frames: u32,
    pub motion_type: i32,
    pub motion_bone: i32,
    pub blends: u32,
    pub anim_offset: u32,
    pub group: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    pub name: String,
    pub flags: i32,
    pub width: u32,
    pub height: u32,
    /// palette indices, row by row from the top
    pub pixels: Vec<u8>,
    /// 256 RGB colours
    pub palette: Vec<u8>,
}

/// One corner of a triangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corner {
    pub vert: u16,
    pub norm: u16,
    pub s: i16,
    pub t: i16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub skin_ref: u32,
    /// triangles, counter-clockwise seen from the front (three.js's default)
    pub tris: Vec<[Corner; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubModel {
    pub name: String,
    /// positions in their bone's space
    pub verts: Vec<[f32; 3]>,
    pub vert_bone: Vec<u8>,
    pub norms: Vec<[f32; 3]>,
    pub norm_bone: Vec<u8>,
    pub meshes: Vec<Mesh>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyPart {
    pub name: String,
    pub base: u32,
    pub models: Vec<SubModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub name: String,
    pub flags: i32,
    pub eye: [f32; 3],
    pub bbox: [[f32; 3]; 2],
    pub bones: Vec<Bone>,
    pub sequences: Vec<Sequence>,
    pub groups: u32,
    pub textures: Vec<Texture>,
    /// skin families × skin refs → texture index
    pub skins: Vec<Vec<u16>>,
    pub bodyparts: Vec<BodyPart>,
    /// the file, for reading animations on demand
    bytes: Vec<u8>,
}

/// A count/offset pair from the file, checked against a cap and the file size (each item
/// `size` bytes).
fn table(r: &Le, count_at: usize, offset_at: usize, size: usize, cap: usize, what: &str) -> Result<(usize, usize)> {
    let (n, off) = (r.i32(count_at)?, r.i32(offset_at)?);
    if n == 0 {
        return Ok((0, 0));
    }
    if n < 0 || n as usize > cap {
        return Err(Error::damaged(WHAT, format!("it says it has {n} {what}")));
    }
    if off < 0 || (off as usize).checked_add(n as usize * size).is_none_or(|end| end > r.len()) {
        return Err(Error::damaged(WHAT, format!("its {what} are past its end")));
    }
    Ok((n as usize, off as usize))
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

fn vec3(r: &Le, at: usize) -> Result<[f32; 3]> {
    let v = r.vec3(at)?;
    Ok(v.map(finite))
}

impl Model {
    /// Reads a model (or a textures-only companion file, `<name>T.mdl`).
    pub fn parse(bytes: Vec<u8>) -> Result<Model> {
        let r = Le::new(&bytes, WHAT);
        let magic = r.bytes(0, 4).map_err(|_| Error::NotFormat { what: WHAT, detail: "it's too short".into() })?;
        if magic == b"IDSQ" {
            return Err(Error::NotFormat { what: WHAT, detail: "it's the animation part of another model".into() });
        }
        if magic != b"IDST" {
            return Err(Error::NotFormat { what: WHAT, detail: "no studio model header".into() });
        }
        let version = r.i32(4)?;
        if version != 10 {
            let detail = if version == 44 || version == 48 || version == 49 { format!("it's a Source model (version {version})") } else { format!("version {version}, not 10") };
            return Err(Error::NotFormat { what: WHAT, detail });
        }
        if r.len() < 244 {
            return Err(Error::damaged(WHAT, "it's shorter than its header"));
        }
        let name = cname(r.bytes(8, 64)?);
        let eye = vec3(&r, 76)?;
        let bbox = [vec3(&r, 88)?, vec3(&r, 100)?];
        let flags = r.i32(136)?;

        let (nb, ob) = table(&r, 140, 144, 112, MAX_BONES, "bones")?;
        let mut bones = Vec::with_capacity(nb);
        for i in 0..nb {
            let o = ob + i * 112;
            let parent = r.i32(o + 32)?;
            if parent < -1 || parent >= i as i32 {
                // a parent must come before its child (the engine builds them in order)
                return Err(Error::damaged(WHAT, format!("bone {i} has parent {parent}")));
            }
            let mut value = [0.0; 6];
            let mut scale = [0.0; 6];
            for k in 0..6 {
                value[k] = finite(r.f32(o + 64 + k * 4)?);
                scale[k] = finite(r.f32(o + 88 + k * 4)?);
            }
            bones.push(Bone { name: cname(r.bytes(o, 32)?), parent, value, scale });
        }
        table(&r, 148, 152, 24, MAX_CONTROLLERS, "bone controllers")?;

        let (ns, os) = table(&r, 164, 168, 176, MAX_SEQUENCES, "sequences")?;
        let (ng, _) = table(&r, 172, 176, 104, 1000, "sequence groups")?;
        let mut sequences = Vec::with_capacity(ns);
        for i in 0..ns {
            let o = os + i * 176;
            let frames = r.i32(o + 56)?;
            let blends = r.i32(o + 120)?;
            let group = r.i32(o + 156)?;
            sequences.push(Sequence {
                name: cname(r.bytes(o, 32)?),
                fps: finite(r.f32(o + 32)?),
                flags: r.i32(o + 36)?,
                activity: r.i32(o + 40)?,
                frames: frames.max(0) as u32,
                motion_type: r.i32(o + 68)?,
                motion_bone: r.i32(o + 72)?,
                blends: blends.max(0) as u32,
                anim_offset: r.u32(o + 124)?,
                group: group.max(0) as u32,
            });
        }

        let (nt, ot) = table(&r, 180, 184, 80, MAX_TEXTURES, "textures")?;
        let mut textures = Vec::with_capacity(nt);
        let mut tex_bytes = 0usize;
        for i in 0..nt {
            let o = ot + i * 80;
            let (w, h) = (r.i32(o + 68)?, r.i32(o + 72)?);
            if w <= 0 || h <= 0 || w as u32 > MAX_TEX_SIDE || h as u32 > MAX_TEX_SIDE {
                return Err(Error::damaged(WHAT, format!("texture {i} is {w}×{h}")));
            }
            let (w, h) = (w as u32, h as u32);
            let n = (w * h) as usize;
            tex_bytes += n;
            if tex_bytes > MAX_TEX_BYTES {
                return Err(Error::TooBig { what: "model's textures", size: tex_bytes as u64, max: MAX_TEX_BYTES as u64 });
            }
            let at = r.u32(o + 76)? as usize;
            let pixels = r.bytes(at, n)?.to_vec();
            let palette = r.bytes(at + n, 768)?.to_vec();
            textures.push(Texture { name: cname(r.bytes(o, 64)?), flags: r.i32(o + 64)?, width: w, height: h, pixels, palette });
        }

        let (refs, fams, oskin) = (r.i32(192)?, r.i32(196)?, r.i32(200)?);
        let mut skins = Vec::new();
        if refs > 0 && fams > 0 {
            if refs as usize > MAX_SKIN_REFS || fams as usize > MAX_SKIN_FAMILIES || oskin < 0 {
                return Err(Error::damaged(WHAT, format!("its skin table is {fams}×{refs}")));
            }
            for f in 0..fams as usize {
                let mut row = Vec::with_capacity(refs as usize);
                for k in 0..refs as usize {
                    row.push(r.u16(oskin as usize + (f * refs as usize + k) * 2)?);
                }
                skins.push(row);
            }
        }

        let (np, op) = table(&r, 204, 208, 76, MAX_BODYPARTS, "body parts")?;
        let mut bodyparts = Vec::with_capacity(np);
        let mut corners = 0usize;
        for i in 0..np {
            let o = op + i * 76;
            let (nm, om) = table(&r, o + 64, o + 72, 112, MAX_SUBMODELS, "body part models")?;
            let base = r.i32(o + 68)?.max(1) as u32;
            let mut models = Vec::with_capacity(nm);
            for k in 0..nm {
                models.push(sub_model(&r, om + k * 112, nb, &mut corners)?);
            }
            bodyparts.push(BodyPart { name: cname(r.bytes(o, 64)?), base, models });
        }

        Ok(Model { name, flags, eye, bbox, bones, sequences, groups: ng as u32, textures, skins, bodyparts, bytes })
    }

    /// Takes the textures and skins of a textures-only companion file (`<name>T.mdl`).
    pub fn take_textures(&mut self, from: Model) {
        if self.textures.is_empty() {
            self.textures = from.textures;
            self.skins = from.skins;
        }
    }

    /// The texture a mesh draws with in a skin family (the first family when out of range).
    pub fn texture_for(&self, skin: usize, skin_ref: u32) -> Option<usize> {
        let row = self.skins.get(skin).or_else(|| self.skins.first());
        let t = match row {
            Some(row) => *row.get(skin_ref as usize)? as usize,
            None => skin_ref as usize,
        };
        (t < self.textures.len()).then_some(t)
    }

    /// Which model each body part shows for a body number (the engine's `body / base % n`).
    #[allow(dead_code)] // the page does this itself; kept as the reference
    pub fn body_choice(&self, body: u32) -> Vec<usize> {
        self.bodyparts.iter().map(|p| if p.models.is_empty() { 0 } else { ((body / p.base.max(1)) as usize) % p.models.len() }).collect()
    }

    /// Each bone's position and rotation (x, y, z, then a quaternion x, y, z, w) relative to
    /// its parent, at `frame` of sequence `seq` (frames clamp to the sequence). The rest pose
    /// when there's no such sequence or its animation is in a group file.
    pub fn pose(&self, seq: usize, frame: u32) -> Result<Vec<[f32; 7]>> {
        match self.sequences.get(seq) {
            Some(s) if s.group == 0 => self.pose_with(seq, frame, &self.bytes),
            _ => Ok(self.rest()),
        }
    }

    /// `pose` reading the animation from `data`: the model itself, or the group file a
    /// sequence names (`<name>01.mdl`, where its offsets point).
    pub fn pose_with(&self, seq: usize, frame: u32, data: &[u8]) -> Result<Vec<[f32; 7]>> {
        let Some(s) = self.sequences.get(seq) else { return Ok(self.rest()) };
        if s.blends == 0 || self.bones.is_empty() {
            return Ok(self.rest());
        }
        let frame = frame.min(s.frames.saturating_sub(1)) as usize;
        let r = Le::new(data, WHAT);
        let base = s.anim_offset as usize;
        r.bytes(base, self.bones.len() * 12)?;
        let mut out = Vec::with_capacity(self.bones.len());
        for (b, bone) in self.bones.iter().enumerate() {
            let rec = base + b * 12;
            let mut v = bone.value;
            for k in 0..6 {
                let off = r.u16(rec + k * 2)? as usize;
                if off != 0 {
                    v[k] += anim_value(&r, rec + off, frame)? as f32 * bone.scale[k];
                }
            }
            if b as i32 == s.motion_bone {
                if s.motion_type & STUDIO_X != 0 {
                    v[0] = 0.0;
                }
                if s.motion_type & STUDIO_Y != 0 {
                    v[1] = 0.0;
                }
                if s.motion_type & STUDIO_Z != 0 {
                    v[2] = 0.0;
                }
            }
            let q = quat([v[3], v[4], v[5]]);
            out.push([finite(v[0]), finite(v[1]), finite(v[2]), q[0], q[1], q[2], q[3]]);
        }
        Ok(out)
    }

    /// The bones' own values, no animation.
    pub fn rest(&self) -> Vec<[f32; 7]> {
        self.bones
            .iter()
            .map(|b| {
                let q = quat([b.value[3], b.value[4], b.value[5]]);
                [b.value[0], b.value[1], b.value[2], q[0], q[1], q[2], q[3]]
            })
            .collect()
    }
}

fn sub_model(r: &Le, o: usize, bones: usize, corners: &mut usize) -> Result<SubModel> {
    let name = cname(r.bytes(o, 64)?);
    let (nmesh, omesh) = table(r, o + 72, o + 76, 20, MAX_MESHES, "meshes")?;
    let (nv, ov) = table(r, o + 80, o + 88, 12, MAX_VERTS, "vertices")?;
    let (nn, on) = table(r, o + 92, o + 100, 12, MAX_VERTS, "normals")?;
    let vbone_at = r.i32(o + 84)?.max(0) as usize;
    let nbone_at = r.i32(o + 96)?.max(0) as usize;
    let bone_list = |at: usize, n: usize| -> Result<Vec<u8>> {
        let b = r.bytes(at, n)?;
        // a vertex on a bone that isn't there would be drawn from garbage: the root instead
        Ok(b.iter().map(|&x| if (x as usize) < bones.max(1) { x } else { 0 }).collect())
    };
    let verts = (0..nv).map(|i| vec3(r, ov + i * 12)).collect::<Result<Vec<_>>>()?;
    let norms = (0..nn).map(|i| vec3(r, on + i * 12)).collect::<Result<Vec<_>>>()?;
    let vert_bone = if nv > 0 { bone_list(vbone_at, nv)? } else { Vec::new() };
    let norm_bone = if nn > 0 { bone_list(nbone_at, nn)? } else { Vec::new() };
    let mut meshes = Vec::with_capacity(nmesh);
    for m in 0..nmesh {
        let mo = omesh + m * 20;
        let at = r.i32(mo + 4)?;
        let skin_ref = r.i32(mo + 8)?.max(0) as u32;
        let tris = if at > 0 { tri_commands(r, at as usize, nv, nn, corners)? } else { Vec::new() };
        meshes.push(Mesh { skin_ref, tris });
    }
    Ok(SubModel { name, verts, vert_bone, norms, norm_bone, meshes })
}

/// Reads a mesh's triangle commands into triangles. Corners naming a vertex or normal the
/// model doesn't have drop their triangle.
fn tri_commands(r: &Le, mut at: usize, nv: usize, nn: usize, corners: &mut usize) -> Result<Vec<[Corner; 3]>> {
    let mut tris = Vec::new();
    loop {
        let n = r.i16(at)?;
        at += 2;
        if n == 0 {
            break;
        }
        let fan = n < 0;
        let count = n.unsigned_abs() as usize;
        *corners += count;
        if *corners > MAX_CORNERS {
            return Err(Error::TooBig { what: "model", size: *corners as u64, max: MAX_CORNERS as u64 });
        }
        let mut list = Vec::with_capacity(count);
        for _ in 0..count {
            let c = Corner { vert: r.u16(at)?, norm: r.u16(at + 2)?, s: r.i16(at + 4)?, t: r.i16(at + 6)? };
            at += 8;
            list.push(c);
        }
        for k in 2..count {
            // the order OpenGL gives a strip or a fan; GoldSrc draws the other side of those
            // (it culls the front), so each triangle is flipped for three.js
            let (a, b, c) = if fan {
                (list[0], list[k - 1], list[k])
            } else if k % 2 == 0 {
                (list[k - 2], list[k - 1], list[k])
            } else {
                (list[k - 1], list[k - 2], list[k])
            };
            let ok = |x: &Corner| (x.vert as usize) < nv && (x.norm as usize) < nn.max(1);
            if ok(&a) && ok(&b) && ok(&c) {
                tris.push([a, c, b]);
            }
        }
    }
    Ok(tris)
}

/// One run-length-encoded animation value at `frame` (see the module notes).
fn anim_value(r: &Le, mut at: usize, frame: usize) -> Result<i16> {
    let mut k = frame;
    // each step skips a run; a file can't make us loop longer than it has runs
    for _ in 0..4096 {
        let (valid, total) = (r.u8(at)? as usize, r.u8(at + 1)? as usize);
        if total == 0 {
            return Err(Error::damaged(WHAT, "an animation has an empty run"));
        }
        if k < total {
            let i = if k < valid { k + 1 } else { valid };
            return r.i16(at + i * 2);
        }
        k -= total;
        at += (valid + 1) * 2;
    }
    Err(Error::damaged(WHAT, "an animation never ends"))
}

/// x/y/z angles (radians; roll, pitch, yaw) to a quaternion (x, y, z, w).
pub fn quat(a: [f32; 3]) -> [f32; 4] {
    let (sr, cr) = (a[0] * 0.5).sin_cos();
    let (sp, cp) = (a[1] * 0.5).sin_cos();
    let (sy, cy) = (a[2] * 0.5).sin_cos();
    let q = [sr * cp * cy - cr * sp * sy, cr * sp * cy + sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy];
    if q.iter().all(|v| v.is_finite()) { q } else { [0.0, 0.0, 0.0, 1.0] }
}

/// A 3×4 matrix (rows; the last column is the translation).
pub type Mat = [[f32; 4]; 3];

fn quat_mat(p: &[f32; 7]) -> Mat {
    let [x, y, z, w] = [p[3], p[4], p[5], p[6]];
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y), p[0]],
        [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x), p[1]],
        [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y), p[2]],
    ]
}

fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let mut m = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..4 {
            m[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + if j == 3 { a[i][3] } else { 0.0 };
        }
    }
    m
}

/// Each bone's matrix in model space for a pose (parents come first, checked in `parse`).
pub fn bone_matrices(bones: &[Bone], pose: &[[f32; 7]]) -> Vec<Mat> {
    let mut out: Vec<Mat> = Vec::with_capacity(bones.len());
    for (i, b) in bones.iter().enumerate() {
        let local = quat_mat(pose.get(i).unwrap_or(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]));
        let m = match out.get(b.parent.max(0) as usize).filter(|_| b.parent >= 0) {
            Some(p) => mat_mul(p, &local),
            None => local,
        };
        out.push(m);
    }
    out
}

pub fn transform(m: &Mat, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] + m[i][3])
}

pub fn rotate(m: &Mat, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// A texture as RGBA (masked textures: palette index 255 is see-through).
pub fn texture_rgba(t: &Texture) -> super::wad::Rgba {
    let masked = t.flags & TEX_MASKED != 0;
    let mut out = Vec::with_capacity(t.pixels.len() * 4);
    for &i in &t.pixels {
        if masked && i == 255 {
            out.extend([0, 0, 0, 0]);
        } else {
            let c = i as usize * 3;
            out.extend([t.palette[c], t.palette[c + 1], t.palette[c + 2], 255]);
        }
    }
    super::wad::Rgba { width: t.width, height: t.height, pixels: out }
}

/// Builds a model file (tests): one bone, one body part with one model of `tris`
/// triangles as a strip, one 8×8 texture, two skin families.
#[cfg(test)]
pub mod fixture {
    pub fn build(external_textures: bool) -> Vec<u8> {
        let mut b = vec![0u8; 244];
        b[0..4].copy_from_slice(b"IDST");
        b[4..8].copy_from_slice(&10i32.to_le_bytes());
        b[8..12].copy_from_slice(b"test");
        let put = |b: &mut Vec<u8>, at: usize, v: i32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let putf = |b: &mut Vec<u8>, at: usize, v: f32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        // bones: root at (0,0,10), child at (5,0,0) turned 90 degrees about z
        let bones = b.len();
        for (name, parent, pos, rot) in [("root", -1, [0.0, 0.0, 10.0], [0.0; 3]), ("arm", 0, [5.0, 0.0, 0.0], [0.0, 0.0, std::f32::consts::FRAC_PI_2])] {
            let mut rec = vec![0u8; 112];
            rec[..name.len()].copy_from_slice(name.as_bytes());
            rec[32..36].copy_from_slice(&(parent as i32).to_le_bytes());
            for k in 0..3 {
                rec[64 + k * 4..68 + k * 4].copy_from_slice(&(pos[k] as f32).to_le_bytes());
                rec[76 + k * 4..80 + k * 4].copy_from_slice(&(rot[k] as f32).to_le_bytes());
                rec[88 + k * 4..92 + k * 4].copy_from_slice(&1.0f32.to_le_bytes());
            }
            b.extend(rec);
        }
        put(&mut b, 140, 2);
        put(&mut b, 144, bones as i32);
        // animation for one sequence: bone 0 x moves 0,1,2 over three frames; bone 1 nothing
        let anim = b.len();
        let mut a = vec![0u8; 24];
        a[0..2].copy_from_slice(&24u16.to_le_bytes()); // offset from this record to the values
        a.extend([3u8, 3]); // valid 3, total 3
        for v in [0i16, 1, 2] {
            a.extend(v.to_le_bytes());
        }
        b.extend(a);
        let seq = b.len();
        let mut s = vec![0u8; 176];
        s[0..4].copy_from_slice(b"idle");
        s[32..36].copy_from_slice(&30f32.to_le_bytes());
        s[36..40].copy_from_slice(&1i32.to_le_bytes());
        s[56..60].copy_from_slice(&3i32.to_le_bytes());
        s[72..76].copy_from_slice(&(-1i32).to_le_bytes());
        s[120..124].copy_from_slice(&1i32.to_le_bytes());
        s[124..128].copy_from_slice(&(anim as u32).to_le_bytes());
        b.extend(s);
        put(&mut b, 164, 1);
        put(&mut b, 168, seq as i32);
        // textures (unless they live in the companion file)
        if !external_textures {
            let data = b.len() + 80;
            let mut t = vec![0u8; 80];
            t[0..8].copy_from_slice(b"skin.bmp");
            t[64..68].copy_from_slice(&64i32.to_le_bytes()); // masked
            t[68..72].copy_from_slice(&8i32.to_le_bytes());
            t[72..76].copy_from_slice(&8i32.to_le_bytes());
            t[76..80].copy_from_slice(&(data as i32).to_le_bytes());
            let tex_at = b.len();
            b.extend(t);
            b.extend((0..64u32).map(|i| if i == 0 { 255 } else { i as u8 }));
            b.extend((0..768u32).map(|i| (i / 3) as u8));
            put(&mut b, 180, 1);
            put(&mut b, 184, tex_at as i32);
            // skins: 2 families × 1 ref
            let skin = b.len();
            b.extend([0u8, 0, 0, 0]);
            put(&mut b, 192, 1);
            put(&mut b, 196, 2);
            put(&mut b, 200, skin as i32);
        }
        // geometry: 4 vertices (two on each bone), 1 normal, a strip of 4 corners = 2 triangles
        let verts = b.len();
        for v in [[0.0f32, 0.0, 0.0], [0.0, 4.0, 0.0], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0]] {
            for c in v {
                b.extend(c.to_le_bytes());
            }
        }
        let vbones = b.len();
        b.extend([0u8, 0, 1, 1]);
        let norms = b.len();
        for c in [0.0f32, 0.0, 1.0] {
            b.extend(c.to_le_bytes());
        }
        let nbones = b.len();
        b.extend([0u8, 0, 0, 0]);
        let tri = b.len();
        b.extend(4i16.to_le_bytes());
        for (v, s, t) in [(0u16, 0i16, 0i16), (1, 0, 8), (2, 8, 0), (3, 8, 8)] {
            b.extend(v.to_le_bytes());
            b.extend(0u16.to_le_bytes());
            b.extend(s.to_le_bytes());
            b.extend(t.to_le_bytes());
        }
        b.extend(0i16.to_le_bytes());
        b.extend([0, 0]);
        let mesh = b.len();
        let mut m = vec![0u8; 20];
        m[0..4].copy_from_slice(&2i32.to_le_bytes());
        m[4..8].copy_from_slice(&(tri as i32).to_le_bytes());
        b.extend(m);
        let model = b.len();
        let mut md = vec![0u8; 112];
        md[0..4].copy_from_slice(b"body");
        let mi = |md: &mut Vec<u8>, at: usize, v: usize| md[at..at + 4].copy_from_slice(&(v as i32).to_le_bytes());
        mi(&mut md, 72, 1);
        mi(&mut md, 76, mesh);
        mi(&mut md, 80, 4);
        mi(&mut md, 84, vbones);
        mi(&mut md, 88, verts);
        mi(&mut md, 92, 1);
        mi(&mut md, 96, nbones);
        mi(&mut md, 100, norms);
        b.extend(md);
        // a second, empty model in the same body part ("off")
        let mut off = vec![0u8; 112];
        off[0..5].copy_from_slice(b"blank");
        b.extend(off);
        let part = b.len();
        let mut p = vec![0u8; 76];
        p[0..4].copy_from_slice(b"main");
        p[64..68].copy_from_slice(&2i32.to_le_bytes());
        p[68..72].copy_from_slice(&1i32.to_le_bytes());
        p[72..76].copy_from_slice(&(model as i32).to_le_bytes());
        b.extend(p);
        put(&mut b, 204, 1);
        put(&mut b, 208, part as i32);
        putf(&mut b, 76, 1.0);
        let len = b.len() as i32;
        put(&mut b, 72, len);
        b
    }

    /// The companion file: textures and skins only.
    pub fn textures_file() -> Vec<u8> {
        let full = build(false);
        let mut b = full.clone();
        // no bones, sequences or body parts
        for at in [140, 164, 204] {
            b[at..at + 4].copy_from_slice(&0i32.to_le_bytes());
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn reads_the_fixture() {
        let m = Model::parse(fixture::build(false)).unwrap();
        assert_eq!(m.name, "test");
        assert_eq!(m.bones.len(), 2);
        assert_eq!(m.bones[1].parent, 0);
        assert_eq!(m.sequences[0].name, "idle");
        assert_eq!(m.sequences[0].frames, 3);
        assert_eq!(m.textures[0].name, "skin.bmp");
        assert_eq!(m.skins, vec![vec![0], vec![0]]);
        assert_eq!(m.bodyparts[0].models.len(), 2);
        let sm = &m.bodyparts[0].models[0];
        assert_eq!(sm.meshes[0].tris.len(), 2);
        // strip 0,1,2 then 2,1,3, each flipped
        let ids = |t: &[Corner; 3]| t.map(|c| c.vert);
        assert_eq!(ids(&sm.meshes[0].tris[0]), [0, 2, 1]);
        assert_eq!(ids(&sm.meshes[0].tris[1]), [2, 3, 1]);
        assert_eq!(m.body_choice(0), vec![0]);
        assert_eq!(m.body_choice(1), vec![1]);
        assert_eq!(m.texture_for(1, 0), Some(0));
        assert_eq!(m.texture_for(5, 0), Some(0));
        assert_eq!(m.texture_for(0, 3), None);
        let rgba = texture_rgba(&m.textures[0]);
        assert_eq!(&rgba.pixels[0..4], &[0, 0, 0, 0]);
        assert_eq!(&rgba.pixels[4..8], &[1, 1, 1, 255]);
    }

    #[test]
    fn poses_and_bones() {
        let m = Model::parse(fixture::build(false)).unwrap();
        // frame 2: root x = 0 + 2 × 1
        let p = m.pose(0, 2).unwrap();
        assert!(close([p[0][0], p[0][1], p[0][2]], [2.0, 0.0, 10.0]));
        // past the end clamps; a missing sequence is the rest pose
        assert_eq!(m.pose(0, 99).unwrap(), p);
        assert_eq!(m.pose(7, 0).unwrap(), m.rest());
        let mats = bone_matrices(&m.bones, &m.rest());
        // the arm bone sits at root + (5,0,0), turned 90 degrees: its +x points along +y
        assert!(close(transform(&mats[1], [0.0; 3]), [5.0, 0.0, 10.0]));
        assert!(close(transform(&mats[1], [1.0, 0.0, 0.0]), [5.0, 1.0, 10.0]));
        assert!(close(rotate(&mats[1], [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn companion_textures() {
        let mut m = Model::parse(fixture::build(true)).unwrap();
        assert!(m.textures.is_empty());
        assert_eq!(m.texture_for(0, 0), None);
        m.take_textures(Model::parse(fixture::textures_file()).unwrap());
        assert_eq!(m.textures.len(), 1);
        assert_eq!(m.texture_for(0, 0), Some(0));
    }

    #[test]
    fn damaged_files_are_errors() {
        let good = fixture::build(false);
        for n in [0, 3, 100, 243, 300, 600, good.len() / 2, good.len() - 30] {
            assert!(Model::parse(good[..n].to_vec()).is_err(), "cut at {n}");
        }
        let mut b = good.clone();
        b[4] = 44;
        assert!(Model::parse(b).unwrap_err().to_string().contains("Source model"));
        let mut b = good.clone();
        b[140..144].copy_from_slice(&100000i32.to_le_bytes());
        assert!(Model::parse(b).is_err());
        let mut b = good.clone();
        b[144..148].copy_from_slice(&(-5i32).to_le_bytes());
        assert!(Model::parse(b).is_err());
        // a bone whose parent comes after it
        let mut b = good.clone();
        b[244 + 32..244 + 36].copy_from_slice(&1i32.to_le_bytes());
        assert!(Model::parse(b).is_err());
        assert!(Model::parse(b"IDSQ\x0a\0\0\0".to_vec()).is_err());
        // every single byte flipped: never a panic
        for i in 0..good.len() {
            let mut b = good.clone();
            b[i] ^= 0xff;
            if let Ok(m) = Model::parse(b) {
                let _ = m.pose(0, 1);
                let _ = bone_matrices(&m.bones, &m.rest());
            }
        }
    }

    #[test]
    fn animation_runs() {
        // runs: {valid 2, total 3}: 5, 6 (frame 2 repeats 6); {valid 1, total 2}: 9
        let mut b = vec![2u8, 3];
        for v in [5i16, 6] {
            b.extend(v.to_le_bytes());
        }
        b.extend([1u8, 2]);
        b.extend(9i16.to_le_bytes());
        let r = Le::new(&b, WHAT);
        let got: Vec<i16> = (0..5).map(|f| anim_value(&r, 0, f).unwrap()).collect();
        assert_eq!(got, vec![5, 6, 6, 9, 9]);
        assert!(anim_value(&r, 0, 6).is_err());
        assert!(anim_value(&Le::new(&[1, 0, 0, 0], WHAT), 0, 0).is_err());
    }

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> reads every model in cstrike, czero and valve,
    /// poses its first sequence and checks the triangles face the way their normals say.
    #[test]
    fn real_models() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = std::path::PathBuf::from(hl);
        let (mut n, mut bad, mut agree, mut disagree) = (0, 0, 0u64, 0u64);
        for game in ["cstrike", "czero", "valve", "cstrike_downloads"] {
            let mut stack = vec![hl.join(game).join("models")];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("mdl")) {
                        continue;
                    }
                    n += 1;
                    let m = match Model::parse(std::fs::read(&p).unwrap()) {
                        Ok(m) => m,
                        Err(e) => {
                            bad += 1;
                            println!("{}: {e}", p.display());
                            continue;
                        }
                    };
                    let pose = m.pose(0, 0).unwrap_or_else(|_| m.rest());
                    let mats = bone_matrices(&m.bones, &pose);
                    for part in &m.bodyparts {
                        for sm in &part.models {
                            for mesh in &sm.meshes {
                                for t in &mesh.tris {
                                    let at = |c: &Corner| transform(&mats[sm.vert_bone[c.vert as usize] as usize], sm.verts[c.vert as usize]);
                                    let (a, b, c) = (at(&t[0]), at(&t[1]), at(&t[2]));
                                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                                    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                                    let x = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                                    let nb = sm.norm_bone.get(t[0].norm as usize).copied().unwrap_or(0) as usize;
                                    let nn = rotate(&mats[nb], sm.norms.get(t[0].norm as usize).copied().unwrap_or([0.0; 3]));
                                    let d = x[0] * nn[0] + x[1] * nn[1] + x[2] * nn[2];
                                    if d > 0.0 {
                                        agree += 1;
                                    } else if d < 0.0 {
                                        disagree += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("{n} models, {bad} not read; triangles facing their normals: {agree}, the other way: {disagree}");
        assert!(agree > disagree * 10, "triangles wind the wrong way");
    }
}
