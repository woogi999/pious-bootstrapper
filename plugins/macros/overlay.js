// Macros' panel in Pious's in-game overlay: run or stop each macro and the
// auto-clicker without leaving the game. It talks to the engine the same way
// the Macros page does (`send` / "state" messages), so both always agree.
(() => {
  const $panel = document.getElementById("panel");
  let state = null;
  let icons = {};

  const send = (msg) => pious.call("send", msg).catch(() => {});

  function h(tag, props, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(props || {})) {
      if (v === undefined || v === null || v === false) continue;
      if (k.startsWith("on")) el.addEventListener(k.slice(2).toLowerCase(), v);
      else el.setAttribute(k, v === true ? "" : v);
    }
    for (const kid of kids.flat(Infinity)) {
      if (kid === null || kid === undefined || kid === false) continue;
      el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
    }
    return el;
  }
  function icon(name) {
    const span = h("span", { class: "icon" });
    span.innerHTML = icons[name] || "";
    return span;
  }
  const keyLabel = (combo) => (combo ? combo.replace(/Key([A-Z])/g, "$1").replace(/Digit(\d)/g, "$1").split("+").join(" + ") : "");

  function render() {
    if (!state) return;
    const running = new Set(state.running || []);
    const clicking = !!state.clicking;
    const macros = state.macros || [];

    const clicker = h(
      "div",
      { class: "clicker" },
      clicking ? h("span", { class: "live" }) : icon("autoclick"),
      h(
        "div",
        { class: "col grow", style: "gap:0" },
        h("span", { class: "item-title" }, "Auto-clicker"),
        h("span", { class: "secondary line" }, clicking ? "Clicking" : state.autoclicker?.hotkey ? `Press ${keyLabel(state.autoclicker.hotkey)} in game` : "Off"),
      ),
      h(
        "button",
        { type: "button", class: "btn small" + (clicking ? " danger" : " primary"), onclick: () => send({ type: "toggleClicker" }) },
        icon(clicking ? "stop" : "play"),
        clicking ? "Stop" : "Start",
      ),
    );

    const list = macros.length
      ? h(
          "div",
          { class: "list" },
          macros.map((m) => {
            const on = running.has(m.id);
            return h(
              "div",
              { class: "macro" },
              on ? h("span", { class: "live" }) : null,
              h(
                "div",
                { class: "col grow", style: "gap:0" },
                h("span", { class: "item-title line" }, m.name || "Macro"),
                m.hotkey ? h("span", { class: "secondary line" }, keyLabel(m.hotkey)) : null,
              ),
              h(
                "button",
                { type: "button", class: "icon-btn" + (on ? " on" : ""), title: on ? "Stop" : "Run", "aria-label": on ? "Stop" : "Run", onclick: () => send({ type: "run", id: m.id }) },
                icon(on ? "stop" : "play"),
              ),
            );
          }),
        )
      : h("span", { class: "meta" }, "No macros yet. Make one on the Macros page.");

    const stopAll = running.size || clicking ? h("button", { type: "button", class: "btn small danger", onclick: () => send({ type: "stopAll" }) }, icon("stop"), "Stop all") : null;
    // (replaceChildren would show a null as the text "null".)
    $panel.replaceChildren(...[clicker, list, stopAll].filter(Boolean));
  }

  pious.on("message", (msg) => {
    if (msg && msg.type === "state") {
      state = msg;
      render();
    }
  });

  pious.ui
    .icons(["play", "stop", "autoclick"])
    .then((found) => (icons = found || {}))
    .catch(() => {})
    .finally(() => {
      render();
      // Ask until the engine answers (it may still be starting).
      let asks = 0;
      const ask = () => {
        if (state) return;
        send({ type: "get" });
        if (++asks < 20) setTimeout(ask, 1000);
        else $panel.replaceChildren(h("span", { class: "meta" }, "Macros isn't running."));
      };
      ask();
    });
})();
