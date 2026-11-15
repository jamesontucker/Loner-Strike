//! Test run: the third check, in the real world. The mode is compiled with the debugger into a
//! hidden server of its own (the Map simulator's way of making one: a copy of one of the
//! player's servers, else a temporary one from the game's files), which plays it with bots for
//! about a minute and a half: a stretch without the mode first (the cost baseline), then the
//! mode, a round restart and a map change. Then it reads what happened: did the plugin load,
//! runtime errors (mapped back to nodes), a crash, the server's cost against the baseline, and
//! which nodes ran. It never touches the player's game or an open game, and works with either
//! open; the hidden server is stopped and tidied afterward.

use super::{gen, graph, Graph};
use crate::server::Shared;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The test run's own hidden server (beside the Map simulator's `.health-check`)
const HIDDEN: &str = ".studio-test";
const MARK: &str = ".loner-strike-studio-test";
const BOTS: u32 = 8;
/// seconds: the baseline, the mode, after the round restart, after the map change
const BASELINE: u64 = 20;
const PLAY: u64 = 45;
const AFTER_RESTART: u64 = 15;
const AFTER_CHANGE: u64 = 20;
/// stock maps tried, in order (the ones with a nav are used)
const MAPS: [&str; 8] = ["de_dust2", "cs_italy", "de_inferno", "de_nuke", "cs_office", "de_aztec", "de_train", "de_dust"];
/// more node runs a second than this: heavy for a server
const HEAVY_RUNS: f64 = 1500.0;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub label: String,
    /// unix ms
    pub at: u64,
    pub secs: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRun {
    pub mode: String,
    /// "running", then "done", "failed" or "stopped"
    pub phase: &'static str,
    pub steps: Vec<Step>,
    pub started: u64,
    /// the summary (when done): verdict, problems, cost, ran / never ran, log
    pub result: Option<Value>,
    pub error: Option<String>,
}

static RUN: Mutex<Option<TestRun>> = Mutex::new(None);
static STOP: AtomicBool = AtomicBool::new(false);
const STOPPED: &str = "stopped";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn step(label: impl Into<String>) {
    if let Some(r) = RUN.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let now = now_ms();
        if let Some(last) = r.steps.last_mut() {
            last.secs.get_or_insert(((now - last.at) as f64 / 100.0).round() / 10.0);
        }
        r.steps.push(Step { label: label.into(), at: now, secs: None });
    }
}

fn stopped() -> Result<(), String> {
    if STOP.load(Ordering::SeqCst) {
        Err(STOPPED.into())
    } else {
        Ok(())
    }
}

/// Waits `secs`, sampling the server's `stats` every 2 s (cpu %, fps). Stops early on Cancel or
/// when the server is gone.
fn sample(rcon: &crate::rcon::Rcon, child: &mut std::process::Child, secs: u64, out: &mut Vec<(f64, f64, f64)>) -> Result<(), String> {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        std::thread::sleep(Duration::from_secs(2));
        stopped()?;
        if matches!(child.try_wait(), Ok(Some(_))) {
            return Err("crashed".into());
        }
        if let Ok(text) = rcon.send("stats") {
            if let Some(s) = parse_stats(&text) {
                out.push(s);
            }
        }
    }
    Ok(())
}

/// `stats`: "CPU In Out Uptime Users FPS Players" and a line of numbers: (cpu, fps, players).
pub fn parse_stats(text: &str) -> Option<(f64, f64, f64)> {
    let mut lines = text.lines().skip_while(|l| !l.trim_start().starts_with("CPU"));
    lines.next()?;
    let nums: Vec<f64> = lines.next()?.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    (nums.len() >= 7).then(|| (nums[0], nums[5], nums[6]))
}

/// Waits for the server to answer (after a start or a map change), up to `secs`.
fn wait_up(rcon: &crate::rcon::Rcon, child: &mut std::process::Child, secs: u64) -> Result<(), String> {
    let end = Instant::now() + Duration::from_secs(secs);
    // (a map change: give it a moment to go down first)
    std::thread::sleep(Duration::from_secs(2));
    while Instant::now() < end {
        stopped()?;
        if matches!(child.try_wait(), Ok(Some(_))) {
            return Err("crashed".into());
        }
        if rcon.send("bot_join_after_player 0").is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(700));
    }
    Err("The hidden server didn't come up in time.".into())
}

