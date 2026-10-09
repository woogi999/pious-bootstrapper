# Plugin UI

Plugin pages can look exactly like the rest of Pious. Pious keeps its
stylesheet, **`pious-ui.css`**, in every plugin folder that has a page (next
to `pious-plugin.js`) and keeps it in step with the user's theme, font and
corner style. Link it and use the classes below.

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <link rel="stylesheet" href="pious-ui.css" />
  </head>
  <body>
    <div class="page">…</div>
    <script src="pious-plugin.js"></script>
  </body>
</html>
```

- Don't edit `pious-ui.css` or ship it: Pious rewrites it when it changes.
- Your own stylesheet can come after it and override anything.
- Colors are CSS variables holding `r g b`: use them as
  `rgb(var(--text))` or `rgb(var(--surface) / 0.06)`. The main ones are
  `--bg`, `--text`, `--muted`, `--faint`, `--accent`, `--accent-ink`,
  `--surface` and `--panel`. Sizes: `--r-sm`, `--r-md`, `--r-lg` (corners),
  `--fast`, `--med` (durations), `--ease`.
- Icons: `await pious.ui.icon("play")` returns Pious's SVG for a name;
  `await pious.ui.icons(["play", "stop"])` returns several at once. Put the
  SVG inside `<span class="icon">`. No permission is needed.

## Layout

| Class | What it is |
| --- | --- |
| `page` | The page: a column with Pious's spacing. |
| `page-header` | Title row: `<h1 class="page-title">` and actions on the right. |
| `group` | A labeled block: `<span class="label">` above a `glass list`. |
| `glass list` | A card holding setting rows, split by `<hr class="divider" />`. |
| `glass`, `glass-base`, `glass-elevated`, `panel` | Surfaces, from flat to raised. |
| `row`, `col`, `grow`, `spacer` | Flex helpers. |
| `tabs` + `tab` (`on`) | A tab bar. |

## Text

`page-title`, `section-title`, `item-title`, `meta` (muted), `secondary`
(faint), `label` (small caps), `line` (one line, cut with …), `kbd`
(a key).

## Controls

| Markup | Looks like |
| --- | --- |
| `<button class="btn">` (`primary`, `tertiary`, `danger`, `small`) | Pious's buttons. |
| `<button class="icon-btn">` | A square icon button. |
| `<button class="chip">` (`on`) | A pill toggle. |
| `<button class="switch on" role="switch" aria-checked="true"><span></span></button>` | An on/off switch; toggle the `on` class. |
| `<input class="input">`, `<textarea class="input">` | Text fields. |
| `<select class="input select">` | A drop-down. |
| `<input type="range">` inside `<div class="slider">` with `<span class="value">` | A slider and its value. |
| `<button class="input hotkey">` (`listening`) | A key picker; add `<span class="pulse-dot"></span>` while listening. |

## Setting rows

What Settings and Tweaks are made of:

```html
<section class="group">
  <span class="label">Recording</span>
  <div class="glass list">
    <div class="setting-row">
      <div class="text">
        <span class="title">Keep the timing</span>
        <span class="description">Wait between actions as long as you did.</span>
      </div>
      <button class="switch on" role="switch" aria-checked="true"><span></span></button>
    </div>
    <hr class="divider" />
    <div class="setting-row off">…greyed out…</div>
  </div>
</section>
```

## Messages and empty states

| Markup | Looks like |
| --- | --- |
| `<div class="notice">` (`caution`) | An info strip (caution: a warning). |
| `<div class="glass-base empty-state">` with `<span class="tile-icon">`, `<span class="title">`, `<span class="body">` | "Nothing here yet". |
| `<span class="badge">` (`accent`, `strong`) | A small tag. |

## Motion

The animations Pious uses are there too: `animation: rise 280ms var(--ease)`,
`fade`, `pop` and `pulse`. Pious turns them off for users who chose reduced
motion.

The Macros plugin (`plugins/macros`) is built entirely from these classes;
read its `index.html` and `ui.js` for a complete example.
