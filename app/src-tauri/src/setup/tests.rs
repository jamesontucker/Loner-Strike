//! Setup tests on throwaway game folders.
//!
//! The full install (downloads + compiling with CrossOver's wine) is ignored by
//! default; run it with a scratch Half-Life folder:
//!   LS_SETUP_TEST_HL=/path/to/fake/Half-Life cargo test setup_end_to_end -- --ignored --nocapture

use super::*;
use std::collections::BTreeMap;

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    crate::archive::walk(root)
        .into_iter()
        .map(|p| (p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), fetch::sha256(&p).unwrap()))
        .collect()
}

fn fake_game(root: &Path, game: &str) {
    let g = root.join(game);
    std::fs::create_dir_all(g.join("dlls")).unwrap();
    std::fs::write(g.join("liblist.gam"), "game \"Counter-Strike\"\r\ngamedll \"dlls\\mp.dll\"\r\ngamedll_linux \"dlls/cs.so\"\r\n").unwrap();
    std::fs::write(g.join("config.cfg"), "bind \"w\" \"+forward\"\r\nname \"Player\"\r\n").unwrap();
    std::fs::write(g.join("dlls/mp.dll"), b"MZ original server code").unwrap();
    if game == "czero" {
        std::fs::write(g.join("BotProfile.db"), "Default\r\n\tSkill = 50\r\n\tAggression = 50\r\nEnd\r\n\r\nTemplate Expert\r\n\tSkill = 90\r\n\tReactionTime = 0.3\r\nEnd\r\n").unwrap();
    }
}

fn install_at(hl: &Path) -> Install {
    let games = GAMES.iter().map(|(g, _)| g.to_string()).filter(|g| hl.join(g).join("liblist.gam").is_file()).collect();
    Install { hl: hl.to_path_buf(), kind: "crossover".into(), bottle: Some("Steam".into()), games, dedicated: false }
}

/// The parts that need no downloads: core settings, bot styles, keys, liblist;
/// then a full uninstall gives back exactly the original files.
#[test]
fn core_parts_install_and_undo() {
    let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "setup-").unwrap();
    let hl = root.join("Half-Life");
    fake_game(&hl, "cstrike");
    fake_game(&hl, "czero");
    let before = snapshot(&hl);
    let install = install_at(&hl);
    let res = repo_root();
    let ctx = Ctx { install: &install, res: &res, progress: &Quiet, store: None };
    for g in ["cstrike", "czero"] {
        let mut rec = Record::open(&hl.join(g));
        rec.begin("foundation");
        foundation::liblist(&mut rec).unwrap();
        rec.done("foundation", json!(1)).unwrap();
        rec.begin("core");
        let modes: BTreeSet<String> = core::CORE_MODES.iter().map(|s| s.to_string()).collect();
        let addons: BTreeSet<String> = ["quakesounds".to_string()].into();
        core::configs(&mut rec, &ctx, &modes, &addons).unwrap();
        core::bot_styles(&mut rec, &ctx, g).unwrap();
        core::binds(&mut rec).unwrap();
        rec.done("core", json!(1)).unwrap();

        let dir = hl.join(g);
        let lib = std::fs::read_to_string(dir.join("liblist.gam")).unwrap();
        assert!(lib.contains("gamedll \"addons\\metamod\\dlls\\metamod.dll\""));
        let cfg = std::fs::read_to_string(dir.join("config.cfg")).unwrap();
        assert!(cfg.ends_with("bind \"F3\" \"gamemenu\"\r\nbind \"F4\" \"gamequick\"\r\n"), "{cfg:?}");
        let modes_ini = std::fs::read_to_string(dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap();
        assert!(modes_ini.contains("classic") && !modes_ini.contains("\nzombie "));
        assert!(!dir.join("addons/amxmodx/configs/gamemodes/zombie.cfg").exists());
        assert!(!dir.join("addons/amxmodx/configs/gamemodes/addons/dice.ini").exists());
        assert!(dir.join("BotProfile_Gods.db").is_file());
        assert!(dir.join("BotProfile_Pub_Solo.db").is_file() && dir.join("BotProfile_Pub_Hold.db").is_file());
        assert!(dir.join("addons/amxmodx/configs/gamemodes/botplay.ini").is_file());
        // CS 1.6 gets Loner-Strike's own bot profile; CZ keeps its own
        let bp = std::fs::read_to_string(dir.join("BotProfile.db")).unwrap();
        assert_eq!(g == "cstrike", bp.starts_with("// BotProfile.db written by Loner-Strike"));

        // adding a mode later puts its settings in; removing it takes them out again
        let mut more = modes.clone();
        more.insert("paintball".into());
        rec.begin("core");
        core::configs(&mut rec, &ctx, &more, &addons).unwrap();
        assert!(dir.join("addons/amxmodx/configs/gamemodes/paintball.cfg").is_file());
        core::configs(&mut rec, &ctx, &modes, &addons).unwrap();
        assert!(!dir.join("addons/amxmodx/configs/gamemodes/paintball.cfg").exists());
        rec.done("core", json!(1)).unwrap();
    }
    for g in ["cstrike", "czero"] {
        Record::open(&hl.join(g)).uninstall(None).unwrap();
    }
    assert_eq!(snapshot(&hl), before);
    // no folders left behind either
    assert!(!hl.join("cstrike/addons").exists() && !hl.join("czero/addons").exists());
    let _ = std::fs::remove_dir_all(root);
}

