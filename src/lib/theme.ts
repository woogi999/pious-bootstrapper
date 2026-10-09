// Turns the user's appearance into CSS variables on <html>. The palette is
// derived the same way the original design system did: muted and faint
// text are the text color mixed toward the background, and the accent
// gets a readable ink color.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Appearance, UiAssets } from "./types";
import { overrides } from "./overrides.svelte";

type RGB = [number, number, number];

const DEFAULTS = { accent: "#F2F2F3", background: "#0A0A0B", surface: "#FFFFFF", text: "#EDEDEF" };

export function parseHex(hex: string): RGB | null {
  const value = hex.trim().replace(/^#/, "");
  if (!/^[0-9a-fA-F]{6}$/.test(value)) return null;
  const n = parseInt(value, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function toHex([r, g, b]: RGB): string {
  return "#" + [r, g, b].map((c) => Math.round(c).toString(16).padStart(2, "0")).join("").toUpperCase();
}

function mix(a: RGB, b: RGB, t: number): RGB {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

function luminance([r, g, b]: RGB): number {
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

const css = ([r, g, b]: RGB) => `${Math.round(r)} ${Math.round(g)} ${Math.round(b)}`;

export function toHsl([r, g, b]: RGB): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

export function fromHsl(h: number, s: number, l: number): RGB {
  h = (((h % 360) + 360) % 360) / 360;
  if (s <= 0) return [l * 255, l * 255, l * 255];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const channel = (t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return [channel(h + 1 / 3) * 255, channel(h) * 255, channel(h - 1 / 3) * 255];
}

export function applyAppearance(look: Appearance, reduceMotion: boolean, searchBlur = 0.5) {
  const pick = (value: string, fallback: string) => parseHex(value) ?? parseHex(fallback)!;
  const background = pick(look.background, DEFAULTS.background);
  const surface = pick(look.surface, DEFAULTS.surface);
  const text = pick(look.text, DEFAULTS.text);
  const accent = pick(look.accent, DEFAULTS.accent);

  const lightAccent = luminance(accent) > 0.5;
  const ink: RGB = lightAccent ? [11, 11, 12] : [247, 247, 248];
  const toward: RGB = lightAccent ? [255, 255, 255] : text;

  const root = document.documentElement.style;
  root.setProperty("--bg", css(background));
  root.setProperty("--surface", css(surface));
  root.setProperty("--text", css(text));
  root.setProperty("--muted", css(mix(text, background, 0.34)));
  root.setProperty("--faint", css(mix(text, background, 0.52)));
  root.setProperty("--accent", css(accent));
  root.setProperty("--accent-hover", css(mix(accent, toward, 0.35)));
  root.setProperty("--accent-pressed", css(mix(accent, background, 0.18)));
  root.setProperty("--accent-ink", css(ink));
  // Light catching glass rims: white with a hint of the accent.
  root.setProperty("--rim", css(mix([255, 255, 255], accent, 0.35)));
  root.setProperty("--panel", css(mix(background, surface, 0.07 * Math.min(look.glass, 1.5))));
  root.setProperty("--glass", String(Math.min(2.5, Math.max(0.3, look.glass))));
  root.setProperty("--window-alpha", String(look.see_through ? Math.min(1, Math.max(0.15, look.window_opacity)) : 1));
  root.setProperty("--image-dim", String(Math.min(1, Math.max(0, look.image_dim))));
  root.setProperty("--image-blur", `${Math.round(look.image_blur * 40)}px`);
  root.setProperty("--search-blur", String(Math.min(1, Math.max(0, searchBlur))));
  // Themes and Settings → Appearance: font, roundness, text size, a second
  // accent and a gradient over the background.
  root.setProperty("--ui-font", look.font?.trim() ? cssFont(look.font) : "Manrope");
  root.setProperty("--radius", String(Math.min(2.5, Math.max(0, look.radius ?? 1))));
  root.setProperty("--font-scale", String(Math.min(1.3, Math.max(0.8, look.font_scale ?? 1))));
  const second = parseHex(look.accent_2 ?? "");
  root.setProperty("--accent-2", second ? css(second) : css(accent));
  const g = look.gradient;
  const from = g && parseHex(g.from);
  const to = g && parseHex(g.to);
  root.setProperty("--gradient", g?.enabled && from && to ? `linear-gradient(${g.angle}deg, rgb(${css(from)}), rgb(${css(to)}))` : "none");
  root.setProperty("--gradient-opacity", String(Math.min(1, Math.max(0, g?.opacity ?? 0.6))));
  document.documentElement.classList.toggle("reduce-motion", reduceMotion);
}

/** A font family name, quoted for CSS (names come from themes and the PC). */
function cssFont(name: string): string {
  return '"' + name.replace(/["\\;{}<>]/g, "") + '"';
}

// ── Plugins' and themes' extras ──────────────────────────────────────────


/** Applies plugins' and the theme's stylesheets, icons, sounds and font,
 * now and whenever they change. Returns a function that stops following. */
export function followUiAssets(): () => void {
  const apply = (a: UiAssets) => {
    let style = document.getElementById("pious-ui-assets") as HTMLStyleElement | null;
    if (!style) {
      style = document.createElement("style");
      style.id = "pious-ui-assets";
      document.head.appendChild(style);
    }
    const font = a.font ? `@font-face { font-family: ${cssFont(a.font[0])}; src: url("${convertFileSrc(a.font[1])}"); }\n` : "";
    // Only styles: a stylesheet can't run code.
    style.textContent = font + a.css.join("\n");
    overrides.icons = Object.fromEntries(Object.entries(a.icons).map(([name, file]) => [name, convertFileSrc(file)]));
    overrides.sounds = Object.fromEntries(Object.entries(a.sounds).map(([name, file]) => [name, convertFileSrc(file)]));
  };
  invoke<UiAssets>("ui_assets").then(apply).catch(() => {});
  const off = listen<UiAssets>("ui-assets", (e) => apply(e.payload));
  return () => {
    off.then((f) => f());
  };
}
