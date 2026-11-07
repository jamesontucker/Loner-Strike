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
pub const OUR_PLUGINS: &[&str] = &["gamemodes", "gm_humanbots", "gm_pubserver", "gm_deathmatch", "gm_botnav", "gm_vip", "gm_extras", "gm_community", "gm_scrim", "gm_botchat", "gm_botping", "gm_content"];
/// listed in plugins.ini. restmenu is AMX Mod X's own (amx_restrict), which its plugins.ini
/// ships switched off: the weapon modes (knife, pistols, deagle, awp, scoutz, gm_weapons)
/// restrict buying with it, so without it they restrict nothing.
pub const ALWAYS_ON: &[&str] = &["gamemodes", "gm_humanbots", "gm_pubserver", "gm_vip", "gm_extras", "gm_community", "gm_botchat", "gm_botping", "restmenu", "gm_content"];
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
    // one folder at a time, so Windows gets backslashes throughout: amxxpc takes the
    // plugin's folder (where its own includes are) from the last backslash, and
    // scripting\basebuilder/basebuilder.sma sent it looking one folder too high
    let under = |rel: &str| rel.split('/').fold(am.join("scripting"), |p, part| p.join(part));
    let src = under(src_rel);
    let out_rel = format!("addons/amxmodx/plugins/{out_name}.amxx");
    let out_full = rec.prepare(&out_rel)?;
    if install_is_crossover(ctx) && !std::path::Path::new(CROSSOVER_WINE).is_file() {
        ctx.progress.note(&format!("Couldn't compile {src_rel}: CrossOver isn't installed in Applications."));
        return Ok(false);
    }
    let compile = || {
        let mut cmd = windows_command(ctx.install, &exe);
        cmd.arg(&src).arg(format!("-o{}", out_full.display()));
        if let Some(inc) = include_rel {
            cmd.arg(format!("-i{}", under(inc).display()));
        }
        // the biggest plugins take about 10 seconds; the limit is only for a real hang, with
        // room for a slow machine
        crate::sys::run_output(cmd, Duration::from_secs(300))
    };
    // a compiler that crashed or hung (no "Done." and no errors of the code's own) gets
    // one more go: under Wine that's usually a one-off
    let mut ran = compile();
    let crashed = |r: &Option<(bool, String, String)>| match r {
        None => true,
        Some((_, out, err)) => {
            let all = format!("{out}{err}");
            !all.contains("Done.") && !all.to_lowercase().contains("error")
        }
    };
    if crashed(&ran) {
        ctx.progress.note(&format!("The compiler stopped part way through {src_rel}; trying it once more."));
        ran = compile();
    }
    let Some((_, out, err)) = ran else {
        ctx.progress.note(&format!("Couldn't compile {src_rel}: the compiler didn't run or took too long."));
        return Ok(false);
    };
    // CrossOver's own chatter (graphics setup, "fixme" notes) isn't the compiler's
    let all: String = format!("{out}{err}").lines().filter(|l| !is_wine_noise(l)).collect::<Vec<_>>().join("\n");
    if !all.contains("Done.") {
        // the compiler's whole say, for Copy diagnostics (the note below is short)
        let tail: String = all.chars().rev().take(3000).collect::<Vec<_>>().into_iter().rev().collect();
        crate::diagnostics::log_error(&format!("compile {src_rel}"), &tail);
    }
    if crashed(&Some((false, all.clone(), String::new()))) {
        ctx.progress.note(&format!(
            "Couldn't compile {src_rel}: the compiler crashed{}. Run full setup again to try it once more.",
            if install_is_crossover(ctx) { " (this happens now and then under CrossOver)" } else { "" }
        ));
        return Ok(false);
    }
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

