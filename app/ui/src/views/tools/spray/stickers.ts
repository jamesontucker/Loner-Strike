// The composer's built-in stickers: simple shapes drawn for Loner-Strike, each a path in a
// 100×100 box centred on 0,0 (so it turns and scales about its middle). `stroke` draws the
// path as lines that wide instead of filling it; `aspect` is height / width.

export type Sticker = { id: string; label: string; colour?: string; aspect?: number; stroke?: number; path: () => Path2D };

const cache = new Map<string, Path2D>();
const once = (id: string, make: (p: Path2D) => void) => () => {
  let p = cache.get(id);
  if (!p) {
    p = new Path2D();
    make(p);
    cache.set(id, p);
  }
  return p;
};

function star(p: Path2D, points: number, outer: number, inner: number, turn = -Math.PI / 2) {
  for (let i = 0; i < points * 2; i++) {
    const r = i % 2 ? inner : outer;
    const a = turn + (i * Math.PI) / points;
    if (i === 0) p.moveTo(Math.cos(a) * r, Math.sin(a) * r);
    else p.lineTo(Math.cos(a) * r, Math.sin(a) * r);
  }
  p.closePath();
}

function circle(p: Path2D, x: number, y: number, r: number) {
  p.moveTo(x + r, y);
  p.arc(x, y, r, 0, Math.PI * 2);
  p.closePath();
}

/** A paint splat: a blob with drips, the same every time. */
function splat(p: Path2D) {
  const lobes = 11;
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const pts: [number, number][] = [];
  for (let i = 0; i < lobes * 2; i++) {
    const a = (i * Math.PI) / lobes;
    const r = i % 2 ? 30 + rnd() * 6 : 38 + rnd() * 12;
    pts.push([Math.cos(a) * r, Math.sin(a) * r]);
  }
  p.moveTo((pts[0][0] + pts[pts.length - 1][0]) / 2, (pts[0][1] + pts[pts.length - 1][1]) / 2);
  for (let i = 0; i < pts.length; i++) {
    const n = pts[(i + 1) % pts.length];
    p.quadraticCurveTo(pts[i][0], pts[i][1], (pts[i][0] + n[0]) / 2, (pts[i][1] + n[1]) / 2);
  }
  p.closePath();
  for (const [x, y, r] of [[-44, -30, 4], [46, 20, 5], [30, -46, 3.5], [-38, 40, 3], [8, 48, 4]] as const) circle(p, x, y, r);
}

