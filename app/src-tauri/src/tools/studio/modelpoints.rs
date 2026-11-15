//! The bones and attachments of a studio model, for the Studio's picker on Get Bone Position
//! and Get Attachment Position (Y). Read from the game's own files (the mod's folders, then
//! valve), only the two name tables of the header:
//! - bones: count at 140, offset at 144, 112 bytes each, name[32] first;
//! - attachments: count at 212, offset at 216, 88 bytes each: name[32], i32 type, i32 bone.
//!
//!   GET /api/tools/studio/model-points?model=models/player.mdl
//!     {bones: ["Bip01", ...], attachments: [{index, name, bone}], fallback?: true}
//! With no game here, models/player.mdl answers with the standard player skeleton (fallback).

use crate::goldsrc::{cname, Le};
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// Counter-Strike's models/player.mdl skeleton (every player model shares it).
pub const PLAYER_BONES: [&str; 53] = [
    "Bip01", "Bip01 Pelvis", "Bip01 Spine", "Bip01 Spine1", "Bip01 Spine2", "Bip01 Spine3", "Bip01 Neck", "Bip01 Head", "Bone01", "Bip01 L Clavicle", "Bip01 L UpperArm", "Bip01 L Forearm",
    "Bip01 L Hand", "Bip01 L Finger0", "Bip01 L Finger01", "Bip01 L Finger1", "Bip01 L Finger11", "-- L knuckle", "-- L wrist", "-- L Elbow", "-- L bicep twist", "-- L shoulder outside",
    "-- L Shoulder inside", "Bip01 R Clavicle", "Bip01 R UpperArm", "Bip01 R Forearm", "Bip01 R Hand", "Bip01 R Finger0", "Bip01 R Finger01", "Bip01 R Finger1", "Bip01 R Finger11",
    "-- R knuckle", "-- R wrist", "-- R Elbow", "-- R bicep twist", "-- R Shoulder inside", "-- R shoulder outside", "-- Neck smooth", "-- R Butt", "-- L butt", "Bip01 L Thigh",
    "Bip01 L Calf", "Bip01 L Foot", "Bip01 L Toe0", "-- L ankle", "-- L Knee", "Bip01 R Thigh", "Bip01 R Calf", "Bip01 R Foot", "Bip01 R Toe0", "-- R Ankle", "-- R Knee", "Object04",
];

/// The names in a model file's bytes: bones, and attachments (number, name, the bone it's on).
pub fn points(bytes: &[u8]) -> Result<(Vec<String>, Vec<Value>), String> {
    let r = Le::new(bytes, "model file");
    let bad = || "That isn't a Half-Life model file (models/...mdl).".to_string();
    if r.bytes(0, 4).map_err(|_| bad())? != b"IDST" || r.i32(4).map_err(|_| bad())? != 10 {
        return Err(bad());
    }
    let table = |count_at: usize, max: usize| -> Result<(usize, usize), String> {
        let n = r.i32(count_at).map_err(|_| bad())?;
        let off = r.i32(count_at + 4).map_err(|_| bad())?;
        if n < 0 || n as usize > max || off < 0 {
            return Err("The model file is damaged.".into());
        }
        Ok((n as usize, off as usize))
    };
    let (nb, ob) = table(140, 128)?;
    let mut bones = Vec::with_capacity(nb);
    for i in 0..nb {
        bones.push(cname(r.bytes(ob + i * 112, 32).map_err(|_| "The model file is damaged.".to_string())?));
    }
    let (na, oa) = table(212, 16)?;
    let mut atts = Vec::with_capacity(na);
    for i in 0..na {
        let at = oa + i * 88;
        let name = cname(r.bytes(at, 32).map_err(|_| "The model file is damaged.".to_string())?);
        let bone = r.i32(at + 36).unwrap_or(-1);
        let on = usize::try_from(bone).ok().and_then(|b| bones.get(b)).cloned().unwrap_or_default();
        atts.push(json!({"index": i, "name": name, "bone": on}));
    }
    Ok((bones, atts))
}

