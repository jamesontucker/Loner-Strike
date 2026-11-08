//! ls-control's version is Loner-Strike's: the [package] version in app/src-tauri/Cargo.toml.
//! Helpers' apps must have the same one, as with the owner's app.

fn main() {
    let manifest = "../src-tauri/Cargo.toml";
    println!("cargo:rerun-if-changed={manifest}");
    let text = std::fs::read_to_string(manifest).expect("app/src-tauri/Cargo.toml (build from the repo checkout)");
    let mut in_package = false;
    let mut version = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_package = t == "[package]";
            continue;
        }
        if in_package {
            if let Some(v) = t.strip_prefix("version").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
                version = Some(v.trim().trim_matches('"').to_string());
                break;
            }
        }
    }
    let v = version.expect("a [package] version in app/src-tauri/Cargo.toml");
    assert!(v.split('.').count() == 3 && v.chars().all(|c| c.is_ascii_digit() || c == '.'), "odd version {v}");
    println!("cargo:rustc-env=LS_VERSION={v}");
}
