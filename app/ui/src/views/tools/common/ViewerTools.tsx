// The map viewer's "Go to" panel: jump to coordinates or a pasted link, camera bookmarks
// (kept per map), a picture of the view, and a link to the spot you're looking at.
//
// ViewerControl goes inside the 3D view (a Scene child) and hands the camera to the panel;
// the panel itself is plain UI. Keys while the viewer is open: G go to, B bookmark, P picture.

import { useThree } from "@react-three/fiber";
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { copyText } from "../../../api";
import { useTools } from "../../../stores/tools";
import { Info, useToast } from "../../../ui";
import { gsToThree, threeToGs, type Vec3 } from "../viewer/coords";
import { parseAddress, parsePoint, spotAddress } from "./address";
import { post } from "./api";
import "./qol.css";

type Pose = { pos: Vec3; target: Vec3 };
type Bookmark = Pose & { name: string };

/** What the panel can do with the camera; set while a 3D view with ViewerControl is up. */
const control: {
  pose?: () => Pose;
  setPose?: (p: Pose) => void;
  shot?: () => Promise<Blob | null>;
} = {};

/** Inside the Canvas: lends the camera to the panel. */
export function ViewerControl() {
  const get = useThree((s) => s.get);
  useEffect(() => {
    control.pose = () => {
      const { camera, controls } = get();
      const c = controls as unknown as { target?: THREE.Vector3 } | null;
      const target = c?.target ? c.target.clone() : camera.position.clone().add(camera.getWorldDirection(new THREE.Vector3()).multiplyScalar(256));
      return { pos: threeToGs(camera.position), target: threeToGs(target) };
    };
    control.setPose = (p) => {
      const t = useTools.getState();
      // a bookmark is a place to stand: from top-down, back to orbiting
      if (t.camera === "top") t.setCamera("orbit");
      // (the camera controls for a new mode mount on the next render)
      setTimeout(() => {
        const { camera, controls, invalidate } = get();
        const c = controls as unknown as { target?: THREE.Vector3; update?: () => void } | null;
        camera.position.copy(gsToThree(p.pos));
        camera.up.set(0, 1, 0);
        camera.lookAt(gsToThree(p.target));
        if (c?.target && c.update) {
          c.target.copy(gsToThree(p.target));
          c.update();
        }
        invalidate();
      }, 60);
    };
    control.shot = () =>
      new Promise((resolve) => {
        const { gl, scene, camera } = get();
        // (the view draws on demand: draw it now, and read it before the browser clears it)
        gl.render(scene, camera);
        gl.domElement.toBlob((b) => resolve(b), "image/png");
      });
    return () => {
      delete control.pose;
      delete control.setPose;
      delete control.shot;
    };
  }, [get]);
  return null;
}

const KEY = (map: string) => `ls.tools.bookmarks.${map}`;
const MAX_BOOKMARKS = 24;

function loadBookmarks(map: string): Bookmark[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY(map)) ?? "[]");
    const vec = (x: unknown): x is Vec3 => Array.isArray(x) && x.length === 3 && x.every((n) => typeof n === "number" && Number.isFinite(n));
    return Array.isArray(v) ? v.filter((b) => b && typeof b.name === "string" && vec(b.pos) && vec(b.target)).slice(0, MAX_BOOKMARKS) : [];
  } catch {
    return [];
  }
}
function saveBookmarks(map: string, list: Bookmark[]) {
  try {
    localStorage.setItem(KEY(map), JSON.stringify(list));
  } catch {
    // storage off: they last until the page closes
  }
}

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));
const round = (v: Vec3) => v.map((n) => Math.round(n)) as Vec3;

