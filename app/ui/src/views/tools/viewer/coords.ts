// GoldSrc <-> three.js coordinates (plan 3.2).
//
// GoldSrc is right-handed with z up (x east, y north); three.js is right-handed with y up.
// Every buffer stays in GoldSrc numbers: one root group rotated -90 degrees about X turns
// (x, y, z) in GoldSrc into (x, z, -y) in three. These helpers are for the few places that
// work in three's world space (cameras, picking).

import { Vector3 } from "three";

export type Vec3 = [number, number, number];

/** The root group's rotation: z-up GoldSrc into y-up three. */
export const ROOT_ROTATION: [number, number, number] = [-Math.PI / 2, 0, 0];

export const gsToThree = ([x, y, z]: Vec3, out = new Vector3()) => out.set(x, z, -y);
export const threeToGs = (v: Vector3): Vec3 => [v.x, -v.z, v.y];

/** View direction in GoldSrc from yaw/pitch in degrees (pitch positive = looking down). */
export function dirFromAngles(yaw: number, pitch: number): Vec3 {
  const y = (yaw * Math.PI) / 180;
  const p = (pitch * Math.PI) / 180;
  return [Math.cos(y) * Math.cos(p), Math.sin(y) * Math.cos(p), -Math.sin(p)];
}

/** Yaw/pitch in degrees (GoldSrc convention) of a three.js direction. */
export function anglesOfThreeDir(d: Vector3): { yaw: number; pitch: number } {
  const [x, y, z] = threeToGs(d);
  const yaw = (Math.atan2(y, x) * 180) / Math.PI;
  const pitch = (-Math.atan2(z, Math.hypot(x, y)) * 180) / Math.PI;
  return { yaw: (yaw + 360) % 360, pitch };
}

/** Player hull: 32x32x72 standing (origin at the centre), 32x32x36 crouched. */
export const HULL_STAND: Vec3 = [32, 32, 72];
export const HULL_DUCK: Vec3 = [32, 32, 36];

/** "x y z" or [x, y, z] (entity keys come as text) into numbers; null if it isn't three numbers. */
export function parseVec(v: unknown): Vec3 | null {
  const parts = Array.isArray(v) ? v : typeof v === "string" ? v.trim().split(/\s+/) : null;
  if (!parts || parts.length < 3) return null;
  const n = parts.slice(0, 3).map(Number);
  return n.every(Number.isFinite) ? (n as Vec3) : null;
}

/** Coordinates as the game prints them: whole units. */
export const fmtVec = (v: Vec3) => v.map((n) => Math.round(n)).join(" ");
