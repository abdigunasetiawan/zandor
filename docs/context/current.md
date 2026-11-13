# Kondisi Saat Ini

> Snapshot, selalu ditimpa. Terakhir diperbarui: 13-11-2026.

## Fase

- **Fase 0 — Spike Risiko:** lulus untuk Windows (ADR-019–027).
- **Fase 2 — Design System:** lulus 13-11-2026 (CI run 43427766836).
- **Fase 1 — Fondasi Repo, Docs, dan CI:** lulus 13-11-2026 (CI GitHub Actions run 43412491664, 14m50s).
- **Fase 3 — App Shell:** lulus (CI hijau).
- **Fase 4 — Engine & Project:** lulus (CI hijau).
- **Fase 5 — Chat Inti:** lulus (CI hijau), pelengkap selesai.
- **Fase aktif: Fase 6 — Provider & Model** (selesai di lokal dengan batasan; belum di-push).

## Checklist Fase 1

- [x] Monorepo Bun workspaces: `apps/desktop`, `packages/{core,ui,i18n,plugin}`
- [x] Biome, TypeScript 7 strict, Clippy `-D warnings`, rustfmt, toolchain di-pin
- [x] Vitest + Testing Library; Playwright + Edge; `cargo test`
- [x] i18n ID/EN bertipe, error boundary per panel, logger dengan redaksi secret, nomor build `0.0.<commit>`
- [x] Audit lisensi JS (`bun run licenses`) + konfigurasi `cargo-deny`
- [x] Workflow CI GitHub Actions (Windows)
- [x] Struktur docs ramping + ADR 001–018 dipecah ke file
- [x] CI lulus di GitHub (`windows-latest`), artefak installer `zandor-windows`

## Pekerjaan terbuka / keputusan tertunda

- Tinjau ADR-028: Tauri membawa 5 crate MPL-2.0 (dikecualikan per crate agar CI lulus).
- PPTX: lisensi MPL-2.0 di `@aiden0z/pptx-renderer` (ADR-026) — diputuskan sebelum Fase 11.
- Persistensi sesi Google di browser bawaan belum konsisten (ADR-020) — diuji ulang di Fase 8.
- Lisensi Zandor sendiri belum ditentukan (paket ditandai `UNLICENSED`/private).
- Ikon aplikasi masih placeholder dari spike.

## Fase 2 — Design System: LULUS (CI hijau, run 43427766836)

## Fase 3 — App Shell: selesai di lokal (menunggu CI hijau)

- [x] Jendela frameless + kontrol kustom (terbukti di jendela asli via smoke native 8/8)
- [x] Top bar: back/next, menu, omnibox/command palette, toggle panel, Pengaturan
- [x] Lima panel: resize, minimum, collapse, auto-collapse deterministik, layout tersimpan; rel memicu Tools
- [x] Zoom `Ctrl +/-/0`
- [x] Pengaturan berbasis JSON Schema + dokumen skema yang dihasilkan
- [x] Test: 264 unit/komponen, 26 e2e (Edge), smoke native
- [ ] CI GitHub hijau untuk commit Fase 3

## Pekerjaan terbuka / keputusan tertunda

- Uji manual: seret jendela lewat top bar, klik dua kali, resize dari tepi, snap layout Windows 11 (ADR-030).
- Tinjau ADR-028: Tauri membawa 5 crate MPL-2.0 (dikecualikan per crate agar CI lulus).
- PPTX: lisensi MPL-2.0 di `@aiden0z/pptx-renderer` (ADR-026) — diputuskan sebelum Fase 11.
- Persistensi sesi Google di browser bawaan belum konsisten (ADR-020) — diuji ulang di Fase 8.
- Lisensi Zandor sendiri belum ditentukan (paket ditandai `UNLICENSED`/private).
- Ikon aplikasi masih placeholder dari spike.
- Menu Edit ditunda sampai ada fitur yang bisa diedit.

## Fase 4 — Engine & Project: selesai di lokal (menunggu CI hijau)

- [x] Supervisor engine produksi (start, health, restart terbatas, mati bersama app), engine di-pin + sha256
- [x] Plugin guard wajib + penanda diverifikasi (temuan: plugin dimuat per-instance; path `\?\` merusak URL)
- [x] Connection manager lokal (struktur remote ada, belum diisi)
- [x] Project (tambah/daftar/ganti nama/lepas), sesi (buat/ganti nama/hapus/pilih), sesi tanpa project, pencarian
- [x] Tab Files: pohon file nyata, ikon Material Icon Theme; ikon UI Lucide
- [x] Terbukti end-to-end: smoke native 12/12 termasuk restart aplikasi → sesi tetap ada
- [ ] Belum: UI warna project, single-instance (DB terkunci bila dua Zandor), remote engine (Fase 11)
- Catatan CI: langkah baru `bun run prepare:engine` dan `ZANDOR_REQUIRE_ENGINE=1`; belum pernah dijalankan di runner.

## Fase 5 — Chat Inti: inti selesai di lokal (belum di-push)

- [x] SSE per direktori + reducer murni + `ChatStore`, diuji dengan rekaman SSE asli
- [x] Transcript Markdown, reasoning, kartu tool call, meta, composer, pilih model, Stop
- [x] Izin bawaan `ask` (temuan: opencode bawaan tidak meminta izin) + kartu izin dengan diff
- [x] Provider kustom OpenAI-compatible lewat `providers.json` (kunci hanya via env; bukan di repo)
- [x] Terbukti nyata: smoke native 15/15 (jawaban streaming, tulis file → izin → selesai, restart → sesi tetap)
- [ ] Belum: UI provider, revert/regenerate/cabang sesi, lampiran, mode izin/reasoning, tab sesi, web search, salin, virtualisasi sungguhan
- Catatan: `providers.json` milik pengguna ada di `%APPDATA%\id.web.zandor.app\` (berisi kunci; jangan di-commit). Fase chat nyata di smoke dilewati di CI (tak ada provider).

## Fase 5 (pelengkap) dan Fase 6 — Provider & Model: selesai di lokal (belum di-push)

- [x] Tab sesi, salin, mode izin, reasoning, web search, diff, lampiran, regenerate, revert, fork, virtualisasi transcript
- [x] Kunci API di keychain OS; `providers.json` v2 tanpa kunci; migrasi otomatis (terbukti pada mesin pengembang)
- [x] Endpoint kustom, lokal (deteksi Ollama/LM Studio), provider katalog (kunci), sign-in OAuth lewat engine
- [x] Probe/ping dari Rust dengan status jujur dan batas waktu; pemilih model penuh (cari, sematkan, uji)
- [x] Tes: 401 unit, 60 e2e, 29 Rust, smoke native 16/16
- [ ] Belum terbukti: chat nyata setelah migrasi keychain (upstream proxy Antigravity sedang HTTP 500), model lokal sungguhan, login OAuth sungguhan
- [ ] Belum: provider katalog yang butuh data tambahan, macOS/Linux
- Catatan: kunci Anda sekarang di Windows Credential Manager (`id.web.zandor`); `providers.json` tidak lagi memuatnya.
