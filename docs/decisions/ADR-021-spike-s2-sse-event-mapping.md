# ADR-021 — Spike S2: pemetaan event SSE `opencode serve` ke transcript

- **Tanggal:** 13-11-2026
- **Status:** Diterima
- **Hasil spike:** ✅ Lulus — transcript lengkap ter-render dari event nyata. Beberapa jenis part
  hanya terbukti lewat skema, belum lewat rekaman (lihat "Belum terbukti").
- **Kode:** `spikes/s2-events/` (`src/reducer.ts`, `src/render.ts`, `test/`, `tools/record.mjs`)
- **Fixture:** `spikes/s2-events/fixtures/recording/` (rekaman nyata, path pribadi disamarkan),
  `fixtures/transcript.html` (hasil render), `fixtures/openapi-1.19.5.json`

## Pertanyaan

Bisakah semua part (thinking, tool, file, patch, permission) dari event SSE `opencode serve`
dipetakan? Lulus jika contoh transcript lengkap ter-render dari event nyata.

## Keputusan

1. **Kontrak tipe = SDK resmi `@opencode-ai/sdk` (MIT), versi sama persis dengan engine yang
   di-pin (1.19.5)**, import dari `@opencode-ai/sdk/v2/types`. Tipe tidak ditulis tangan. Upgrade
   engine berarti upgrade SDK di commit yang sama.
2. **Stream:** `GET /event?directory=<project>` (per instance) lewat `fetch` + `ReadableStream`
   dengan header `Authorization` (lihat ADR-019: `EventSource` tidak bisa mengirim header).
3. **Model state = "part utuh menang":**
   - `message.updated` → upsert info pesan.
   - `message.part.updated` → ganti part utuh (selalu membawa state lengkap).
   - `message.part.delta {partID, field, delta}` → tambahkan string ke `part[field]`, hanya untuk
     streaming. Dipakai untuk `text` **dan** `reasoning` (field `"text"`). Delta yang datang sebelum
     part-nya dikenal ditahan di buffer.
   - Urutan pesan dan part = urutan ID (monoton naik).
4. **Izin:** `permission.asked` membawa `tool.callID` → kartu izin ditempel ke tool call yang
   bersangkutan. `permission.replied` menandai `once | always | reject`. Balasan lewat
   `POST /permission/{id}/reply {reply, message?}`. Metadata izin `edit` sudah memuat **diff**, sehingga
   kartu izin bisa menampilkan perubahan sebelum disetujui.
5. **Klasifikasi semua 89 jenis event di skema** (dijaga oleh test):
   - _Transcript_ (19): `session.created/updated/deleted/status/idle/error/diff/compacted`,
     `message.updated/removed`, `message.part.updated/delta/removed`, `permission.asked/replied`,
     `question.asked/replied/rejected`, `todo.updated`.
   - _Milik panel lain / infrastruktur_: `file.*`, `pty.*` (Terminal), `vcs.branch.updated` (Git),
     `lsp.*`, `project.*`, `plugin.added`, `mcp.*`, `workspace.*`, `worktree.*`, `tui.*`, dst.
   - _Keluarga v2_ (`session.next.*`, `permission.v2.*`, `question.v2.*`): ada di skema tetapi
     **tidak dipancarkan** `/event` di 1.19.5. Untuk sekarang diabaikan, dan test akan gagal begitu
     engine mulai memancarkannya.
6. **Render:** `switch` exhaustive atas `Part["type"]` dengan `assertNever`. Bila SDK menambah jenis
   part, `tsc` gagal. Tool dirender khusus per nama (`read`, `edit/write/apply_patch` + diff,
   `bash` + output, `todowrite`, `webfetch`, `websearch`, `task`); tool lain memakai fallback generik
   (input JSON + judul), tidak pernah disembunyikan.

## Bukti

Rekaman: `node spikes/s2-events/tools/record.mjs --model opencode/big-pickle` (model dipilih
pemilik). Workspace sementara berupa repo git, `permission: {edit: ask, bash: ask}`, dan data
opencode diisolasi lewat `XDG_DATA_HOME` sementara supaya sesi uji tidak masuk riwayat user.

