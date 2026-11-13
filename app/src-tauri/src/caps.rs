//! [Caps] Shared size limits for things people hand the app: pictures (by pixels, not just
//! bytes, so a small file can't decode into gigabytes), files read whole into memory, and
//! answers from other computers. Each error says what was too big and what the most is.
//! The full list of limits, and the real files they were checked against: docs/notes/Caps.md.

use std::io::Read;
use std::path::Path;

/// The biggest picture side the app decodes from anyone's file. 8192 covers an 8K screenshot
/// (7680x4320); the game's own pictures are 1024 or smaller.
pub const MAX_PICTURE_SIDE: u32 = 8192;
/// Memory one decoded picture may take (8192x8192 RGBA is 256 MB).
pub const MAX_PICTURE_ALLOC: u64 = 300 << 20;

/// Plain size for messages: "48 MB", "900 KB", "12 bytes".
pub fn size(n: u64) -> String {
    if n >= 10 << 20 {
        format!("{} MB", n >> 20)
    } else if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / (1 << 20) as f64)
    } else if n >= 1 << 10 {
        format!("{} KB", n >> 10)
    } else {
        format!("{n} bytes")
    }
}

/// Reads a whole file, refusing it before reading when it's over `max`.
/// `what` names it for the message: "That {what} is too big: 80 MB — the most is 64 MB."
pub fn read_file(path: &Path, what: &str, max: u64) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map_err(|_| format!("That {what} couldn't be opened."))?.len();
    if len > max {
        return Err(format!("That {what} is too big: {} — the most is {}.", size(len), size(max)));
    }
    let mut out = Vec::with_capacity(len as usize);
    // (take: the file may grow between the check and the read)
    std::fs::File::open(path).and_then(|f| f.take(max + 1).read_to_end(&mut out)).map_err(|_| format!("That {what} couldn't be read."))?;
    if out.len() as u64 > max {
        return Err(format!("That {what} is too big — the most is {}.", size(max)));
    }
    Ok(out)
}