fn avg(v: &[(f64, f64, f64)]) -> Option<(f64, f64, f64)> {
    let n = v.len() as f64;
    (!v.is_empty()).then(|| (v.iter().map(|x| x.0).sum::<f64>() / n, v.iter().map(|x| x.1).sum::<f64>() / n, v.iter().map(|x| x.2).sum::<f64>() / n))
}

/// AMXX's log lines about the mode's plugin: (load failures, runtime errors with the plugin's
/// line when the trace has one).
pub fn read_amxx_log(text: &str, plugin: &str) -> (Vec<String>, Vec<(String, Option<usize>)>) {
    let mut load = Vec::new();
    let mut runtime: Vec<(String, Option<usize>)> = Vec::new();
    let file = format!("{plugin}.amxx");
    let src = format!("{plugin}.sma");
    let lines: Vec<&str> = text.lines().collect();
    let strip = |l: &str| l.split(": ").skip(1).collect::<Vec<_>>().join(": ").trim().to_string();
    for (i, l) in lines.iter().enumerate() {
        let low = l.to_lowercase();
        if low.contains(&file) && (low.contains("failed to load") || low.contains("bad load") || low.contains("not found") || low.contains("plugin uses an unknown function")) {
            load.push(strip(l));
        }
        if (low.contains("native") && low.contains("not found")) || low.contains("module") && low.contains("required") && low.contains(&file) {
            if !load.contains(&strip(l)) {
                load.push(strip(l));
            }
        }
        if low.contains("run time error") {
            // the trace follows: [0] studio_x.sma::fn (line N), or the plugin's name
            let mut line_no = None;
            let mut ours = false;
            // (the trace: the lines right after, "[0] file::fn (line N)"; it ends at the first other line)
            for t in lines.iter().skip(i + 1).take(8).take_while(|t| t.split("[AMXX]").nth(1).is_some_and(|r| r.trim_start().starts_with('[') || r.contains("Displaying debug trace"))) {
                if t.contains(&file) || t.contains(&src) {
                    ours = true;
                }
                if let Some(p) = t.find(&format!("{src}::")) {
                    ours = true;
                    if let Some(n) = t[p..].split("(line ").nth(1).and_then(|r| r.split(')').next()).and_then(|n| n.trim().parse().ok()) {
                        line_no.get_or_insert(n);
                    }
                }
            }
            if ours {
                runtime.push((strip(l).replace("[AMXX] ", ""), line_no));
            }
        }
    }
    (load, runtime)
}

/// The debug events: how often each node ran, and the nodes' own errors (a refused command...).
fn read_events(text: &str) -> (BTreeMap<String, u64>, Vec<(String, String)>, bool) {
    let mut ran = BTreeMap::new();
    let mut errors = Vec::new();
    let mut session = false;
    for l in text.lines() {
        let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
        let node = e["node"].as_str().unwrap_or_default().to_string();
        match e["e"].as_str() {
            Some("node_enter") | Some("node_exit") if !node.is_empty() => {
                // (a sampled run carries how many runs before it weren't written)
                if e["e"] == "node_enter" || !ran.contains_key(&node) {
                    *ran.entry(node).or_insert(0) += u64::from(e["e"] == "node_enter") * (1 + e["skipped"].as_u64().unwrap_or(0));
                }
            }
            Some("session") => session = true,
            Some("node_error") if !node.is_empty() => {
                let m = e["message"].as_str().unwrap_or_default().to_string();
                if !errors.iter().any(|(n, x)| *n == node && *x == m) {
                    errors.push((node, m));
                }
            }
            _ => {}
        }
    }
    (ran, errors, session)
}

fn file_text(path: &Path) -> String {
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default()
}

struct Hidden {
    client: crate::steam::Install,
    dir: PathBuf,
    game: String,
    /// made from the game for this run (removed after)
    temporary: bool,
}