export const STICKERS: Sticker[] = [
  { id: "splat", label: "Splat", colour: "#d23a2a", path: once("splat", splat) },
  {
    id: "skull",
    label: "Skull",
    colour: "#f1ead8",
    aspect: 1.1,
    path: once("skull", (p) => {
      // head and jaw
      p.moveTo(-40, 4);
      p.bezierCurveTo(-46, -46, 46, -46, 40, 4);
      p.bezierCurveTo(38, 16, 28, 18, 26, 24);
      p.lineTo(26, 40);
      p.quadraticCurveTo(26, 46, 20, 46);
      p.lineTo(-20, 46);
      p.quadraticCurveTo(-26, 46, -26, 40);
      p.lineTo(-26, 24);
      p.bezierCurveTo(-28, 18, -38, 16, -40, 4);
      p.closePath();
      // eyes, nose, teeth (holes)
      p.moveTo(-6, 4);
      p.bezierCurveTo(-6, -12, -30, -12, -30, 2);
      p.bezierCurveTo(-30, 12, -8, 14, -6, 4);
      p.closePath();
      p.moveTo(6, 4);
      p.bezierCurveTo(6, -12, 30, -12, 30, 2);
      p.bezierCurveTo(30, 12, 8, 14, 6, 4);
      p.closePath();
      p.moveTo(0, 14);
      p.lineTo(6, 26);
      p.lineTo(-6, 26);
      p.closePath();
      for (const x of [-15, -5, 5]) p.rect(x, 33, 8, 10);
    }),
  },
  { id: "star", label: "Star", colour: "#f2c230", path: once("star", (p) => star(p, 5, 50, 21)) },
  { id: "burst", label: "Burst", colour: "#ff7818", path: once("burst", (p) => star(p, 14, 50, 36)) },
  {
    id: "heart",
    label: "Heart",
    colour: "#e0405a",
    aspect: 0.9,
    path: once("heart", (p) => {
      p.moveTo(0, 46);
      p.bezierCurveTo(-30, 22, -50, 4, -50, -18);
      p.bezierCurveTo(-50, -40, -26, -54, 0, -30);
      p.bezierCurveTo(26, -54, 50, -40, 50, -18);
      p.bezierCurveTo(50, 4, 30, 22, 0, 46);
      p.closePath();
    }),
  },
  {
    id: "bolt",
    label: "Bolt",
    colour: "#ffd23c",
    aspect: 1.4,
    path: once("bolt", (p) => {
      p.moveTo(14, -50);
      p.lineTo(-30, 6);
      p.lineTo(-2, 6);
      p.lineTo(-14, 50);
      p.lineTo(32, -10);
      p.lineTo(4, -10);
      p.closePath();
    }),
  },
  {
    id: "flame",
    label: "Flame",
    colour: "#ff6a1a",
    aspect: 1.25,
    path: once("flame", (p) => {
      p.moveTo(0, 50);
      p.bezierCurveTo(-36, 50, -44, 20, -34, -4);
      p.bezierCurveTo(-28, 8, -20, 12, -16, 10);
      p.bezierCurveTo(-26, -20, -8, -36, 4, -50);
      p.bezierCurveTo(4, -30, 18, -22, 22, -8);
      p.bezierCurveTo(26, -14, 28, -20, 28, -26);
      p.bezierCurveTo(44, -4, 42, 50, 0, 50);
      p.closePath();
    }),
  },
  {
    id: "crown",
    label: "Crown",
    colour: "#f2c230",
    aspect: 0.75,
    path: once("crown", (p) => {
      p.moveTo(-46, 34);
      p.lineTo(-50, -26);
      p.lineTo(-24, 0);
      p.lineTo(0, -38);
      p.lineTo(24, 0);
      p.lineTo(50, -26);
      p.lineTo(46, 34);
      p.closePath();
      for (const x of [-50, 0, 50]) circle(p, x, x === 0 ? -42 : -30, 6);
    }),
  },
  {
    id: "shield",
    label: "Shield",
    colour: "#3a6ea8",
    aspect: 1.15,
    path: once("shield", (p) => {
      p.moveTo(0, -50);
      p.lineTo(46, -36);
      p.bezierCurveTo(46, 10, 30, 36, 0, 50);
      p.bezierCurveTo(-30, 36, -46, 10, -46, -36);
      p.closePath();
    }),
  },
  {
    id: "target",
    label: "Crosshair",
    colour: "#3ce05a",
    stroke: 7,
    path: once("target", (p) => {
      circle(p, 0, 0, 34);
      circle(p, 0, 0, 4);
      for (const [x1, y1, x2, y2] of [[0, -50, 0, -18], [0, 18, 0, 50], [-50, 0, -18, 0], [18, 0, 50, 0]]) {
        p.moveTo(x1, y1);
        p.lineTo(x2, y2);
      }
    }),
  },
  { id: "circle", label: "Circle", colour: "#ff7818", path: once("circle", (p) => circle(p, 0, 0, 50)) },
  {
    id: "ring",
    label: "Ring",
    colour: "#ffffff",
    path: once("ring", (p) => {
      circle(p, 0, 0, 50);
      circle(p, 0, 0, 38);
    }),
  },
  {
    id: "square",
    label: "Square",
    colour: "#2a2a2a",
    path: once("square", (p) => {
      p.roundRect(-50, -50, 100, 100, 10);
    }),
  },
  {
    id: "diamond",
    label: "Diamond",
    colour: "#38b4c8",
    path: once("diamond", (p) => {
      p.moveTo(0, -50);
      p.lineTo(50, 0);
      p.lineTo(0, 50);
      p.lineTo(-50, 0);
      p.closePath();
    }),
  },
  {
    id: "triangle",
    label: "Triangle",
    colour: "#f2c230",
    aspect: 0.88,
    path: once("triangle", (p) => {
      p.moveTo(0, -50);
      p.lineTo(50, 50);
      p.lineTo(-50, 50);
      p.closePath();
    }),
  },
  {
    id: "arrow",
    label: "Arrow",
    colour: "#ffffff",
    aspect: 0.6,
    path: once("arrow", (p) => {
      p.moveTo(-50, -14);
      p.lineTo(10, -14);
      p.lineTo(10, -46);
      p.lineTo(50, 0);
      p.lineTo(10, 46);
      p.lineTo(10, 14);
      p.lineTo(-50, 14);
      p.closePath();
    }),
  },
  {
    id: "bubble",
    label: "Speech",
    colour: "#ffffff",
    aspect: 0.8,
    path: once("bubble", (p) => {
      p.moveTo(-50, -10);
      p.bezierCurveTo(-50, -44, 50, -44, 50, -10);
      p.bezierCurveTo(50, 20, 10, 26, -6, 24);
      p.lineTo(-30, 46);
      p.lineTo(-24, 20);
      p.bezierCurveTo(-40, 14, -50, 4, -50, -10);
      p.closePath();
    }),
  },
  {
    id: "banner",
    label: "Banner",
    colour: "#b02a2a",
    aspect: 0.32,
    path: once("banner", (p) => {
      p.moveTo(-50, -40);
      p.lineTo(50, -40);
      p.lineTo(40, 0);
      p.lineTo(50, 40);
      p.lineTo(-50, 40);
      p.lineTo(-40, 0);
      p.closePath();
    }),
  },
];
