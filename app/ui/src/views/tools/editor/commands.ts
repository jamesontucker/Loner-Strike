// P5: every editor command in one list (id, plain label, menu, keys, what it does), so the
// menu bar, the command palette, the cheat sheet and the shortcut editor all agree. Keys are
// in Shortcuts.ts; the old key handler in MapEditor.tsx still does the keys that aren't here
// (arrows nudge, Esc, 1-9 camera marks, Alt to ignore the grid).

import { goToLeak, loadLeakFile } from "./LeakTools"; // P5
import { useLeak } from "./compileApi"; // P5
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { selectionIds } from "./EditorScene";
import { copySelection, paste } from "./MapEditor";
import { deleteSelection, duplicate, hideSelection, isolateSelection, showAll, TOOLS } from "./Panels";
import { frameFlat, LAYOUTS, setLayout, toggleMaximise, usePanes } from "./panes";
import { cmd2, importRmf, useP5, type Ids, type SideTab } from "./p5store";
import { modeFor, setPaneMode, type RenderMode } from "./renderModes";
import { NAV_PRESETS, setNavPreset, stepSpeed } from "./navigation";
import { applyDockPreset, DOCK_PRESETS, showPanel, type DockPresetId } from "./dockState";
import { DOCK_PANELS } from "./dockPanels";
import { EMPTY_SEL, selectionBounds, useEditor, type ViewId } from "./store";
import { useNav } from "../../../store";
import { mapNameOf } from "./ComponentPanel";
import { startTour } from "./Tour";
import { useBrowser } from "./contentApi";
import { toggleFrameTime } from "./FrameTime"; // MCP: frame-time readout
import { budgetWarnings } from "./BudgetPanel";

export type Menu = "File" | "Edit" | "View" | "Select" | "Tools" | "Map" | "Help";
export type Command = {
  id: string;
  label: string;
  menu: Menu;
  /** a line under the label in the palette */
  hint?: string;
  run: () => void | Promise<void>;
  /** greyed out when false */
  when?: () => boolean;
  /** works while flying in the 3D view (else the key goes to the camera) */
  fly?: boolean;
  /** shown in menus after a line */
  sep?: boolean;
  /** not in the menu bar (palette and keys only) */
  hidden?: boolean;
};

// ---- helpers -----------------------------------------------------------------------------

const S = () => useEditor.getState();
const hasSel = () => {
  const s = S().sel;
  return s.brushes.length + s.entities.length > 0;
};
const brushSel = () => selectionIds().brushes.length > 0;

/** Picks these, the way a click would: brushes of brush entities pick the entity. */
export function selectIds(ids: Ids, add = false) {
  const s = S();
  const owner = new Map((s.doc?.brushes ?? []).map((b) => [b.id, b.entity]));
  const ents = new Set(ids.entities);
  const brushes: number[] = [];
  for (const b of ids.brushes) {
    const e = owner.get(b);
    if (e != null) ents.add(e);
    else brushes.push(b);
  }
  const cur = s.sel;
  s.select(add ? { brushes: [...new Set([...cur.brushes, ...brushes])], entities: [...new Set([...cur.entities, ...ents])], face: null } : { brushes, entities: [...ents], face: null });
  const n = brushes.length + ents.size;
  s.notify(n ? `Picked ${n} thing${n === 1 ? "" : "s"}.` : "Nothing like that found.");
}

/** Zooms every view to what's picked (else the whole map). */
export function frame(all = false) {
  const s = S();
  const b = all ? null : selectionBounds();
  const box = b ?? s.doc?.bounds;
  if (!box) return;
  const [min, max] = box;
  const centre = min.map((x, i) => (x + max[i]) / 2) as Vec3;
  useTools.getState().look(centre, Math.max(64, ...max.map((x, i) => x - min[i])));
  frameFlat(min, max);
}

/** Asks before an action (set by the menu bar to the app's confirm sheet). */
export const askHooks: { ask: (text: string, ok: string) => Promise<boolean> } = { ask: async () => true };

