// One sound at a time for the whole editor: the Content Browser's tiles, its details pane and
// the Inspect panel's Sound file buttons all share this player, so starting one stops the
// others and every button shows whether its sound is the one playing.

import { useSyncExternalStore } from "react";
import { soundUrl } from "./editorApi";

export type SoundState = "idle" | "loading" | "playing" | "paused" | "missing";

let audio: HTMLAudioElement | null = null;
let current: { path: string; state: SoundState } = { path: "", state: "idle" };
const listeners = new Set<() => void>();

function set(path: string, state: SoundState) {
  current = { path, state };
  listeners.forEach((l) => l());
}

/** Play `path`; if it is already playing, pause it; if paused, carry on. Stops any other sound. */
export function toggleSound(path: string) {
  const p = path.trim();
  if (!p) return;
  if (audio && current.path === p) {
    if (current.state === "playing" || current.state === "loading") {
      audio.pause();
      return set(p, "paused");
    }
    if (current.state === "paused") {
      set(p, "playing");
      void audio.play().catch(() => set(p, "missing"));
      return;
    }
  }
  audio?.pause();
  const a = new Audio(soundUrl(p));
  audio = a;
  a.onplaying = () => audio === a && set(p, "playing");
  a.onended = () => audio === a && set(p, "idle");
  a.onerror = () => audio === a && set(p, "missing");
  set(p, "loading");
  void a.play().catch(() => audio === a && current.state !== "paused" && set(p, "missing"));
}

/** Start `path` unless it is already playing (hover-to-listen). */
export function playSound(path: string) {
  const p = path.trim();
  if (current.path === p && (current.state === "playing" || current.state === "loading")) return;
  if (current.path === p && current.state === "paused") audio = null;
  toggleSound(p);
}

/** Stop whatever is playing. */
export function stopSound() {
  audio?.pause();
  audio = null;
  if (current.path) set("", "idle");
}

const subscribe = (l: () => void) => (listeners.add(l), () => listeners.delete(l));
const snapshot = () => current;

/** This path's state in the shared player. */
export function useSound(path: string): SoundState {
  const c = useSyncExternalStore(subscribe, snapshot);
  return c.path === path.trim() ? c.state : "idle";
}
