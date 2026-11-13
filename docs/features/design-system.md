# Fitur: Design system

- **Status:** selesai untuk Fase 2 (Windows). Organisms/Panels/Pages menunggu Fase 3.
- **Keputusan:** [ADR-029](../decisions/ADR-029-design-system-base-ui.md)
- **Kode:** `packages/ui` (komponen, token), `packages/core/src/shortcuts.ts`, `apps/desktop/src/design-system/`

## Cara membuka

Jalankan aplikasi lalu buka `#/design-system` (tautan "Design system" di kaki kerangka aplikasi).
Kontrol tema (Gelap/Terang/Ikuti sistem) dan skala UI (75–150%) ada di header; zoom juga lewat `Ctrl +/-/0`.

## Isi

| Tingkat                    | Komponen                                                                                                              |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Tokens                     | warna, tipografi per variabel, ruang, radius, bayangan, font                                                          |
| Atoms (14)                 | Button, IconButton, Input, Textarea, Select, Switch, Checkbox, Slider, Badge, Avatar, Kbd, Tooltip, Spinner, Skeleton |
| Molecules (10)             | Menu, ContextMenu, Popover, Tabs, Toast, Dialog, ConfirmDialog (`useConfirm`), Combobox, SearchField, Field           |
| Organisms / Panels / Pages | belum ada (Fase 3) — ditandai jujur di halaman                                                                        |
| Shortcut                   | daftar pintasan yang terdaftar (global + scope)                                                                       |

## Pintasan bawaan Fase 2

| Pintasan                       | Aksi                                 |
| ------------------------------ | ------------------------------------ |
| `Ctrl+Alt+T`                   | ganti tema gelap/terang              |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | perbesar / perkecil / reset skala UI |

## Aturan memakai

- Warna dan ukuran hanya lewat `var(--z-*)`; `bun run design-tokens` menolak nilai mentah.
- Komponen baru: tambah ke `packages/ui/src/catalog.ts`, ekspor di `index.ts`, buat demo di `demos.tsx`, dan test.
- Dialog konfirmasi: `useConfirm()`; jangan pakai `window.confirm/alert/prompt`.
- String UI lewat `t()`; komponen dengan teks bawaan memakai kunci `ui.*`.

## Cara test

```
bun run test            # 170+ test unit/komponen (atoms, molecules, shortcut, token, katalog)
bun run design-tokens   # anti hardcode warna/ukuran dan dialog bawaan OS
bun run e2e             # 15 skenario Playwright di Edge (tema, zoom, menu, konfirmasi, fokus, font, rute)
```
Manual: buka halaman, pindah tema dan skala, coba keyboard-only (Tab, panah, Esc) pada setiap demo.

## Belum dicakup

Pembaca layar nyata, WebKit/macOS/Linux, virtualisasi daftar panjang, tema preset (Fase 10).
