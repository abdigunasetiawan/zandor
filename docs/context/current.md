# Kondisi Saat Ini

> Snapshot, selalu ditimpa. Terakhir diperbarui: 13-11-2026.

## Fase

- **Fase 0 — Spike Risiko:** lulus untuk Windows (ADR-019–027).
- **Fase 2 — Design System:** lulus 13-11-2026 (CI run 43427766836).
- **Fase 1 — Fondasi Repo, Docs, dan CI:** lulus 13-11-2026 (CI GitHub Actions run 43412491664, 14m50s).
- **Fase aktif: Fase 3 — App Shell** (kode selesai, menunggu CI).

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
