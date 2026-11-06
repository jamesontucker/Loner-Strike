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
    Install { hl: hl.to_path_buf(), kind: "crossover".into(), bottle: Some("Steam".into()), games }
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
        ["czero/metamod", "czero/amxmodx", "czero/regamedll", "czero/liblist", "czero/mode-paintball", "czero/core-plugins", "czero/core-configs", "czero/core-bots", "czero/core-keys"]
    );
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
            // compiled plugins carry the source's full path: compare their unpacked
            // code with the game folder's path taken out
            let unpacked_amxx = |path: &Path, root: &Path| -> String {
                use std::io::Read;
                let b = std::fs::read(path).unwrap();
                for i in 0..b.len().saturating_sub(1) {
                    if b[i] == 0x78 && [0x01, 0x5e, 0x9c, 0xda].contains(&b[i + 1]) {
                        let mut out = Vec::new();
                        if flate2::read::ZlibDecoder::new(&b[i..]).read_to_end(&mut out).is_ok() {
                            let text: String = out.iter().map(|&c| c as char).collect();
                            let _ = root;
                            // (as Z:\...\Half-Life\ through wine; same-length scratch paths keep the sizes equal)
                            let re = regex::Regex::new(r"[A-Za-z]:\\[^\x00]*?\\Half-Life\\").unwrap();
                            return fetch::hex_sha256(re.replace_all(&text, "<hl>").as_bytes());
                        }
                    }
                }
                panic!("no code in {}", path.display());
            };
            let skip = |root: &Path, m: BTreeMap<String, String>| -> BTreeMap<String, String> {
                m.into_iter()
                    .filter(|(k, _)| k != "addons/loner-strike/record.json")
                    .map(|(k, v)| if k.ends_with(".amxx") { (k.clone(), unpacked_amxx(&root.join(&k), root)) } else { (k, v) })
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
