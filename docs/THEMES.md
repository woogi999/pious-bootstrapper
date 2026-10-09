# Making a theme

A theme is a folder with a `theme.json`. Everything in it is optional:
leave out what you don't want to change.

```
themes\
└── midnight\
    ├── theme.json
    ├── background.jpg     a background picture (optional)
    ├── theme.css          extra styles (optional)
    ├── font.ttf           a font of its own (optional)
    └── icons\home.svg     icons (optional)
```

## Installing, using and removing

- **Install:** put the folder in the `themes` folder (Settings → Appearance
  → **Themes folder**). Pious notices it within a few seconds.
- **Use:** Settings → Appearance → Themes → click it. Its look goes into
  your appearance settings (adjust anything after) and its stylesheet,
  icons, sounds and font apply while it's the theme in use.
- **Stop using:** pick another theme, or **Reset** in Appearance.
- **Remove:** delete its folder.
- **From a plugin:** list theme folders in the plugin's `themes` (see
  Plugins). They show while the plugin is on.

## theme.json

```json
{
  "id": "midnight",
  "name": "Midnight",
  "version": "1.0.0",
  "author": "You",
  "description": "Deep blue with a violet glow.",
  "colors": {
    "accent": "#8B7CFF",
    "accent_2": "#4FD1C5",
    "background": "#07070F",
    "surface": "#C9C8FF",
    "text": "#ECEBFF"
  },
  "gradient": { "from": "#241A5C", "to": "#07070F", "angle": 160, "opacity": 0.8 },
  "font": "Segoe UI",
  "font_file": "font.ttf",
  "radius": 1.2,
  "glass": 1.3,
  "font_scale": 1.0,
  "see_through": true,
  "window_opacity": 0.8,
  "blur": "Frosted",
  "background_image": "background.jpg",
  "image_dim": 0.6,
  "image_blur": 0.2,
  "css": "theme.css",
  "icons": { "home": "icons/home.svg" },
  "sounds": { "notification": "ping.ogg" }
}
```

## Properties

| Property | Type | What it does |
| --- | --- | --- |
| `id` | text | A unique ID (default: the folder's name) |
| `name`, `version`, `author`, `description` | text | Shown in Settings |
| `colors.accent` | `#RRGGBB` | Buttons, switches, highlights |
| `colors.accent_2` | `#RRGGBB` | A second accent (`--accent-2` in CSS) |
| `colors.background` | `#RRGGBB` | The window's background |
| `colors.surface` | `#RRGGBB` | The tint of glass panels (usually white or near) |
| `colors.text` | `#RRGGBB` | Text; dimmer text is mixed from it and the background |
| `gradient` | object | `from`, `to` (colors), `angle` (degrees, CSS-style), `opacity` (0–1) |
| `font` | text | A font family: one installed on the PC, or the family name of `font_file` |
| `font_file` | path | A `.ttf`/`.otf`/`.woff2` in the theme, loaded as `font` |
| `radius` | 0–2.5 | Corner roundness (1 = Pious's own, 0 = square) |
| `glass` | 0.3–2.5 | How strong glass panels are |
| `font_scale` | 0.8–1.3 | Text size |
| `see_through` | true/false | Let the desktop show through |
| `window_opacity` | 0.15–1 | How opaque the window is while see-through |
| `blur` | `"Off"`, `"Frosted"`, `"Diffused"`, `"Adjustable"` | How what's behind is blurred |
| `background_image` | path | A picture behind everything |
| `image_dim`, `image_blur` | 0–1 | How dimmed and blurred the picture is |
| `css` | path | A stylesheet for Pious's windows |
| `icons` | name → path | Replace Pious's icons (SVG or PNG) |
| `sounds` | name → path | Replace Pious's sounds (`notification`) |

Paths are relative to the theme's folder and must stay inside it. Colors
must look like `#1A2B3C`. A theme with a mistake is listed with the reason
and can't be used; it never stops Pious.

## Styles (theme.css)

Pious's colors are CSS variables on `<html>`, as `r g b` so you can add
transparency: `--bg`, `--surface`, `--panel`, `--text`, `--muted`,
`--faint`, `--accent`, `--accent-2`, `--accent-ink`, `--rim`. Sizes and
more: `--radius`, `--r-sm`, `--r-md`, `--r-lg`, `--r-xl`, `--font-scale`,
`--ui-font`, `--glass`, `--gradient`.

```css
/* Accent-colored page titles and a softer sidebar. */
.page-title { color: rgb(var(--accent)); }
aside { background: rgb(var(--bg) / 0.4); }
.btn.primary { background: linear-gradient(90deg, rgb(var(--accent)), rgb(var(--accent-2))); }
```

Class names can change between versions; prefer the variables where you
can. Stylesheets can't run code.

## Icons

Icon names are Pious's own: `home`, `game`, `server`, `friends`,
`accounts`, `versions`, `sliders` (Tweaks), `keyboard`, `macros`,
`running`, `news`, `settings`, `book-open` (Help), `search`, `play`,
`launch`, `add`, `remove`, `edit`, `heart`, `star`, `bell`, `chat`…
(any name the interface uses). An icon is drawn in the current text color,
using the picture's shape, so single-color SVGs work best.