fn clean(model: &str) -> Option<String> {
    let m = model.trim().replace('\\', "/").to_ascii_lowercase();
    let ok = m.starts_with("models/") && m.ends_with(".mdl") && !m.contains("..") && m.len() <= 64 && m.chars().all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));
    ok.then_some(m)
}

/// A model's points from the game's folders.
pub fn lookup(hl: Option<(&Path, &str)>, model: &str) -> Value {
    let Some(m) = clean(model) else { return json!({"error": "Pick a model file: models/...mdl."}) };
    if let Some((hl, game)) = hl {
        if let Some(p) = crate::tools::doctor::game_path(hl, game).iter().map(|d| d.join(&m)).find(|p| p.is_file()) {
            return match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| points(&b)) {
                Ok((bones, atts)) => json!({"model": m, "bones": bones, "attachments": atts}),
                Err(e) => json!({"error": e}),
            };
        }
    }
    if m == "models/player.mdl" {
        return json!({"model": m, "bones": PLAYER_BONES.to_vec(), "attachments": [], "fallback": true});
    }
    json!({"error": format!("{m} isn't in the game on this computer.")})
}

#[derive(Deserialize)]
struct Q {
    model: String,
}

async fn api_points(State(st): State<Shared>, Query(q): Query<Q>) -> Json<Value> {
    let g = st.local_game();
    let v = tokio::task::spawn_blocking(move || lookup(g.as_ref().map(|g| (g.hl.as_path(), g.name.as_str())), &q.model)).await.unwrap_or_else(|_| json!({"error": "That didn't finish."}));
    Json(v)
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/studio/model-points", get(api_points))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny model: two bones, one attachment on the second.
    fn model() -> Vec<u8> {
        let mut b = vec![0u8; 244 + 2 * 112 + 88];
        b[0..4].copy_from_slice(b"IDST");
        b[4..8].copy_from_slice(&10i32.to_le_bytes());
        b[140..144].copy_from_slice(&2i32.to_le_bytes());
        b[144..148].copy_from_slice(&244i32.to_le_bytes());
        b[212..216].copy_from_slice(&1i32.to_le_bytes());
        b[216..220].copy_from_slice(&(244i32 + 224).to_le_bytes());
        b[244..249].copy_from_slice(b"Bip01");
        b[356..368].copy_from_slice(b"Bip01 R Hand");
        b[468 + 36..468 + 40].copy_from_slice(&1i32.to_le_bytes());
        b
    }

    #[test]
    fn bones_and_attachments_are_read() {
        let (bones, atts) = points(&model()).unwrap();
        assert_eq!(bones, vec!["Bip01", "Bip01 R Hand"]);
        assert_eq!(atts, vec![json!({"index": 0, "name": "", "bone": "Bip01 R Hand"})]);
        assert!(points(b"IDSQ....").is_err());
        let mut bad = model();
        bad[140..144].copy_from_slice(&9999i32.to_le_bytes());
        assert!(points(&bad).is_err(), "a damaged count is refused");
    }

    #[test]
    fn player_skeleton_without_a_game_and_paths_are_checked() {
        let v = lookup(None, "models/player.mdl");
        assert_eq!(v["fallback"], true);
        assert!(v["bones"].as_array().unwrap().iter().any(|b| b == "Bip01 R Hand"));
        assert!(lookup(None, "../../etc/passwd").get("error").is_some());
        assert!(lookup(None, "models/v_ak47.mdl")["error"].as_str().unwrap().contains("isn't in the game"));
    }

    /// The real player model, when this computer has the game in CrossOver: the same skeleton.
    #[test]
    fn real_player_model_has_the_standard_skeleton() {
        let hl = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life");
        if !hl.join("cstrike/models/player.mdl").is_file() {
            return;
        }
        let v = lookup(Some((&hl, "cstrike")), "models/player.mdl");
        let got: Vec<&str> = v["bones"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(got, PLAYER_BONES.to_vec());
    }
}
