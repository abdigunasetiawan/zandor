# Rust GPUI (`apps/desktop-gpui`)

> Aplikasi utama Zandor sejak ADR-078. Tauri/React (`apps/desktop`) dibekukan sebagai referensi paritas.

## Menjalankan dan membangun

Dari `apps/desktop-gpui` (crate terpisah dari workspace, GPUI dari git Zed):

```bash
CARGO_INCREMENTAL=0 cargo check --offline
CARGO_INCREMENTAL=0 cargo test --offline          # 216 tes lulus (6 diabaikan: butuh jaringan/mesin nyata)
cargo build --release --offline                    # pakai ini untuk menilai performa; build debug jauh lebih lambat
./target/release/zandor-gpui.exe
```

Hentikan instans lama sebelum build ulang (`taskkill //IM zandor-gpui.exe //F`), exe yang berjalan terkunci.
Ikon aplikasi ditanam lewat `build.rs` + `app.rc` (`assets/icon.ico`).

## Struktur

| Folder | Isi |
| --- | --- |
| `src/core` | Logika tanpa UI: engine, chat, pengaturan, memori, pencarian, trajectory, sesi demo, dll. Port dari `packages/core`. |
| `src/shell` | Halaman dan panel: layout lima panel, sidebar, chat, model picker, pengaturan, onboarding, dokumen, terminal, jobs. |
| `src/ui` | Desain sistem Rust: tombol, select, menu, input teks (caret halus), toast, markdown/mermaid/matematika. |
| `src/theme.rs` | Token warna/ukuran (rem) dan `reduced_motion()` (pengaturan animasi Windows). |
| `assets` | i18n ID+EN, font, ikon, `demo-sessions.json` (diekspor dari React), skema pengaturan. |

## Pola performa

- Panel besar memakai `AnyView::cached`; panel harus berlangganan perubahan sendiri (observer) karena view ter-cache
  tidak dirender ulang otomatis.
- Daftar panjang (pesan chat, model picker, tab Uji) memakai `gpui::list` / `uniform_list`.
- Regex dikompilasi sekali lewat `core::re::re`.
- Caret input mengikuti animasi geser + kedip dengan timer, bukan frame terus-menerus; mati bila animasi Windows dimatikan.

## Sesi demo

Tiga sesi demo disematkan (Trajectory, Render Kaya, Izin & Pertanyaan). Sumber tunggal ada di React; perbarui dengan:

```bash
bun apps/desktop-gpui/scripts/export-demos.ts
```

## Status uji (19-11-2026)

- Tes Rust: 216 lulus. E2E React yang relevan (`chat.spec.ts`, `chat2.spec.ts`): 34/34.
- Diverifikasi langsung di aplikasi: tampilan, popup model (tab Model dan Uji), tempel teks panjang menjadi lampiran,
  tombol jawaban dan reset sesi demo, hover pilihan reasoning, nama mode izin baru.
- **Belum terverifikasi:** scroll tab Uji (skrip uji tidak bisa mengirim roda mouse ke aplikasi), kirim dengan
  Ctrl+Enter, konfirmasi hapus yang dimatikan, kelancaran resize/popup (belum diukur).
