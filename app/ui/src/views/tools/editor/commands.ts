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
import { canPasteTransform, clipNow, copyProps, copyTransform, pasteProps, pasteTransform } from "./inspectClip"; // Inspector Copy / Paste
import { dropToFloor } from "./dropToFloor";

// 1.0.1: Brush (Hammer's Tools menu) holds what shapes brushes, so Edit fits a short window
export type Menu = "File" | "Edit" | "Brush" | "View" | "Select" | "Tools" | "Map" | "Help";
export type Command = {
  id: string;
  label: string;
  menu: Menu;
  /** a line under the label in the palette */
  hint?: string;
  run: () => void | Promise<void>;
  /** greyed out when false */
  when?: () => boolean;
  /** what a key press says when `when` is false ("Pick a brush first to hollow it.") */
  needs?: string;
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

/** Frames what's picked in every view (else the whole map). */
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
  const order: RenderMode[] = ["textured", "flat", "wire", "texwire", "collision", "bytype", "missing"];
  const cur = modeFor(p.modes?.[p.active], p.views[p.active]);
  setPaneMode(p.active, order[(order.indexOf(cur) + 1) % order.length]);
}

const AX = ["X", "Y", "Z"];

const PICK = "Pick something first";
const PICK_BRUSH = "Pick a brush first";