/// Settings files the player is told to edit: an update keeps their changes (the new version
/// beside them as .new), and replaces them when they're still what Loner-Strike put there,
/// also for a 0.1.2 install that has no note of what it wrote.
#[test]
fn edited_configs_are_kept() {
    let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "edits-").unwrap();
    let hl = root.join("Half-Life");
    fake_game(&hl, "cstrike");
    let install = install_at(&hl);
    let res = repo_root();
    let ctx = Ctx { install: &install, res: &res, progress: &Quiet, store: None };
    let modes: BTreeSet<String> = core::CORE_MODES.iter().map(|s| s.to_string()).collect();
    let addons = BTreeSet::new();
    let dir = hl.join("cstrike/addons/amxmodx/configs/gamemodes");
    let shipped = |f: &str| std::fs::read(res.join("plugins/configs/gamemodes").join(f)).unwrap();
    let configs = || {
        let mut rec = Record::open(&hl.join("cstrike"));
        rec.begin("core");
        core::configs(&mut rec, &ctx, &modes, &addons).unwrap();
        rec.done("core", json!(1)).unwrap();
    };
    configs();
    for f in core::EDITABLE {
        assert_eq!(std::fs::read(dir.join(f)).unwrap(), shipped(f), "{f}");
    }
    // edited: kept, with this version's beside it
    std::fs::write(dir.join("botchat.ini"), "; mine\r\n[kill]\r\nmy line\r\n").unwrap();
    // left as it was, but as a Windows editor saves it: still Loner-Strike's
    std::fs::write(dir.join("adverts.ini"), String::from_utf8(shipped("adverts.ini")).unwrap().replace('\n', "\r\n")).unwrap();
    configs();
    assert_eq!(std::fs::read_to_string(dir.join("botchat.ini")).unwrap(), "; mine\r\n[kill]\r\nmy line\r\n");
    assert_eq!(std::fs::read(dir.join("botchat.ini.new")).unwrap(), shipped("botchat.ini"));
    assert_eq!(std::fs::read(dir.join("adverts.ini")).unwrap(), shipped("adverts.ini"));
    assert!(!dir.join("adverts.ini.new").exists());
    // their edits taken back out: replaced again, and the .new goes
    std::fs::write(dir.join("botchat.ini"), shipped("botchat.ini")).unwrap();
    configs();
    assert!(!dir.join("botchat.ini.new").exists());
    // a 0.1.2 install (no note of what it wrote): 0.1.2's own text is replaced, an edited one kept
    let mut rec = Record::open(&hl.join("cstrike"));
    rec.data.extra.remove("config_hashes");
    rec.save().unwrap();
    // (0.1.2's own file from the repo's history, when it's there)
    let old_botchat = std::process::Command::new("git")
        .args(["show", "v0.1.2:plugins/configs/gamemodes/botchat.ini"])
        .current_dir(&res)
        .output()
        .ok()
        .filter(|o| o.status.success() && !o.stdout.is_empty())
        .map(|o| o.stdout);
    if let Some(old) = &old_botchat {
        std::fs::write(dir.join("botchat.ini"), old).unwrap();
    }
    std::fs::write(dir.join("regular_names.ini"), "; mine\n").unwrap();
    configs();
    assert_eq!(std::fs::read(dir.join("botchat.ini")).unwrap(), shipped("botchat.ini"));
    assert!(!dir.join("botchat.ini.new").exists());
    assert_eq!(std::fs::read_to_string(dir.join("regular_names.ini")).unwrap(), "; mine\n");
    assert_eq!(std::fs::read(dir.join("regular_names.ini.new")).unwrap(), shipped("regular_names.ini"));
    let _ = std::fs::remove_dir_all(root);
}

