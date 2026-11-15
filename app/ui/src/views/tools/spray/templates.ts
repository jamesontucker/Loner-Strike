// Starting points for the composer: original designs made of the built-in stickers and words,
// ready to change. Each one is checked at the game's real size (96×96, the in-game preview):
// big shapes, few words, strong outlines, so it still reads on a wall from across a room.
// Sixteen of them (Blank included), so the "Start from" grid fills its rows of four.

import { blankDoc, FONTS, shapeLayer, textLayer, type Doc, type TextLayer } from "./doc";

export type Template = { id: string; label: string; note: string; make: () => Doc };

const [CS, IMPACT, BLACK, , , GEORGIA, COURIER] = FONTS.map(([f]) => f);
/** words without the default shadow */
const flat = (o: Partial<TextLayer>): Partial<TextLayer> => ({ shadow: false, ...o });

export const TEMPLATES: Template[] = [
  { id: "blank", label: "Blank", note: "An empty square to fill yourself", make: () => blankDoc("square") },
  {
    id: "stencil",
    label: "Stencil tag",
    note: "Your tag cut out of a yellow plate",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "plate", { w: 480, h: 300, y: 236, colour: "#f2c230" }));
      d.layers.push(shapeLayer(d, "drips", { w: 400, h: 200, y: 418, colour: "#f2c230" }));
      d.layers.push(textLayer(d, "YOUR\nTAG", flat({ font: CS, size: 118, y: 236, colour: "#141414", outline: 0 })));
      return d;
    },
  },
  {
    id: "graffiti",
    label: "Graffiti tag",
    note: "A name in loud letters on a paint splat",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "splat", { w: 500, h: 500, colour: "#ff3d8b" }));
      d.layers.push(shapeLayer(d, "crown", { w: 150, h: 112, x: 150, y: 120, rot: -16, colour: "#ffe600", outline: 6 }));
      d.layers.push(textLayer(d, "NAME", { font: IMPACT, size: 170, italic: true, rot: -10, y: 270, colour: "#ffe600", outline: 12, outlineColour: "#160a24" }));
      return d;
    },
  },
  {
    id: "arcade",
    label: "Arcade",
    note: "Retro GAME OVER with three lives left",
    make: () => {
      const d = blankDoc("wide");
      d.layers.push(textLayer(d, "GAME\nOVER", flat({ font: BLACK, size: 118, x: 266, y: 158, colour: "#00e5ff", outline: 0 })));
      d.layers.push(textLayer(d, "GAME\nOVER", flat({ font: BLACK, size: 118, x: 256, y: 150, colour: "#ff2fd0", outline: 7, outlineColour: "#1a0033" })));
      for (const x of [176, 256, 336]) d.layers.push(shapeLayer(d, "pixel-heart", { w: 64, h: 57, x, y: 330, colour: "#ffe600" }));
      return d;
    },
  },
  {
    id: "clan",
    label: "Clan emblem",
    note: "Shield, wings and crossed rifles over your clan tag",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "wings", { w: 500, h: 300, y: 190, colour: "#e2b23c" }));
      d.layers.push(shapeLayer(d, "shield", { w: 250, h: 288, y: 220, colour: "#1f3c66", outline: 9, outlineColour: "#e2b23c" }));
      d.layers.push(shapeLayer(d, "rifles", { w: 190, h: 190, y: 210, colour: "#efe6cc" }));
      d.layers.push(shapeLayer(d, "banner", { w: 430, h: 120, y: 420, colour: "#8f1d1d", outline: 6, outlineColour: "#e2b23c" }));
      d.layers.push(textLayer(d, "CLAN", flat({ font: BLACK, size: 64, y: 418, colour: "#f8f0dc", outline: 0 })));
      return d;
    },
  },
  {
    id: "planted",
    label: "Planted here",
    note: "A bomb and the words, for the spot you planted",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "burst", { w: 330, h: 330, y: 166, colour: "#ff7818" }));
      d.layers.push(shapeLayer(d, "bomb", { w: 230, h: 166, y: 176, colour: "#d8c49a", outline: 7, outlineColour: "#161616" }));
      d.layers.push(textLayer(d, "PLANTED\nHERE", { font: CS, size: 82, y: 400, colour: "#ffffff", outline: 8 }));
      return d;
    },
  },
  {
    id: "nicetry",
    label: "Nice try",
    note: "A speech bubble for the one who almost had you",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "bubble", { w: 490, h: 392, y: 236, colour: "#ffffff", outline: 9, outlineColour: "#141414" }));
      d.layers.push(textLayer(d, "NICE\nTRY", flat({ font: BLACK, size: 104, y: 206, rot: -4, colour: "#141414", outline: 0 })));
      return d;
    },
  },
  {
    id: "noob",
    label: "Noob",
    note: "Tall, with an arrow pointing down at the spot",
    make: () => {
      const d = blankDoc("tall");
      d.layers.push(textLayer(d, "NOOB", { font: IMPACT, size: 150, y: 122, colour: "#3ce05a", outline: 10 }));
      d.layers.push(shapeLayer(d, "arrow", { w: 300, h: 180, y: 350, rot: 90, colour: "#3ce05a", outline: 10, outlineColour: "#111111" }));
      return d;
    },
  },
  {
    id: "onetap",
    label: "One tap",
    note: "A crosshair and two words, wide",
    make: () => {
      const d = blankDoc("wide");
      d.layers.push(shapeLayer(d, "target", { w: 250, h: 250, x: 140, y: 192, colour: "#ff3b30" }));
      d.layers.push(textLayer(d, "ONE\nTAP", { font: IMPACT, size: 128, x: 370, y: 192, colour: "#ffffff", outline: 9 }));
      return d;
    },
  },
  {
    id: "pixelheart",
    label: "Pixel heart",
    note: "Big chunky pixels: crisp even at the smallest size",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "pixel-heart", { w: 456, h: 405, y: 250, colour: "#e8344a", outline: 10, outlineColour: "#1a0a10" }));
      d.layers.push(textLayer(d, "GG", flat({ font: COURIER, bold: true, size: 120, y: 214, colour: "#ffffff", outline: 0 })));
      return d;
    },
  },
  {
    id: "pixelskull",
    label: "Pixel skull",
    note: "A pixel skull on black, with RIP under it",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "square", { w: 470, h: 470, colour: "#141414" }));
      d.layers.push(shapeLayer(d, "pixel-skull", { w: 300, h: 300, y: 200, colour: "#f1ead8" }));
      d.layers.push(textLayer(d, "RIP", flat({ font: COURIER, bold: true, size: 104, y: 418, colour: "#ff3b30", outline: 0 })));
      return d;
    },
  },
  {
    id: "ct",
    label: "Counter-Terrorist",
    note: "A blue CT badge (pairs with the T one)",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "shield", { w: 400, h: 460, colour: "#2a5db0", outline: 10, outlineColour: "#e8eef8" }));
      d.layers.push(shapeLayer(d, "chevrons", { w: 150, h: 150, y: 128, colour: "#e8eef8" }));
      d.layers.push(textLayer(d, "CT", flat({ font: CS, size: 140, y: 300, colour: "#e8eef8", outline: 0 })));
      return d;
    },
  },
  {
    id: "t",
    label: "Terrorist",
    note: "A red T badge (pairs with the CT one)",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "shield", { w: 400, h: 460, colour: "#b3261e", outline: 10, outlineColour: "#f6e7c8" }));
      d.layers.push(shapeLayer(d, "bomb", { w: 150, h: 108, y: 132, colour: "#f6e7c8" }));
      d.layers.push(textLayer(d, "T", flat({ font: CS, size: 200, y: 316, colour: "#f6e7c8", outline: 0 })));
      return d;
    },
  },
  {
    id: "monogram",
    label: "Monogram",
    note: "One letter in a ring; good in one color too",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "ring", { w: 470, h: 470, colour: "#f4efe4" }));
      d.layers.push(textLayer(d, "A", flat({ font: GEORGIA, bold: true, size: 300, y: 262, colour: "#f4efe4", outline: 0 })));
      return d;
    },
  },
  {
    id: "gg",
    label: "GG WP",
    note: "Gold laurels around a good game",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "laurel", { w: 500, h: 500, colour: "#e2b23c" }));
      d.layers.push(textLayer(d, "GG", { font: CS, size: 170, y: 228, colour: "#ffffff", outline: 8 }));
      d.layers.push(textLayer(d, "WP", flat({ font: BLACK, size: 62, y: 344, colour: "#e2b23c", outline: 6, outlineColour: "#111111" })));
      return d;
    },
  },
  {
    id: "loner",
    label: "Loner-Strike mark",
    note: "A small LS mark, only if you'd like it",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "circle", { w: 400, h: 400, colour: "#1d211b", outline: 8, outlineColour: "#c9a44a" }));
      d.layers.push(shapeLayer(d, "star", { w: 90, h: 90, y: 130, colour: "#c9a44a" }));
      d.layers.push(textLayer(d, "LS", flat({ font: CS, size: 170, y: 290, colour: "#efe6cc", outline: 0 })));
      return d;
    },
  },
];
