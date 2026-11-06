//! Loner-Strike's own plugins, the Game Mode Menu configs, bot styles and keys,
//! and compiling plugins.

use super::botprofile;
use super::foundation::tree;
use super::record::Record;
use super::{splitlines, Ctx};
use regex::Regex;
use std::collections::BTreeSet;
use std::time::Duration;

/// Modes that need nothing beyond Loner-Strike's own plugins
pub const CORE_MODES: &[&str] = &["classic", "csdm", "csdm_ffa", "knife", "pistols", "deagle", "awp", "scoutz", "surf", "match"];
/// miscstats.amxx comes with AMX Mod X
pub const CORE_ADDONS: &[&str] = &["quakesounds"];
pub const OUR_PLUGINS: &[&str] = &["gamemodes", "gm_humanbots", "gm_pubserver", "gm_deathmatch", "gm_botnav", "gm_vip", "gm_extras", "gm_community", "gm_scrim", "gm_botchat"];
/// listed in plugins.ini
pub const ALWAYS_ON: &[&str] = &["gamemodes", "gm_humanbots", "gm_pubserver", "gm_vip", "gm_extras", "gm_community", "gm_botchat"];
pub const BINDS: &[&str] = &["bind \"F3\" \"gamemenu\"", "bind \"F4\" \"gamequick\""];

const CROSSOVER_WINE: &str = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";

/// A command that runs a Windows program (the plugin compiler) for this install:
/// through CrossOver's wine on a Mac, directly on Windows.
pub fn windows_command(install: &crate::steam::Install, exe: &std::path::Path) -> std::process::Command {
    if install.kind == "crossover" {
        let mut c = crate::sys::command(CROSSOVER_WINE);
        c.arg("--bottle").arg(install.bottle.as_deref().unwrap_or("Steam")).arg("--wait-children").arg(exe);
        c
    } else {
        crate::sys::command(exe)
    }
}

/// Compiles addons/amxmodx/scripting/<src_rel> into plugins/<out_name>.amxx.
/// Ok(true) when it compiled; Ok(false) (with the reason shown) when it didn't.
pub fn compile_plugin(rec: &mut Record, ctx: &Ctx, src_rel: &str, out_name: &str, include_rel: Option<&str>) -> Result<bool, String> {
    let am = rec.game.join("addons").join("amxmodx");
    let exe = am.join("scripting").join("amxxpc.exe");
    let src = am.join("scripting").join(src_rel);
    let out_rel = format!("addons/amxmodx/plugins/{out_name}.amxx");
    let out_full = rec.prepare(&out_rel)?;
    if install_is_crossover(ctx) && !std::path::Path::new(CROSSOVER_WINE).is_file() {
        ctx.progress.note(&format!("Couldn't compile {src_rel}: CrossOver isn't installed in Applications."));
        return Ok(false);
    }
    let mut cmd = windows_command(ctx.install, &exe);
    cmd.arg(&src).arg(format!("-o{}", out_full.display()));
    if let Some(inc) = include_rel {
        cmd.arg(format!("-i{}", am.join("scripting").join(inc).display()));
    }
    let Some((_, out, err)) = crate::sys::run_output(cmd, Duration::from_secs(300)) else {
        ctx.progress.note(&format!("Couldn't compile {src_rel}: the compiler didn't run or took too long."));
        return Ok(false);
    };
    let all = format!("{out}{err}");
    if !all.contains("Done.") {
        let errors: Vec<&str> = all.lines().filter(|l| l.to_lowercase().contains("error")).take(5).collect();
        let why = if errors.is_empty() {
            let t = all.trim();
            // the last 300 characters (not bytes: the output can hold any character)
            let from = t.char_indices().rev().nth(299).map(|(i, _)| i).unwrap_or(0);
            t[from..].to_string()
        } else {
            errors.join(" / ")
        };
        ctx.progress.note(&format!("Couldn't compile {src_rel}: {}", why.trim()));
        return Ok(false);
    }
    Ok(true)
}

fn install_is_crossover(ctx: &Ctx) -> bool {
    ctx.install.kind == "crossover"
}

/// Our plugin sources, compiled. Ok(false) if any didn't compile.
pub fn plugins(rec: &mut Record, ctx: &Ctx) -> Result<bool, String> {
    let src = ctx.res.join("plugins").join("scripting");
    for name in OUR_PLUGINS {
        rec.copy(&src.join(format!("{name}.sma")), &format!("addons/amxmodx/scripting/{name}.sma"))?;
    }
    let mut ok = true;
    for (i, name) in OUR_PLUGINS.iter().enumerate() {
        ctx.progress.detail(&format!("Compiling {name} ({} of {})", i + 1, OUR_PLUGINS.len()));
        ctx.progress.fraction(i as f64 / OUR_PLUGINS.len() as f64);
        ok = compile_plugin(rec, ctx, &format!("{name}.sma"), name, None)? && ok;
    }
    let pl = rec.read("addons/amxmodx/configs/plugins.ini").unwrap_or_default();
    let add: Vec<&str> = ALWAYS_ON
        .iter()
        .copied()
        .filter(|n| !Regex::new(&format!(r"(?m)^\s*{}\.amxx", regex::escape(n))).unwrap().is_match(&pl))
        .collect();
    if !add.is_empty() {
        let lines: String = add.iter().map(|n| format!("{n}.amxx\t\t; Loner-Strike\r\n")).collect();
        rec.write("addons/amxmodx/configs/plugins.ini", &format!("{}\r\n{lines}", pl.trim_end_matches(['\r', '\n'])))?;
    }
    Ok(ok)
}

