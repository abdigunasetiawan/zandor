# ADR-029 — Design system: Base UI (headless) + token CSS sendiri

- **Tanggal:** 13-11-2026
- **Status:** Diterima
- **Terkait:** PRD §4.3–4.5, ADR-014 (zoom token), ADR-025 (rem), ADR-028 (lisensi)

## Konteks

Fase 2 membutuhkan 14 atom dan 10 molekul yang aksesibel (fokus, keyboard, ARIA), tanpa dialog bawaan OS,
dengan token warna/ukuran yang tidak boleh di-hardcode. Menulis sendiri perilaku menu, popover, combobox,
dialog, dan toast dengan benar sangat mahal dan rawan galat aksesibilitas.

## Keputusan

1. **Perilaku & ARIA: `@base-ui/react` 1.9.0 (MIT)**, di-pin persis. Dipakai untuk Tooltip, Popover, Menu,
   ContextMenu, Dialog, AlertDialog, Select, Combobox, Tabs, Toast, Slider, Switch, Checkbox, Input,
   Avatar. Tampilan 100% milik Zandor (CSS sendiri); Base UI tidak membawa gaya.
2. **API komponen Zandor membungkus Base UI** dan menyembunyikan detailnya: callback hanya menerima nilai
   (mis. `onCheckedChange(boolean)`, bukan argumen kedua milik Base UI), menu berbasis data
   (`entries: MenuEntry[]`), `Field` render-prop. Jadi mengganti pustaka dasar tidak memengaruhi pemakai.
3. **Token CSS** di satu file (`packages/ui/src/styles/tokens.css`): warna dark/light/ikut-sistem, tipografi
   per variabel (ukuran, ketebalan, tinggi baris, jarak huruf untuk h1–h4, p, small, link, button, label, code),
   spacing, radius, bayangan, ukuran kontrol, gerak, lapisan. Semua ukuran rem; root font-size =
   `16px × --ui-scale`.
4. **Font:** Plus Jakarta Sans Variable dan JetBrains Mono Variable via `@fontsource-variable` (OFL-1.1),
   dibundel (tanpa CDN, sesuai local-first). CSP: `font-src 'self' data:`.
5. **Penjagaan otomatis**:
   - `bun run design-tokens`: gagal bila ada warna hex/`rgb()`/ukuran `px` (kecuali garis `1px`) di luar
     `tokens.css`, atau `alert/confirm/prompt` bawaan OS.
   - Test kontras WCAG AA (4,5:1 teks, 3:1 fokus) untuk semua pasangan token di dark, light, dan ikut-sistem.
   - Test "katalog = demo = ekspor": komponen baru wajib terdaftar di `catalog.ts` dan punya demo di halaman
     design system.
6. **Pengganti dialog bawaan OS:** `useConfirm()` mengembalikan `Promise<boolean>` (Esc/batal = false).
7. **Shortcut:** `ShortcutRegistry` murni di `@zandor/core`; scope diambil dari atribut
   `data-shortcut-scope` pada leluhur elemen fokus. Scope `modal` eksklusif. Tanpa modifier tidak berlaku
   saat mengetik di input kecuali `allowInInput`. Binding bentrok (combo sama, scope sama) ditolak.
8. **Halaman design system** di `#/design-system[/bagian]`, urutan: Tokens → Atoms → Molecules → Organisms →
   Panels → Menus → Modals → Pages (+ Shortcut). Organisms/Panels/Pages ditandai "Belum ada" (diisi Fase 3).

## Temuan selama pengerjaan

- Base UI memanggil `onCheckedChange(checked, details)`; tanpa pembungkus, argumen kedua bocor ke pemakai.
- Tombol tutup `Toast` sengaja `aria-hidden` sampai toast difokus/di-hover; test mengikuti alur keyboard nyata.
- `Tabs`: panah kiri/kanan hanya memindahkan fokus, Enter/Spasi memilih (pola WAI-ARIA manual activation).
- Daftar shortcut yang dibaca saat render pertama kosong karena pendaftaran terjadi di effect; registry kini
  bisa di-`subscribe` dan `useShortcutList` memakai `useSyncExternalStore`.
- `sideOffset` Base UI memakai piksel; dihitung dari rem lewat `offsetRem()` agar ikut `--ui-scale`.

## Batas / belum teruji

- Hanya Windows/Edge (WebView2); belum di WebKit (macOS/Linux), ditunda (ADR-027).
- Belum ada audit aksesibilitas otomatis dengan pembaca layar nyata; yang ada: role/label di test, kontras,
  fokus terlihat, dan navigasi keyboard (e2e).
- Belum ada virtualisasi untuk daftar panjang (Combobox/Select) — diperlukan saat daftar model ratusan (Fase 6).
- Bundel JS > 500 kB (peringatan Vite); code-splitting dipertimbangkan saat ukuran menjadi masalah nyata.
- `IconButton` memakai Tooltip; perilaku tooltip pada layar sentuh belum dipertimbangkan (mobile di luar scope).
