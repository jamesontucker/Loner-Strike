// What the builder offers: presets, what keys can do and the game's key names.
// The server (tools/playercfg.rs) checks every value and action again.

export const COLOURS: [string, string][] = [
  ["Green", "50 250 50"],
  ["Red", "250 50 50"],
  ["Blue", "50 50 250"],
  ["Yellow", "250 250 50"],
  ["Cyan", "50 250 250"],
];

export type Preset = { id: string; label: string; note: string; values: Record<string, string> };

/** Presets set the crosshair, rates and a few habits; they leave binds, sensitivity and sound alone. */
export const PRESETS: Preset[] = [
  {
    id: "default",
    label: "The game's own",
    note: "How the game comes: a green crosshair that opens up as you move and shoot.",
    values: { crosshair: "1", cl_crosshair_color: "50 250 50", cl_crosshair_size: "auto", cl_crosshair_translucent: "1", cl_dynamiccrosshair: "1", rate: "25000", cl_updaterate: "60", cl_cmdrate: "60", ex_interp: "0.1", fps_max: "100", hud_fastswitch: "0", m_rawinput: "1", zoom_sensitivity_ratio: "1.2", m_filter: "0" },
  },
  {
    id: "pro",
    label: "Pro",
    note: "What competitive players used: a small solid crosshair that stays still, fast rates, fast weapon switch.",
    values: { crosshair: "1", cl_crosshair_color: "50 250 250", cl_crosshair_size: "small", cl_crosshair_translucent: "0", cl_dynamiccrosshair: "0", rate: "100000", cl_updaterate: "101", cl_cmdrate: "101", ex_interp: "0.01", fps_max: "101", hud_fastswitch: "1", m_rawinput: "1", zoom_sensitivity_ratio: "1", m_filter: "0" },
  },
  {
    id: "classic",
    label: "Classic 2004",
    note: "Like a pub in 2004: a medium see-through crosshair and the rates of the time.",
    values: { crosshair: "1", cl_crosshair_color: "50 250 50", cl_crosshair_size: "medium", cl_crosshair_translucent: "1", cl_dynamiccrosshair: "1", rate: "25000", cl_updaterate: "30", cl_cmdrate: "30", ex_interp: "0.1", fps_max: "100", hud_fastswitch: "0", zoom_sensitivity_ratio: "1.2" },
  },
];

export type NetPreset = { id: string; label: string; values: Record<string, string> };
export const NET: NetPreset[] = [
  { id: "fast", label: "Fast connection", values: { rate: "100000", cl_updaterate: "101", cl_cmdrate: "101", ex_interp: "0.01" } },
  { id: "normal", label: "Normal", values: { rate: "25000", cl_updaterate: "60", cl_cmdrate: "60", ex_interp: "0.1" } },
  { id: "slow", label: "Slow or far away", values: { rate: "10000", cl_updaterate: "30", cl_cmdrate: "30", ex_interp: "0.1" } },
];

export type Action = { id: string; label: string; note: string };
export type ActionGroup = { title: string; actions: Action[] };

export const ACTIONS: ActionGroup[] = [
  {
    title: "Moving",
    actions: [
      { id: "jumpduck", label: "Jump and duck at once", note: "Gets you onto higher boxes in one press." },
      { id: "jump", label: "Jump", note: "A second jump key, often the mouse wheel." },
      { id: "duck", label: "Duck", note: "A second duck key." },
    ],
  },
  {
    title: "Weapons",
    actions: [
      { id: "lastinv", label: "Last weapon", note: "Swaps to the weapon you had before (quick switch)." },
      { id: "qswitch", label: "AWP quick switch", note: "Knife and straight back, to skip the slow AWP bolt. Works best with Fast weapon switch on." },
      { id: "drop", label: "Drop weapon", note: "" },
      { id: "use", label: "Use", note: "Doors, hostages, defusing." },
    ],
  },
  {
    title: "Talking",
    actions: [
      { id: "voice", label: "Push to talk", note: "Hold to talk on the microphone." },
      { id: "chat", label: "Chat to everyone", note: "" },
      { id: "teamchat", label: "Chat to your team", note: "" },
    ],
  },
  {
    title: "Buying",
    actions: [
      { id: "buymenu", label: "Buy menu", note: "" },
      { id: "buy_rifle", label: "Buy the team's rifle", note: "AK-47 as a Terrorist, M4A1 as a Counter-Terrorist." },
      { id: "buy_awp", label: "Buy an AWP", note: "" },
      { id: "buy_deagle", label: "Buy a Desert Eagle", note: "" },
      { id: "buy_armor", label: "Buy armor and helmet", note: "" },
      { id: "buy_defuser", label: "Buy a defuse kit", note: "Counter-Terrorists only." },
      { id: "buy_nades", label: "Buy grenades", note: "One HE, two flashbangs and a smoke." },
      { id: "buy_ammo", label: "Buy full ammo", note: "For both guns." },
    ],
  },
  {
    title: "Other",
    actions: [{ id: "netgraph", label: "Show the net graph", note: "Press to show your ping and frame rate, press again to hide it." }],
  },
];