/// A 0.1.2 game (foundation noted as 1, the original ReGameDLL) is offered the forked one
/// once; after that the foundation isn't flagged again
#[test]
fn foundation_from_0_1_2_is_updated_once() {
    let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "fver-").unwrap();
    let mut rec = Record::open(&root);
    rec.done("foundation", json!(1)).unwrap();
    assert_eq!(outdated(&repo_root(), &rec), vec!["foundation".to_string()]);
    rec.done("foundation", json!(foundation_version())).unwrap();
    assert!(outdated(&repo_root(), &rec).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn quake_sounds_switched_on() {
    let shipped = ";Generated by Stats Configuration Plugin. Do not modify!\r\nShowAttackers            ;Show Attackers\r\nShowStats\r\n;MultiKill\r\n";
    let out = core::stats_ini_with_quake(shipped);
    assert!(out.starts_with(";Generated by Stats Configuration Plugin. Do not modify!\r\nShowAttackers            ;Show Attackers\r\nShowStats\r\n"), "what was there stays");
    for name in core::QUAKE_ON {
        assert_eq!(out.lines().filter(|l| l.trim() == *name).count(), 1, "{name} on once");
    }
    assert_eq!(core::stats_ini_with_quake(&out), out, "the same twice");
    // one already on (from AMX Mod X's own menu) isn't added again
    let out = core::stats_ini_with_quake("MultiKill  ;Multi kill\r\n");
    assert_eq!(out.lines().filter(|l| l.split_whitespace().next() == Some("MultiKill")).count(), 1);
}

/// Every sound miscstats.ini names for an announcement setup switches on is downloaded,
/// pinned and checked.
#[test]
fn quake_sounds_files_are_listed() {
    let dir = repo_root().join("plugins/configs/quake");
    let list: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("sounds.json")).unwrap()).unwrap();
    let fetched: Vec<String> = list["fetch"].as_array().unwrap().iter().map(|i| {
        assert!(i["url"].as_str().unwrap().starts_with("https://raw.githubusercontent.com/ClaudiuHKS/AdvancedQuakeSounds/"));
        assert_eq!(i["sha256"].as_str().unwrap().len(), 64);
        i["to"].as_str().unwrap().to_string()
    }).collect();
    for (_, file) in core::ANNOUNCERS {
    let ini = std::fs::read_to_string(dir.join(file)).unwrap();
    for line in ini.lines().filter(|l| !l.starts_with(';') && !l.trim().is_empty()) {
        let mut w = line.split_whitespace();
        let (key, sound) = (w.next().unwrap(), w.next().unwrap());
        if sound.starts_with("AQS/") {
            assert!(fetched.contains(&format!("sound/{sound}.wav")), "{key}: {sound} isn't downloaded");
        } else {
            // the bomb and grenade ones (no sounds in the game) stay switched off
            assert!(!["GrenadeKill", "GrenadeSuicide", "BombPlanted", "BombDefused", "BombFailed"].iter().any(|k| key.starts_with(k) && core::QUAKE_ON.contains(&format!("{k}Sound").as_str())), "{key}");
        }
    }
    }
    assert_eq!(core::announcer_of(&dir), "classic", "without a choice saved");
}