/// A line Wine or CrossOver printed while running the compiler, not the compiler itself.
fn is_wine_noise(line: &str) -> bool {
    let l = line.trim();
    l.is_empty()
        || l.contains("fixme:")
        || l.contains("err:")
        || l.contains("warn:")
        || l.contains("[mvk-")
        || l.contains("MoltenVK")
        || l.contains("Vulkan")
        || l.starts_with("VK_")
        || l.contains("macOS GPU Family")
        || l.starts_with("wine:")
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
/// The Quake sounds announcements miscstats plays, switched on in stats.ini: the sounds
/// and the HUD lines that go with them. (Bomb and grenade ones stay off: their sounds
/// aren't in the game.)
pub const QUAKE_ON: &[&str] = &[
    "MultiKill", "MultiKillSound", "KillingStreak", "KillingStreakSound", "DoubleKill", "DoubleKillSound", "FirstBloodSound",
    "HeadShotKill", "HeadShotKillSound", "KnifeKill", "KnifeKillSound", "LastMan", "LastManSound", "RoundCounter", "RoundCounterSound",
];

/// stats.ini with the Quake sounds announcements switched on, the rest kept as it is
/// (statscfg.amxx writes this file too: one setting per line, ; for comments).
pub fn stats_ini_with_quake(text: &str) -> String {
    let on: BTreeSet<String> = splitlines(text, false).iter().filter(|l| !is_comment_or_blank(l)).filter_map(|l| l.split_whitespace().next()).map(String::from).collect();
    let mut lines: Vec<String> = splitlines(text, false).iter().map(|l| l.to_string()).collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    for name in QUAKE_ON.iter().filter(|n| !on.contains(**n)) {
        lines.push(name.to_string());
    }
    lines.join("\r\n") + "\r\n"
}

/// The announcers to choose from (per game or server, on the Sounds page): id, its
/// miscstats.ini in plugins/configs/quake.
pub const ANNOUNCERS: &[(&str, &str)] = &[("classic", "miscstats.ini"), ("alt", "miscstats-alt.ini")];

/// The announcer a game or server uses: data/gamemodes/announcer.txt, classic without it.
pub fn announcer_of(game_dir: &std::path::Path) -> &'static str {
    let saved = std::fs::read_to_string(game_dir.join("addons/amxmodx/data/gamemodes/announcer.txt")).unwrap_or_default();
    ANNOUNCERS.iter().map(|(id, _)| *id).find(|id| *id == saved.trim()).unwrap_or("classic")
}

/// An announcer's miscstats.ini, with Windows line ends (as AMX Mod X ships it).
pub fn announcer_ini(res: &std::path::Path, which: &str) -> Result<String, String> {
    let file = ANNOUNCERS.iter().find(|(id, _)| *id == which).map(|(_, f)| *f).ok_or("Unknown announcer.")?;
    let text = crate::sys::read_latin1(&res.join("plugins").join("configs").join("quake").join(file)).map_err(|e| format!("Loner-Strike's files are incomplete ({file}: {e})."))?;
    Ok(text.replace("\r\n", "\n").replace('\n', "\r\n"))
}

/// Quake sounds (AMX Mod X's miscstats): the announcer's sounds, its miscstats.ini naming
/// them, and the announcements switched on. AMX Mod X ships all of them off and CS 1.6
/// without the sounds, so without this it says nothing. Servers get the sounds like any
/// mode's (players download them).
pub fn quake_sounds(rec: &mut Record, ctx: &Ctx) -> Result<(), String> {
    let dir = ctx.res.join("plugins").join("configs").join("quake");
    let list: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("sounds.json")).map_err(|e| format!("Loner-Strike's files are incomplete (quake sounds: {e})."))?)
        .map_err(|e| format!("Loner-Strike's files are damaged (quake sounds: {e})."))?;
    let tmp = super::TempDir::new()?;
    super::components::fetch_all(rec, ctx, &serde_json::json!({"name": "Quake sounds", "fetch": list["fetch"]}), &tmp)?;
    let ini = announcer_ini(ctx.res, announcer_of(&rec.game))?;
    rec.write("addons/amxmodx/configs/miscstats.ini", &ini)?;
    let stats = rec.read("addons/amxmodx/configs/stats.ini").unwrap_or_default();
    rec.write("addons/amxmodx/configs/stats.ini", &stats_ini_with_quake(&stats))
}

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

