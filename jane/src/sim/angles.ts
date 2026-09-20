// The sim never calls Math.sin / cos / atan2 while running: their last bits are
// allowed to differ between JS engines, and one differing bit is a desync.
// Directions are unit vectors; turning is "rotate this vector by a table angle".
// The table is built once and rounded to 1e-9, which erases engine differences.

const STEPS = 360;
const COS = new Float64Array(STEPS);
const SIN = new Float64Array(STEPS);
for (let d = 0; d < STEPS; d++) {
  const r = (d * Math.PI) / 180;
  COS[d] = Math.round(Math.cos(r) * 1e9) / 1e9;
  SIN[d] = Math.round(Math.sin(r) * 1e9) / 1e9;
}

export type Vec = { x: number; y: number };

/** Rotate (x, y) by whole degrees, clockwise on screen (y grows down). */
export function rotate(x: number, y: number, degrees: number): Vec {
  const d = ((Math.round(degrees) % STEPS) + STEPS) % STEPS;
  return { x: x * COS[d] - y * SIN[d], y: x * SIN[d] + y * COS[d] };
}

export function normalize(x: number, y: number): Vec {
  const len = Math.sqrt(x * x + y * y);
  return len > 1e-9 ? { x: x / len, y: y / len } : { x: 0, y: 0 };
}

/**
 * Turn heading (hx, hy) toward (dx, dy) by at most `maxDegrees`. Both unit vectors.
 * This is `clamp(angle_difference(dir, heading), -max, max)` without the angles.
 */
export function turnToward(hx: number, hy: number, dx: number, dy: number, maxDegrees: number): Vec {
  const max = Math.max(1, Math.round(maxDegrees));
  const dot = hx * dx + hy * dy;
  if (dot >= COS[Math.min(max, 179)]) return { x: dx, y: dy };
  const cross = hx * dy - hy * dx;
  const r = rotate(hx, hy, cross >= 0 ? max : -max);
  return normalize(r.x, r.y);
}