/** Why a command can't run now, for a key press that did nothing (1.0.1). */
export function whyNot(c: Command): string {
  if (c.needs) return c.needs;
  if (c.when === brushSel) return `${PICK_BRUSH}.`;
  return `${PICK}.`;
}

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
    { id: "edit.undo", label: "Undo", menu: "Edit", hint: s.doc?.undo ? `Undo ${s.doc.undo}` : undefined, when: () => !!S().doc?.undo, needs: "Nothing to undo.", run: () => void S().undo(), fly: true },
    { id: "edit.redo", label: "Redo", menu: "Edit", hint: s.doc?.redo ? `Redo ${s.doc.redo}` : undefined, when: () => !!S().doc?.redo, needs: "Nothing to redo.", run: () => void S().redo(), fly: true },
    { id: "edit.history", label: "Undo history", menu: "Edit", hint: "Click a step to go back to it", run: () => openTab("history") },
    { id: "edit.cut", label: "Cut", menu: "Edit", sep: true, when: hasSel, needs: `${PICK} to cut it.`, run: () => void copySelection(true) },
    { id: "edit.copy", label: "Copy", menu: "Edit", when: hasSel, needs: `${PICK} to copy it.`, run: () => void copySelection(false) },
    { id: "edit.paste", label: "Paste", menu: "Edit", run: () => void paste() },
    { id: "edit.pasteSpecial", label: "Paste special…", menu: "Edit", hint: "Copies in a row, ring or spiral", when: hasSel, needs: `${PICK} to paste copies of it.`, run: () => S().set({ dialog: "paste" }) },
    { id: "edit.duplicate", label: "Duplicate", menu: "Edit", hint: "A copy one grid step over, with new target names", when: hasSel, needs: `${PICK} to duplicate it.`, run: () => void duplicateWith([S().grid, S().grid, 0]) },
    { id: "edit.duplicateHere", label: "Duplicate in place", menu: "Edit", hint: "A copy right on top (move it next)", when: hasSel, needs: `${PICK} to duplicate it.`, run: () => void duplicateWith([0, 0, 0]) },
    { id: "edit.duplicateSame", label: "Duplicate (keep names)", menu: "Edit", hidden: true, hint: "A copy with the same target names", when: hasSel, needs: `${PICK} to duplicate it.`, run: () => void duplicate() },
    { id: "edit.delete", label: "Delete", menu: "Edit", when: hasSel, needs: `${PICK} to delete it.`, run: () => deleteSelection() },
    {
      id: "edit.applyTexture",
      label: "Apply the current texture",
      menu: "Edit",
      sep: true,
      hint: "To the picked faces, or every face of the picked brushes",
      when: () => hasSel() || !!S().sel.face,
      needs: "Pick a brush or a face first to apply the texture.",
      run: () => {
        const s = S();
        const faces = s.sel.faces?.length ? s.sel.faces : s.sel.face ? [s.sel.face] : [];
        if (faces.length) void s.cmd({ op: "set_texture", faces: faces.map((f) => [f.brush, f.face]), texture: s.texture });
        else void s.cmd({ op: "set_texture", faces: selectionIds().brushes.map((b) => [b, null]), texture: s.texture });
      },
    },
    // Unreal habits (the Inspector's Copy / Paste, End drops to the floor)
    { id: "edit.copyProps", label: "Copy properties", menu: "Edit", sep: true, hint: "The picked entity's keys (not its class, place, brush model or name), to paste onto others", when: () => S().sel.entities.length > 0, needs: "Pick an entity first to copy its properties.", run: () => copyProps() },
    { id: "edit.pasteProps", label: "Paste properties", menu: "Edit", hint: "Onto every picked entity; keys a class doesn't have are skipped", when: () => S().sel.entities.length > 0 && clipNow()?.kind === "props", needs: "Copy an entity's properties first, then pick the entities to paste them onto.", run: () => void pasteProps() },
    { id: "edit.copyTransform", label: "Copy transform", menu: "Edit", hint: "Location, rotation, size and scale of what's picked", when: hasSel, needs: `${PICK} to copy its transform.`, run: () => copyTransform() },
    { id: "edit.pasteTransform", label: "Paste transform", menu: "Edit", hint: "Onto what's picked: its middle goes to the copied location", when: () => canPasteTransform(), needs: "Copy a transform first, then pick what gets it.", run: () => void pasteTransform() },
    { id: "edit.dropToFloor", label: "Drop to the floor", menu: "Edit", hint: "Down onto the brush below (like Unreal's End)", when: hasSel, needs: `${PICK} to drop it to the floor.`, run: () => void dropToFloor() },
    { id: "edit.find", label: "Find and replace…", menu: "Edit", sep: true, hint: "Textures, classnames, keys and values", run: () => useP5.getState().set({ dialog: "find" }) },

    // Brush (1.0.1: Hammer's Tools menu; the ids stay edit.* so keys and layouts carry over)
    { id: "edit.transform", label: "Transform…", menu: "Brush", hint: "Move, turn or stretch by exact numbers", when: hasSel, needs: `${PICK} to transform it.`, run: () => useP5.getState().set({ dialog: "transform" }) },
    { id: "edit.replaceWith", label: "Replace with entity or prefab…", menu: "Brush", hint: "Swap what's picked for another entity or a prefab", when: hasSel, needs: `${PICK} to replace it.`, run: () => useP5.getState().set({ dialog: "replaceWith" }) },
    { id: "edit.group", label: "Group", menu: "Brush", sep: true, when: hasSel, needs: `${PICK} to group it.`, run: () => void S().cmd({ op: "group", name: `Group ${(S().doc?.groups.length ?? 0) + 1}`, brushes: S().sel.brushes, entities: S().sel.entities }) },
    {
      id: "edit.ungroup",
      label: "Ungroup",
      menu: "Brush",
      when: () => ungroupTarget() != null,
      needs: "Pick something in a group first to ungroup it.",
      run: () => {
        const g = ungroupTarget();
        if (g != null) void S().cmd({ op: "ungroup", group: g });
      },
    },
    { id: "edit.tie", label: "Tie to entity", menu: "Brush", hint: "func_wall (pick another in Inspect)", when: brushSel, needs: `${PICK_BRUSH} to tie it to an entity.`, run: async () => {
      // the new entity is what's picked after (so Move to world works straight away)
      const r = await S().cmd({ op: "tie", brushes: selectionIds().brushes, classname: "func_wall" });
      if (r.created?.entities.length) S().select({ brushes: [], entities: r.created.entities, face: null });
    } },
    {
      id: "edit.toWorld",
      label: "Move to world",
      menu: "Brush",
      hint: "Brush entities become world brushes",
      when: () => S().sel.entities.some((e) => (S().doc?.entities.find((x) => x.id === e)?.brushes.length ?? 0) > 0),
      needs: "Pick a brush entity (a func_wall, a door, a trigger) first to move it to the world.",
      run: async () => {
        for (const e of S().sel.entities) if (S().doc?.entities.find((x) => x.id === e)?.brushes.length) await S().cmd({ op: "to_world", entity: e });
      },
    },
    { id: "edit.hollow", label: "Hollow", menu: "Brush", sep: true, when: brushSel, needs: `${PICK_BRUSH} to hollow it.`, run: () => void S().cmd({ op: "hollow", brushes: selectionIds().brushes, thickness: S().thickness }) },
    { id: "edit.carve", label: "Carve", menu: "Brush", hint: "Cut this brush out of what it overlaps", when: () => selectionIds().brushes.length === 1, needs: "Pick one brush first: Carve cuts it out of what it overlaps.", run: () => void S().cmd({ op: "carve", cutter: selectionIds().brushes[0], targets: [...S().brushes.keys()] }) },
    { id: "edit.merge", label: "Merge", menu: "Brush", when: () => selectionIds().brushes.length > 1, needs: "Pick two or more brushes first to merge them.", run: () => void S().cmd({ op: "merge", brushes: selectionIds().brushes }) },
    { id: "edit.snap", label: "Snap to grid", menu: "Brush", when: brushSel, needs: `${PICK_BRUSH} to snap it to the grid.`, run: () => void S().cmd({ op: "snap", brushes: selectionIds().brushes, grid: S().grid }) },
    ...[0, 1, 2].map((a): Command => ({ id: `edit.mirror${AX[a]}`, label: `Flip along ${AX[a]}`, menu: "Brush", sep: a === 0, when: hasSel, needs: `${PICK} to flip it.`, run: () => mirror(a) })),
    ...[0, 1, 2].map((a): Command => ({ id: `edit.turn${AX[a]}`, label: `Turn 90° around ${AX[a]}`, menu: "Brush", when: hasSel, needs: `${PICK} to turn it.`, run: () => turn(a, 90) })),
    ...[0, 1, 2].flatMap((a): Command[] =>
      (["min", "centre", "max"] as const).map((edge) => ({
        id: `edit.align${AX[a]}${edge}`,
        label: `Line up on ${AX[a]}: ${edge === "min" ? (a === 2 ? "bottoms" : "low sides") : edge === "max" ? (a === 2 ? "tops" : "high sides") : "middles"}`,
        menu: "Brush",
        hidden: true,
        when: () => selectionIds().brushes.length + S().sel.entities.length > 1,
        needs: "Pick two or more things first to line them up.",
        run: () => void align(a, edge),
      })),
    ),
    ...[0, 1, 2].map((a): Command => ({ id: `edit.distribute${AX[a]}`, label: `Spread out evenly along ${AX[a]}`, menu: "Brush", hidden: true, when: () => selectionIds().brushes.length + S().sel.entities.length > 2, needs: "Pick three or more things first to spread them out.", run: () => void distribute(a) })),

    // Select
    { id: "sel.all", label: "Select all", menu: "Select", run: selectAll },
    { id: "sel.none", label: "Select none", menu: "Select", run: () => S().select(EMPTY_SEL) },
    { id: "sel.invert", label: "Invert selection", menu: "Select", run: invert },
    { id: "sel.texture", label: "Select same texture", menu: "Select", sep: true, hint: "Everything with the picked brush's or face's textures", when: () => hasSel() || !!S().sel.face, needs: "Pick a brush or a face first to find others with its texture.", run: () => void similar({ by: "texture" }) },
    { id: "sel.class", label: "Select same type", menu: "Select", hint: "Every entity of the picked kind (world brushes: all world brushes)", when: hasSel, needs: `${PICK} to find others of its type.`, run: () => void similar({ by: "class" }) },
    {
      id: "sel.value",
      label: "Select same name",
      menu: "Select",
      hint: "Entities sharing the picked one's targetname",
      when: () => S().sel.entities.length > 0,
      needs: "Pick an entity first to find others with its name.",
      run: () => void similar({ by: "value", key: "targetname" }),
    },
    { id: "sel.touching", label: "Select touching", menu: "Select", sep: true, hint: "Things whose box touches the selection's", when: hasSel, needs: `${PICK} to find what touches it.`, run: () => void touching(false) },
    { id: "sel.inside", label: "Select inside", menu: "Select", hint: "Things wholly inside the selection's box", when: hasSel, needs: `${PICK} to find what's inside it.`, run: () => void touching(true) },
    { id: "sel.goto", label: "Go to brush or entity number…", menu: "Select", hint: "For compiler messages (Entity 3, Brush 12)", run: () => useP5.getState().set({ dialog: "goto" }) },
    { id: "sel.hide", label: "Hide", menu: "Select", sep: true, when: hasSel, needs: `${PICK} to hide it.`, run: () => hideSelection() },
    { id: "sel.isolate", label: "Hide everything else", menu: "Select", when: hasSel, needs: `${PICK} to hide everything else.`, run: () => isolateSelection() },
    { id: "sel.showAll", label: "Show everything", menu: "Select", run: () => showAll() },
    { id: "sel.hideEntities", label: "Hide all point entities", menu: "Select", hint: "Spawns, lights, sounds…", run: () => hideKind("point") },
    { id: "sel.hideTriggers", label: "Hide triggers and zones", menu: "Select", hint: "trigger_*, buy zones, bomb sites, rescue zones", run: () => hideKind("trigger") },
    { id: "sel.hideWorld", label: "Hide world brushes", menu: "Select", run: () => hideKind("world") },
    { id: "sel.lock", label: "Lock", menu: "Select", hint: "Can't be picked until unlocked", when: hasSel, needs: `${PICK} to lock it.`, run: () => lockSelection(true) },
    { id: "sel.unlockAll", label: "Unlock everything", menu: "Select", when: () => useP5.getState().locked.brushes.size + useP5.getState().locked.entities.size > 0, needs: "Nothing is locked.", run: () => lockSelection(false) },
    { id: "sel.groups", label: "Clicks pick whole groups", menu: "Select", sep: true, run: () => S().set({ selectMode: "groups" }) },
    { id: "sel.objects", label: "Clicks pick single objects", menu: "Select", run: () => S().set({ selectMode: "objects" }) },

    // View
    ...LAYOUTS.map((l, i): Command => ({ id: `view.layout.${l.id}`, label: `Layout: ${l.label}`, menu: "View", hint: l.tip, run: () => setLayout(l.id), fly: true, sep: i === 0 })),
    { id: "view.maximise", label: "Maximize the pane / put it back", menu: "View", when: () => usePanes.getState().layout !== "single", needs: "One pane already fills the view: pick a layout with more panes first (View > Layout).", run: () => toggleMaximise(), fly: true },
    ...(["3d", "top", "front", "side"] as ViewId[]).map((v, i): Command => ({ id: `view.${v}`, label: `${v === "3d" ? "3D" : v[0].toUpperCase() + v.slice(1)} view`, menu: "View", sep: i === 0, hint: "In the active pane", run: () => setView(v), fly: true })),
    { id: "view.frame", label: "Frame selection", menu: "View", sep: true, when: () => hasSel() || !!S().sel.face, needs: `${PICK} to frame it.`, run: () => frame(false) },
    { id: "view.frameAll", label: "Frame the whole map", menu: "View", run: () => frame(true) },
    { id: "view.fly", label: "Fly / orbit camera", menu: "View", run: () => useTools.getState().setCamera(useTools.getState().camera === "fly" ? "orbit" : "fly"), fly: true },
    { id: "view.gridDown", label: "Smaller grid", menu: "View", sep: true, run: () => stepGrid(-1), fly: true },
    { id: "view.gridUp", label: "Bigger grid", menu: "View", run: () => stepGrid(1), fly: true },
    { id: "view.gridShow", label: "Show / hide the grid", menu: "View", run: () => useP5.getState().set({ showGrid: !useP5.getState().showGrid }), fly: true },
    { id: "view.gameView", label: "Game view on / off", menu: "View", hint: "Hide the editor's helpers (grid, target links, previews) to see the map as it plays, like Unreal's G", run: () => useP5.getState().set({ gameView: !useP5.getState().gameView }), fly: true },
    { id: "view.gridSettings", label: "Grid and snapping…", menu: "View", hint: "Grid size, turn steps, snap to corners and edges", run: () => useP5.getState().set({ dialog: "grid" }) },
    { id: "view.texLock", label: "Texture lock on / off", menu: "View", run: () => S().set({ textureLock: !S().textureLock }) },
    { id: "view.frameTime", label: "Show frame time", menu: "View", sep: true, hint: "Frames a second and frame time (p50, p95) over the last 5 seconds; Ctrl+Alt+Shift+F", run: () => toggleFrameTime() }, // MCP
    { id: "view.snap", label: "Snap to grid on / off", menu: "View", hint: "Alt still flips it while held", run: () => S().set({ snapOff: !S().snapOff }), fly: true },
    { id: "view.geoSnap", label: "Snap to corners and edges on / off", menu: "View", run: () => useP5.getState().set({ geoSnap: !useP5.getState().geoSnap }) },
    ...NAV_PRESETS.map((n, i): Command => ({ id: `view.nav.${n.id}`, label: `Move like ${n.label}`, menu: "View", sep: i === 0, hint: n.tip, run: () => setNavPreset(n.id), fly: true })),
    { id: "view.speedUp", label: "Camera faster", menu: "View", hidden: true, run: () => stepSpeed(1), fly: true },
    { id: "view.speedDown", label: "Camera slower", menu: "View", hidden: true, run: () => stepSpeed(-1), fly: true },
    { id: "view.models", label: "Models in place of entity boxes on / off", menu: "View", hint: "Hostages, spawns, weapons, cyclers drawn as their models", run: () => useP5.getState().set({ showModels: !useP5.getState().showModels }), fly: true },
    { id: "view.paneMode", label: "Next drawing style for the active pane", menu: "View", hint: "Textured, flat, wireframe, textured + wire, collision, by entity type, missing textures", run: () => cyclePaneMode(), fly: true },
    { id: "view.style.collision", label: "Draw what blocks what (Collision)", menu: "View", hint: "Solid gray, CLIP orange, NULL red, triggers purple, ladders green, water blue, illusionary ghosted; model boxes as wireframes", run: () => setPaneMode(usePanes.getState().active, "collision") },
    { id: "view.style.bytype", label: "Draw brushes by entity type", menu: "View", hint: "World, func_detail, func_wall, doors, breakables, illusionary, triggers", run: () => setPaneMode(usePanes.getState().active, "bytype") },
    { id: "view.style.missing", label: "Show missing textures", menu: "View", hint: "Textures that can't be found in magenta, everything else muted: find them before you compile", run: () => setPaneMode(usePanes.getState().active, "missing") },

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
    { id: "view.content", label: "Content browser", menu: "View", sep: true, hint: "Models, sprites, sounds, textures, prefabs and GameBanana: drag onto the map", run: () => showPanel("content") },
    { id: "view.contentDrawer", label: "Content drawer", menu: "View", hint: "The Content browser over the bottom panels (a click away closes it)", run: () => useBrowser.getState().set({ drawer: !useBrowser.getState().drawer }), fly: true },
    { id: "view.budget", label: "Budget", menu: "View", hint: "How close the map is to the engine's limits", run: () => showPanel("budget") },
    { id: "view.io", label: "I/O: what triggers what", menu: "View", hint: "Targets, multi_managers, masters and broken links; pick an entity for its chain", run: () => showPanel("io") },
    { id: "map.publish", label: "Publish…", menu: "Map", hint: "A package to share this map in Tools > Community (compile it into the game first)", run: publishMap },
    { id: "map.playtest", label: "Playtest", menu: "Map", hint: "Compile, put it in the game and start it with bots, as the Compile panel is set", run: playtestNow },
    { id: "map.goToLeak", label: "Go to leak", menu: "Map", hint: "Fly along the leak line to the hole and pick the brush beside it", when: () => !!useLeak.getState().points, needs: "No leak line yet: compile first, or load a pointfile (Map menu).", run: () => goToLeak() },
    { id: "map.loadPointfile", label: "Load pointfile…", menu: "Map", hint: "A leak line (.lin or .pts) from another compile", run: () => void loadLeakFile() },
    { id: "map.leakLine", label: "Show or hide the leak line", menu: "Map", when: () => !!useLeak.getState().points, needs: "No leak line yet: compile first, or load a pointfile (Map menu).", run: () => useLeak.getState().toggle() },
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