/// game_init.cfg with bot_enable set to 1, or None if it already is.
pub fn enable_bots(cfg: &str) -> Option<String> {
    let re = Regex::new(r#"(?m)^[ \t]*bot_enable[ \t]+"?(\d+)"?[ \t]*\r?$"#).unwrap();
    match re.captures(cfg) {
        Some(c) if &c[1] == "1" => None,
        Some(c) => {
            let m = c.get(0).unwrap();
            let eol = if m.as_str().ends_with('\r') { "\r" } else { "" };
            Some(format!("{}bot_enable \"1\"{eol}{}", &cfg[..m.start()], &cfg[m.end()..]))
        }
        None => Some(format!("{}\r\nbot_enable \"1\"\r\n", cfg.trim_end_matches(['\r', '\n']))),
    }
}

/// BotChatter.db for CS 1.6 without Condition Zero (see bot_styles).
const BOT_CHATTER: &str = include_str!("botchatter.db");

/// Bot styles: personality files made from the game's bot profile. CS 1.6 gets
/// Loner-Strike's own BotProfile.db first if it has none.
pub fn bot_styles(rec: &mut Record, ctx: &Ctx, game: &str) -> Result<(), String> {
    if game == "cstrike" && !rec.exists("BotProfile.db") {
        rec.write("BotProfile.db", &botprofile::generate())?;
    }
    // CS 1.6 has no BotChatter.db (the bots' place names and chatter phrases). Without one,
    // ReGameDLL's bots crash hl.exe on real Windows soon after they join (Wine on the Mac
    // happens to survive it). Condition Zero's when that's installed, else Loner-Strike's
    // own: the same place and phrase names with no voice lines. CS 1.6 has none of CZ's
    // radio\bot sounds, so both sound the same (the bots use the normal radio).
    // (CrossOver runs the same Windows game: it gets one too)
    if (cfg!(windows) || install_is_crossover(ctx)) && game == "cstrike" && !rec.exists("BotChatter.db") {
        let cz = rec.game.parent().map(|hl| hl.join("czero").join("BotChatter.db"));
        match cz.filter(|p| p.is_file()) {
            Some(p) => rec.copy(&p, "BotChatter.db")?,
            None => rec.write("BotChatter.db", BOT_CHATTER)?,
        }
    }
    // ReGameDLL only lets bots into a CS 1.6 dedicated server with bot_enable 1 in
    // game_init.cfg, read once as the server starts (a listen server always has them)
    if game == "cstrike" {
        if let Some(cfg) = rec.read("game_init.cfg") {
            if let Some(on) = enable_bots(&cfg) {
                rec.write("game_init.cfg", &on)?;
            }
        }
    }
    let Some(base) = rec.read("BotProfileEnhanced.db").or_else(|| rec.read("BotProfile.db")) else {
        ctx.progress.note("No bot profile found, so the bot styles keep the game's own personalities.");
        return Ok(());
    };
    // one file per style and way of playing together (configs/gamemodes/botplay.ini picks)
    for style in botprofile::styles(ctx.res) {
        for flavour in botprofile::Flavour::ALL {
            let text = format!(
                "// Made by Loner-Strike for the {} bot style ({:?} play)\r\n{}",
                style.name,
                flavour,
                botprofile::reshape(&base, &style, flavour, 27015)
            );
            rec.write(&botprofile::style_file(&style.name, flavour), &text)?;
        }
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

#[cfg(test)]
mod noise_tests {
    #[test]
    fn wine_chatter_is_not_the_compilers() {
        for l in ["103c:fixme:thread:get_thread_times not implemented", "[mvk-info] Created VkInstance for Vulkan version 1.0.0", "VK_KHR_external_memory_capabilities v1", "macOS GPU Family 1 v1"] {
            assert!(super::is_wine_noise(l), "{l}");
        }
        for l in ["basebuilder.sma(120) : error 017: undefined symbol", "Done."] {
            assert!(!super::is_wine_noise(l), "{l}");
        }
    }
}
