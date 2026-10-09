// Text formatting shared across the interface.

import { app } from "./state.svelte";
import type { Account, Snapshot, VersionChoice, ServerChoice, Uuid, Game, Recommendation } from "./types";

export function relative(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, (now - Date.parse(iso)) / 1000);
  if (seconds < 60) return "Just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  if (days < 30) return `${Math.floor(days / 7)} week${days < 14 ? "" : "s"} ago`;
  return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

/** "5 minutes ago" → "5 minutes ago", "Yesterday" → "yesterday", for mid-sentence use. */
export function relativeInline(iso: string, now = Date.now()): string {
  const text = relative(iso, now);
  return text === "Just now" || text === "Yesterday" ? text.toLowerCase() : text;
}

export function runtime(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - Date.parse(iso)) / 1000));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return h > 0 ? `${h}h ${m.toString().padStart(2, "0")}m` : `${m}:${s.toString().padStart(2, "0")}`;
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let value = n / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

export function compact(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1).replace(/\.0$/, "")}K`;
  return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
}

export function oneLine(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

export function plural(n: number, noun: string): string {
  return n === 1 ? `1 ${noun}` : `${n} ${noun}s`;
}

export function greeting(): string {
  const hour = new Date().getHours();
  if (hour < 5) return "Good night";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

// ── Library labels ─────────────────────────────────────────────────────────

export function accountLabel(a: Pick<Account, "alias" | "display_name">): string {
  return a.alias ?? who(a.display_name);
}

/** A Roblox name as it may be shown: streamer mode keeps only its first
 * two characters. */
export function who(name: string | null | undefined): string {
  if (!name) return "";
  if (!app.snap?.bootstrapper.preferences.streamer_mode) return name;
  const chars = [...name];
  return chars.slice(0, 2).join("") + "•".repeat(Math.max(2, Math.min(6, chars.length - 2)));
}

export function accountName(snap: Snapshot, id: Uuid | null): string {
  const account = id ? snap.bootstrapper.accounts.find((a) => a.id === id) : null;
  return account ? accountLabel(account) : "No account";
}

export function versionTitle(snap: Snapshot, hash: string | null): string {
  if (!hash) return "System Roblox";
  return snap.version_titles[hash] ?? hash;
}

export function versionLabel(snap: Snapshot, choice: VersionChoice): string {
  if (choice === "Default") {
    return snap.default_version ? `Default · ${versionTitle(snap, snap.default_version)}` : "Default";
  }
  if (choice === "Latest") {
    return snap.latest_installed ? `Latest · ${versionTitle(snap, snap.latest_installed)}` : "Latest";
  }
  if ("Profile" in choice) {
    return `Profile · ${choice.Profile}`;
  }
  const title = snap.version_titles[choice.Specific];
  return title ?? `${choice.Specific} (missing)`;
}

export function shortVersion(snap: Snapshot, choice: VersionChoice): string {
  if (choice === "Default") return "Default version";
  if (choice === "Latest") return "Latest";
  return versionLabel(snap, choice);
}

export function serverLabel(snap: Snapshot, server: ServerChoice): string {
  if (server === "Public") return "Public server";
  return snap.bootstrapper.servers.find((s) => s.id === server.Private)?.name ?? "Public server";
}

export function sameServer(a: ServerChoice, b: ServerChoice): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function sameVersion(a: VersionChoice, b: VersionChoice): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function gameUrl(placeId: number): string {
  return `https://www.roblox.com/games/${placeId}`;
}

export function artworkKey(g: Game | Recommendation): string {
  return `game-${g.place_id}`;
}

export function avatarKey(a: Account): string {
  return `avatar-${a.user_id}`;
}

export function recommendationReason(rec: Recommendation): string {
  // The backend sends account labels; show them as accountLabel would (so
  // streamer mode hides display names here too).
  const shown = (label: string | undefined) => {
    if (label === undefined) return "";
    const account = app.snap?.bootstrapper.accounts.find((x) => (x.alias ?? x.display_name) === label);
    return account ? accountLabel(account) : who(label);
  };
  const [a, b] = [shown(rec.accounts[0]), shown(rec.accounts[1])];
  const game = rec.because[0];
  if (rec.accounts.length > 2) return `Fits what ${a}, ${b} and others play`;
  if (rec.accounts.length === 2) return `Fits what ${a} and ${b} play`;
  if (rec.accounts.length === 1 && game) return `${a} played ${game}`;
  if (game) return `Because you like ${game}`;
  return "Popular with players like you";
}

/** A stable small number from a string, for placeholder shades. */
export function seed(text: string): number {
  let h = 7;
  for (let i = 0; i < text.length; i++) h = (h * 31 + text.charCodeAt(i)) >>> 0;
  return h;
}
