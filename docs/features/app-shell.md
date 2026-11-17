# Fitur: App shell

- **Status:** selesai untuk Fase 3 (Windows). Isi panel menunggu Fase 4–8.
- **Keputusan:** [ADR-030](../decisions/ADR-030-app-shell.md)
- **Kode:** `apps/desktop/src/shell/`, `apps/desktop/src/App.tsx`, `packages/core/src/{layout-plan,commands,settings,navigation}.ts`

## Yang ada

| Bagian          | Perilaku                                                                                                        |
| --------------- | --------------------------------------------------------------------------------------------------------------- |
| Jendela         | Frameless; minimalkan/maksimalkan-pulihkan/tutup kustom (hanya di Tauri)                                        |
| Top bar         | Back/next, menu (Berkas, Tampilan, Bantuan, Dev Tools), omnibox = command palette, toggle panel, Pengaturan     |
| Lima panel      | Sidebar · Chat · Dokumen · Tools · Rel. Bisa di-resize, punya lebar minimum (rem), dilipat manual atau otomatis |
| Tombol tutup    | Panel Chat, Dokumen, dan Tools punya tombol tutup di baris yang sama dengan tab/header (rata kanan)             |
| Auto-collapse   | Dokumen → Sidebar → Tools; Chat tidak pernah. Terbuka lagi sendiri bila ruang cukup                             |
| Layout          | Ukuran dan panel yang dilipat pengguna tersimpan (bisa dimatikan di Pengaturan)                                 |
| Rel (P5)        | Ikon Terminal/Git/Browser/Info/Log: membuka panel Tools dan tab-nya; klik ikon aktif menutup                    |
| Command palette | `Ctrl+K` / `Ctrl+Shift+P`: cari fuzzy, ↑↓ Enter Esc                                                             |
| Pengaturan      | Dirender dari JSON Schema: bahasa, tema, skala UI, ingat tata letak; reset per pengaturan; JSON saat ini        |
| Empty state     | Setiap panel menyatakan fase hadirnya; tidak ada konten palsu                                                   |
| Tur pengenalan  | Saat dibuka tanpa project: nama, status model, project/Chat, pintasan (`Onboarding.tsx`); sekali, lalu dari palet |

## Pintasan

| Pintasan                   | Aksi                                                                                                       |
| -------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `Ctrl+K`, `Ctrl+Shift+P`   | command palette                                                                                            |
| `Ctrl+,`                   | Pengaturan                                                                                                 |
| `Alt+←` / `Alt+→`          | kembali / maju                                                                                             |
| `Ctrl+1` … `Ctrl+5`        | tampil/sembunyi panel 1 Sidebar, 2 Chat, 3 Dokumen, 4 Tools, 5 Rel (minimal satu dari 2/3/4 tetap terbuka) |
| `Ctrl+Alt+0`               | smart resize panel yang terbuka (juga tombol pertama di grup toggle panel; ADR-066)                        |
| `Ctrl +` `Ctrl -` `Ctrl 0` | zoom UI 75–150%                                                                                            |
| `Ctrl+Alt+T`               | ganti tema                                                                                                 |

## Cara test

```
bun run check                      # 264 test + lint + typecheck + lisensi + anti-hardcode
bun run e2e                        # 26 skenario Playwright (Edge), termasuk auto-collapse/zoom/persistensi
cd apps/desktop && bun run tauri build --debug --no-bundle --ci
bun run e2e:native <path exe>      # smoke native: kontrol jendela asli via CDP
```
Manual (belum otomatis): seret jendela lewat top bar, klik dua kali top bar, tepi jendela untuk resize, snap layout.

## Referensi pengaturan

[`docs/manual/settings.md`](../manual/settings.md) dan [`settings.schema.json`](../manual/settings.schema.json) (dihasilkan).