| Skenario                         | Durasi   | Event |
| -------------------------------- | -------- | ----- |
| baca + lampiran file + reasoning | 9,9 dtk  | 138   |
| todo + edit + bash (izin `once`) | 21,6 dtk | 124   |
| bash ditolak (izin `reject`)     | 5,7 dtk  | 51    |

Part yang terekam: `text`, `file`, `reasoning`, `tool` (`read`, `todowrite`, `edit`, `bash`
completed + `bash` error), `patch`, `step-start`, `step-finish`. Ada 3 izin (edit once, bash once,
bash reject).

`bun test` → **12/12 lulus**, `tsc --noEmit` bersih:

- **Hasil reduce semua event == `GET /session/:id/message`** (info dan setiap part, deep-equal) —
  pemetaan tidak kehilangan apa pun.
- Teks hasil delta == teks final untuk setiap part `text`/`reasoning` (≥ 4 part dicek).
- Tidak ada event yang tidak tertangani. Setiap jenis event di skema SDK sudah diklasifikasikan.
- Izin terhubung ke tool call yang benar; reject menghasilkan tool `error` + pesan penolakan.
- Render di tengah streaming: blok thinking terbuka dan parsial.
- Snapshot HTML transcript penuh (`test/__snapshots__`) + `fixtures/transcript.html`.

## Temuan

1. **Lampiran file berubah menjadi part teks sintetis** (`synthetic: true`) di pesan user: server
   menyisipkan "Called the Read tool…" dan isi file. UI harus melipat part sintetis secara default.
2. Output `bash` mengalir lewat beberapa `message.part.updated` berstatus `running`
   (`state.metadata.output`), bukan lewat delta.
3. Part `patch` / snapshot **hanya muncul bila workspace adalah repo git.** Di folder non-git,
   edit tetap berjalan tanpa part `patch`. UI tidak boleh mengandalkan `patch` untuk menampilkan diff;
   diff utama ada di `tool.state.metadata.diff` (edit) dan `session.diff`.
4. Ada event yang tidak ada di skema: `server.heartbeat`. Reducer harus toleran terhadap event
   tak dikenal (dicatat, tidak melempar).
5. `session.status` sering berulang `busy`; `session.idle` adalah sinyal selesai yang andal.
6. Config global user ikut termuat (46 event `plugin.added`). Isolasi config → ADR-019 poin 5 / S4.

## Belum terbukti (jujur)

- Jenis part **`subtask`, `agent`, `retry`, `compaction`, `snapshot`** serta event
  **`question.*`, `session.error`, `session.compacted`, `message.removed`, `message.part.removed`**
  ditangani dan lolos typecheck, tetapi **belum muncul di rekaman nyata**. Perlu skenario tambahan
  (sub-agent, error provider, kompaksi, revert) sebelum Fase 5 selesai.
- Tool `webfetch`, `websearch`, `task`, `write`, `apply_patch` belum terekam.
- Hanya satu model (big-pickle). Model lain bisa berbeda pola reasoning-nya (mis. reasoning
  terenkripsi/kosong). Belum diuji.
- Belum diuji `/global/event` (stream lintas project dengan pembungkus `payload`) untuk
  multi-project; spike ini memakai `/event` per directory. Di S1 baru dicek event pertama.
- Reconnect SSE (putus/sambung, event yang hilang) belum diuji. Rencana: setelah reconnect,
  sinkron ulang dengan `GET /session/:id/message`. Test "reduce == server" membuktikan bentuk
  datanya kompatibel.
- Reducer spike memakai mutasi; produk butuh store immutable/selector untuk React + virtualisasi.

## Konsekuensi

- `packages/core` (Fase 1/5) memakai pola reducer ini dan fixture rekaman sebagai test kontrak.
- Upgrade opencode wajib: rekam ulang fixture, jalankan test kelengkapan, perbarui SDK.
- Fase 5: kartu izin memakai `metadata.diff`; part sintetis dilipat; diff tidak bergantung pada git.

## Cara mengulang

```
cd spikes/s2-events
bun install
bun test && bun x tsc --noEmit
node tools/record.mjs --model opencode/big-pickle   # rekam ulang (memakai provider user)
```