/** Tools > Community's Publish, for the map compiled from this one (P4's, moved here with the one
 *  bar). P5: a limit over 90% (Budget panel) is said first. */
export async function publishMap() {
  const d = S().doc;
  if (!d) return;
  const over = await budgetWarnings();
  if (over.length && !(await askHooks.ask(`Near the engine's limits: ${over.join("; ")}. Players may not be able to load it. Publish anyway? (The Budget panel shows what uses most.)`, "Publish anyway"))) return;
  useNav.getState().go("tools", `community:publish:${mapNameOf(d.name)}`);
}

/** Playtest from anywhere (F5, Map > Playtest, the editor bar): the Compile panel opens and starts it. */
export function playtestNow() {
  openTab("compile");
  const p = useP5.getState();
  p.set({ playtestAsked: p.playtestAsked + 1 });
}

export function openTab(tab: SideTab) {
  useP5.getState().set({ tab });
  showPanel(tab);
}

async function similar(by: Record<string, unknown>) {
  const ids = selectionIds();
  const s = S();
  const faceTex = s.sel.face ? s.brushes.get(s.sel.face.brush)?.faces.find((f) => f.face === s.sel.face!.face)?.texture : null;
  const body = by.by === "texture" && faceTex ? { ...by, textures: [faceTex] } : by;
  const r = await cmd2({ op: "similar", ...ids, similar: body });
  if (r.select) selectIds(r.select);
}

async function touching(inside: boolean) {
  const r = await cmd2({ op: "touching", ...selectionIds(), inside });
  if (r.select) selectIds(r.select, false);
}

/** Copies of what's picked: `offset` grid steps over (0: in place), with new target names. */
/** Duplicates what's picked (and picks the copies). `merge`: a Shift-drag copy's key, so the
 *  move that follows joins it as one "Copy" undo step. */
export async function duplicateWith(offset: Vec3, merge?: string) {
  const ids = selectionIds();
  if (!ids.brushes.length && !ids.entities.length) return;
  const r = await cmd2({ op: "duplicate", ...ids, offset, rename: true, ...(merge ? { merge } : {}) });
  if (r.created) {
    const owned = new Set(r.created.entities);
    const brushes = r.created.brushes.filter((b) => !owned.has(S().brushes.get(b)?.entity ?? -1));
    S().select({ brushes, entities: r.created.entities, face: null });
  }
}

function invert() {
  const s = S();
  const pickedB = new Set(s.sel.brushes);
  const pickedE = new Set(s.sel.entities);
  const { locked } = useP5.getState();
  const brushes = [...s.brushes.values()].filter((b) => b.entity == null && !pickedB.has(b.id) && !s.hidden.brushes.has(b.id) && !locked.brushes.has(b.id)).map((b) => b.id);
  const entities = (s.doc?.entities ?? []).filter((e) => !pickedE.has(e.id) && !s.hidden.entities.has(e.id) && !locked.entities.has(e.id)).map((e) => e.id);
  s.select({ brushes, entities, face: null });
}

function selectAll() {
  const s = S();
  s.select({ brushes: [...s.brushes.keys()].filter((id) => s.brushes.get(id)?.entity == null && !s.hidden.brushes.has(id)), entities: s.doc?.entities.filter((x) => !s.hidden.entities.has(x.id)).map((x) => x.id) ?? [], face: null });
}

export function lockSelection(lock: boolean) {
  const p = useP5.getState();
  const ids = selectionIds();
  const brushes = new Set(p.locked.brushes);
  const entities = new Set(p.locked.entities);
  if (lock) {
    if (!ids.brushes.length && !ids.entities.length) return;
    ids.brushes.forEach((b) => brushes.add(b));
    S().sel.entities.forEach((e) => entities.add(e));
    p.set({ locked: { brushes, entities } });
    S().select(EMPTY_SEL);
    S().notify("Locked: it can't be picked in the views until you unlock it (Outline, or Shift+L for all).");
  } else p.set({ locked: { brushes: new Set(), entities: new Set() } });
}

