# Aturan Agent — Zandor

## Urutan baca di awal sesi

1. `docs/agents.md` (file ini)
2. `docs/context/current.md` — kondisi saat ini, fase aktif, pekerjaan terbuka
3. `docs/roadmap.md` — fase aktif dan definisi "selesai"
4. Bila menyentuh area tertentu: `docs/architecture.md`, `docs/rules.md`, `docs/design.md`, dan ADR terkait
   di `docs/decisions/`

## Aturan tetap

- Jawab dan dokumentasikan dalam **Bahasa Indonesia**. Jujur bila ada yang gagal, tidak teruji, atau
  rencana kurang tepat. Jangan menyatakan "selesai" tanpa bukti (test, output, atau langkah uji).
- **Tidak ada UI palsu** (ADR-018): fitur belum jadi disembunyikan atau diberi label "segera hadir".
- **Lisensi bersih** (PRD §4.8, ADR-009): hanya permisif. `bun run licenses` dan `cargo deny` wajib lulus.
  Dilarang menyalin kode dari `reference/openbot` atau folder `/ee` OpenWork.
- **UI selalu lewat skill `zandor-ui`** (`.claude/skills/zandor-ui/SKILL.md`): setiap membangun/mengubah/memperbaiki UI
  (komponen, halaman, CSS, token, ikon, teks i18n, tema) WAJIB memakai skill ini secara otomatis. Sebelum menulis kode,
  jalankan inventaris (`bun .claude/skills/zandor-ui/scripts/inventory.ts <kata kunci>`) untuk memeriksa apakah elemen/token/kelas
  sudah ada di desain sistem, lalu ikuti langkah token, aksesibilitas, i18n, dan verifikasinya. Agent lain tanpa skill
  mekanisme: baca berkas SKILL.md itu dan ikuti isinya.
- **Windows dulu** (ADR-027). Kode khusus Windows boleh; padanan unix ditandai `TODO(port)`.
- Kode di `spikes/` hanya referensi: pindahkan polanya, jangan mengimpornya.
- Bawaan wajib dari Fase 0 (lihat roadmap): plugin guard env, reset Ctrl+C, test kontrak saat upgrade
  opencode, build UI sebelum build Tauri.
- Keputusan arsitektur baru → ADR baru di `docs/decisions/` + satu baris di `docs/decisions.md`.
- Setelah menyelesaikan pekerjaan, perbarui `docs/context/current.md`.

## Cek sebelum menyerahkan pekerjaan

```
bun run check            # Biome, typecheck, Vitest, audit lisensi JS
bun run e2e              # Playwright (Edge)
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo deny check licenses bans sources
```