/// The hidden server: a copy of one of the player's servers brought up to date, else one set up
/// from the game's files (the same as the Map simulator does, in its own folder).
fn prepare(st: &Shared) -> Result<(Hidden, std::sync::Arc<crate::game::Game>), String> {
    let game = st.local_game().ok_or(super::NO_GAME)?;
    let client = crate::servers::client(st).ok_or(super::NO_GAME)?;
    if client.kind == "crossover" && !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The hidden server runs through it, like the game.".into());
    }
    let gf = super::super::health::game_folder(&game).to_string();
    // (tests never use the player's real servers folder: LS_SERVERS_DIR points at a test one,
    // QA4-B found a test run writing into the app's own Servers/.studio-test)
    #[cfg(test)]
    if std::env::var_os("LS_SERVERS_DIR").is_none() {
        return Err("Under test, a test run needs LS_SERVERS_DIR (a servers folder of its own).".into());
    }
    let root = crate::servers::root_for(&client)?;
    let dir = root.join(HIDDEN);
    // (testing: LS_TEST_RUN_SOURCE, a server folder to copy when the app lists none: a sim copy)
    let src = super::super::health::sources(&gf).into_iter().next().map(|e| (e.id, e.name, e.dir)).or_else(|| {
        std::env::var_os("LS_TEST_RUN_SOURCE").map(PathBuf::from).filter(|d| d.join("hlds.exe").is_file()).map(|d| ("test-source".to_string(), "the test server".to_string(), d))
    });
    let id = src.as_ref().map(|e| e.0.clone()).unwrap_or_else(|| "game".into());
    let mark = dir.join(MARK);
    if dir.exists() && std::fs::read_to_string(&mark).map(|s| s.trim() != id).unwrap_or(true) {
        step("Clearing the old hidden server");
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Couldn't clear the old hidden server ({e})."))?;
    }
    let first = !dir.join("hlds.exe").is_file();
    let temporary = src.is_none();
    match src {
        Some((_, name, from)) => {
            step(if first { format!("Making a hidden copy of \u{201c}{name}\u{201d} (the first time takes a while)") } else { format!("Bringing the hidden copy of \u{201c}{name}\u{201d} up to date") });
            std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the hidden server's folder ({e})."))?;
            let _ = std::fs::write(&mark, &id);
            super::super::health::sync_copy(&from, &dir, &STOP)?;
        }
        None if first => {
            if crate::setup::current().state == "running" {
                return Err("Setup is busy right now (installing, or teaching bots a map). Try again when it's done.".into());
            }
            // no server to copy: a hidden temporary one from the game's own files, as the Map
            // simulator makes it (servers::start_hidden_server, run as the setup job)
            step("Setting up a hidden temporary server from your game's files");
            let res = crate::setup_api::resources_for(st).map_err(|_| "Loner-Strike's own files couldn't be found, so a hidden server can't be set up. Reinstall the app.".to_string())?;
            let job = crate::servers::start_hidden_server(client.clone(), dir.clone(), gf.clone(), res)?;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                stopped()?;
                let v = crate::setup::current();
                if v.id != job {
                    return Err("The hidden server's setup was replaced by another setup job. Try again.".into());
                }
                match v.state.as_str() {
                    "running" => continue,
                    "done" => break,
                    _ => return Err(format!("The hidden server couldn't be set up: {}", v.error.unwrap_or_else(|| "setup stopped.".into()))),
                }
            }
            let _ = std::fs::write(&mark, &id);
        }
        None => {}
    }
    Ok((Hidden { client, dir, game: gf, temporary }, game))
}