function transformSel(body: Record<string, unknown>) {
  const ids = selectionIds();
  if (!ids.brushes.length && !ids.entities.length) return;
  const b = selectionBounds();
  const pivot = b ? b[0].map((x, i) => (x + b[1][i]) / 2) : undefined;
  void S().cmd({ op: "transform", ...ids, pivot, lock: S().textureLock, ...body });
}
const mirror = (axis: number) => transformSel({ scale: [0, 1, 2].map((i) => (i === axis ? -1 : 1)) });
const turn = (axis: number, degrees: number) => transformSel({ rotate: { axis: [0, 1, 2].map((i) => (i === axis ? 1 : 0)), degrees } });

async function align(axis: number, edge: "min" | "centre" | "max") {
  await cmd2({ op: "align", ...selectionIds(), axis, edge, lock: S().textureLock });
}
async function distribute(axis: number) {
  await cmd2({ op: "distribute", ...selectionIds(), axis, lock: S().textureLock });
}

function setView(v: ViewId) {
  S().set({ view: v });
}

/** File actions that replace the open map ask first when there are unsaved changes (set by the menu bar). */
export const fileHooks: { replace: (what: "new" | "open" | "import" | "other") => void } = { replace: (w) => void (w === "other" ? importOther() : S().file(w)) };

/** Hammer/J.A.C.K. import (tools/editor/workspace.rs); what didn't come over is said. */
export async function importOther() {
  const s = S();
  if (s.fileBusy) return;
  s.set({ fileBusy: "Importing…" });
  try {
    const r = await importRmf();
    if (r.error) return s.notify(r.error, true);
    if (r.cancelled) return;
    await S().load();
    const notes = r.notes ?? [];
    S().notify(notes.length ? `Imported. ${notes.join(" ")}` : "Imported. Save it to make a project.");
  } finally {
    useEditor.setState({ fileBusy: null });
  }
}

function cyclePaneMode() {
  const p = usePanes.getState();
  const order: RenderMode[] = ["textured", "flat", "wire", "texwire"];
  const cur = modeFor(p.modes?.[p.active], p.views[p.active]);
  setPaneMode(p.active, order[(order.indexOf(cur) + 1) % order.length]);
}

const AX = ["X", "Y", "Z"];

// ---- the list ------------------------------------------------------------------------------

