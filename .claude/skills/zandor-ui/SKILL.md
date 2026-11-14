---

name: zandor-ui
description: WAJIB dipakai otomatis setiap kali membangun, mengubah, atau memperbaiki UI Zandor (komponen React, halaman, panel, CSS, token, ikon, teks UI/i18n, tema). Memaksa pengecekan desain sistem lebih dulu — apakah elemen sudah ada, apakah token dan kelas sesuai, aksesibilitas, i18n, dan tes — sebelum menulis kode. Gunakan juga saat mereview atau menambal bug tampilan.
---

# zandor-ui — membangun dan memperbaiki UI Zandor

Aturan proyek: **tidak ada warna/ukuran hard-coded, tidak ada UI palsu, tidak ada dialog bawaan OS, semuanya komponen,
semua teks ID + EN**. Skill ini adalah urutan kerja yang menjaga itu. Jangan menulis JSX/CSS sebelum **Langkah 1–3** selesai.

## Langkah 1 — Inventaris: apakah sudah ada?

Jalankan dari akar repo dengan kata kunci tentang yang akan dibuat (boleh beberapa, dalam bahasa Indonesia atau Inggris):

```bash
bun .claude/skills/zandor-ui/scripts/inventory.ts tabel chip dialog
```

Keluarannya mendaftar yang **sudah ada** dan cocok: token `--z-*`, atom/molekul (`catalog.ts`), hook/util `@zandor/ui`, ikon,
**kelas CSS**, komponen aplikasi di `apps/desktop/src/shell`, dan kunci i18n. Tanpa kata kunci → ringkasan semuanya.

Keputusan setelah inventaris (urutan prioritas):

1. **Pakai komponen yang ada** (`Button`, `IconButton`, `Field`, `Dialog`, `ConfirmDialog` via `useConfirm`, `Tabs`, `Select`,
   `Switch`, `Badge`, `Toast` via `useToast`, `Menu`/`ContextMenu`, `Palette`, …). Jangan membuat `<button>`/`<select>`/modal
   sendiri bila ada padanannya. Cek juga komponen shell yang mirip (mis. kartu izin, `QuestionCard`, `ComposerInfo`) dan
   halaman referensi `#/design-system`.
2. **Perluas komponen yang ada** (varian/prop baru) bila hampir cocok — ubah di `packages/ui`, bukan menyalin.
3. **Buat komponen baru hanya bila benar-benar belum ada.** Komponen reusable baru di `packages/ui`:
   daftarkan di `packages/ui/src/catalog.ts` (ATOMS/MOLECULES), beri demo di `apps/desktop/src/design-system/demos.tsx`,
   tes di `*.test.tsx` (`coverage.test.tsx` menjaga katalog ↔ demo ↔ ekspor). Komponen khusus satu fitur boleh di
   `apps/desktop/src/shell/`, tetap memakai atom/molekul yang ada.

Bila ragu, sebutkan temuan inventaris di jawaban (apa yang dipakai ulang, apa yang baru, dan alasannya).

## Langkah 2 — Token: apakah nilainya sesuai?

Sumber kebenaran **satu-satunya** untuk nilai mentah: `packages/ui/src/styles/tokens.css`.

- Warna: `var(--z-color-*)` (bg, surface, raised, hover, active, border, border-strong, fg, fg-muted, fg-subtle, accent,
  on-accent, focus, danger, success, warning). Sintaks kode: `--z-code-*`. Panel chat: `--z-chat-*`.
- Ukuran: **`rem`** lewat token `--z-space-*`, `--z-radius-*`, `--z-icon-*`, `--z-control-*`, `--z-type-*-size`; boleh
  `calc(var(--z-space-N) * k)`. Garis `1px` hanya lewat `--z-border-width`. Dilarang `px`, `#hex`, `rgb()/hsl()` di luar `tokens.css`
  (juga di atribut `placeholder`/teks JSX — gunakan teks i18n).
- Tipografi: kelas `z-h1..z-h4`, `z-p`, `z-small`, `z-label`, atau token `--z-type-*`; font lewat `--z-font-*`.
- Gerak: `--z-duration-*`/`--z-ease` (hormati `prefers-reduced-motion` yang sudah diatur di token).
- Perlu token baru? Tambahkan di `tokens.css` untuk **gelap, terang, dan blok `prefers-color-scheme: light`**, periksa kontras
  (WCAG AA: teks ≥ 4.5:1), dan bila relevan masukkan ke `COLOR_KEYS` agar bisa dikustomisasi (`packages/core/src/theme`).
- Tema dan zoom harus tetap benar: uji gelap + terang; zoom 75–150% tidak boleh membuat luapan/pemotongan.

Verifikasi otomatis (wajib lulus): `bun run design-tokens`.

## Langkah 3 — CSS dan markup

- Kelas baru: cek **tabrakan nama** di keluaran inventaris (pelajaran: `.chip` dipakai lampiran; chip saran harus punya kelas
  sendiri). Gaya khusus fitur di `apps/desktop/src/styles.css` (BEM `blok__elemen`), gaya komponen reusable di
  `packages/ui/src/styles/*.css`.
