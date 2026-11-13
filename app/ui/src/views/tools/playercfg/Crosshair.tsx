// The crosshair as the game draws it (the CS client's DrawCrosshair, as far as it's known):
// four 1-pixel lines around the middle of the screen, their gap and length growing with the
// weapon's spread when the crosshair is dynamic; the size setting picks the screen width the
// crosshair is drawn for (small 1024, medium 800, large 640, auto from the real width);
// see-through means drawn added onto the picture (it fades on bright walls), solid means
// painted over it. Drawn 1:1 over a sandy, dust-like scene at the resolution picked.

import { useEffect, useRef } from "react";

export type Motion = "still" | "run" | "jump" | "shoot";

type Props = {
  colour: string;
  size: string;
  translucent: boolean;
  dynamic: boolean;
  show: boolean;
  motion: Motion;
  /** the game's screen width the preview stands for */
  width: number;
  zoom: number;
};

const W = 360;
const H = 220;

function rgb(c: string): [number, number, number] {
  const p = c.split(/\s+/).map((n) => Math.max(0, Math.min(255, Number(n) || 0)));
  return [p[0] ?? 50, p[1] ?? 250, p[2] ?? 50];
}

/** A plain dust-like view: sky, a sandstone wall with a doorway, crates and a floor. */
function scene(g: CanvasRenderingContext2D) {
  const sky = g.createLinearGradient(0, 0, 0, H * 0.45);
  sky.addColorStop(0, "#7fa6c9");
  sky.addColorStop(1, "#c9d8e0");
  g.fillStyle = sky;
  g.fillRect(0, 0, W, H);
  // far wall
  g.fillStyle = "#c8a46a";
  g.fillRect(0, H * 0.28, W, H * 0.42);
  for (let y = H * 0.28; y < H * 0.7; y += 12) {
    g.fillStyle = "rgba(90,60,30,.18)";
    g.fillRect(0, y, W, 1);
    for (let x = (y / 12) % 2 ? 0 : 20; x < W; x += 40) g.fillRect(x, y, 1, 12);
  }
  // doorway in shadow, right where the crosshair sits
  g.fillStyle = "#3a2a1c";
  g.fillRect(W / 2 - 26, H * 0.36, 52, H * 0.34);
  g.fillStyle = "#1c140d";
  g.fillRect(W / 2 - 20, H * 0.42, 40, H * 0.28);
  // floor
  const floor = g.createLinearGradient(0, H * 0.7, 0, H);
  floor.addColorStop(0, "#b48f58");
  floor.addColorStop(1, "#8a6a3e");
  g.fillStyle = floor;
  g.fillRect(0, H * 0.7, W, H * 0.3);
  // crates
  const crate = (x: number, y: number, s: number) => {
    g.fillStyle = "#8a6232";
    g.fillRect(x, y, s, s);
    g.strokeStyle = "#5a3e1e";
    g.lineWidth = 2;
    g.strokeRect(x + 1, y + 1, s - 2, s - 2);
    g.beginPath();
    g.moveTo(x + 2, y + 2);
    g.lineTo(x + s - 2, y + s - 2);
    g.moveTo(x + s - 2, y + 2);
    g.lineTo(x + 2, y + s - 2);
    g.stroke();
  };
  crate(24, H * 0.7 - 58, 58);
  crate(82, H * 0.7 - 40, 40);
  crate(W - 96, H * 0.7 - 70, 70);
  // a bright patch of sun on the left wall: see-through crosshairs fade there
  g.fillStyle = "rgba(255,245,215,.35)";
  g.fillRect(W / 2 + 40, H * 0.3, 70, H * 0.38);
}

/** Gap and line length for a rifle (distance 4, more per shot) in the motion asked for. */
function spread(dynamic: boolean, motion: Motion, size: string, width: number) {
  let dist = 4;
  if (dynamic) {
    if (motion === "jump") dist *= 2;
    else if (motion === "run") dist *= 1.5;
    else if (motion === "shoot") dist = 4 + 4 * 2.2;
  }
  dist = Math.min(dist, 15);
  let bar = Math.floor((dist - 4) * 0.5) + 5;
  const base = size === "small" ? 1024 : size === "medium" ? 800 : size === "large" ? 640 : width < 640 ? 1024 : width < 1024 ? 800 : 640;
  dist = (width * dist) / base;
  bar = Math.max(1, Math.round((width * bar) / base));
  return { dist: Math.round(dist), bar };
}

export function CrosshairPreview({ colour, size, translucent, dynamic, show, motion, width, zoom }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    const g = c?.getContext("2d");
    if (!c || !g) return;
    g.globalCompositeOperation = "source-over";
    scene(g);
    if (!show) return;
    const [r, gg, b] = rgb(colour);
    const { dist, bar } = spread(dynamic, motion, size, width);
    const x = Math.floor(W / 2);
    const y = Math.floor(H / 2);
    // see-through: added onto the picture, as the game's FillRGBA does
    g.globalCompositeOperation = translucent ? "lighter" : "source-over";
    g.fillStyle = translucent ? `rgba(${r},${gg},${b},0.78)` : `rgb(${r},${gg},${b})`;
    g.fillRect(x - dist - bar + 1, y, bar, 1);
    g.fillRect(x + dist, y, bar, 1);
    g.fillRect(x, y - dist - bar + 1, 1, bar);
    g.fillRect(x, y + dist, 1, bar);
    g.globalCompositeOperation = "source-over";
  }, [colour, size, translucent, dynamic, show, motion, width]);
  return <canvas ref={ref} className="pc-canvas" width={W} height={H} style={{ width: W * zoom }} role="img" aria-label="Your crosshair on a dust-like wall, as the game draws it" />;
}