/// Two stock maps with a nav (the same one twice when there's only one), copied in.
fn copy_maps(game: &crate::game::Game, gdir: &Path) -> Result<(String, String), String> {
    let maps = gdir.join("maps");
    let _ = std::fs::remove_dir_all(&maps);
    std::fs::create_dir_all(&maps).map_err(|e| format!("Couldn't make the hidden server's maps folder ({e})."))?;
    let mut found = Vec::new();
    for name in MAPS {
        let Some(m) = super::super::mapinfo::locate(game, name) else { continue };
        let (nav, lsv) = super::super::health::nav_files(&game.hl, &m);
        let Some(nav) = nav else { continue };
        std::fs::copy(&m.bsp, maps.join(format!("{}.bsp", m.name))).map_err(|e| format!("The map couldn't be copied ({e})."))?;
        std::fs::copy(&nav, maps.join(format!("{}.nav", m.name))).map_err(|e| format!("The map's nav couldn't be copied ({e})."))?;
        if let Some(l) = lsv {
            let _ = std::fs::copy(l, maps.join(format!("{}.lsv", m.name)));
        }
        found.push(m.name.clone());
        if found.len() == 2 {
            break;
        }
    }
    let a = found.first().cloned().ok_or("None of the stock maps has a nav here, so bots can't play one. Teach bots de_dust2 first (Maps, then Teach bots).")?;
    let b = found.get(1).cloned().unwrap_or_else(|| a.clone());
    Ok((a, b))
}

fn write(p: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(p, text).map_err(|e| format!("Couldn't write the hidden server's {} ({e}).", p.file_name().unwrap_or_default().to_string_lossy()))
}

/// Switches the hidden server's next map to a mode (as the app does before a start).
fn use_mode(am: &Path, mode: &str) -> Result<(), String> {
    let data = am.join("data/gamemodes");
    write(&data.join("current.txt"), &format!("{mode}\n"))?;
    let list = crate::sys::read_latin1(&am.join(format!("configs/gamemodes/{mode}.plugins.ini"))).unwrap_or_default();
    write(&am.join("configs/plugins-gamemode.ini"), &format!("; Generated by gamemodes.amxx - edit configs/gamemodes/{mode}.plugins.ini instead\n{list}"))
}

