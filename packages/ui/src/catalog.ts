// Katalog komponen: SATU daftar nama komponen UI per tingkat. Halaman design system wajib punya demo
// untuk setiap entri (dijaga test) dan setiap komponen yang diekspor wajib terdaftar di sini.

export const ATOMS = [
  "Button",
  "IconButton",
  "Input",
  "Textarea",
  "Select",
  "Switch",
  "Checkbox",
  "Slider",
  "Badge",
  "Avatar",
  "Kbd",
  "Tooltip",
  "Spinner",
  "Skeleton",
] as const;

export const MOLECULES = [
  "Menu",
  "ContextMenu",
  "Popover",
  "Tabs",
  "Toast",
  "Dialog",
  "ConfirmDialog",
  "Combobox",
  "SearchField",
  "Palette",
  "Field",
] as const;

/** Komponen infrastruktur (provider/pembungkus) yang tidak punya demo sendiri. */
export const INFRASTRUCTURE = [
  "ErrorBoundary",
  "ToastProvider",
  "ConfirmProvider",
  "ShortcutProvider",
  "ShortcutScope",
] as const;

export type AtomName = (typeof ATOMS)[number];
export type MoleculeName = (typeof MOLECULES)[number];

export const CATALOG = { atoms: ATOMS, molecules: MOLECULES, infrastructure: INFRASTRUCTURE } as const;