/// Keeps comment lines and lines whose first word (an id) passes keep(id).
fn filter_lines(text: &str, keep: impl Fn(&str) -> bool) -> String {
    let mut out = Vec::new();
    for line in splitlines(text, false) {
        let s = line.trim();
        if s.is_empty() || s.starts_with(';') || s.starts_with("//") || keep(s.split_whitespace().next().unwrap_or("")) {
            out.push(line);
        }
    }
    out.join("\r\n") + "\r\n"
}

/// Python's str.strip() / lstrip() whitespace test for one char.
fn is_comment_or_blank(l: &str) -> bool {
    l.trim().is_empty() || l.trim_start().starts_with(';')
}

/// The Game Mode Menu's settings, limited to the modes and add-ons installed.
/// Settings of modes that are no longer installed are taken out again.
pub fn configs(rec: &mut Record, ctx: &Ctx, modes: &BTreeSet<String>, addons: &BTreeSet<String>) -> Result<(), String> {
    let cfg_src = ctx.res.join("plugins").join("configs").join("gamemodes");
    let quoted = Regex::new(r#""([^"]*)""#).unwrap();
    let mode_ok = |m: &str| modes.contains(m);
    let mut wanted = BTreeSet::new();
    for (full, rel) in tree(&cfg_src) {
        let dest = format!("addons/amxmodx/configs/gamemodes/{rel}");
        let base = rel.rsplit('/').next().unwrap_or(&rel).to_string();
        let mode_id = base.split('.').next().unwrap_or("").to_string();
        if rel.starts_with("maps/") && base != "README.txt" && !mode_ok(&mode_id) {
            continue;
        }
        if rel.starts_with("addons/") && !addons.contains(&mode_id) {
            continue;
        }
        if (base.ends_with(".plugins.ini") || (base.ends_with(".cfg") && !rel.contains('/') && base != "base.cfg")) && !mode_ok(&mode_id) {
            continue;
        }
        let text = crate::sys::read_latin1(&full).map_err(|e| format!("Couldn't read {rel} from Loner-Strike's files ({e})."))?;
        let text = match base.as_str() {
            "modes.ini" => filter_lines(&text, mode_ok),
            "addons.ini" => filter_lines(&text, |a| addons.contains(a)),
            "mappresets.ini" => {
                let keep: Vec<&str> = splitlines(&text, false)
                    .into_iter()
                    .filter(|l| is_comment_or_blank(l) || mode_ok(l.split_whitespace().last().unwrap_or("")))
                    .collect();
                keep.join("\r\n") + "\r\n"
            }
            "presets.ini" => {
                let keep: Vec<&str> = splitlines(&text, false)
                    .into_iter()
                    .filter(|l| {
                        if is_comment_or_blank(l) {
                            return true;
                        }
                        let m: Vec<&str> = quoted.captures_iter(l).map(|c| c.get(1).unwrap().as_str()).collect();
                        m.len() < 3 || mode_ok(m[2])
                    })
                    .collect();
                keep.join("\r\n") + "\r\n"
            }
            _ => text,
        };
        rec.write(&dest, &text)?;
        wanted.insert(dest);
    }
    // a mode or add-on that was removed: its settings go too
    let stale: Vec<String> = rec
        .data
        .files
        .iter()
        .filter(|(rel, info)| rel.starts_with("addons/amxmodx/configs/gamemodes/") && !wanted.contains(*rel) && info.owners().iter().any(|o| o == "core"))
        .map(|(rel, _)| rel.clone())
        .collect();
    for rel in stale {
        rec.release(&rel)?;
    }
    Ok(())
}

/// Bot styles: personality files made from the game's bot profile. CS 1.6 gets
/// Loner-Strike's own BotProfile.db first if it has none.
pub fn bot_styles(rec: &mut Record, ctx: &Ctx, game: &str) -> Result<(), String> {
    if game == "cstrike" && !rec.exists("BotProfile.db") {
        rec.write("BotProfile.db", &botprofile::generate())?;
    }
    let Some(base) = rec.read("BotProfileEnhanced.db").or_else(|| rec.read("BotProfile.db")) else {
        ctx.progress.note("No bot profile found, so the bot styles keep the game's own personalities.");
        return Ok(());
    };
    for style in botprofile::styles(ctx.res) {
        let text = format!("// Made by Loner-Strike for the {} bot style\r\n{}", style.name, botprofile::reshape(&base, &style, 27015));
        rec.write(&format!("BotProfile_{}.db", style.name), &text)?;
    }
    Ok(())
}

/// F3 opens the Game Mode Menu, F4 the quick menu.
pub fn binds(rec: &mut Record) -> Result<(), String> {
    let cfg = rec.read("config.cfg").unwrap_or_default();
    let add: Vec<&str> = BINDS.iter().copied().filter(|b| !cfg.contains(b.split('"').nth(3).unwrap_or(""))).collect();
    if !add.is_empty() {
        rec.write("config.cfg", &format!("{}\r\n{}\r\n", cfg.trim_end_matches(['\r', '\n']), add.join("\r\n")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_like_python() {
        let t = "; comment\r\n\r\nclassic \"C\"\r\nzombie \"Z\"\r\n";
        assert_eq!(filter_lines(t, |m| m == "classic"), "; comment\r\n\r\nclassic \"C\"\r\n");
    }
}
