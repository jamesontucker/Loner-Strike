use std::io::Write;
use std::path::Path;

fn main() {
    // the Linux bundle carries the programs it sends to Linux servers gzipped
    // (tauri.linux.conf.json: the AppImage's packager rewrites bundled programs, which would
    // break their checksums; sys::read_shipped unpacks them)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        for p in ["ls-control", "ls-voice", "ls-intent"] {
            gzip_program(&Path::new("resources/linux").join(p), p);
        }
    }
    tauri_build::build()
}

/// <dir>/<name>.gz from <dir>/<name> when that's newer; without the program (a checkout that
/// hasn't built it: scripts/build-server-agent.sh, build-voice-agent.sh) an empty .gz and
/// .sha256, which the app reports as the program not being included, so the build still works
fn gzip_program(dir: &Path, name: &str) {
    let src = dir.join(name);
    let gz = dir.join(format!("{name}.gz"));
    let sum = dir.join(format!("{name}.sha256"));
    println!("cargo:rerun-if-changed={}", src.display());
    let _ = std::fs::create_dir_all(dir);
    let stale = |a: &Path, b: &Path| match (a.metadata().and_then(|m| m.modified()), b.metadata().and_then(|m| m.modified())) {
        (Ok(a), Ok(b)) => a > b,
        (Ok(_), Err(_)) => true,
        _ => false,
    };
    let data = std::fs::read(&src).unwrap_or_default();
    if src.is_file() && !stale(&src, &gz) {
        return;
    }
    if !src.is_file() && gz.is_file() {
        return;
    }
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(&data).expect("gzip");
    std::fs::write(&gz, e.finish().expect("gzip")).expect("writing the .gz");
    if !sum.is_file() {
        std::fs::write(&sum, "").expect("writing the .sha256");
    }
}
