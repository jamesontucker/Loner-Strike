//! The map editor core: a brush-based map document (the source of truth the 3D view draws),
//! brush geometry and CSG, GoldSrc .map read/write, undo/redo and pre-compile checks.
//! Pure Rust with no routes yet; see docs/TOOLS-EDITOR.md for how the UI will drive it.
#![allow(dead_code)]

pub mod checks;
pub mod fgd;
pub mod doc;
pub mod geom;
pub mod history;
pub mod mapfile;
pub mod math;
pub mod ops;
pub mod prefab;
pub mod shapes;
pub mod texture;
pub mod vertex;
