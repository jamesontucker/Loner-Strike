//! The game's own binaries, by the system the game runs on: the Windows game (hl.exe: on
//! Windows, on a Mac inside CrossOver, and on Linux through Proton) loads .dll files, the native
//! Linux game (hl_linux) loads .so files. Paths are '/'-separated and relative to the game folder
//! (cstrike, czero).
//!
//! On Linux the same Steam folder can run either: the native build, or the Windows one when
//! Proton is turned on for the game (many players do, as the native build has open crash
//! reports in online play: github.com/ValveSoftware/halflife/issues/3895). So setup there puts
//! both sets in side by side (`FLAVORS`): liblist.gam's gamedll and gamedll_linux, and Metamod's
//! win32 and linux lines, each name their own.

pub const LINUX: bool = cfg!(target_os = "linux");

/// One set of the game's binaries.
pub struct Flavor {
    /// ReGameDLL (the bot brain) in place of the game's server code
    pub mp: &'static str,
    /// where it is in a ReGameDLL release zip
    pub mp_in_regamedll: &'static str,
    pub regamedll_cfg_dir: &'static str,
    pub metamod: &'static str,
    /// as liblist.gam names it
    pub metamod_liblist: &'static str,
    pub liblist_key: &'static str,
    pub amxx_mm: &'static str,
    /// Metamod's plugins.ini line that loads AMX Mod X
    pub amxx_mm_line: &'static str,
    pub ham: &'static str,
    /// Loner-Strike's fixed Ham Sandwich under the app's resources
    pub ham_fixed_res: &'static str,
}

pub const WINDOWS_FLAVOR: Flavor = Flavor {
    mp: "dlls/mp.dll",
    mp_in_regamedll: "bin/win32/cstrike/dlls/mp.dll",
    regamedll_cfg_dir: "bin/win32/cstrike",
    metamod: "addons/metamod/dlls/metamod.dll",
    metamod_liblist: r"addons\metamod\dlls\metamod.dll",
    liblist_key: "gamedll",
    amxx_mm: "addons/amxmodx/dlls/amxmodx_mm.dll",
    amxx_mm_line: r"win32 addons\amxmodx\dlls\amxmodx_mm.dll",
    ham: "addons/amxmodx/modules/hamsandwich_amxx.dll",
    ham_fixed_res: "windows/hamsandwich/hamsandwich_amxx.dll",
};

pub const LINUX_FLAVOR: Flavor = Flavor {
    mp: "dlls/cs.so",
    mp_in_regamedll: "bin/linux32/cstrike/dlls/cs.so",
    regamedll_cfg_dir: "bin/linux32/cstrike",
    metamod: "addons/metamod/dlls/metamod.so",
    metamod_liblist: "addons/metamod/dlls/metamod.so",
    liblist_key: "gamedll_linux",
    amxx_mm: "addons/amxmodx/dlls/amxmodx_mm_i386.so",
    amxx_mm_line: "linux addons/amxmodx/dlls/amxmodx_mm_i386.so",
    ham: "addons/amxmodx/modules/hamsandwich_amxx_i386.so",
    ham_fixed_res: "linux/hamsandwich/hamsandwich_amxx_i386.so",
};

/// The sets setup puts in on this computer: the native one first.
pub const FLAVORS: &[Flavor] = if LINUX { &[LINUX_FLAVOR, WINDOWS_FLAVOR] } else { &[WINDOWS_FLAVOR] };

/// This computer's native set (the first of FLAVORS)
pub const NATIVE: &Flavor = &FLAVORS[0];

pub const MP: &str = NATIVE.mp;
#[allow(dead_code)] // (tests)
pub const METAMOD: &str = NATIVE.metamod;
#[allow(dead_code)] // (tests)
pub const METAMOD_LIBLIST: &str = NATIVE.metamod_liblist;
pub const LIBLIST_KEY: &str = NATIVE.liblist_key;
pub const AMXX_MM: &str = NATIVE.amxx_mm;
#[allow(dead_code)] // (tests)
pub const HAM: &str = NATIVE.ham;
#[allow(dead_code)] // (tests)
pub const HAM_FIXED_RES: &str = NATIVE.ham_fixed_res;

/// The plugin compiler under addons/amxmodx/scripting (the native one: its .amxx files load in
/// either build of the game)
pub const AMXXPC: &str = if LINUX { "amxxpc" } else { "amxxpc.exe" };
