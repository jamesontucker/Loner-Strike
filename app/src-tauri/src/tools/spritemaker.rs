//! The Sprite maker (T1fix): pictures in, a GoldSrc .spr out (goldsrc/sprwrite.rs).
//!   POST /api/tools/sprites/build  {frames: [base64 or data: URL of a PNG, JPEG, BMP, TGA or WebP],
//!                                  width, height, orientation (0-4), format (normal | additive |
//!                                  indexalpha | alphatest), fit (contain | cover | stretch),
//!                                  crop?: {x, y, w, h} (fractions of each picture), key?: "#rrggbb"
//!                                  (a color made clear), tint?: "#rrggbb"}
//!   → {ok, data (the .spr, base64), size, colors, frames, preview: [PNG data: URLs, as the game
//!     draws each frame]} | {error}
//! Nothing is written here: the page saves the result with the pickers' Import
//! (/api/tools/assets/import, the game's sprites/custom/) or the Content Browser's
//! (/api/tools/editor/content/import, the open map project), so the checks, the no-overwrite rule
//! and the rename offer are theirs.

use crate::goldsrc::spr::{self, TexFormat};
use crate::goldsrc::sprwrite::{self, Fit, Options};
use crate::server::Shared;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use image::RgbaImage;
use serde::Deserialize;
use serde_json::{json, Value};

/// The biggest picture side read in (pictures are scaled down to the sprite anyway).
const MAX_PICTURE: u32 = 4096;
/// The most bytes of pictures in one request.
const MAX_BODY: usize = 64 << 20;

#[derive(Deserialize, Default, Clone, Copy)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Deserialize)]
pub struct BuildReq {
    pub frames: Vec<String>,
    pub width: u32,
    pub height: u32,
    #[serde(default = "parallel")]
    pub orientation: i32,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub fit: String,
    #[serde(default)]
    pub crop: Option<Crop>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub tint: Option<String>,
}

fn parallel() -> i32 {
    2
}

fn hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(std::array::from_fn(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap_or(0)))
}

/// One picture from the page (a data: URL or bare base64), refusing huge or unreadable ones.
pub fn picture(data: &str) -> Result<RgbaImage, String> {
    let b64 = data.split_once("base64,").map_or(data, |x| x.1);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim().as_bytes()).map_err(|_| "A picture couldn't be read.".to_string())?;
    let mut r = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|_| "A picture couldn't be read.".to_string())?;
    if r.format().is_none() {
        return Err("That isn't a picture the Sprite maker reads. Use PNG, JPEG, BMP, TGA or WebP.".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_PICTURE);
    limits.max_image_height = Some(MAX_PICTURE);
    r.limits(limits);
    Ok(r
        .decode()
        .map_err(|e| match e {
            image::ImageError::Limits(_) => format!("A picture is too big: {MAX_PICTURE} pixels each way at most."),
            _ => "A picture looks damaged. Save it again, or pick another.".to_string(),
        })?
        .to_rgba8())
}

/// The part of a picture the crop keeps (fractions, clamped; the whole picture when absent).
fn cropped(img: RgbaImage, c: Option<Crop>) -> RgbaImage {
    let Some(c) = c else { return img };
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x = (c.x.clamp(0.0, 1.0) * w).floor() as u32;
    let y = (c.y.clamp(0.0, 1.0) * h).floor() as u32;
    let cw = ((c.w.clamp(0.0, 1.0) * w).round() as u32).clamp(1, img.width().saturating_sub(x).max(1));
    let ch = ((c.h.clamp(0.0, 1.0) * h).round() as u32).clamp(1, img.height().saturating_sub(y).max(1));
    if x == 0 && y == 0 && cw == img.width() && ch == img.height() {
        return img;
    }
    image::imageops::crop_imm(&img, x, y, cw, ch).to_image()
}