/// Stops our hidden server (only ours: its folder's name is in the pattern).
fn pkill_ours(dir: &Path) {
    #[cfg(target_os = "macos")]
    {
        let parent = dir.parent().and_then(|p| p.file_name()).map(|n| regex::escape(&n.to_string_lossy())).unwrap_or_default();
        let mut c = crate::sys::command("pkill");
        c.arg("-f").arg(format!(r"{parent}.\.studio-test.hlds\.exe"));
        let _ = crate::sys::run_output(c, Duration::from_secs(5));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = dir;
}

/// The whole test run (on its own thread): the summary.
fn run(st: &Shared, g: Graph) -> Result<Value, String> {
    let mut problems: Vec<Value> = Vec::new();
    let quick = graph::validate(&g);
    if graph::has_errors(&quick) {
        return Ok(json!({"verdict": "fail", "summary": ["The quick check has errors: fix those first."], "problems": quick, "log": ""}));
    }
    let (h, game) = prepare(st)?;
    stopped()?;
    let gdir = h.dir.join(&h.game);
    let am = gdir.join("addons/amxmodx");
    step("Copying two stock maps and their navs");
    let (map1, map2) = copy_maps(&game, &gdir)?;
    // the plugin, with the debugger, never stopping at a breakpoint
    step("Compiling with the debugger");
    let mut g = g;
    for n in g.nodes.iter_mut() {
        n.breakpoint = false;
    }
    let out = gen::generate(&g, true);
    let plugin = gen::plugin_name(&g.mode.id);
    let scripting = am.join("scripting");
    let inc = scripting.join("studio/include");
    super::write_includes(&inc).map_err(|e| format!("Couldn't write the plugin ({e})."))?;
    let src = scripting.join(format!("{plugin}.sma"));
    write(&src, &out.source)?;
    let built = am.join("plugins").join(format!("{plugin}.amxx"));
    let install = crate::steam::Install { hl: h.dir.clone(), kind: h.client.kind.clone(), bottle: h.client.bottle.clone(), games: vec![h.game.clone()], dedicated: true };
    let (ok, compiled) = super::run_amxxpc(&install, &scripting, &src, &inc, &built)?;
    if !ok {
        let diags = super::parse_compiler(&compiled, &format!("{plugin}.sma"));
        return Ok(json!({"verdict": "fail", "summary": ["The compiler said no (Compile shows where)."], "problems": super::diag_problems(&diags, &out), "log": compiled}));
    }
    super::register_mode(&am.join("configs"), &g, &plugin)?;
    // (the debug flag: AMXX's traces name the plugin's lines)
    let pini = am.join(format!("configs/gamemodes/{}.plugins.ini", g.mode.id));
    let text = crate::sys::read_latin1(&pini).unwrap_or_default().replace(&format!("{plugin}.amxx"), &format!("{plugin}.amxx debug"));
    write(&pini, &text)?;
    // bots only, no limits; the baseline first: Classic (or the first mode listed)
    let modes = crate::extras::parse_modes(&crate::sys::read_latin1(&am.join("configs/gamemodes/modes.ini")).unwrap_or_default());
    let base = modes.iter().map(|(m, _)| m.clone()).find(|m| m == "classic").or_else(|| modes.iter().map(|(m, _)| m.clone()).find(|m| *m != g.mode.id)).unwrap_or_else(|| g.mode.id.clone());
    use_mode(&am, &base)?;
    let data = am.join("data/gamemodes");
    for f in ["lastpreset.txt", "premode.txt"] {
        let _ = std::fs::remove_file(data.join(f));
    }
    let limits = "mp_timelimit 0\nmp_fraglimit 0\nmp_maxrounds 0\nmp_winlimit 0\n";
    write(&data.join("friends.cfg"), &format!("// Loner-Strike Studio test run\nhostname \"Loner-Strike test run\"\nsv_password \"\"\nsv_lan 1\nbot_join_after_player 0\n{limits}"))?;
    let global = crate::sys::read_latin1(&data.join("global.cfg")).unwrap_or_else(|_| "// Saved by the Game Mode Menu\n".into());
    write(&data.join("global.cfg"), &super::super::health::set_cvar(&global, "bot_quota", &BOTS.to_string()))?;
    write(&gdir.join("health.cfg"), &format!("sv_lan 1\nbot_join_after_player 0\nbot_auto_vacate 0\nbot_quota_mode normal\nbot_quota {BOTS}\n{limits}"))?;
    let events = am.join(format!("data/studio/{}.events", g.mode.id));
    let _ = std::fs::remove_dir_all(am.join("data/studio"));
    let _ = std::fs::remove_dir_all(am.join("logs"));
    let logs = [h.dir.join("qconsole.log"), gdir.join("qconsole.log"), h.dir.join("studio-test.log")];
    for l in &logs {
        let _ = std::fs::remove_file(l);
    }
    for e in std::fs::read_dir(&gdir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with("ls_crash") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let password: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..20).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect()
    };
    write(&data.join("panel_rcon.txt"), &password)?;
    stopped()?;

    step(format!("Starting a hidden server on {map1}"));
    let port = super::super::health::free_port()?;
    let log = std::fs::File::create(&logs[2]).map_err(|e| e.to_string())?;
    let mut child = super::super::health::spawn(&h.client, &h.dir, &super::super::health::server_args(&h.game, port, &password, &map1), log)?;
    crate::sys::tie_to_app(&child);
    let rcon = crate::rcon::Rcon::at(password.clone(), "127.0.0.1", port);
    let mut crashed = false;
    let (mut base_stats, mut mode_stats) = (Vec::new(), Vec::new());
    let mut plugins_text = String::new();
    let mut play_secs = 0u64;
    let mut bots_seen = 0usize;
    // (the debugger's log starts again on each map: what the first map wrote is kept here)
    let mut events_before = String::new();
    let played: Result<(), String> = (|| {
        wait_up(&rcon, &mut child, 240)?;
        let _ = rcon.send(&format!("bot_quota {BOTS}"));
        step("Measuring the server without the mode");
        sample(&rcon, &mut child, BASELINE, &mut base_stats)?;
        step("Loading the mode");
        use_mode(&am, &g.mode.id)?;
        let _ = rcon.send(&format!("changelevel {map1}"));
        wait_up(&rcon, &mut child, 120)?;
        let _ = rcon.send(&format!("bot_quota {BOTS}"));
        plugins_text = rcon.send("amxx plugins").unwrap_or_default();
        step(format!("Playing {map1}"));
        let t = Instant::now();
        sample(&rcon, &mut child, PLAY, &mut mode_stats)?;
        // (stats counts people only: the bots from status)
        bots_seen = rcon.send("status").unwrap_or_default().lines().filter(|l| l.contains(" BOT ")).count();
        step("Restarting the round");
        let _ = rcon.send("sv_restartround 1");
        sample(&rcon, &mut child, AFTER_RESTART, &mut mode_stats)?;
        step(format!("Changing map to {map2}"));
        std::thread::sleep(Duration::from_millis(600));
        events_before = file_text(&events);
        let _ = rcon.send(&format!("changelevel {map2}"));
        wait_up(&rcon, &mut child, 120)?;
        let _ = rcon.send(&format!("bot_quota {BOTS}"));
        step(format!("Playing {map2}"));
        sample(&rcon, &mut child, AFTER_CHANGE, &mut mode_stats)?;
        play_secs = t.elapsed().as_secs();
        Ok(())
    })();
    match &played {
        Err(e) if e == "crashed" => crashed = true,
        Err(e) if e == STOPPED => {}
        Err(e) => problems.push(json!({"level": "error", "message": e})),
        Ok(()) => {}
    }
    step("Checking what happened");
    let _ = rcon.send("quit");
    for _ in 0..25 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    crate::sys::stop_child(child, Duration::from_secs(3));
    pkill_ours(&h.dir);
    if played.as_ref().is_err_and(|e| e == STOPPED) {
        tidy(&h);
        return Err(STOPPED.into());
    }

    // what the server and AMXX said
    let mut log = String::new();
    for l in &logs {
        log.push_str(&super::super::health::tail(l, 200_000));
    }
    let mut amxx = String::new();
    for e in std::fs::read_dir(am.join("logs")).into_iter().flatten().flatten() {
        amxx.push_str(&super::super::health::tail(&e.path(), 200_000));
        amxx.push('\n');
    }
    let mut crash_text = String::new();
    for e in std::fs::read_dir(&gdir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with("ls_crash") {
            crash_text.push_str(&super::super::health::tail(&e.path(), 20_000));
        }
    }
    let (load, runtime) = read_amxx_log(&format!("{amxx}\n{log}"), &plugin);
    let (ran, node_errors, session) = read_events(&format!("{events_before}\n{}", file_text(&events)));
    // running: the debugger heard from it, or `amxx plugins` lists it (its file cut to 11 letters there)
    let short: String = format!("{plugin}.amxx").chars().take(11).collect();
    let listed = plugins_text.lines().any(|l| l.contains(&short) && l.contains("running"));
    let loaded = session || !ran.is_empty() || listed;

    let mut summary: Vec<String> = Vec::new();
    let mut fail = false;
    let mut warn = false;
    if crashed {
        fail = true;
        summary.push("The server crashed while playing the mode.".into());
        problems.push(json!({"level": "error", "message": format!("The server crashed.{}", if crash_text.is_empty() { "" } else { " Its crash report is in the log." }) }));
    }
    for l in &load {
        fail = true;
        problems.push(json!({"level": "error", "message": format!("The plugin didn't load: {l}")}));
    }
    if !loaded && load.is_empty() && !crashed && played.is_ok() {
        fail = true;
        problems.push(json!({"level": "error", "message": "The plugin isn't running on the server (amxx plugins doesn't list it as running)."}));
    }
    if fail && !crashed {
        summary.push("The mode's plugin didn't load.".into());
    }
    for (m, line) in &runtime {
        fail = true;
        let node = line.and_then(|n| out.node_at(n)).map(str::to_string);
        let msg = match (line, &node) {
            (Some(n), Some(_)) => format!("Run-time error (plugin line {n}): {m}"),
            (Some(n), None) => format!("Run-time error (plugin line {n}, not tied to a node; Show code in the menu shows that line): {m}"),
            _ => format!("Run-time error: {m}"),
        };
        problems.push(json!({"level": "error", "node": node, "message": msg}));
    }
    if !runtime.is_empty() {
        summary.push(format!("{} run-time error{} in the mode.", runtime.len(), if runtime.len() == 1 { "" } else { "s" }));
    }
    for (node, m) in &node_errors {
        warn = true;
        problems.push(json!({"level": "warning", "node": node, "message": m}));
    }
    // the cost: the server's frame rate and CPU with the mode against without; node runs
    let (b, m) = (avg(&base_stats), avg(&mode_stats));
    let runs: u64 = ran.values().sum();
    let per_sec = if play_secs > 0 { runs as f64 / play_secs as f64 } else { 0.0 };
    let per_sec = (per_sec * 100.0).round() / 100.0;
    let mut heaviest: Vec<(&String, &u64)> = ran.iter().collect();
    heaviest.sort_by(|a, b| b.1.cmp(a.1));
    let mut heavy = Vec::new();
    if let (Some(b), Some(m)) = (b, m) {
        if m.0 > b.0 + 25.0 {
            heavy.push(format!("CPU {:.0}% with the mode against {:.0}% without", m.0, b.0));
        }
        if b.1 > 0.0 && m.1 < b.1 * 0.7 {
            heavy.push(format!("{:.0} frames a second with the mode against {:.0} without", m.1, b.1));
        }
    }
    if per_sec > HEAVY_RUNS {
        heavy.push(format!("{per_sec:.0} node runs a second"));
    }
    if !heavy.is_empty() {
        warn = true;
        summary.push(format!("Heavy for a server: {}.", heavy.join("; ")));
        problems.push(json!({"level": "warning", "node": heaviest.first().map(|(n, _)| n.to_string()), "message": format!("Heavy for a server: {}. The busiest node is selected.", heavy.join("; "))}));
    }
    // which nodes ran (information, not a problem)
    let flow: Vec<&super::graph::Node> = g.nodes.iter().filter(|n| graph::registry().get(&n.kind).is_some_and(|d| !d.pure && !d.reroute)).collect();
    let never: Vec<String> = flow.iter().filter(|n| !ran.contains_key(&n.id)).map(|n| n.id.clone()).collect();
    if !never.is_empty() && !fail {
        summary.push(format!("{} of {} nodes never ran during the test (that can be fine: some wait for something the bots didn't do).", never.len(), flow.len()));
    }
    if bots_seen < 2 && !fail && played.is_ok() {
        warn = true;
        summary.push("The bots didn't join, so the mode wasn't really played. Try again; if it keeps happening, check the server in the Map simulator.".into());
    }
    let verdict = if fail { "fail" } else if warn { "warn" } else { "pass" };
    if summary.is_empty() || verdict == "pass" {
        summary.insert(0, format!("The mode loaded and played {map1}, a round restart and {map2} with {BOTS} bots without errors."));
    }
    let full_log = format!("== amxx plugins\n{plugins_text}\n== AMX Mod X logs\n{amxx}\n== server console\n{log}{}", if crash_text.is_empty() { String::new() } else { format!("\n== crash report\n{crash_text}") });
    tidy(&h);
    let seen: HashSet<&String> = ran.keys().collect();
    Ok(json!({
        "verdict": verdict,
        "summary": summary,
        "problems": problems,
        "ran": seen,
        "neverRan": never,
        "cost": {"baseline": b.map(|x| json!({"cpu": x.0, "fps": x.1, "players": x.2})), "mode": m.map(|x| json!({"cpu": x.0, "fps": x.1, "players": x.2})), "nodeRunsPerSec": per_sec, "busiest": heaviest.iter().take(3).map(|(n, c)| json!({"node": n, "runs": c})).collect::<Vec<_>>()},
        "maps": [map1, map2],
        "bots": bots_seen,
        "log": full_log.chars().rev().take(400_000).collect::<String>().chars().rev().collect::<String>(),
    }))
}

