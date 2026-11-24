# Aturan Agent — Zandor

## Fokus saat ini: Rust GPUI (ADR-078)

Pengembangan aktif hanya di **`apps/desktop-gpui`** (Rust GPUI). **Jangan menambah fitur ke
Tauri/React** (`apps/desktop`, `packages/*`): itu dibekukan sebagai referensi paritas 1:1 dan sumber teks/demo. Panduan,
perintah build/tes, dan struktur: [`docs/features/gpui.md`](features/gpui.md).

## Urutan baca di awal sesi

1. `docs/agents.md` (file ini)
2. `docs/context/current.md` — kondisi saat ini, fase aktif, pekerjaan terbuka
3. `docs/roadmap.md` — fase aktif dan definisi "selesai"
4. Bila menyentuh area tertentu: `docs/architecture.md`, `docs/rules.md`, `docs/design.md`, dan ADR terkait
   di `docs/decisions/`

## Branch aktif

Kerjakan di branch pengembangan aktif (saat ini **`zandor/release`**, atau branch kerja yang sedang aktif di worktree). Jangan checkout ke branch lain kecuali diminta secara eksplisit.

## Workflow Development & Isolasi (wajib untuk semua AI)

- **Dilarang keras menyentuh Zandor terpasang:** pemilik menjalankan aplikasi Zandor produksi sehari-hari di komputernya.
  AI **tidak boleh** membaca, mengubah, menghapus, menginstal ulang, atau mematikan proses/program/data milik Zandor produksi
  (`Zandor.exe` di luar folder repo, `%LOCALAPPDATA%\Programs\Zandor\`, maupun `%APPDATA%\Zandor\`).
- **Zandor Dev terpisah penuh:** build debug (`dev.cmd`) memakai nama jendela `"Zandor Dev"`, mutex tunggal `Local\Zandor-GPUI-dev-single-instance`,
  folder data `%APPDATA%\ZandorDev`, dan proses `apps/desktop-gpui/target/run/zandor-gpui.exe`. `scripts/dev-watch.ps1` hanya
  mengelola proses di dalam folder repo.
- **Iterasi build cepat (tanpa menunggu build rilis lama):** `scripts/dev-watch.ps1` mendeteksi otomatis `rust-lld.exe` (`lld-link`)
  dari `rustc --print sysroot` lewat `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER`, `[profile.dev]` memakai
  `opt-level = 0, debug = 0` (dependensi tetap `opt-level = 3`), dan `dev-watch.ps1` memakai `CARGO_INCREMENTAL=1`.
  Saat menyelesaikan fitur/perbaikan harian:
  1. Verifikasi dengan `cargo test --offline` (atau filter tes terkait, lalu `cargo test --offline` penuh).
  2. **Jangan** menjalankan `cargo build --release --offline` di setiap perubahan kecil bila tidak sedang mengaudit performa
     atau menyiapkan rilis — biarkan `dev.cmd` melakukan build debug inkremental.
- **Penanda versi build dev (wajib di akhir pesan):** di panel Sidebar di bawah username, build dev menampilkan
  `v<ZANDOR_VERSION> · <ZANDOR_DEV_BUILD>` (dari `apps/desktop-gpui/dev-build.txt`, di-gitignore). Setiap kali selesai
  menyelesaikan fitur atau perbaikan pada masa development:
  1. Jalankan `bun scripts/bump-dev-build.ts` (menaikkan `dev.<N>` di `dev-build.txt` dan memicu rebuild cepat di `dev.cmd`).
  2. **Wajib** tulis persis versinya di akhir laporan ke pemilik (contoh: `Versi Dev: v0.0.145 · dev.1`) agar pemilik tahu
     bahwa Zandor Dev yang sedang berjalan sudah memuat perubahan terbaru.
  3. Saat aplikasi dibangun untuk rilis (`--release` / `--profile dist` / `bun scripts/rilis.ts`), `cfg!(debug_assertions)`
     dan `ZANDOR_DEV_BUILD` kosong otomatis menghapus informasi versi build tersebut dari Sidebar.

## Aturan tetap

- Jawab dan dokumentasikan dalam **Bahasa Indonesia**. Jujur bila ada yang gagal, tidak teruji, atau
  rencana kurang tepat. Jangan menyatakan "selesai" tanpa bukti (test, output, atau langkah uji).
- **Tidak ada UI palsu** (ADR-018): fitur belum jadi disembunyikan atau diberi label "segera hadir".
- **Lisensi bersih** (PRD §4.8, ADR-009): hanya permisif. `bun run licenses` dan `cargo deny` wajib lulus.
  Dilarang menyalin kode dari `reference/openbot` atau folder `/ee` OpenWork.
- **Windows: jangan menyunting sumber lewat PowerShell `Get/Set-Content`** (merusak UTF-8); pakai alat edit atau Python.
- **UI selalu lewat skill `zandor-ui`** (`.claude/skills/zandor-ui/SKILL.md`): setiap membangun/mengubah/memperbaiki UI
  (komponen, halaman, CSS, token, ikon, teks i18n, tema) WAJIB memakai skill ini secara otomatis. Sebelum menulis kode,
  jalankan inventaris (`bun .claude/skills/zandor-ui/scripts/inventory.ts <kata kunci>`) untuk memeriksa apakah elemen/token/kelas
  sudah ada di desain sistem, lalu ikuti langkah token, aksesibilitas, i18n, dan verifikasinya. Agent lain tanpa skill
  mekanisme: baca berkas SKILL.md itu dan ikuti isinya.
- **Jangan menyebut nomor panel ("Panel 4") di teks untuk pengguna atau model** (i18n, deskripsi tool/skill, hasil tool): nomor tidak tampil di UI dan bergeser
  saat panel dibuka/ditutup. Pakai nama: panel Sidebar, Chat, Dokumen, Alat, Browser. Dijaga tes `i18n::tests::tanpa_nomor_panel_di_teks_pengguna_dan_model`.
- **Windows dulu** (ADR-027). Kode khusus Windows boleh; padanan unix ditandai `TODO(port)`.
- Kode di `spikes/` hanya referensi: pindahkan polanya, jangan mengimpornya.
- Bawaan wajib dari Fase 0 (lihat roadmap): plugin guard env, reset Ctrl+C, test kontrak saat upgrade
  opencode, build UI sebelum build Tauri.
- Keputusan arsitektur baru → ADR baru di `docs/decisions/` + satu baris di `docs/decisions.md`.
- Setelah menyelesaikan pekerjaan, perbarui `docs/context/current.md`.

## Cek sebelum menyerahkan pekerjaan

**Rust GPUI (utama)** — jalankan dari `apps/desktop-gpui`:

```
cargo test --offline
bun ../../scripts/bump-dev-build.ts     # wajib di akhir fitur dev; cantumkan hasilnya di akhir pesan
# hanya bila mengaudit performa/visual akhir atau menyiapkan rilis:
CARGO_INCREMENTAL=0 cargo build --release --offline
```

**Tauri/React (hanya bila menyentuh berkas di `apps/desktop` atau `packages/*`)**:

```
bun run check            # Biome, typecheck, Vitest, audit lisensi JS
bun run e2e              # Playwright (Edge)
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo deny check licenses bans sources
```