/// The sprite for a request, and how the game would draw each frame.
pub fn build(req: &BuildReq) -> Result<Value, String> {
    if let Some(p) = sprwrite::size_problem(req.width, req.height, req.frames.len()) {
        return Err(p);
    }
    let format = match req.format.as_str() {
        "" | "normal" => TexFormat::Normal,
        "additive" => TexFormat::Additive,
        "indexalpha" => TexFormat::IndexAlpha,
        "alphatest" => TexFormat::AlphaTest,
        _ => return Err("Pick a render format: normal, additive, index alpha or alpha test.".into()),
    };
    if !(0..=4).contains(&req.orientation) {
        return Err("Pick how it faces: 0 to 4.".into());
    }
    let fit = match req.fit.as_str() {
        "" | "contain" => Fit::Contain,
        "cover" => Fit::Cover,
        "stretch" => Fit::Stretch,
        _ => return Err("Pick how pictures fit: contain, cover or stretch.".into()),
    };
    let key = match req.key.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => Some(hex(k).ok_or("The clear color isn't a color (#rrggbb).")?),
        None => None,
    };
    let tint = match req.tint.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => hex(k).ok_or("The color isn't a color (#rrggbb).")?,
        None => [0, 0, 255],
    };
    let pictures: Vec<RgbaImage> = req.frames.iter().map(|f| picture(f).map(|p| cropped(p, req.crop))).collect::<Result<_, _>>()?;
    let o = Options { width: req.width, height: req.height, orientation: req.orientation, format, fit, key, tint };
    let made = sprwrite::make(&pictures, &o)?;
    // the preview is the file read back, so what you see is what the game gets
    let s = spr::parse(&made.bytes, false).map_err(|e| format!("The sprite came out wrong ({e})."))?;
    let mut preview = Vec::new();
    for i in 0..s.frames.len() {
        let (w, h, px) = s.rgba(i).ok_or("The sprite came out wrong.")?;
        let img = RgbaImage::from_raw(w, h, px).ok_or("The sprite came out wrong.")?;
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
        preview.push(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png)));
    }
    Ok(json!({
        "ok": true,
        "data": base64::engine::general_purpose::STANDARD.encode(&made.bytes),
        "size": made.bytes.len(),
        "colors": made.colors,
        "frames": s.frames.len(),
        "width": s.width,
        "height": s.height,
        "preview": preview,
    }))
}

async fn api_build(h: HeaderMap, Json(req): Json<BuildReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let r = tokio::task::spawn_blocking(move || build(&req)).await.unwrap_or_else(|_| Err("The sprite couldn't be made.".into()));
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/sprites/build", post(api_build).layer(axum::extract::DefaultBodyLimit::max(MAX_BODY / 3 * 4 + (1 << 20))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> String {
        let img = RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&out))
    }

    fn req(frames: Vec<String>) -> BuildReq {
        BuildReq { frames, width: 32, height: 32, orientation: 3, format: "alphatest".into(), fit: "contain".into(), crop: None, key: None, tint: Some("#0000ff".into()) }
    }

    #[test]
    fn builds_a_sprite_with_its_preview() {
        let a = png(40, 20, |x, _| if x < 20 { [255, 128, 0, 255] } else { [0, 0, 0, 0] });
        let b = png(40, 20, |_, y| if y < 10 { [0, 200, 255, 255] } else { [0, 0, 0, 0] });
        let v = build(&req(vec![a.clone(), b])).unwrap();
        assert_eq!((v["frames"].as_u64(), v["width"].as_u64(), v["height"].as_u64()), (Some(2), Some(32), Some(32)));
        assert_eq!(v["preview"].as_array().unwrap().len(), 2);
        let bytes = base64::engine::general_purpose::STANDARD.decode(v["data"].as_str().unwrap()).unwrap();
        let s = spr::parse(&bytes, false).unwrap();
        assert_eq!((s.orientation, s.format), (3, TexFormat::AlphaTest));
        // contain: the 2:1 picture leaves clear bands above and below
        assert_eq!(s.frames[0].pixels[0], 255);
        // a crop to the left half: all solid orange
        let mut r = req(vec![a]);
        r.crop = Some(Crop { x: 0.0, y: 0.0, w: 0.5, h: 1.0 });
        r.fit = "stretch".into();
        let s = spr::parse(&base64::engine::general_purpose::STANDARD.decode(build(&r).unwrap()["data"].as_str().unwrap()).unwrap(), false).unwrap();
        assert!(s.frames[0].pixels.iter().all(|&i| i != 255));
    }

    #[test]
    fn refuses_bad_input() {
        let ok = png(8, 8, |_, _| [9, 9, 9, 255]);
        let bad = |f: &dyn Fn(&mut BuildReq)| {
            let mut r = req(vec![ok.clone()]);
            f(&mut r);
            build(&r).unwrap_err()
        };
        assert!(bad(&|r| r.width = 30).contains("multiple of 8"));
        assert!(bad(&|r| r.height = 1024).contains("multiple of 8"));
        assert!(bad(&|r| r.frames.clear()).contains("Add a picture"));
        assert!(bad(&|r| r.frames = vec![ok.clone(); 65]).contains("64 frames"));
        assert!(bad(&|r| r.format = "glow".into()).contains("render format"));
        assert!(bad(&|r| r.orientation = 7).contains("faces"));
        assert!(bad(&|r| r.key = Some("red".into())).contains("clear color"));
        assert!(bad(&|r| r.frames = vec!["data:image/png;base64,AAAA".into()]).contains("picture"));
        assert!(bad(&|r| r.frames = vec!["@@@".into()]).contains("couldn't be read"));
    }
}