/// After a run: the maps, events, logs and password go; a server made from the game goes whole.
fn tidy(h: &Hidden) {
    pkill_ours(&h.dir);
    if h.temporary {
        if h.dir.file_name().is_some_and(|n| n == HIDDEN) && std::env::var_os("LS_KEEP_SIM_SERVER").is_none() {
            let _ = std::fs::remove_dir_all(&h.dir);
        }
        return;
    }
    let gdir = h.dir.join(&h.game);
    let am = gdir.join("addons/amxmodx");
    let _ = std::fs::remove_dir_all(gdir.join("maps"));
    let _ = std::fs::remove_dir_all(am.join("data/studio"));
    let _ = std::fs::remove_file(am.join("data/gamemodes/panel_rcon.txt"));
}

/// Starts a test run of this graph (one at a time).
pub fn start(st: &Shared, g: Graph) -> Result<Value, String> {
    {
        let mut cur = RUN.lock().unwrap_or_else(|e| e.into_inner());
        if cur.as_ref().is_some_and(|r| r.phase == "running") {
            return Err("A test run is already going. Wait for it, or cancel it first.".into());
        }
        *cur = Some(TestRun { mode: g.mode.id.clone(), phase: "running", started: now_ms(), ..Default::default() });
    }
    STOP.store(false, Ordering::SeqCst);
    step("Getting ready");
    let st = st.clone();
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&st, g))).unwrap_or_else(|_| Err(super::INTERNAL.into()));
        step("");
        if let Some(cur) = RUN.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            cur.steps.pop();
            match r {
                Ok(v) => {
                    cur.phase = "done";
                    cur.result = Some(v);
                }
                Err(e) if e == STOPPED => cur.phase = "stopped",
                Err(e) => {
                    cur.phase = "failed";
                    cur.error = Some(e);
                }
            }
        }
    });
    Ok(json!({"ok": true}))
}

