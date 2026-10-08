// The Pious mark: an icosahedron seen straight down one face. These are
// the exact vertices the logo SVGs are drawn from (assets/brand), already
// turned to the logo's orientation: x right, y up, z toward the viewer.
// Faces wind outward.

export type Vec = [number, number, number];

export const VERTICES: Vec[] = [
  [0, 1.154701, 1.511523],
  [-1, -0.57735, 1.511523],
  [1, -0.57735, 1.511523],
  [-1.618034, -0.934172, -0.356822],
  [1.618034, -0.934172, -0.356822],
  [0, 1.868345, -0.356822],
  [1.618034, 0.934172, 0.356822],
  [-1.618034, 0.934172, 0.356822],
  [0, -1.868345, 0.356822],
  [0, -1.154701, -1.511523],
  [1, 0.57735, -1.511523],
  [-1, 0.57735, -1.511523],
];

export const FACES: [number, number, number][] = [
  [0, 1, 2], [0, 7, 1], [0, 2, 6], [0, 6, 5], [0, 5, 7],
  [1, 8, 2], [1, 7, 3], [1, 3, 8], [2, 4, 6], [2, 8, 4],
  [3, 7, 11], [3, 9, 8], [3, 11, 9], [4, 10, 6], [4, 8, 9],
  [4, 9, 10], [5, 6, 10], [5, 11, 7], [5, 10, 11], [9, 11, 10],
];

/** Where the light comes from (upper left, in front). */
export const LIGHT: Vec = [-0.375652742008357, 0.5426095162342934, 0.751305484016714];

/** The space between faces in the flat mark, in its 100-unit box. */
export const GAP = 2.4;

export const sub = (a: Vec, b: Vec): Vec => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const dot = (a: Vec, b: Vec) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export function cross(a: Vec, b: Vec): Vec {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}
export function normalize(a: Vec): Vec {
  const l = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
}

/** Rotates around y (yaw), then x (pitch), then z (roll). */
export function rotate(v: Vec, yaw: number, pitch: number, roll: number): Vec {
  let [x, y, z] = v;
  let c = Math.cos(yaw), s = Math.sin(yaw);
  [x, z] = [x * c + z * s, -x * s + z * c];
  c = Math.cos(pitch);
  s = Math.sin(pitch);
  [y, z] = [y * c - z * s, y * s + z * c];
  c = Math.cos(roll);
  s = Math.sin(roll);
  [x, y] = [x * c - y * s, x * s + y * c];
  return [x, y, z];
}

/** How the solid is turned and drawn. */
export interface Pose {
  /** Turns around y, then x, then z (radians). */
  yaw: number;
  pitch: number;
  roll: number;
  /** 0 = shaded, holographic solid; 1 = the flat, solid-white mark. */
  flat: number;
  /** Size multiplier (a small "breath"). */
  scale: number;
  /** Where the holographic reflection is pointing (radians). */
  shine: number;
}

/** The logo's scale: units of its 100-unit box per unit of radius. */
const UNIT = 24.620724194343204;

/** Draws the mark into a `size`×`size` (CSS pixel) area of `ctx`, which
 * must already be scaled for the device pixel ratio. `text` is the color
 * of the flat mark. */
export function drawMark(ctx: CanvasRenderingContext2D, size: number, pose: Pose, text: [number, number, number]) {
  const unit = (size / 100) * UNIT;
  const flat = Math.min(1, Math.max(0, pose.flat));
  const gap = ((1.2 + (GAP - 1.2) * flat) * size) / 100;
  const reach = size * 0.75;
  const holo = ctx.createLinearGradient(
    size / 2 - Math.cos(pose.shine) * reach,
    size / 2 - Math.sin(pose.shine) * reach,
    size / 2 + Math.cos(pose.shine) * reach,
    size / 2 + Math.sin(pose.shine) * reach,
  );
  ["#ff9ad5", "#a99bff", "#7fe3ff", "#b6ffcf", "#ffe6a3", "#ff9ad5"].forEach((c, i, all) => holo.addColorStop(i / (all.length - 1), c));

  const verts = VERTICES.map((v) => rotate(v, pose.yaw, pose.pitch, pose.roll));
  const faces = FACES.map((f) => {
    const [a, b, c] = f.map((i) => verts[i]) as [Vec, Vec, Vec];
    const n = normalize(cross(sub(b, a), sub(c, a)));
    const depth = (a[2] + b[2] + c[2]) / 3;
    return { a, b, c, n, depth };
  })
    // Only faces turned toward the viewer, back to front.
    .filter((f) => f.n[2] > 0.0001)
    .sort((p, q) => p.depth - q.depth);

  ctx.clearRect(0, 0, size, size);
  const px = (v: Vec): [number, number] => [size / 2 + v[0] * unit * pose.scale, size / 2 - v[1] * unit * pose.scale];
  for (const face of faces) {
    let pts = [px(face.a), px(face.b), px(face.c)];
    // The gaps between faces.
    const cx = (pts[0][0] + pts[1][0] + pts[2][0]) / 3;
    const cy = (pts[0][1] + pts[1][1] + pts[2][1]) / 3;
    pts = pts.map(([x, y]) => {
      const dx = cx - x,
        dy = cy - y,
        l = Math.hypot(dx, dy) || 1;
      return [x + (dx / l) * gap, y + (dy / l) * gap];
    });
    const lit = Math.max(0, dot(face.n, LIGHT));
    const path = new Path2D();
    path.moveTo(pts[0][0], pts[0][1]);
    path.lineTo(pts[1][0], pts[1][1]);
    path.lineTo(pts[2][0], pts[2][1]);
    path.closePath();

    // White shaded by the light, blending into the flat mark's color.
    const shade = 255 * (0.5 + 0.5 * lit);
    const mix = (target: number, extra = 0) => Math.round(Math.min(255, shade + extra) + (target - Math.min(255, shade + extra)) * flat);
    ctx.globalAlpha = 1;
    ctx.fillStyle = `rgb(${mix(text[0])},${mix(text[1])},${mix(text[2], 3)})`;
    ctx.fill(path);
    // The holographic reflection, fading out as it flattens.
    if (flat < 1) {
      ctx.globalCompositeOperation = "multiply";
      ctx.globalAlpha = (1 - flat) * (0.14 + 0.18 * (1 - lit));
      ctx.fillStyle = holo;
      ctx.fill(path);
      ctx.globalCompositeOperation = "source-over";
    }
  }
  ctx.globalAlpha = 1;
}

/** The interface's text color as [r, g, b] (from the `--text` variable). */
export function textColor(): [number, number, number] {
  const value = getComputedStyle(document.documentElement).getPropertyValue("--text").trim() || "237 237 239";
  const [r, g, b] = value.split(/\s+/).map(Number);
  return [r || 237, g || 237, b || 239];
}
