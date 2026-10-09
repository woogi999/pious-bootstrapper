// Icons and sounds that plugins and the theme in use replace (filled in by
// `followUiAssets` in theme.ts). Reactive, so icons change as soon as a
// plugin or theme is turned on or off.

export const overrides = $state({
  /** Pious icon name → picture URL. */
  icons: {} as Record<string, string>,
  /** Pious sound ("notification") → sound URL. */
  sounds: {} as Record<string, string>,
});
