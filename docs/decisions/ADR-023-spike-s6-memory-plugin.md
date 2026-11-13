# ADR-023 — Spike S6: plugin OpenCode untuk memori & pemicu skill creator

- **Tanggal:** 13-11-2026
- **Status:** Diterima, dengan risiko API "experimental" (lihat risiko 1)
- **Hasil spike:** ✅ Lulus — plugin menyuntik blok memori, menambah tool, ikut dalam kompaksi, dan
  memicu antrean skill, diuji pada `opencode serve` 1.19.5 asli (Windows)
- **Kode:** `spikes/s6-memory-plugin/` (`src/plugin.ts`, `test/plugin.e2e.test.ts`)
- **Melanjutkan:** ADR-011 (MemoryProvider), ADR-022 (hook `shell.env`)

## Pertanyaan
Apakah plugin OpenCode bisa (a) menyuntik blok memori ke system prompt, (b) ikut menentukan isi
kompaksi, dan (c) menambah tool kustom?
Kriteria lulus: plugin bisa menyuntik blok memori dan menambah tool.

## Keputusan
1. **Satu plugin Zandor** (calon `packages/plugin`), ditulis dalam TypeScript dengan tipe resmi
   `@opencode-ai/plugin@1.19.5` (MIT, versi sama dengan engine). Dikirim sebagai **satu file bundel**
   (`bun build --target bun`) dan dimuat lewat `OPENCODE_CONFIG_CONTENT`:
   `"plugin": [["file:///…/zandor-plugin.js", { "memoryDir": "…" }]]`. Config milik user tidak
   disentuh, dan opsi diterima plugin tanpa lewat env.
2. **Hook yang dipakai** (semuanya terbukti dipanggil binary 1.19.5):

   | Kebutuhan PRD | Hook / fitur | Status API |
   |---|---|---|
   | Blok memori di system prompt | `experimental.chat.system.transform` | experimental |
   | Sematkan ke konteks (tahan kompaksi) | `experimental.session.compacting` + disuntik ulang tiap giliran lewat system transform | experimental |
   | Tool memori | `tool: { memory_write, memory_search }` | stabil |
   | Pemicu skill: koreksi user | `chat.message` | stabil |
   | Pemicu skill: jumlah tool call | `tool.execute.after` | stabil |
   | Lindungi secret (ADR-022) | `shell.env` | stabil |
3. **MemoryProvider bawaan** (ADR-011) terbukti jalan di runtime plugin:
   - Markdown: `global.md`, `projects/<hash-worktree>.md`, `pins/<sessionID>.md`, dibaca ulang setiap
     giliran sehingga edit dari UI Zandor langsung berlaku tanpa restart engine.
   - Pencarian: **SQLite FTS5 lewat `bun:sqlite`** (tersedia di runtime Bun milik binary opencode,
     tanpa dependensi native tambahan).
   - Opt-in/opt-out lewat `settings.json` (`enabled`, `disabledSessions`). Saat nonaktif, tidak ada
     suntikan, dan tool menolak menulis.
4. **Skill creator** di plugin hanya **mencatat kandidat** ke `skill-queue.jsonl` dengan status `draft`
   (antrean persetujuan). Pembuatan draf skill oleh model dan UI persetujuan dikerjakan di Fase 9.
5. Data memori berada di **mesin engine** (`memoryDir`), sesuai prinsip local-first dan model
   "sumber mesin" di PRD §7.

## Bukti
`bun test` → **10/10 lulus**, `tsc --noEmit` bersih. Model = mock OpenAI-compatible lokal yang
mencatat setiap request dan menjalankan skenario tool call. Data opencode diisolasi (`XDG_*_HOME`).

