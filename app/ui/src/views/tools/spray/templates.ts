// Starting points for the composer: a few original designs made of the built-in stickers
// and words, ready to change.

import { blankDoc, shapeLayer, textLayer, type Doc } from "./doc";

export type Template = { id: string; label: string; note: string; make: () => Doc };

export const TEMPLATES: Template[] = [
  { id: "blank", label: "Blank", note: "An empty square", make: () => blankDoc("square") },
  {
    id: "tag",
    label: "Name tag",
    note: "Your name on a paint splat",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "splat", { w: 470, h: 470, colour: "#c0302a" }));
      d.layers.push(textLayer(d, "YOUR NAME", { size: 84, rot: -8, outline: 7 }));
      return d;
    },
  },
  {
    id: "skull",
    label: "Skull banner",
    note: "A skull over a ribbon with a word on it",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "skull", { w: 300, h: 330, y: 200 }));
      d.layers.push(shapeLayer(d, "banner", { w: 470, h: 120, y: 410 }));
      d.layers.push(textLayer(d, "NO MERCY", { size: 64, y: 410, outline: 0, shadow: false, colour: "#f8f0dc", font: "'Arial Black', 'Arial Bold', Gadget, sans-serif" }));
      return d;
    },
  },
  {
    id: "badge",
    label: "Team badge",
    note: "A shield with a star and a short tag",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "shield", { w: 400, h: 460, outline: 10, outlineColour: "#e8e0c8" }));
      d.layers.push(shapeLayer(d, "star", { w: 180, h: 180, y: 190 }));
      d.layers.push(textLayer(d, "LS", { size: 140, y: 340, font: "Counter-Strike", outline: 0 }));
      return d;
    },
  },
  {
    id: "headshot",
    label: "Headshot",
    note: "A crosshair and one word, wide",
    make: () => {
      const d = blankDoc("wide");
      d.layers.push(shapeLayer(d, "target", { w: 300, h: 300, x: 150, y: 192 }));
      d.layers.push(textLayer(d, "HS", { size: 150, x: 370, y: 192, colour: "#3ce05a", rot: -6 }));
      return d;
    },
  },
  {
    id: "gg",
    label: "GG",
    note: "Big letters in the game's font, in a burst",
    make: () => {
      const d = blankDoc("square");
      d.layers.push(shapeLayer(d, "burst", { w: 480, h: 480, colour: "#f2c230" }));
      d.layers.push(textLayer(d, "GG", { size: 200, font: "Counter-Strike", colour: "#202020", outline: 0, shadow: false }));
      return d;
    },
  },
];