- Biome menegakkan urutan spesifisitas (`noDescendingSpecificity`): letakkan aturan spesifik **setelah** yang umum.
- Gunakan elemen semantik (`button`, `fieldset/legend`, `section`, `dl`, `ul/ol`) dan label pada setiap kontrol.
  Biome a11y: tanpa `role`/`aria-label` pada elemen yang tidak mendukungnya, tanpa `tabIndex` pada elemen non-interaktif
  (kecuali area gulir yang diberi `biome-ignore` beralasan), `<label>` harus terhubung ke kontrol.
- Ikon: tambah di `packages/ui/src/icons.tsx` (lucide) lalu impor dari `@zandor/ui`; ukuran lewat prop `size`, bukan px.
- Teks dari model/berkas/jaringan **tidak dipercaya**: render sebagai teks/elemen React; HTML mentah hanya dari library yang
  menyanitasi (Mermaid strict). Dilarang `dangerouslySetInnerHTML` tanpa alasan tertulis (`biome-ignore` + komentar).
- Objek `components`/handler yang diteruskan ke pustaka render (mis. `react-markdown`) harus **stabil** (di luar render), jika
  tidak anak-anaknya di-remount dan state hilang.

## Langkah 4 — Perilaku wajib

- **Tidak ada UI palsu** (ADR-018): fitur yang belum jadi disembunyikan atau diberi label jujur; tanpa data dummy di build
  biasa. Di luar Tauri tampilkan penjelasan/pratinjau berlabel, bukan data buatan.
- **Tidak ada `alert/confirm/prompt`**: pakai `useConfirm`/`Dialog`/`useToast`.
- **i18n**: setiap string UI baru ada di `packages/i18n/src/locales/id.ts` **dan** `en.ts` (tipe `MessageKey` menjaga
  paritas). Tidak ada teks hard-coded di JSX, termasuk `aria-label`, `placeholder`, `title`.
- **Keyboard-first**: aksi penting punya shortcut lewat `useShortcut`/perintah palet (`CommandCenter`); fokus terlihat; dialog
  menjebak fokus (komponen `Dialog` sudah melakukannya).
- **Overlay vs browser bawaan**: dialog/menu yang menimpa Panel 4 harus menyembunyikan child webview (ADR-025) — gunakan
  komponen overlay yang ada agar otomatis.
- **Status jujur**: loading, kosong, galat (dengan pesan penyebab), dan tak-tersedia masing-masing punya tampilan; galat
  menampilkan sebab nyata dan jalan keluar (coba lagi/buka pengaturan).
- Performa: daftar panjang divirtualisasi (`@tanstack/react-virtual`); modul berat dimuat malas (`lazy`).
- Pengaturan baru → `packages/core/src/settings.ts` (JSON Schema) + `docs/manual/`; kustomisasi tampilan → `AppearanceStore`.

## Langkah 5 — Verifikasi (jangan menyatakan selesai tanpa bukti)

```bash
bun run design-tokens                     # token/dialog
bun x biome check --write .               # lint + a11y + format
bun run typecheck
bun x vitest run <berkas-terkait>         # tes komponen/unit (jsdom, Testing Library)
cd apps/desktop && bun x playwright test e2e/<spec-terkait>.spec.ts   # alur utama di Edge
bun run check                             # semuanya sebelum menyerahkan
```

- Tambah tes untuk perilaku nyata: komponen (`*.test.tsx`) dan e2e (`apps/desktop/e2e`, mock engine di `mock-engine.ts`).
  Pakai query aksesibel (`getByRole`, label), bukan kelas CSS; `data-testid` hanya bila tak ada peran yang jelas.
- Untuk perubahan visual, lihat hasilnya sungguhan: `bun run dev` (aplikasi), atau `bun run --filter @zandor/desktop dev`
  lalu buka `http://localhost:1420` di Browser pane; cek gelap/terang, zoom, lebar sempit, dan keyboard. Bila mengubah
  perilaku native (Tauri), jalankan smoke native (`apps/desktop/e2e-native/smoke.ts`).
- Perubahan di halaman design system / komponen baru: pastikan `coverage.test.tsx` lulus.
- Perbarui dokumen fitur `docs/features/<fitur>.md` (spek, status, cara test) dan `docs/manual/` bila perilaku pengguna berubah;
  keputusan desain baru → ADR + satu baris di `docs/decisions.md`; lalu `docs/context/current.md`.

## Daftar periksa cepat (centang sebelum menyerahkan)

- [ ] Sudah menjalankan inventaris dan memakai ulang yang ada (atau menjelaskan mengapa membuat baru)?
- [ ] Nol warna/ukuran mentah di luar `tokens.css`; `bun run design-tokens` hijau?
- [ ] Gelap + terang + zoom 75–150% diperiksa; kontras teks ≥ 4.5:1?
- [ ] Label/peran/keyboard benar; fokus terlihat; tanpa dialog bawaan OS?
- [ ] Teks UI ada di id.ts dan en.ts?
- [ ] Status loading/kosong/galat/tak-tersedia jujur, tanpa data dummy?
- [ ] Komponen baru terdaftar di katalog + demo design system + tes?
- [ ] Tes komponen/e2e baru lulus; `bun run check` hijau; dokumen fitur diperbarui?