export const ALL_ACTIONS = ACTIONS.flatMap((g) => g.actions);

/** The app's own menus, which the builder leaves alone. */
export const RESERVED: Record<string, string> = { f3: "the Game Mode Menu", f4: "the quick menu" };

const CODES: Record<string, string> = {
  Space: "space", Tab: "tab", Enter: "enter", ShiftLeft: "shift", ShiftRight: "shift", ControlLeft: "ctrl", ControlRight: "ctrl", AltLeft: "alt", AltRight: "alt",
  Backspace: "backspace", CapsLock: "capslock", Insert: "ins", Delete: "del", Home: "home", End: "end", PageUp: "pgup", PageDown: "pgdn", Pause: "pause",
  ArrowUp: "uparrow", ArrowDown: "downarrow", ArrowLeft: "leftarrow", ArrowRight: "rightarrow",
  Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", BracketLeft: "[", BracketRight: "]", Minus: "-", Equal: "=", Backslash: "\\",
  Numpad7: "kp_home", Numpad8: "kp_uparrow", Numpad9: "kp_pgup", Numpad4: "kp_leftarrow", Numpad5: "kp_5", Numpad6: "kp_rightarrow", Numpad1: "kp_end", Numpad2: "kp_downarrow",
  Numpad3: "kp_pgdn", Numpad0: "kp_ins", NumpadDecimal: "kp_del", NumpadEnter: "kp_enter", NumpadDivide: "kp_slash", NumpadSubtract: "kp_minus", NumpadAdd: "kp_plus",
};

/** The game's name for a pressed key (KeyboardEvent.code), or null for one it can't bind here. */
export function keyFromCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(code)) return code.toLowerCase();
  return CODES[code] ?? null;
}

/** MouseEvent.button to the game's name. */
export const MOUSE = ["mouse1", "mouse3", "mouse2", "mouse4", "mouse5"];

const SHOW: Record<string, string> = {
  space: "Space", tab: "Tab", enter: "Enter", shift: "Shift", ctrl: "Ctrl", alt: "Alt", backspace: "Backspace", capslock: "Caps Lock", ins: "Insert", del: "Delete", home: "Home", end: "End",
  pgup: "Page Up", pgdn: "Page Down", pause: "Pause", uparrow: "Up", downarrow: "Down", leftarrow: "Left", rightarrow: "Right", mouse1: "Left mouse", mouse2: "Right mouse", mouse3: "Middle mouse",
  mouse4: "Mouse 4", mouse5: "Mouse 5", mwheelup: "Wheel up", mwheeldown: "Wheel down", kp_enter: "Keypad Enter", kp_5: "Keypad 5",
};

/** A key's name as people say it. */
export function keyLabel(k: string): string {
  if (SHOW[k]) return SHOW[k];
  if (k.startsWith("kp_")) return "Keypad " + k.slice(3).replace("arrow", "");
  return k.length === 1 ? k.toUpperCase() : k.toUpperCase();
}

/** Plain words for a command already on a key (from config.cfg), for "replaces …". */
const COMMAND_WORDS: Record<string, string> = {
  "+forward": "walk forward", "+back": "walk back", "+moveleft": "step left", "+moveright": "step right", "+jump": "jump", "+duck": "duck", "+attack": "shoot", "+attack2": "the second fire",
  "+reload": "reload", "+use": "use", "+speed": "walk quietly", "+showscores": "scores", "+voicerecord": "push to talk", "drop": "drop weapon", "lastinv": "last weapon", "buy": "the buy menu",
  "buyequip": "the equipment menu", "buyammo1": "buy ammo", "buyammo2": "buy pistol ammo", "chooseteam": "choose team", "messagemode": "chat", "messagemode2": "team chat", "impulse 100": "flashlight",
  "impulse 201": "spray", "radio1": "radio", "radio2": "radio", "radio3": "radio", "nightvision": "night vision", "autobuy": "auto buy", "rebuy": "rebuy", "invprev": "previous weapon",
  "invnext": "next weapon", "slot1": "weapon 1", "slot2": "weapon 2", "slot3": "knife", "slot4": "grenades", "slot5": "bomb", "toggleconsole": "the console", "snapshot": "screenshot",
};
export const commandWords = (c: string) => COMMAND_WORDS[c.toLowerCase()] ?? c;
