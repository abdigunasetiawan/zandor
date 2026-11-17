# Memori & Skill Creator (Fase 9)

- **Status:** selesai di lokal dengan batasan (di bawah) · **ADR:** [ADR-037](../decisions/ADR-037-memori-dan-skill-creator.md)
- **Manual pengguna:** [memori-dan-skill.md](../manual/memori-dan-skill.md)

## Spek

- **Opt-in** (bawaan mati): sesi → project → bawaan; skill creator punya saklar sendiri. Dibaca ulang tiap giliran.
- **Memori:** `global.md`, `projects/<key>.md`, sematan `pins/<sesi>.md` (tahan kompaksi); blok `<zandor-memory>` maks 6000
  karakter disuntik plugin; tool `memory_write` (menolak secret) dan `memory_search` (FTS5, cadangan pemindaian teks).
- **Skill creator:** pemicu koreksi / `/reflect` / ≥ 5 tool call → kandidat → petunjuk satu kali → tool `skill_propose`
  (validasi + pemindai secret) → antrean `pending` → **disetujui pengguna** → `SKILL.md` (global atau project).
- **UI:** halaman `/memory` (Pengaturan, Memori, Antrean skill, Skill terpasang), tombol sidebar, perintah palet,
  opt-in per sesi di Info sesi, dan sakelar memori project langsung di dialog percayai folder saat menambah project
  (17-11-2026; override disimpan hanya bila berbeda dari bawaan, tidak tampil untuk project di engine remote).
  Di luar Tauri: pratinjau di memori proses dengan label jujur.
- **Keamanan:** perintah Rust hanya untuk berkas daftar-putih; nama skill ketat; skill project hanya di project terdaftar.

## Cara test

| Lapisan               | Perintah                                                                                                               | Cakupan                                                                                                                                                 |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plugin (unit)         | `bun x vitest run packages/plugin`                                                                                     | kunci project, pemindai secret, opt-in, memori, antrean, hook (28 tes)                                                                                  |
| Plugin vs engine asli | `cd packages/plugin && bun run e2e:engine` (butuh `bun run prepare:engine`)                                            | 20 skenario di opencode 1.19.5 dengan model tiruan (isolasi project, lintas sesi, memory_search, skill per project/sesi, /reflect, tool-count, dispose) |
| Store + paritas       | `bun x vitest run packages/core/src/memory`                                                                            | 14 tes, termasuk paritas dengan plugin                                                                                                                  |
| Komponen              | `bun x vitest run apps/desktop/src/shell/MemoryPage.test.tsx`                                                          | 7 tes: antrean, secret, skill terpasang, opt-in                                                                                                         |
| Rust                  | `cargo test --workspace`                                                                                               | daftar putih berkas, nama skill, tulis/hapus aman                                                                                                       |
| E2E web               | `bun x playwright test e2e/memory.spec.ts`                                                                             | 3 tes: halaman, palet, opt-in + secret                                                                                                                  |
| Native                | `bun run tauri build --debug --no-bundle` lalu `ZANDOR_SMOKE_FORCE_CHAT=1 bun e2e-native/smoke.ts <exe>` (48/48 lulus) | plugin memuat `memoryDir`, UI menulis berkas asli, skill terpasang, **model nyata menjawab dari memori**                                                |

Smoke native menyentuh data memori asli (settings, `global.md`, antrean) dan memulihkannya di akhir.

## Batasan jujur

- Hook `experimental.*` opencode bisa berubah; ulangi `e2e:engine` saat menaikkan versi pin.
- Kualitas skill dan kapan model memanggil `skill_propose` bergantung pada model; yang terbukti: mekanismenya dengan model
  tiruan, dan memori global dengan model nyata (`opencode/deepseek-v4.1-flash-free`; model smoke bawaan kini DeepSeek karena upstream Antigravity/Gemini tidak stabil — Gemini 3.8 Flash (High) tetap bisa dipilih lewat `ZANDOR_SMOKE_MODEL`).
- Skill baru dimuat ulang lewat dispose instance (ADR-063); perilaku SSE setelahnya belum diverifikasi di aplikasi native.
- Smoke native model nyata (`apps/desktop/e2e-native/memory-smoke.ts`, 15-11-2026, Gemini 3.8 Flash (High) via Omni Route): memori project/global, isolasi, `memory_write`/`memory_search`, penolakan secret, koreksi → `skill_propose` → approve di UI → skill terbaca engine setelah dispose (project A tetap bisa chat) lulus 20/20 pada satu run penuh. Run lain kadang macet karena model/upstream (kirim ulang otomatis hingga 2 kali), bukan karena aplikasi. Jalankan: `ZANDOR_SMOKE_MODEL="Gemini 3.8 Flash (High)" ZANDOR_SMOKE_FORCE_CHAT=1 bun e2e-native/memory-smoke.ts <exe>` (Zandor Dev harus tertutup; butuh provider di data dev).
- Belum: adapter Hindsight/Graphify, opt-in di dialog "Tambah project", memori di mesin remote (Fase 11).
- Sematkan ke konteks: menu "Simpan ke catatan" pada pesan (chat) punya item **Sematkan ke konteks sesi** yang menambahkan isi ke `pins/<sesi>.md` (tahan kompaksi); bisa diedit/dihapus di tab Memori. Secret dipindai dan ditolak.
- Antrean ditulis ulang utuh oleh UI (dibaca ulang tepat sebelum menulis); balapan dengan plugin sangat kecil tetapi mungkin.

## Toggle di composer (ADR-060)

Dua tombol di composer: _Ingat memori_ dan _Skill creator_. Di project, tombol mengubah pengaturan project (dibagi semua sesinya); di Chat tanpa project, hanya sesi itu. Skill creator mengikuti resolusi sesi → project → bawaan (`skillCreatorSessions`/`skillCreatorProjects` di `settings.json`). Diuji: unit (inti + plugin + paritas) dan e2e `memory.spec.ts`. Belum diuji di aplikasi native.
