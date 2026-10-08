// Feeds the cursor position to the glass surface under it: --mx/--my (in
// pixels) for its rim light and spotlight, --sx (as a percentage) for a
// card's sheen, and --rx/--ry for a card's tilt toward the cursor.
// One listener for the whole window, at most once per frame.

const SURFACES = ".card, .btn, .chip, .shine, .glass, .glass-elevated, .panel, .lit";
/** How far a card tilts toward the cursor, in degrees. */
const TILT = 3.5;

export function trackShine() {
  let pending: PointerEvent | null = null;
  let lit: HTMLElement[] = [];

  const clear = (el: HTMLElement) => {
    for (const name of ["--mx", "--my", "--sx", "--rx", "--ry"]) el.style.removeProperty(name);
  };

  const update = () => {
    const event = pending;
    pending = null;
    if (!event) return;
    const reduced = document.documentElement.classList.contains("reduce-motion");
    const next: HTMLElement[] = [];
    // The surface under the cursor and the glass panels around it.
    let el = (event.target as HTMLElement | null)?.closest<HTMLElement>(SURFACES) ?? null;
    while (el && next.length < 3) {
      const rect = el.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;
      el.style.setProperty("--mx", `${x}px`);
      el.style.setProperty("--my", `${y}px`);
      if (el.classList.contains("card")) {
        el.style.setProperty("--sx", `${(x / rect.width) * 100}%`);
        if (!reduced) {
          // -1…1 from the center; the card leans toward the cursor.
          const nx = (x / rect.width) * 2 - 1;
          const ny = (y / rect.height) * 2 - 1;
          el.style.setProperty("--ry", `${(nx * TILT).toFixed(2)}deg`);
          el.style.setProperty("--rx", `${(-ny * TILT).toFixed(2)}deg`);
        }
      }
      next.push(el);
      el = el.parentElement?.closest<HTMLElement>(SURFACES) ?? null;
    }
    for (const old of lit) {
      if (!next.includes(old)) clear(old);
    }
    lit = next;
  };

  window.addEventListener(
    "pointermove",
    (event) => {
      if (!pending) requestAnimationFrame(update);
      pending = event;
    },
    { passive: true },
  );
  // Leaving the window lets everything settle back.
  document.addEventListener("pointerleave", () => {
    for (const old of lit) clear(old);
    lit = [];
  });
}
