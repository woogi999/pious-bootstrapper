// Heroicons used across the app, inlined as SVG markup by name.

const solid = import.meta.glob("../../assets/icons/solid/*.svg", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const outline = import.meta.glob("../../assets/icons/outline/*.svg", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

function byName(files: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [path, svg] of Object.entries(files)) {
    const name = path.split("/").pop()!.replace(/\.svg$/, "");
    // Size and color come from CSS.
    out[name] = svg.replace(/\s(width|height)="[^"]*"/g, "").replace(/<svg/, '<svg aria-hidden="true"');
  }
  return out;
}

const SOLID = byName(solid);
const OUTLINE = byName(outline);

/** The app's names for icons, mapped to Heroicons files. */
const NAMES: Record<string, string> = {
  home: "home",
  library: "book-open",
  game: "puzzle-piece",
  server: "server-stack",
  account: "user-circle",
  accounts: "users",
  "add-account": "user-plus",
  versions: "square-3-stack-3d",
  running: "computer-desktop",
  settings: "cog-6-tooth",
  search: "magnifying-glass",
  heart: "heart",
  star: "star",
  play: "play",
  launch: "rocket-launch",
  add: "plus",
  remove: "trash",
  edit: "pencil",
  more: "ellipsis-horizontal",
  success: "check-circle",
  warning: "exclamation-circle",
  error: "x-circle",
  refresh: "arrow-path",
  lightning: "bolt",
  download: "arrow-down-tray",
  cloud: "cloud",
  clock: "clock",
  history: "arrow-uturn-left",
  "caret-down": "chevron-down",
  "caret-right": "chevron-right",
  "arrow-right": "arrow-right",
  close: "x-mark",
  "sign-in": "arrow-right-end-on-rectangle",
  focus: "window",
  power: "power",
  external: "arrow-top-right-on-square",
  link: "link",
  folder: "folder",
  sort: "bars-arrow-down",
  crown: "trophy",
  cube: "cube",
  pin: "bookmark",
  info: "information-circle",
  globe: "globe-alt",
  lock: "lock-closed",
  check: "check",
  copy: "document-duplicate",
  hash: "hashtag",
  grid: "squares-2x2",
  list: "list-bullet",
  back: "arrow-left",
  key: "key",
  eye: "eye",
  palette: "swatch",
  picture: "photo",
  browser: "window",
  phone: "device-phone-mobile",
  shield: "shield-check",
  sliders: "adjustments-horizontal",
  contrast: "sun",
  sparkles: "sparkles",
  blur: "cloud",
  users: "user-group",
  update: "arrow-up-circle",
  motion: "pause-circle",
  hop: "arrows-right-left",
  "lock-open": "lock-open",
  moon: "moon",
  sun: "sun",
  "sidebar-collapse": "chevron-double-left",
  chat: "chat-bubble-left-right",
  send: "paper-airplane",
  record: "video-camera",
  film: "film",
  clip: "scissors",
  signal: "signal",
  mic: "microphone",
  sound: "speaker-wave",
  keyboard: "keyboard",
  shortcuts: "command-line",
  media: "musical-note",
  next: "forward",
  previous: "backward",
  pause: "pause",
  uninstall: "user-minus",
  friends: "user-group",
  macros: "cursor-arrow-rays",
  autoclick: "cursor-arrow-ripple",
  puzzle: "squares-plus",
  plugins: "squares-plus",
  streamer: "eye-slash",
  stop: "stop",
  stats: "chart-bar",
  tip: "light-bulb",
  up: "arrow-up",
  down: "arrow-down",
  eyedropper: "eye-dropper",
  target: "viewfinder-circle",
  ai: "cpu-chip",
  changelog: "document-text",
  gift: "gift",
  "play-circle": "play-circle",
  text: "language",
  "sort-up": "bars-arrow-up",
  news: "newspaper",
  emoji: "face-smile",
  location: "map-pin",
  overlay: "rectangle-group",
  remap: "arrows-up-down",
  bell: "bell",
  studio: "cube",
};

const WINDOW: Record<string, string> = {
  minimize: "minus",
  maximize: "stop",
  restore: "square-2-stack",
  "window-close": "x-mark",
  fullscreen: "arrows-pointing-out",
  "exit-fullscreen": "arrows-pointing-in",
  "pin-window": "pushpin",
};

export function icon(name: string): string {
  if (WINDOW[name]) return OUTLINE[WINDOW[name]] ?? "";
  return SOLID[NAMES[name] ?? name] ?? "";
}