#[test]
fn plan_checks_requests() {
    let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "plan-").unwrap();
    let hl = root.join("Half-Life");
    fake_game(&hl, "czero");
    let res = repo_root();
    let install = install_at(&hl);
    assert!(plan_install(install.clone(), &["cstrike".into()], &[], &res, false).is_err());
    assert!(plan_install(install.clone(), &["czero".into()], &["mode-nope".into()], &res, false).is_err());
    assert!(plan_install(install.clone(), &["czero".into()], &["foundation".into()], &res, false).is_err());
    let plan = plan_install(install.clone(), &["czero".into()], &["mode-paintball".into()], &res, false).unwrap();
    let ids: Vec<&str> = plan.steps().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        ["czero/metamod", "czero/amxmodx", "czero/regamedll", "czero/liblist", "czero/mode-paintball", "czero/core-plugins", "czero/core-configs", "czero/core-quake", "czero/core-bots", "czero/core-keys", "czero/core-funmaps"]
    );
    // a dedicated server doesn't get the fun maps
    let mut dedicated = install.clone();
    dedicated.dedicated = true;
    let plan = plan_install(dedicated, &["czero".into()], &["mode-paintball".into()], &res, false).unwrap();
    assert!(!plan.steps().iter().any(|s| s.id.ends_with("core-funmaps")));
    // a damaged record stops everything
    std::fs::create_dir_all(hl.join("czero/addons/loner-strike")).unwrap();
    std::fs::write(hl.join("czero/addons/loner-strike/record.json"), "{not json").unwrap();
    assert!(plan_install(install.clone(), &["czero".into()], &[], &res, false).err().unwrap().contains("damaged"));
    assert!(plan_uninstall(install, &["czero".into()], None, &res).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn catalog_lists_installable_components() {
    let cat = catalog_summary(&repo_root()).unwrap();
    let ids: Vec<&str> = cat.iter().filter_map(|c| c["id"].as_str()).collect();
    assert!(ids.contains(&"mode-paintball") && ids.contains(&"addon-dice"));
    assert!(!ids.contains(&"foundation") && !ids.contains(&"mode-classic"));
    for c in &cat {
        assert_eq!(c["games"], json!(["cstrike", "czero"]), "{}", c["id"]);
    }
    let kz = cat.iter().find(|c| c["id"] == "mode-kreedz").unwrap();
    assert_eq!(kz["bots"], "none");
    let dice = cat.iter().find(|c| c["id"] == "addon-dice").unwrap();
    assert_eq!(dice["bots"], "great");
}

#[test]
fn splitlines_like_python() {
    assert_eq!(splitlines("a\r\nb\rc\nd", false), ["a", "b", "c", "d"]);
    assert_eq!(splitlines("a\r\nb\n", true), ["a\r\n", "b\n"]);
    assert_eq!(splitlines("", false), Vec::<&str>::new());
    assert_eq!(splitlines("x\x0cy", false), ["x", "y"]);
}

/// Full install into a scratch Half-Life folder (downloads, CrossOver compile),
/// a component removed on its own, then everything removed: the folder must end
/// up exactly as it started.
#[test]
#[ignore]
fn setup_end_to_end() {
    let Some(hl) = std::env::var_os("LS_SETUP_TEST_HL").map(PathBuf::from) else {
        panic!("set LS_SETUP_TEST_HL to a scratch Half-Life folder");
    };
    assert!(!hl.to_string_lossy().contains("/steamapps/common/"), "use a scratch copy, not the real game");
    let before = snapshot(&hl);
    let install = install_at(&hl);
    let res = repo_root();
    let games: Vec<String> = install.games.clone();
    let comps = vec!["mode-paintball".to_string(), "addon-dice".to_string()];

    let plan = plan_install(install.clone(), &games, &comps, &res, false).unwrap();
    let job = job_for_test("install", plan.steps());
    let t = std::time::Instant::now();
    run_install(&job, &plan, &res, None);
    let v = job.view();
    println!("install took {:?}", t.elapsed());
    for s in &v.steps {
        println!("  [{}] {} {} {}", s.state, s.id, s.label, s.detail);
    }
    println!("notes: {:?}", v.notes);
    assert_eq!(v.state, "done", "{:?}", v.error);
    assert!(v.failed_compiles.is_empty(), "{:?} {:?}", v.failed_compiles, v.notes);
    assert_eq!(v.progress, 1.0);

    for g in &games {
        let dir = hl.join(g);
        let rec = Record::open(&dir);
        for c in ["foundation", "core", "mode-paintball", "addon-dice"] {
            assert!(rec.installed(c), "{g}: {c}");
        }
        for p in ["gamemodes", "gm_humanbots", "gm_pubserver", "gm_deathmatch", "gm_botnav", "pb_gameplay", "pb_weapons", "dice_main", "dice_items_cstrike"] {
            assert!(dir.join(format!("addons/amxmodx/plugins/{p}.amxx")).is_file(), "{g}: {p}.amxx");
        }
        assert!(dir.join("addons/metamod/dlls/metamod.dll").is_file());
        assert!(dir.join("sound/DICE/zeus.wav").is_file());
        assert!(dir.join("models/paintballR/v_marker_b.mdl").is_file());
        let lib = std::fs::read_to_string(dir.join("liblist.gam")).unwrap();
        assert!(lib.contains(foundation::METAMOD_DLL));
        let modes = std::fs::read_to_string(dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap();
        assert!(modes.contains("paintball"));
        let addons = std::fs::read_to_string(dir.join("addons/amxmodx/configs/gamemodes/addons.ini")).unwrap();
        assert!(addons.contains("dice "));
        // the DICE patches went in
        let ini = std::fs::read_to_string(dir.join("addons/amxmodx/scripting/dice/dice_main.sma")).unwrap();
        assert!(ini.to_lowercase().contains("bot"));
        assert!(rec.data.files["dlls/mp.dll"].backup.is_some());

        // the same as the Python installer made from the same starting files
        if let Some(py) = std::env::var_os("LS_SETUP_TEST_PYTHON_HL").map(PathBuf::from) {
            // amxxpc's output isn't reproducible (it embeds the source path and stray
            // memory that changes with the environment), so plugins only need to exist
            let skip = |root: &Path, m: BTreeMap<String, String>| -> BTreeMap<String, String> {
                m.into_iter()
                    .filter(|(k, _)| k != "addons/loner-strike/record.json")
                    .map(|(k, v)| if k.ends_with(".amxx") { (k.clone(), format!("compiled: {}", root.join(&k).metadata().unwrap().len() > 0)) } else { (k, v) })
                    .collect()
            };
            let mine = skip(&dir, snapshot(&dir));
            let theirs = skip(&py.join(g), snapshot(&py.join(g)));
            let differ: Vec<&String> = mine.keys().chain(theirs.keys()).filter(|k| mine.get(*k) != theirs.get(*k)).collect();
            assert!(differ.is_empty(), "{g}: differs from the Python install: {differ:?}");
            let py_rec = Record::open(&py.join(g));
            assert_eq!(serde_json::to_value(&rec.data.files).unwrap(), serde_json::to_value(&py_rec.data.files).unwrap());
            let keys = |r: &Record| r.data.components.keys().cloned().collect::<BTreeSet<_>>();
            assert_eq!(keys(&rec), keys(&py_rec));
            println!("{g}: {} files identical to the Python install", mine.len());
        }
    }

    // installing again changes nothing that's installed (only core runs)
    let again = plan_install(install.clone(), &games, &comps, &res, false).unwrap();
    assert!(again.steps().iter().all(|s| s.id.contains("/core-")), "{:?}", again.steps().iter().map(|s| &s.id).collect::<Vec<_>>());

    // remove Paintball on its own
    let plan = plan_uninstall(install.clone(), &games, Some(vec!["mode-paintball".into()]), &res).unwrap();
    let job = job_for_test("uninstall", plan.steps());
    run_uninstall(&job, &plan, &res, None);
    assert_eq!(job.view().state, "done", "{:?}", job.view().error);
    for g in &games {
        let dir = hl.join(g);
        let rec = Record::open(&dir);
        assert!(!rec.installed("mode-paintball") && rec.installed("addon-dice"));
        assert!(!dir.join("addons/amxmodx/plugins/pb_gameplay.amxx").exists());
        assert!(!dir.join("models/paintballR").exists());
        assert!(!dir.join("addons/amxmodx/configs/gamemodes/paintball.cfg").exists());
        let modes = std::fs::read_to_string(dir.join("addons/amxmodx/configs/gamemodes/modes.ini")).unwrap();
        assert!(!modes.contains("paintball"));
        assert!(dir.join("addons/amxmodx/plugins/dice_main.amxx").is_file());
    }

    // and then everything
    let plan = plan_uninstall(install, &games, None, &res).unwrap();
    let job = job_for_test("uninstall", plan.steps());
    run_uninstall(&job, &plan, &res, None);
    assert_eq!(job.view().state, "done", "{:?}", job.view().error);
    assert_eq!(snapshot(&hl), before);
    for g in &games {
        assert!(!hl.join(g).join("addons").exists(), "{g}/addons left behind");
    }
}

/// Removing an install the Python installer made (same record format).
///   LS_SETUP_TEST_PY_HL=/path/to/scratch/Half-Life cargo test uninstall_python_install -- --ignored
#[test]
#[ignore]
fn uninstall_python_install() {
    let hl = PathBuf::from(std::env::var_os("LS_SETUP_TEST_PY_HL").expect("set LS_SETUP_TEST_PY_HL"));
    assert!(!hl.to_string_lossy().contains("/steamapps/common/"), "use a scratch copy, not the real game");
    let install = install_at(&hl);
    let games = install.games.clone();
    let plan = plan_uninstall(install, &games, None, &repo_root()).unwrap();
    let job = job_for_test("uninstall", plan.steps());
    run_uninstall(&job, &plan, &repo_root(), None);
    assert_eq!(job.view().state, "done", "{:?}", job.view().error);
    for g in &games {
        let dir = hl.join(g);
        assert!(!dir.join("addons").exists());
        let lib = std::fs::read_to_string(dir.join("liblist.gam")).unwrap();
        assert!(lib.contains("gamedll \"dlls\\mp.dll\""));
        let files: Vec<_> = crate::archive::walk(&dir).into_iter().map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().to_string()).collect();
        println!("{g}: {files:?}");
    }
}

/// A mode that comes with GameBanana maps: they're installed through the map store
/// and removed again with the mode.
///   LS_SETUP_TEST_HL=<scratch Half-Life> LS_SETUP_TEST_APP=<scratch app folder> cargo test mode_with_maps -- --ignored
#[test]
#[ignore]
fn mode_with_maps() {
    let hl = PathBuf::from(std::env::var_os("LS_SETUP_TEST_HL").expect("set LS_SETUP_TEST_HL"));
    let app = PathBuf::from(std::env::var_os("LS_SETUP_TEST_APP").expect("set LS_SETUP_TEST_APP"));
    assert!(!hl.to_string_lossy().contains("/steamapps/common/"), "use a scratch copy, not the real game");
    let before = snapshot(&hl);
    let install = install_at(&hl);
    let res = repo_root();
    let store = Arc::new(MapStore::new(&hl, &hl.join("cstrike"), &app, Arc::new(Mutex::new(None))));
    let games = vec!["cstrike".to_string()];
    let plan = plan_install(install.clone(), &games, &["mode-hns".into()], &res, false).unwrap();
    let job = job_for_test("install", plan.steps());
    run_install(&job, &plan, &res, Some(store.clone()));
    let v = job.view();
    println!("{:?} notes {:?}", v.error, v.notes);
    assert_eq!(v.state, "done");
    let rec = Record::open(&hl.join("cstrike"));
    let mods = rec.map_mods("mode-hns");
    assert_eq!(mods.len(), 2, "{mods:?}");
    for m in ["hns_floppytown", "hns_2k12"] {
        assert!(hl.join(format!("cstrike/maps/{m}.bsp")).is_file(), "{m}");
    }
    assert!(store.records().contains_key(&mods[0].to_string()));

    let plan = plan_uninstall(install.clone(), &games, Some(vec!["mode-hns".into()]), &res).unwrap();
    let job = job_for_test("uninstall", plan.steps());
    run_uninstall(&job, &plan, &res, Some(store.clone()));
    assert_eq!(job.view().state, "done", "{:?}", job.view().error);
    assert!(!hl.join("cstrike/maps/hns_2k12.bsp").exists());
    assert!(store.records().is_empty());
    assert!(Record::open(&hl.join("cstrike")).data.maps.is_empty());

    let plan = plan_uninstall(install, &games, None, &res).unwrap();
    let job = job_for_test("uninstall", plan.steps());
    run_uninstall(&job, &plan, &res, Some(store));
    assert_eq!(snapshot(&hl), before);
}

#[test]
fn bots_enabled_in_game_init() {
    use crate::setup::core::enable_bots;
    assert_eq!(enable_bots("// x\r\nbot_enable \"0\"\r\nhostage_ai_enable \"0\"\r\n").unwrap(), "// x\r\nbot_enable \"1\"\r\nhostage_ai_enable \"0\"\r\n");
    assert_eq!(enable_bots("bot_enable 1\n"), None);
    assert_eq!(enable_bots("sv_x 1").unwrap(), "sv_x 1\r\nbot_enable \"1\"\r\n");
}

