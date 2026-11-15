// How imported sounds are converted (tools/soundconv.rs): WAV, MP3, Ogg or FLAC in, a WAV the
// engine plays well out. One small "Sound options" disclosure wherever sounds are imported (the
// asset pickers, the Content Browser); the choice is remembered on this computer.

import { useState } from "react";
import "./soundOptions.css";

export type SoundOpts = { rate: 11025 | 22050 | 44100; bits: 8 | 16; stereo: boolean; trim: boolean };
export const SOUND_DEFAULTS: SoundOpts = { rate: 22050, bits: 16, stereo: false, trim: false };
/** The sound files an import takes (they become .wav). */
export const SOUND_ACCEPT = ".wav,.mp3,.ogg,.flac";
export const SOUND_EXT = /\.(wav|mp3|ogg|oga|flac)$/i;
const KEY = "ls.import.sound";

/** The options to send with an import (the last ones picked, or the defaults). */
export function soundOptions(): SoundOpts {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) || "null") as Partial<SoundOpts> | null;
    if (!v) return SOUND_DEFAULTS;
    return {
      rate: v.rate === 11025 || v.rate === 44100 ? v.rate : 22050,
      bits: v.bits === 8 ? 8 : 16,
      stereo: !!v.stereo,
      trim: !!v.trim,
    };
  } catch {
    return SOUND_DEFAULTS;
  }
}

function save(o: SoundOpts) {
  try {
    localStorage.setItem(KEY, JSON.stringify(o));
  } catch {
    /* (private window: not remembered) */
  }
}

/** "Sound options": sample rate, 8/16-bit, 2D only (keeps stereo), trim silence. */
export function SoundOptions() {
  const [o, setO] = useState<SoundOpts>(soundOptions);
  const set = (p: Partial<SoundOpts>) => {
    const n = { ...o, ...p };
    setO(n);
    save(n);
  };
  return (
    <details className="ai-sound-opts">
      <summary>Sound options</summary>
      <p className="muted">WAV, MP3, Ogg and FLAC become a WAV the game plays well.</p>
      <label>
        Rate{" "}
        <select value={o.rate} aria-label="Sample rate" onChange={(e) => set({ rate: Number(e.target.value) as SoundOpts["rate"] })}>
          <option value={11025}>11 kHz (small)</option>
          <option value={22050}>22 kHz</option>
          <option value={44100}>44.1 kHz</option>
        </select>
      </label>
      <label>
        Bits{" "}
        <select value={o.bits} aria-label="Bit depth" onChange={(e) => set({ bits: Number(e.target.value) as SoundOpts["bits"] })}>
          <option value={16}>16-bit</option>
          <option value={8}>8-bit (small)</option>
        </select>
      </label>
      <label className="tl-check" title="Stereo plays only for sounds without a place in the world (music, menus). Sounds in the world are mono.">
        <input type="checkbox" checked={o.stereo} onChange={(e) => set({ stereo: e.target.checked })} /> 2D only (keep stereo)
      </label>
      <label className="tl-check">
        <input type="checkbox" checked={o.trim} onChange={(e) => set({ trim: e.target.checked })} /> Trim silence
      </label>
    </details>
  );
}