export function commands(): Command[] {
  const s = S();
  const list: Command[] = [
    // File
    { id: "file.new", label: "New map", menu: "File", run: () => fileHooks.replace("new") },
    { id: "file.open", label: "Open a project…", menu: "File", run: () => fileHooks.replace("open") },
    { id: "file.import", label: "Import a .map…", menu: "File", hint: "From TrenchBroom, Hammer, J.A.C.K. or any GoldSrc editor; TrenchBroom's groups and layers come in as groups", run: () => fileHooks.replace("import") },
    { id: "file.importOther", label: "Import a Hammer .rmf or J.A.C.K. .jmf…", menu: "File", hint: "Brushes, entities, groups and visgroups come over", run: () => fileHooks.replace("other") },
    { id: "file.save", label: "Save", menu: "File", sep: true, run: () => void S().file("save"), fly: true },
    { id: "file.saveAs", label: "Save as…", menu: "File", run: () => void S().file("saveAs"), fly: true },
    { id: "file.export", label: "Export a .map…", menu: "File", hint: "For any GoldSrc compiler, and back to TrenchBroom with its groups and layers", run: () => void S().file("export") },
    { id: "file.project", label: "Project files", menu: "File", sep: true, hint: "The map's files, compile output, WADs and sounds it uses", run: () => openTab("project") },
    { id: "file.props", label: "Map properties…", menu: "File", hint: "Sky, WADs, sun and rules", run: () => S().set({ dialog: "props" }) },

    // Edit
    { id: "edit.undo", label: "Undo", menu: "Edit", hint: s.doc?.undo ? `Undo ${s.doc.undo}` : undefined, when: () => !!S().doc?.undo, run: () => void S().undo(), fly: true },
    { id: "edit.redo", label: "Redo", menu: "Edit", hint: s.doc?.redo ? `Redo ${s.doc.redo}` : undefined, when: () => !!S().doc?.redo, run: () => void S().redo(), fly: true },
    { id: "edit.history", label: "Undo history", menu: "Edit", hint: "Click a step to go back to it", run: () => openTab("history") },
    { id: "edit.cut", label: "Cut", menu: "Edit", sep: true, when: hasSel, run: () => void copySelection(true) },
    { id: "edit.copy", label: "Copy", menu: "Edit", when: hasSel, run: () => void copySelection(false) },
    { id: "edit.paste", label: "Paste", menu: "Edit", run: () => void paste() },
    { id: "edit.pasteSpecial", label: "Paste special…", menu: "Edit", hint: "Copies in a row, ring or spiral", when: hasSel, run: () => S().set({ dialog: "paste" }) },
    { id: "edit.duplicate", label: "Duplicate", menu: "Edit", hint: "A copy one grid step over, with new target names", when: hasSel, run: () => void duplicateWith([S().grid, S().grid, 0]) },
    {
      id: "edit.applyTexture",
      label: "Apply the current texture",
      menu: "Edit",
      hint: "To the picked faces, or every face of the picked brushes",
      when: () => hasSel() || !!S().sel.face,
      run: () => {
        const s = S();
        const faces = s.sel.faces?.length ? s.sel.faces : s.sel.face ? [s.sel.face] : [];
        if (faces.length) void s.cmd({ op: "set_texture", faces: faces.map((f) => [f.brush, f.face]), texture: s.texture });
        else void s.cmd({ op: "set_texture", faces: selectionIds().brushes.map((b) => [b, null]), texture: s.texture });
      },
    },
    { id: "edit.duplicateHere", label: "Duplicate in place", menu: "Edit", hint: "A copy right on top (move it next)", when: hasSel, run: () => void duplicateWith([0, 0, 0]) },
    { id: "edit.duplicateSame", label: "Duplicate (keep names)", menu: "Edit", hidden: true, hint: "A copy with the same target names", when: hasSel, run: () => void duplicate() },
    { id: "edit.delete", label: "Delete", menu: "Edit", when: hasSel, run: () => deleteSelection() },
    { id: "edit.find", label: "Find and replace…", menu: "Edit", sep: true, hint: "Textures, classnames, keys and values", run: () => useP5.getState().set({ dialog: "find" }) },
    { id: "edit.transform", label: "Transform…", menu: "Edit", hint: "Move, turn or stretch by exact numbers", when: hasSel, run: () => useP5.getState().set({ dialog: "transform" }) },
    { id: "edit.replaceWith", label: "Replace with entity or prefab…", menu: "Edit", hint: "Swap what's picked for another entity or a prefab", when: hasSel, run: () => useP5.getState().set({ dialog: "replaceWith" }) },
    { id: "edit.group", label: "Group", menu: "Edit", sep: true, when: hasSel, run: () => void S().cmd({ op: "group", name: `Group ${(S().doc?.groups.length ?? 0) + 1}`, brushes: S().sel.brushes, entities: S().sel.entities }) },
    {
      id: "edit.ungroup",
      label: "Ungroup",
      menu: "Edit",
      when: () => ungroupTarget() != null,
      run: () => {
        const g = ungroupTarget();
        if (g != null) void S().cmd({ op: "ungroup", group: g });
      },
    },
    { id: "edit.tie", label: "Tie to entity", menu: "Edit", hint: "func_wall (pick another in Inspect)", when: brushSel, run: async () => {
      // the new entity is what's picked after (so Move to world works straight away)
      const r = await S().cmd({ op: "tie", brushes: selectionIds().brushes, classname: "func_wall" });
      if (r.created?.entities.length) S().select({ brushes: [], entities: r.created.entities, face: null });
    } },
    {
      id: "edit.toWorld",
      label: "Move to world",
      menu: "Edit",
      hint: "Brush entities become world brushes",
      when: () => S().sel.entities.some((e) => (S().doc?.entities.find((x) => x.id === e)?.brushes.length ?? 0) > 0),
      run: async () => {
        for (const e of S().sel.entities) if (S().doc?.entities.find((x) => x.id === e)?.brushes.length) await S().cmd({ op: "to_world", entity: e });
      },
    },
    { id: "edit.hollow", label: "Hollow", menu: "Edit", sep: true, when: brushSel, run: () => void S().cmd({ op: "hollow", brushes: selectionIds().brushes, thickness: S().thickness }) },
    { id: "edit.carve", label: "Carve", menu: "Edit", hint: "Cut this brush out of what it overlaps", when: () => selectionIds().brushes.length === 1, run: () => void S().cmd({ op: "carve", cutter: selectionIds().brushes[0], targets: [...S().brushes.keys()] }) },
    { id: "edit.merge", label: "Merge", menu: "Edit", when: () => selectionIds().brushes.length > 1, run: () => void S().cmd({ op: "merge", brushes: selectionIds().brushes }) },
    { id: "edit.snap", label: "Snap to grid", menu: "Edit", when: brushSel, run: () => void S().cmd({ op: "snap", brushes: selectionIds().brushes, grid: S().grid }) },
    ...[0, 1, 2].map((a): Command => ({ id: `edit.mirror${AX[a]}`, label: `Flip along ${AX[a]}`, menu: "Edit", sep: a === 0, when: hasSel, run: () => mirror(a) })),
    ...[0, 1, 2].map((a): Command => ({ id: `edit.turn${AX[a]}`, label: `Turn 90° around ${AX[a]}`, menu: "Edit", when: hasSel, run: () => turn(a, 90) })),
    ...[0, 1, 2].flatMap((a): Command[] =>
      (["min", "centre", "max"] as const).map((edge) => ({
        id: `edit.align${AX[a]}${edge}`,
        label: `Line up on ${AX[a]}: ${edge === "min" ? (a === 2 ? "bottoms" : "low sides") : edge === "max" ? (a === 2 ? "tops" : "high sides") : "middles"}`,
        menu: "Edit",
        hidden: true,
        when: () => selectionIds().brushes.length + S().sel.entities.length > 1,
        run: () => void align(a, edge),
      })),
    ),
    ...[0, 1, 2].map((a): Command => ({ id: `edit.distribute${AX[a]}`, label: `Spread out evenly along ${AX[a]}`, menu: "Edit", hidden: true, when: () => selectionIds().brushes.length + S().sel.entities.length > 2, run: () => void distribute(a) })),

    // Select
    { id: "sel.all", label: "Select all", menu: "Select", run: selectAll },
    { id: "sel.none", label: "Select none", menu: "Select", run: () => S().select(EMPTY_SEL) },
    { id: "sel.invert", label: "Invert selection", menu: "Select", run: invert },
    { id: "sel.texture", label: "Select same texture", menu: "Select", sep: true, hint: "Everything with the picked brush's or face's textures", when: () => hasSel() || !!S().sel.face, run: () => void similar({ by: "texture" }) },
    { id: "sel.class", label: "Select same type", menu: "Select", hint: "Every entity of the picked kind (world brushes: all world brushes)", when: hasSel, run: () => void similar({ by: "class" }) },
    {
      id: "sel.value",
      label: "Select same name",
      menu: "Select",
      hint: "Entities sharing the picked one's targetname",
      when: () => S().sel.entities.length > 0,
      run: () => void similar({ by: "value", key: "targetname" }),
    },
    { id: "sel.touching", label: "Select touching", menu: "Select", sep: true, hint: "Things whose box touches the selection's", when: hasSel, run: () => void touching(false) },
    { id: "sel.inside", label: "Select inside", menu: "Select", hint: "Things wholly inside the selection's box", when: hasSel, run: () => void touching(true) },
    { id: "sel.goto", label: "Go to brush or entity number…", menu: "Select", hint: "For compiler messages (Entity 3, Brush 12)", run: () => useP5.getState().set({ dialog: "goto" }) },
    { id: "sel.hide", label: "Hide", menu: "Select", sep: true, when: hasSel, run: () => hideSelection() },
    { id: "sel.isolate", label: "Hide everything else", menu: "Select", when: hasSel, run: () => isolateSelection() },
    { id: "sel.showAll", label: "Show everything", menu: "Select", run: () => showAll() },
    { id: "sel.hideEntities", label: "Hide all point entities", menu: "Select", hint: "Spawns, lights, sounds…", run: () => hideKind("point") },
    { id: "sel.hideTriggers", label: "Hide triggers and zones", menu: "Select", hint: "trigger_*, buy zones, bomb sites, rescue zones", run: () => hideKind("trigger") },
    { id: "sel.hideWorld", label: "Hide world brushes", menu: "Select", run: () => hideKind("world") },
    { id: "sel.lock", label: "Lock", menu: "Select", hint: "Can't be picked until unlocked", when: hasSel, run: () => lockSelection(true) },
    { id: "sel.unlockAll", label: "Unlock everything", menu: "Select", when: () => useP5.getState().locked.brushes.size + useP5.getState().locked.entities.size > 0, run: () => lockSelection(false) },
    { id: "sel.groups", label: "Clicks pick whole groups", menu: "Select", sep: true, run: () => S().set({ selectMode: "groups" }) },
    { id: "sel.objects", label: "Clicks pick single objects", menu: "Select", run: () => S().set({ selectMode: "objects" }) },

    // View
    ...LAYOUTS.map((l, i): Command => ({ id: `view.layout.${l.id}`, label: `Layout: ${l.label}`, menu: "View", hint: l.tip, run: () => setLayout(l.id), fly: true, sep: i === 0 })),
    { id: "view.maximise", label: "Maximize the pane / put it back", menu: "View", when: () => usePanes.getState().layout !== "single", run: () => toggleMaximise(), fly: true },
    ...(["3d", "top", "front", "side"] as ViewId[]).map((v, i): Command => ({ id: `view.${v}`, label: `${v === "3d" ? "3D" : v[0].toUpperCase() + v.slice(1)} view`, menu: "View", sep: i === 0, hint: "In the active pane", run: () => setView(v), fly: true })),
    { id: "view.frame", label: "Zoom to selection", menu: "View", sep: true, when: hasSel, run: () => frame(false) },
    { id: "view.frameAll", label: "Zoom to the whole map", menu: "View", run: () => frame(true) },
    { id: "view.fly", label: "Fly / orbit camera", menu: "View", run: () => useTools.getState().setCamera(useTools.getState().camera === "fly" ? "orbit" : "fly"), fly: true },
    { id: "view.gridDown", label: "Smaller grid", menu: "View", sep: true, run: () => stepGrid(-1), fly: true },
    { id: "view.gridUp", label: "Bigger grid", menu: "View", run: () => stepGrid(1), fly: true },
    { id: "view.gridShow", label: "Show / hide the grid", menu: "View", run: () => useP5.getState().set({ showGrid: !useP5.getState().showGrid }), fly: true },
    { id: "view.gridSettings", label: "Grid and snapping…", menu: "View", hint: "Grid size, turn steps, snap to corners and edges", run: () => useP5.getState().set({ dialog: "grid" }) },
    { id: "view.texLock", label: "Texture lock on / off", menu: "View", run: () => S().set({ textureLock: !S().textureLock }) },
    { id: "view.frameTime", label: "Show frame time", menu: "View", sep: true, hint: "Frames a second and frame time (p50, p95) over the last 5 seconds; Ctrl+Alt+Shift+F", run: () => toggleFrameTime() }, // MCP
    { id: "view.snap", label: "Snap to grid on / off", menu: "View", hint: "Alt still flips it while held", run: () => S().set({ snapOff: !S().snapOff }), fly: true },
    { id: "view.geoSnap", label: "Snap to corners and edges on / off", menu: "View", run: () => useP5.getState().set({ geoSnap: !useP5.getState().geoSnap }) },
    ...NAV_PRESETS.map((n, i): Command => ({ id: `view.nav.${n.id}`, label: `Move like ${n.label}`, menu: "View", sep: i === 0, hint: n.tip, run: () => setNavPreset(n.id), fly: true })),
    { id: "view.speedUp", label: "Camera faster", menu: "View", hidden: true, run: () => stepSpeed(1), fly: true },
    { id: "view.speedDown", label: "Camera slower", menu: "View", hidden: true, run: () => stepSpeed(-1), fly: true },
    { id: "view.models", label: "Models in place of entity boxes on / off", menu: "View", hint: "Hostages, spawns, weapons, cyclers drawn as their models", run: () => useP5.getState().set({ showModels: !useP5.getState().showModels }), fly: true },
    { id: "view.paneMode", label: "Next drawing style for the active pane", menu: "View", hint: "Textured, flat, wireframe, textured + wire", run: () => cyclePaneMode(), fly: true },

    // View > Panels and layouts
    ...DOCK_PANELS.map((p, i): Command => ({ id: `panel.${p.id}`, label: `Panel: ${p.title}`, menu: "View", sep: i === 0, hint: p.subtitle ? `${p.subtitle}. Shows it wherever it's docked or floating.` : "Show it (wherever it's docked or floating)", run: () => showPanel(p.id) })),
    ...(Object.keys(DOCK_PRESETS) as DockPresetId[]).map((id, i): Command => ({ id: `dock.${id}`, label: `Panel layout: ${DOCK_PRESETS[id].label}`, menu: "View", sep: i === 0, hint: DOCK_PRESETS[id].tip, run: () => applyDockPreset(id) })),
    { id: "dock.reset", label: "Reset the panel layout", menu: "View", run: () => applyDockPreset("default") },

    // Tools
    ...TOOLS.map((t, i): Command => ({ id: `tool.${t.id}`, label: `${t.label} tool`, menu: "Tools", hint: t.tip, sep: i === 0 ? false : t.id === "vertex" || t.id === "box", run: () => S().set({ tool: t.id }) })),

    // Map
    { id: "map.props", label: "Map properties…", menu: "Map", run: () => S().set({ dialog: "props" }) },
    { id: "map.report", label: "Entity report…", menu: "Map", run: () => S().set({ dialog: "report" }) },
    { id: "map.info", label: "Map information…", menu: "Map", hint: "Counts, textures and texture files in use", run: () => useP5.getState().set({ dialog: "info" }) },
    { id: "map.checks", label: "Check the map", menu: "Map", sep: true, run: () => openTab("checks") },
    { id: "view.content", label: "Content Browser", menu: "View", sep: true, hint: "Models, sprites, sounds, textures, prefabs and GameBanana: drag onto the map", run: () => showPanel("content") },
    { id: "view.contentDrawer", label: "Content drawer", menu: "View", hint: "The Content Browser over the bottom panels (a click away closes it)", run: () => useBrowser.getState().set({ drawer: !useBrowser.getState().drawer }), fly: true },
    { id: "view.budget", label: "Budget", menu: "View", hint: "How close the map is to the engine's limits", run: () => showPanel("budget") },
    { id: "view.io", label: "I/O: what triggers what", menu: "View", hint: "Targets, multi_managers, masters and broken links; pick an entity for its chain", run: () => showPanel("io") },
    { id: "map.publish", label: "Publish…", menu: "Map", hint: "A package to share this map in Tools > Community (compile it into the game first)", run: publishMap },
    { id: "map.playtest", label: "Playtest", menu: "Map", hint: "Compile, put it in the game and start it with bots, as the Compile panel is set", run: playtestNow },
    { id: "map.goToLeak", label: "Go to leak", menu: "Map", hint: "Fly along the leak line to the hole and pick the brush beside it", when: () => !!useLeak.getState().points, run: () => goToLeak() },
    { id: "map.loadPointfile", label: "Load pointfile…", menu: "Map", hint: "A leak line (.lin or .pts) from another compile", run: () => void loadLeakFile() },
    { id: "map.leakLine", label: "Show or hide the leak line", menu: "Map", when: () => !!useLeak.getState().points, run: () => useLeak.getState().toggle() },
    { id: "map.compile", label: "Compile and play…", menu: "Map", hint: "Compile with SDHLT, show the leak line, playtest with bots", run: () => openTab("compile") },
    { id: "map.outline", label: "Outline", menu: "Map", sep: true, hint: "Everything in the map as a list", run: () => openTab("outline") },
    { id: "map.textures", label: "Textures", menu: "Map", run: () => openTab("textures") },
    { id: "map.inspect", label: "Properties of what's picked", menu: "Map", run: () => openTab("inspect") },

    // Help
    { id: "help.palette", label: "Find a command…", menu: "Help", run: () => useP5.getState().set({ dialog: "palette", paletteText: "" }), fly: true },
    { id: "help.cheats", label: "Keys cheat sheet", menu: "Help", run: () => useP5.getState().set({ dialog: "cheats" }), fly: true },
    { id: "help.coords", label: "Coordinates and axes", menu: "Help", hint: "Z up, X east, Y north: the engine's own numbers", run: () => useP5.getState().set({ dialog: "coords" }), fly: true },
    { id: "help.tour", label: "First map tour", menu: "Help", hint: "Draw a room, hollow it, add a light and spawns, then play it: each step ticks off as you go", run: () => startTour() },
    { id: "help.keys", label: "Change keys…", menu: "Help", hint: "Hammer and TrenchBroom key sets, or your own", run: () => useP5.getState().set({ dialog: "keys" }) },
  ];
  return list;
}

