# Rust GPUI (`apps/desktop-gpui`)

> Aplikasi utama Zandor sejak ADR-078. Tauri/React (`apps/desktop`) dibekukan sebagai referensi paritas.

## Menjalankan dan membangun

Dari `apps/desktop-gpui` (crate terpisah dari workspace, GPUI dari git Zed):

```bash
cargo check --offline
cargo test --offline                              # tes unit + kontrak (6+ diabaikan: butuh jaringan/mesin nyata)
bun ../../scripts/bump-dev-build.ts               # naikkan penanda dev.<N> di Sidebar (khusus development)
CARGO_INCREMENTAL=0 cargo build --release --offline   # untuk audit performa/rilis; harian cukup dev.cmd (debug + lld-link + incremental)
```

Hentikan instans **Zandor Dev** di dalam repo sebelum build ulang manual (`dev.cmd` otomatis melakukannya untuk proses di dalam folder repo saja; **jangan pernah** mematikan atau mengubah Zandor produksi yang terpasang). Ikon aplikasi ditanam lewat `build.rs` + `app.rc` (`assets/icon.ico`).

## Isolasi Development & Build Cepat

- **Linker cepat + profil dev:** `scripts/dev-watch.ps1` mendeteksi otomatis `rust-lld.exe` (`lld-link`) dari `rustc --print sysroot` lewat `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` serta mengaktifkan `CARGO_INCREMENTAL=1`. `[profile.dev]` memakai `opt-level = 0, debug = 0` sementara semua dependensi eksternal (`[profile.dev.package."*"]`) tetap `opt-level = 3`.
- **Isolasi penuh dari Zandor terpasang:** build debug memakai folder data `%APPDATA%\ZandorDev`, judul jendela `"Zandor Dev"`, dan mutex `Local\Zandor-GPUI-dev-single-instance`. Build rilis memakai `%APPDATA%\Zandor`, judul `"Zandor"`, dan `Local\Zandor-GPUI-single-instance`.
- **Versi di Sidebar (khusus dev):** `build.rs` membaca `apps/desktop-gpui/dev-build.txt` (di-gitignore) hanya pada profil debug tanpa `ZANDOR_RELEASE_VERSION`, lalu `shell/sidebar.rs` menampilkannya di bawah username (`v<ZANDOR_VERSION> · <ZANDOR_DEV_BUILD>`). Saat build rilis (`--release`/`--profile dist`/`scripts/rilis.ts`), informasi ini otomatis hilang.

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