/// `read_file` as text (lossy UTF-8).
pub fn read_text(path: &Path, what: &str, max: u64) -> Result<String, String> {
    read_file(path, what, max).map(|b| String::from_utf8(b).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// At most `max` bytes from a reader (a network answer, a child process); more is an error.
pub fn read_up_to(r: impl Read, what: &str, max: u64) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    r.take(max + 1).read_to_end(&mut out).map_err(|_| format!("The {what} couldn't be read."))?;
    if out.len() as u64 > max {
        return Err(format!("The {what} was too big — the most is {}.", size(max)));
    }
    Ok(out)
}

/// `read_up_to` as text (lossy UTF-8).
pub fn text_up_to(r: impl Read, what: &str, max: u64) -> Result<String, String> {
    read_up_to(r, what, max).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// Decodes a picture with a pixel limit checked from its header before any pixels are
/// decoded. `format` None guesses from the bytes (TGA has no magic, so pass it for TGA).
pub fn decode_picture(bytes: &[u8], format: Option<image::ImageFormat>, max_side: u32) -> Result<image::DynamicImage, String> {
    let cur = std::io::Cursor::new(bytes);
    let mut r = match format {
        Some(f) => image::ImageReader::with_format(cur, f),
        None => image::ImageReader::new(cur).with_guessed_format().map_err(|_| "That picture couldn't be read.".to_string())?,
    };
    if r.format().is_none() {
        return Err("That isn't a picture Loner-Strike can read. Use a PNG, JPEG, BMP, TGA or WebP file.".into());
    }
    // the size from the header first, so the message can say how big it is
    let (w, h) = {
        let cur = std::io::Cursor::new(bytes);
        let probe = match format {
            Some(f) => image::ImageReader::with_format(cur, f),
            None => image::ImageReader::new(cur).with_guessed_format().map_err(|_| "That picture couldn't be read.".to_string())?,
        };
        probe.into_dimensions().map_err(|_| "That picture looks damaged.".to_string())?
    };
    if w > max_side || h > max_side {
        return Err(format!("That picture is too big: {w}×{h} — the most is {max_side}×{max_side}."));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_side);
    limits.max_image_height = Some(max_side);
    limits.max_alloc = Some(MAX_PICTURE_ALLOC);
    r.limits(limits);
    r.decode().map_err(|e| match e {
        image::ImageError::Limits(_) => format!("That picture is too big: {w}×{h} — it needs too much memory to open."),
        _ => "That picture looks damaged. Try saving it again, or pick another.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = Vec::new();
        image::GrayImage::new(w, h).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
        b
    }

    #[test]
    fn huge_picture_refused_from_its_header_with_its_size() {
        // 9000x9000 grey compresses to a few KB but would decode to 81 MB+ (RGBA: 324 MB)
        let b = png(9000, 9000);
        assert!(b.len() < 1 << 20, "a small file: {}", b.len());
        let e = decode_picture(&b, None, MAX_PICTURE_SIDE).unwrap_err();
        assert_eq!(e, "That picture is too big: 9000×9000 — the most is 8192×8192.");
    }

    #[test]
    fn normal_pictures_open() {
        // an 8K screenshot and a 1024x768 overview are fine
        assert_eq!(decode_picture(&png(7680, 4320), None, MAX_PICTURE_SIDE).unwrap().width(), 7680);
        let mut tga = Vec::new();
        image::RgbImage::new(1024, 768).write_to(&mut std::io::Cursor::new(&mut tga), image::ImageFormat::Tga).unwrap();
        assert_eq!(decode_picture(&tga, Some(image::ImageFormat::Tga), MAX_PICTURE_SIDE).unwrap().height(), 768);
    }

    #[test]
    fn not_a_picture() {
        assert!(decode_picture(b"hello there", None, 64).unwrap_err().contains("isn't a picture"));
    }

    #[test]
    fn read_file_refuses_before_reading() {
        let d = std::env::temp_dir().join(format!("caps-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("x.bin");
        std::fs::write(&p, vec![0u8; 3 << 20]).unwrap();
        assert_eq!(read_file(&p, "file", 2 << 20).unwrap_err(), "That file is too big: 3.0 MB — the most is 2.0 MB.");
        assert_eq!(read_file(&p, "file", 3 << 20).unwrap().len(), 3 << 20);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn read_up_to_stops_at_the_cap() {
        let big = vec![b'a'; 1001];
        assert!(read_up_to(&big[..], "answer", 1000).unwrap_err().contains("the most is 1000 bytes"));
        assert_eq!(text_up_to(&big[..1000], "answer", 1000).unwrap().len(), 1000);
    }

    // ---- community publish ----

    fn b64(b: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(b)
    }

    #[test]
    fn publish_screenshot_bomb_refused_real_one_kept() {
        use crate::tools::community::{decode_shot, Shot};
        let e = decode_shot(&Shot { name: "a.png".into(), data: b64(&png(9000, 9000)) }).unwrap_err();
        assert_eq!(e, "A screenshot is too big: 9000×9000 — the most is 8192×8192.");
        assert_eq!(decode_shot(&Shot { name: "b.png".into(), data: b64(&png(1920, 1080)) }).unwrap().1, "png");
    }

    #[test]
    fn publish_package_message() {
        let e = crate::tools::community::package_too_big(300 << 20);
        assert!(e.starts_with("The package is 300 MB — the most is 256 MB"), "{e}");
    }

    #[tokio::test]
    async fn publish_route_takes_real_screenshots() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use std::sync::{Arc, Mutex, OnceLock, RwLock};
        use tower::ServiceExt;
        let st = Arc::new(crate::server::AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let post = |len: usize| {
            let body = format!("{{\"title\":\"{}\"}}", "a".repeat(len));
            Request::builder().method("POST").uri("/api/tools/community/publish/de_x").header("x-panel", "1").header("content-type", "application/json").body(Body::from(body)).unwrap()
        };
        // six 3 MB screenshots' worth (24 MB of base64) gets past the body limit (to "no game")
        let r = crate::tools::community::routes().with_state(st.clone()).oneshot(post(24 << 20)).await.unwrap();
        assert_ne!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let r = crate::tools::community::routes().with_state(st).oneshot(post(crate::tools::community::PUBLISH_BODY + 1)).await.unwrap();
        assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    // ---- rcon ----

    #[test]
    fn rcon_flood_stops_at_the_cap() {
        use std::net::UdpSocket;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let srv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = srv.local_addr().unwrap().port();
        let done = Arc::new(AtomicBool::new(false));
        let d2 = done.clone();
        let t = std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            let (_, from) = srv.recv_from(&mut buf).unwrap();
            srv.send_to(b"\xff\xff\xff\xffchallenge rcon 12345\n", from).unwrap();
            let _ = srv.recv_from(&mut buf).unwrap();
            // a server that never stops answering
            let mut pkt = b"\xff\xff\xff\xffl".to_vec();
            pkt.extend(std::iter::repeat(b'A').take(1300));
            while !d2.load(Ordering::Relaxed) {
                if srv.send_to(&pkt, from).is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        });
        let started = std::time::Instant::now();
        let r = crate::rcon::Rcon::new("pw".into(), port).send("status");
        done.store(true, Ordering::Relaxed);
        t.join().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(25), "{:?}", started.elapsed());
        let text = r.unwrap_or_default();
        assert!(text.len() <= 4 << 20, "{}", text.len());
    }

    // ---- MCP stdio bridge ----

    #[test]
    fn mcp_line_cap_drops_the_long_line_and_keeps_going() {
        use crate::mcp::stdio::{read_line_capped, Line};
        let mut input = Vec::new();
        input.extend(std::iter::repeat(b'x').take(300_000));
        input.extend(b"\n{\"ok\":1}\r\nlast");
        let mut r = std::io::BufReader::new(&input[..]);
        assert!(matches!(read_line_capped(&mut r, 1000), Line::TooLong));
        assert!(matches!(read_line_capped(&mut r, 1000), Line::Text(t) if t == "{\"ok\":1}"));
        assert!(matches!(read_line_capped(&mut r, 1000), Line::Text(t) if t == "last"));
        assert!(matches!(read_line_capped(&mut r, 1000), Line::End));
        // exactly at the cap is fine
        let exact = format!("{}\n", "y".repeat(1000));
        assert!(matches!(read_line_capped(&mut std::io::BufReader::new(exact.as_bytes()), 1000), Line::Text(t) if t.len() == 1000));
    }

    // ---- other pictures ----

    #[test]
    fn prefab_thumbnail_bomb_refused() {
        let e = crate::editor::prefab::check_png(&png(5000, 5000)).unwrap_err();
        assert_eq!(e, "The thumbnail is too big: 5000×5000 — the most is 1024×1024.");
        assert!(crate::editor::prefab::check_png(&png(256, 192)).is_ok());
    }

    #[test]
    fn map_picture_convert_refuses_pixel_bombs() {
        let d = std::env::temp_dir().join(format!("caps-conv-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("big.png"), png(9000, 9000)).unwrap();
        let e = crate::maps::convert(&d.join("big.png"), &d.join("out.png"), crate::maps::Shrink::Width(530)).unwrap_err();
        assert_eq!(e, "That picture is too big: 9000×9000 — the most is 8192×8192.");
        std::fs::write(d.join("ok.png"), png(1920, 1080)).unwrap();
        crate::maps::convert(&d.join("ok.png"), &d.join("out.png"), crate::maps::Shrink::Width(530)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}