function ungroupTarget(): number | null {
  const s = S();
  const groups = s.doc?.groups ?? [];
  const pickedGroups = new Set<number>();
  for (const b of s.doc?.brushes ?? []) if (s.sel.brushes.includes(b.id) && b.group != null) pickedGroups.add(b.group);
  for (const e of s.doc?.entities ?? []) if (s.sel.entities.includes(e.id) && e.group != null) pickedGroups.add(e.group);
  const top = [...pickedGroups].map((g) => {
    let cur = g;
    for (let i = 0; i < 32; i++) {
      const p = groups.find((x) => x.id === cur)?.parent;
      if (p == null) break;
      cur = p;
    }
    return cur;
  });
  return top[0] ?? null;
}

/** Hide a whole kind of thing (on top of what's hidden already). */
function hideKind(kind: "point" | "trigger" | "world") {
  const s = S();
  const brushes = new Set(s.hidden.brushes);
  const entities = new Set(s.hidden.entities);
  for (const e of s.doc?.entities ?? []) {
    if (kind === "point" && !e.brushes.length) entities.add(e.id);
    if (kind === "trigger" && e.brushes.length && /^trigger_|^func_(buyzone|bomb_target|hostage_rescue|vip_safetyzone|escapezone)$/.test(e.classname)) entities.add(e.id);
  }
  if (kind === "world") for (const b of s.doc?.brushes ?? []) if (b.entity == null) brushes.add(b.id);
  s.set({ hidden: { brushes, entities } });
}

function stepGrid(d: number) {
  const grids = [1, 2, 4, 8, 16, 32, 64, 128, 256];
  const g = S().grid;
  const i = grids.indexOf(g);
  S().set({ grid: grids[Math.max(0, Math.min(grids.length - 1, (i < 0 ? 4 : i) + d))] });
}