| Skenario | Hasil |
|---|---|
| Plugin termuat dengan opsi dari config; `index.sqlite` dibuat | ✓ |
| Blok `<zandor-memory>` + isi `global.md` ada di system prompt yang dikirim ke model | ✓ |
| `memory_write` & `memory_search` muncul di daftar tool yang dikirim ke model | ✓ |
| Agent memanggil `memory_write` → file project terisi → request **berikutnya** sudah memuat fakta baru | ✓ |
| `memory_search` (FTS5) → output tool `(project) Proyek ini memakai [Tauri] 2 dan Bun.` | ✓ |
| Koreksi user ("salah, bukan begitu") → kandidat skill `correction` | ✓ |
| ≥ 5 tool call dalam satu sesi (termasuk 3 tool call paralel) → kandidat skill `tool-count` | ✓ |
| `POST /session/:id/summarize` → hook kompaksi dipanggil, prompt kompaksi memuat sematan; giliran setelah kompaksi masih memuat sematan + memori | ✓ |
| Opt-out per sesi lalu global → tidak ada suntikan; `memory_write` menolak | ✓ |
| Plugin sebagai bundel satu file (0,45 MB, tanpa `node_modules`) → 10/10 lulus juga | ✓ |

Jumlah panggilan hook dalam satu run: `system.transform` 16, `chat.message` 9,
`tool.execute.after` 6, `session.compacting` 1, `init` 1 (plugin diinisialisasi per instance/direktori).

## Temuan
1. **Tipe plugin ≠ perilaku binary.** `permission.ask` dan `experimental.provider.small_model` ada
   di `@opencode-ai/plugin@1.19.5`, tetapi **tidak pernah dipanggil** binary 1.19.5 (tidak ada
   `trigger(...)`). Hook harus selalu diverifikasi lewat test e2e, bukan dari tipe saja.
2. Hook system transform dipanggil juga untuk request non-chat (judul sesi, kompaksi). Blok memori
   ikut terkirim ke model kecil, yang berarti token tambahan. Perlu dibedakan di produk (mis. hanya
   untuk agent utama).
3. Plugin berjalan di dalam proses engine dengan hak penuh, termasuk mengakses file dan env proses.
   Plugin Zandor adalah kode tepercaya kita sendiri. Plugin pihak ketiga dari config user ikut termuat
   juga (ADR-019 poin 5).

## Risiko & yang belum terbukti (jujur)
1. **Fitur inti memori bergantung pada hook `experimental.*`**, yang bisa berubah atau hilang di
   versi opencode berikutnya. Mitigasi: versi engine di-pin (ADR-019), suite e2e S6 menjadi test
   kontrak wajib setiap upgrade, dan ada rencana cadangan kalau hook hilang: suntik memori sebagai
   part teks sintetis lewat `experimental.chat.messages.transform` (juga experimental) atau lewat
   prompt `system` per pesan dari Zandor (`POST /session/:id/message` menerima `system`). Cadangan ini
   belum diuji.
2. **Model yang dipakai adalah mock.** Belum terbukti model sungguhan benar-benar memakai blok memori
   dengan baik, mempertahankan sematan dalam ringkasan, atau memanggil `memory_write` di saat yang
   tepat. Itu masalah kualitas prompt/evaluasi (Fase 9), bukan kelayakan teknis.
3. Anggaran blok memori masih dalam karakter (6000), belum dalam token. Dampaknya pada konteks
   model kecil belum diukur.
4. Belum diuji: beberapa instance (multi-project) menulis ke `memoryDir` dan `index.sqlite` yang sama
   secara bersamaan; perilaku engine ketika plugin melempar error saat init; plugin di macOS/Linux.
5. Pembuatan skill sebenarnya (model menyusun SKILL.md, pemindai secret, persetujuan) belum dibuat.
   Yang terbukti baru pemicu dan antreannya.
6. Adapter Hindsight / Graphify tidak disentuh.

## Konsekuensi
- `packages/plugin` dimulai dari `src/plugin.ts` (guard `shell.env` + memori), dibundel dan dikirim
  bersama app. Supervisor (Fase 4) selalu memuatnya lewat `OPENCODE_CONFIG_CONTENT`.
- Upgrade opencode: jalankan ulang suite S2, S4, dan S6 sebelum versi pin dinaikkan.
- Fase 9 bisa fokus pada kualitas (prompt, evaluasi, UI) karena mekanisme dasarnya sudah terbukti.

## Cara mengulang
```
cd spikes
cargo build -p zandor-spike-s1            # menyalin sidecar opencode ke target/debug
cd s6-memory-plugin && bun install
bun test && bun x tsc --noEmit
bun build src/plugin.ts --target bun --outfile dist/zandor-plugin.js
ZANDOR_PLUGIN_PATH=dist/zandor-plugin.js bun test   # uji versi bundel
```