/** The panel, for the map viewer's side column. */
export function ViewerTools({ map }: { map: string }) {
  const toast = useToast();
  const [text, setText] = useState("");
  const [bad, setBad] = useState(false);
  const [marks, setMarks] = useState<Bookmark[]>(() => loadBookmarks(map));
  const [saving, setSaving] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => setMarks(loadBookmarks(map)), [map]);

  const goTo = () => {
    const t = text.trim();
    if (!t) return;
    const addr = /[a-z]+:/i.test(t) ? parseAddress(t) : null;
    const at = addr?.at ?? parsePoint(t);
    if (addr && addr.map && addr.map !== map && (addr.tool === "map" || addr.tool === "")) {
      // a link to another map: open it there
      useTools.getState().openMap(addr.map);
      if (addr.at) useTools.getState().look(addr.at, addr.size ?? 192);
      setText("");
      return;
    }
    if (!at) {
      setBad(true);
      return;
    }
    setBad(false);
    useTools.getState().look(at, addr?.size ?? 192);
  };

  const addBookmark = () => {
    const p = control.pose?.();
    if (!p) return toast("The 3D view isn't up yet.", true);
    const name = `Spot ${marks.length + 1}`;
    const next = [...marks, { name, pos: round(p.pos), target: round(p.target) }].slice(-MAX_BOOKMARKS);
    setMarks(next);
    saveBookmarks(map, next);
    toast(`Kept as “${name}”.`);
  };
  const removeBookmark = (i: number) => {
    const next = marks.filter((_, k) => k !== i);
    setMarks(next);
    saveBookmarks(map, next);
  };
  const rename = (i: number, name: string) => {
    const next = marks.map((b, k) => (k === i ? { ...b, name: name.slice(0, 40) || b.name } : b));
    setMarks(next);
    saveBookmarks(map, next);
  };

  const picture = async () => {
    if (saving) return;
    const blob = await control.shot?.();
    if (!blob) return toast("Couldn't take a picture of the view.", true);
    setSaving(true);
    try {
      const name = `${map}-${new Date().toISOString().slice(0, 19).replace(/[T:]/g, "-")}.png`;
      // the app saves it where you choose; a browser (development) downloads it
      const r = await post<{ ok?: boolean; cancelled?: boolean; message?: string }>(`/api/tools/screenshot?name=${encodeURIComponent(name)}`, await blobToBase64(blob));
      if (r.ok) toast(r.message ?? "Saved.");
      else if (!r.cancelled) {
        const a = document.createElement("a");
        a.href = URL.createObjectURL(blob);
        a.download = name;
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
      }
    } finally {
      setSaving(false);
    }
  };

  const copyLink = async () => {
    const p = control.pose?.();
    const t = useTools.getState();
    const at = t.selection?.kind === "face" ? t.selection.point : p ? p.target : t.status?.pos;
    const link = `ls-tools:${spotAddress("map", map, at ?? null)}`;
    if (await copyText(link)) toast("Copied a link to this spot. Paste it in Go to, here or in another tool.");
    else toast("Couldn't copy.", true);
  };

  // G go to, B bookmark, P picture (not while typing, flying with the mouse, or in a dialog)
  const keys = useRef({ addBookmark, picture });
  keys.current = { addBookmark, picture };
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (isField(e.target) || e.metaKey || e.ctrlKey || e.altKey || document.pointerLockElement || document.querySelector(".sheet-backdrop")) return;
      if (e.code === "KeyG") {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      } else if (e.code === "KeyB") keys.current.addBookmark();
      else if (e.code === "KeyP") void keys.current.picture();
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, []);

  return (
    <section className="tools-panel tools-goto" aria-label="Go to">
      <h3>
        Go to <Info text="Type x y z (game units, as the status bar shows them, or a setpos line from the console), or paste a link to a spot. Keys: G go to, B keep this camera, P picture." />
      </h3>
      <form
        className="row tools-goto-row"
        onSubmit={(e) => {
          e.preventDefault();
          goTo();
        }}
      >
        <input
          ref={input}
          className={bad ? "search bad" : "search"}
          aria-invalid={bad || undefined}
          aria-label="Where to go: x y z, or a link"
          placeholder="x y z, or a link"
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setBad(false);
          }}
          onKeyDown={(e) => e.key === "Escape" && (e.target as HTMLInputElement).blur()}
        />
        <button className="btn small" type="submit" disabled={!text.trim()}>
          Go
        </button>
      </form>
      {bad && <p className="small-note bad-text">That isn't three numbers or a link to a spot.</p>}
      <div className="row tools-goto-actions">
        <button className="btn small" onClick={addBookmark} title="Keep where the camera is now (B)">
          Keep this view
        </button>
        <button className="btn small" onClick={() => void picture()} disabled={saving} title="Save a picture of the view (P)">
          {saving ? (
            <>
              <span className="spinner" aria-hidden="true" /> Saving…
            </>
          ) : (
            "Picture"
          )}
        </button>
        <button className="btn small" onClick={() => void copyLink()} title="Copy a link to the spot you're looking at">
          Copy link
        </button>
      </div>
      {marks.length > 0 && (
        <ul className="tools-bookmarks">
          {marks.map((b, i) => (
            <li key={i} className="row">
              <button className="link inline" title={`From ${b.pos.join(" ")}`} onClick={() => control.setPose?.(b)}>
                {b.name}
              </button>
              <span className="push" />
              <button
                className="link inline muted"
                aria-label={`Rename ${b.name}`}
                onClick={() => {
                  const name = window.prompt("Name this view", b.name);
                  if (name !== null) rename(i, name.trim());
                }}
              >
                Rename
              </button>
              <button className="link inline muted" aria-label={`Forget ${b.name}`} onClick={() => removeBookmark(i)}>
                Forget
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function blobToBase64(b: Blob): Promise<{ png: string }> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve({ png: String(r.result).replace(/^data:image\/png;base64,/, "") });
    r.onerror = () => reject(r.error);
    r.readAsDataURL(b);
  });
}
