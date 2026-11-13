// Keys that work on every Tools page, and the sheet that lists each tool's keys (press ?).
//
//   ?          this sheet
//   /          the page's search box (Ctrl/⌘+F too, when the page has one)
//
// The lists below are what each tool's own key handlers do; keep them in step when a tool
// gains a key. "Mod" shows as ⌘ on a Mac and Ctrl elsewhere.

import { useEffect, useState } from "react";
import { copyText } from "../../../api";
import { useToolPage } from "../../../stores/toolPage";
import { Sheet, useToast } from "../../../ui";
import { TOOLS } from "../registry";
import { currentAddress } from "./address";
import "./qol.css";

type Keys = [string, string][];

const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
const mod = (k: string) => k.replace(/Mod\+/g, MAC ? "⌘" : "Ctrl+");

const EVERYWHERE: Keys = [
  ["?", "This list of keys"],
  ["/", "Search (the page's search box)"],
  ["Esc", "Close this list, let go of the mouse, clear what's picked"],
];

const VIEW_3D: Keys = [
  ["O", "Orbit camera: drag to turn, right-drag to pan, scroll to zoom"],
  ["F", "Fly camera: click the view, then WASD to move, Shift faster"],
  ["T", "Top-down camera"],
  ["G", "Go to a spot (type x y z, or paste a link)"],
  ["B", "Keep this camera as a bookmark"],
  ["P", "Save a picture of the view"],
];

export const SHORTCUTS: Record<string, Keys> = {
  map: VIEW_3D,
  replay: [...VIEW_3D, ["Space", "Play or pause"]],
  navfix: [
    ["V", "Select (Shift-click adds)"],
    ["D", "Draw an area (left-drag; right-drag pans)"],
    ["L", "Link: click one area, then the next (Shift: a one-way drop)"],
    ["O / T", "Orbit or top-down camera"],
    ["Delete", "Delete the picked areas"],
    ["Mod+Z", "Undo"],
    ["Shift+Mod+Z / Ctrl+Y", "Redo"],
    ["Esc", "Clear the pick and the highlighted problem"],
  ],
  callouts: [
    ["Mod+Z", "Undo"],
    ["Shift+Mod+Z", "Redo"],
    ["Esc", "Clear the pick"],
  ],
  editor: [
    ["Q W E R", "Select, Move, Rotate, Scale"],
    ["V", "Vertex: drag a brush's corners"],
    ["B / C", "Draw a box / another shape"],
    ["X", "Clip (Tab: which side stays, Enter: cut)"],
    ["N", "Place an entity"],
    ["T", "Texture: paint, slide and turn textures"],
    ["M", "Measure"],
    ["F", "Fly or orbit (3D view)"],
    ["Shift+1-4", "3D, top, front or side view"],
    ["1-9 / Ctrl+1-9", "Go to / keep a camera"],
    ["[ ]", "Smaller or bigger grid"],
    ["Alt", "Hold to ignore the grid"],
    ["Arrows, PgUp/PgDn", "Nudge one grid step"],
    ["Mod+D", "Duplicate"],
    ["Mod+C / Mod+X / Mod+V", "Copy, cut, paste (Shift+Mod+V: paste special)"],
    ["Mod+H", "Hollow"],
    ["H / Shift+H / U", "Hide, hide the rest, show all"],
    ["Mod+A", "Pick everything"],
    ["Delete", "Delete what's picked"],
    ["Mod+Z / Shift+Mod+Z", "Undo / redo"],
    ["Mod+S / Shift+Mod+S", "Save / save as"],
  ],
};

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** The page's search box: the first visible one in the content area. */
function searchBox(): HTMLInputElement | null {
  const boxes = document.querySelectorAll<HTMLInputElement>('.content input[type="search"], .tools-dev input[type="search"]');
  for (const b of boxes) if (b.offsetParent !== null && !b.disabled) return b;
  return null;
}

/** ? opens the keys for the open tool; / goes to the page's search. Mounted once by Tools. */
export function ToolKeys() {
  const [open, setOpen] = useState(false);
  const tool = useToolPage((s) => s.tool);
  const toast = useToast();

  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (e.altKey || document.querySelector(".sheet-backdrop:not(.tools-keysheet-backdrop)")) return;
      const typing = isField(e.target);
      if (e.key === "?" && !typing && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        setOpen((v) => !v);
        return;
      }
      const find = (e.key === "/" && !typing && !e.metaKey && !e.ctrlKey) || (e.key.toLowerCase() === "f" && (e.metaKey || e.ctrlKey) && !e.shiftKey);
      if (find) {
        const box = searchBox();
        if (!box) return;
        e.preventDefault();
        box.focus();
        box.select();
      }
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, []);

  if (!open) return null;
  const entry = TOOLS.find((t) => t.id === tool);
  const keys = SHORTCUTS[tool] ?? [];
  const copyLink = async () => {
    const a = currentAddress();
    if (await copyText(`ls-tools:${a || "overview"}`)) toast("Copied. Paste it in Go to (G) in the map viewer, or send it to someone with Loner-Strike.");
    else toast("Couldn't copy.", true);
  };
  return (
    <Sheet label="Keys" onClose={() => setOpen(false)} className="sheet tools-keysheet">
      <h2>Keys{entry ? ` · ${entry.label}` : ""}</h2>
      {keys.length > 0 && <KeyTable keys={keys} />}
      <h3>On every tool</h3>
      <KeyTable keys={EVERYWHERE} />
      <div className="row tools-keysheet-foot">
        {tool && (
          <button className="btn small" onClick={() => void copyLink()} title="A link to this tool and map (and the spot you're looking at)">
            Copy link to here
          </button>
        )}
        <span className="push" />
        <button className="btn small primary" onClick={() => setOpen(false)}>
          Done
        </button>
      </div>
    </Sheet>
  );
}

function KeyTable({ keys }: { keys: Keys }) {
  return (
    <dl className="tools-keysheet-list">
      {keys.map(([k, what]) => (
        <div key={k + what}>
          <dt>
            {mod(k)
              .split(" / ")
              .map((part, i) => (
                <span key={part}>
                  {i > 0 && <span className="muted"> or </span>}
                  <kbd>{part}</kbd>
                </span>
              ))}
          </dt>
          <dd>{what}</dd>
        </div>
      ))}
    </dl>
  );
}
