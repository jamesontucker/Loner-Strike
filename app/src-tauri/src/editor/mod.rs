//! The map editor core: a brush-based map document (the source of truth the 3D view draws),
//! brush geometry and CSG, GoldSrc .map read/write, undo/redo and pre-compile checks.
//! Pure Rust with no routes yet; see docs/TOOLS-EDITOR.md for how the UI will drive it.
#![allow(dead_code)]

pub mod bulk; // P5
pub mod checks;
pub mod fgd;
pub mod doc;
pub mod extrude; // Parity A: pull a new brush out of a face
pub mod geom;
pub mod history;
pub mod mapfile;
pub mod math;
pub mod ops;
pub mod prefab;
pub mod rmf; // P5: Hammer .rmf and J.A.C.K. .jmf import
pub mod shapes;
pub mod shear; // Parity A: skew the selection
pub mod texture;
pub mod vertex;