pub fn progress() -> Value {
    match RUN.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        Some(r) => json!({"run": r, "now": now_ms()}),
        None => json!({"run": null}),
    }
}

pub fn cancel() -> Value {
    STOP.store(true, Ordering::SeqCst);
    json!({"ok": true})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_and_logs() {
        let s = "CPU   In    Out   Uptime  Users   FPS    Players\n 12.50  0.00  0.00       3     0   497.31       8\n";
        assert_eq!(parse_stats(s), Some((12.5, 497.31, 8.0)));
        assert_eq!(parse_stats("nothing"), None);
        let log = "L 11/14/2026 - 10:00:00: [AMXX] Run time error 4: index out of bounds \n\
L 11/14/2026 - 10:00:00: [AMXX]    [0] studio_x.sma::lsg_n3 (line 412)\n\
L 11/14/2026 - 10:00:01: [AMXX] Run time error 10: native error (native \"get_user_team\")\n\
L 11/14/2026 - 10:00:01: [AMXX]    [0] other.sma::f (line 9)\n\
L 11/14/2026 - 10:00:02: [AMXX] Plugin \"studio_x.amxx\" failed to load: Plugin uses an unknown function (name \"ls_foo\") - check your modules.ini.\n";
        let (load, rt) = read_amxx_log(log, "studio_x");
        assert_eq!(load.len(), 1, "{load:?}");
        assert!(load[0].contains("unknown function"));
        assert_eq!(rt, vec![("Run time error 4: index out of bounds".to_string(), Some(412))]);
    }

    #[test]
    fn events_count_runs_and_errors() {
        let d = std::env::temp_dir().join(format!("ls-testrun-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.events");
        std::fs::write(&f, "{\"e\":\"node_enter\",\"node\":\"a\"}\n{\"e\":\"node_enter\",\"node\":\"a\",\"skipped\":5}\n{\"e\":\"node_exit\",\"node\":\"b\",\"result\":\"ran\"}\n{\"e\":\"node_error\",\"node\":\"c\",\"message\":\"Refused\"}\nnot json\n").unwrap();
        let (ran, errs, _) = read_events(&std::fs::read_to_string(&f).unwrap());
        // (a sampled run carries the runs before it that weren't written)
        assert_eq!(ran.get("a"), Some(&7));
        assert!(ran.contains_key("b"), "an event node shows by its exit");
        assert_eq!(errs, vec![("c".to_string(), "Refused".to_string())]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
