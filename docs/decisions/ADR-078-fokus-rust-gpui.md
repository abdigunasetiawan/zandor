# ADR-078 — Fokus ke Rust GPUI; Tauri/React dibekukan

Tanggal: 19-11-2026 · Status: diterima · Menggantikan: ADR-003

## Konteks

ADR-003 mencoret GPUI karena GPUI tidak punya webview, sehingga remake berarti menulis ulang semuanya. Sejak itu
aplikasi Tauri + React berkembang sampai 899 tes unit dan 214 tes e2e. Pemilik memutuskan menulis ulang Zandor
sebagai aplikasi native Rust GPUI (`apps/desktop-gpui`, branch `zandor/rewrite-gpui`) dengan tampilan dan fitur 1:1
dengan versi React. Hasil rewrite ini sudah mencakup shell lima panel, chat, model picker, pengaturan, onboarding,
dokumen, terminal, jobs, trajectory, sesi demo, dan seterusnya.

## Keputusan

1. **Pengembangan aktif hanya di `apps/desktop-gpui`.** Fitur, perbaikan, dan tes baru ditulis di sana.
2. **`apps/desktop` (Tauri + React) dan `packages/*` dibekukan sebagai referensi.** Tidak ada fitur baru. Perbaikan
   hanya bila diperlukan untuk menjaga paritas atau sumber data bersama:
   - `packages/i18n` tetap sumber teks (aset `apps/desktop-gpui/assets/i18n/*.json` diekspor/diselaraskan darinya).
   - `apps/desktop-gpui/scripts/export-demos.ts` mengekspor sesi demo dari sumber React ke `assets/demo-sessions.json`.
   - Tes e2e React tetap dijalankan hanya bila berkas React berubah.
3. **Paritas 1:1 dengan React** tetap menjadi kriteria: UI, teks (ID + EN), perilaku, dan tema. Perbedaan harus
   dicatat sebagai batasan di `docs/context/current.md`.
4. **Crate terpisah dari workspace Rust utama** (`[workspace]` kosong di `apps/desktop-gpui/Cargo.toml`) karena GPUI
   diambil dari git Zed dan berat. Cek wajib: `cargo test --offline` dan `cargo build --release --offline` di folder itu.
5. Aturan lama yang tetap berlaku: Bahasa Indonesia, tanpa UI palsu (ADR-018), lisensi bersih (ADR-009), Windows dulu
   (ADR-027), skill `zandor-ui` untuk keputusan tampilan (token/komponen dari desain sistem, kini versi Rust di `src/ui`).

## Konsekuensi

- Webview tidak ada: dokumen (Markdown, Mermaid, matematika, PDF, kode) dirender native (`ui/markdown.rs`,
  `ui/mermaid.rs`, `ui/math.rs`, `shell/pdf_view.rs`, `ui/highlight.rs`). Browser bawaan Panel 4 dan HTML live
  server memakai jalur terpisah (`shell/browser.rs`).
- Performa dijaga lewat build release (profil `dev`/`release` di `Cargo.toml`), view ter-cache, dan daftar virtual
  (`gpui::list`/`uniform_list`) untuk chat dan model picker.
- Risiko: batasan GPUI (mis. sebagian gestur/hover tidak bisa diuji otomatis lewat skrip), dependensi git Zed yang
  bergerak, dan biaya menjaga dua basis kode selama masa transisi.
