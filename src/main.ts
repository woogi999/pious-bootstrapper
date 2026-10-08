import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import Hud from "./Hud.svelte";
import Overlay from "./Overlay.svelte";
import ChatWindow from "./ChatWindow.svelte";
import EmojiPopup from "./EmojiPopup.svelte";
import InputOverlay from "./InputOverlay.svelte";
import Notify from "./Notify.svelte";
import Splash from "./components/Splash.svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { trackShine } from "./lib/shine";

// Interface errors go to ui-errors.log in the data folder.
const report = (message: string) => invoke("report_error", { message }).catch(() => {});
window.addEventListener("error", (event) => report(`${event.message} at ${event.filename}:${event.lineno}`));
window.addEventListener("unhandledrejection", (event) => report(`Unhandled: ${String(event.reason)}`));

// The window has no native context menu; Pious shows its own.
window.addEventListener("contextmenu", (event) => {
  const target = event.target as HTMLElement;
  if (!target.closest("input, textarea, .selectable")) event.preventDefault();
});

// The same interface runs in the main window, the in-game overlay, the
// notices over the game, the chat window, the pop-ups and the startup logo.
const label = getCurrentWindow().label;
if (!["hud", "splash", "inputs", "emoji", "notify"].includes(label)) trackShine();
const roots = { overlay: Overlay, hud: Hud, chat: ChatWindow, splash: Splash, inputs: InputOverlay, emoji: EmojiPopup, notify: Notify } as Record<string, typeof App>;
const root = roots[label] ?? App;
export default mount(root, { target: document.getElementById("app")! });
