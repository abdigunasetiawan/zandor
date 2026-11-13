# ADR-033 — Chat inti, izin default `ask`, dan provider kustom (Fase 5)

- **Tanggal:** 13-11-2026
- **Status:** Diterima
- **Terkait:** ADR-021 (engine pin), ADR-022 (env guard), ADR-023 (plugin), ADR-032 (engine & project)

## Keputusan

1. **Alur data:** UI → `ChatStore` (core) → `EngineApi` (SDK resmi v2). Prompt dikirim lewat `prompt_async`;
   hasilnya hanya mengalir lewat **SSE `GET /event?directory=`** (satu langganan per direktori, reconnect otomatis
   dengan backoff 0,5→5 dtk). Setiap (re)koneksi memuat ulang riwayat (`GET /session/:id/message`) supaya event
   yang terlewat terkejar; event pesan yang tiba **saat** riwayat sedang diambil memicu pengambilan ulang
   (maks 3×), bukan dibuang.
2. **Reducer murni** (`applyEvent`) dengan normalisasi defensif (`toEngineEvent`/`toPart`): bentuk tak dikenal →
   `null`/part `other`, tidak pernah melempar. Delta yang tiba sebelum part-nya ditahan lalu diterapkan.
   Diverifikasi terhadap **rekaman SSE asli** (`packages/core/src/chat/fixtures/real-turn.ndjson`).
3. **Izin bawaan = `ask`** untuk `edit`, `bash`, `webfetch`, `external_directory`, diset di konfigurasi engine
   (`engine.rs`). Temuan: **opencode bawaan menulis file dan menjalankan shell tanpa bertanya.** Dengan `ask`
   engine menahan tool call sampai `POST /permission/:id/reply` (`once`/`always`/`reject`); terbukti pada engine
   asli (file tidak ada sebelum izin, ada sesudahnya). Kartu izin menampilkan pola path dan diff dari engine.
4. **Provider kustom (endpoint kompatibel OpenAI)** disimpan Zandor di `app_data/providers.json` (di luar repo).
   Konfigurasi engine hanya memuat rujukan `{env:ZANDOR_SECRET_PROVIDER_<ID>}`; nilai kunci dikirim lewat
   environment proses engine dan dibersihkan plugin guard dari proses anak (ADR-022). Kunci **tidak pernah**
   masuk `OPENCODE_CONFIG_CONTENT`, log, atau repo (diuji: `config_never_contains_the_key`). Adapter npm
   `@ai-sdk/openai-compatible` diunduh engine saat pertama dipakai (butuh internet sekali).
5. **Markdown** dengan `react-markdown` + `remark-gfm` (MIT): HTML mentah tidak dirender, hanya tautan
   `http(s)`/`mailto` yang aktif (dibuka di jendela terpisah), gambar tidak dimuat.
6. **Pilihan model** disimpan di `localStorage` (`zandor.model`) sebagai kenyamanan; model yang hilang dari daftar
   kembali ke model pertama.

## Temuan lain

- Engine memberi **judul sesi otomatis** setelah percakapan; jangan mengandalkan judul bawaan "New session".
- Satu giliran bisa berupa beberapa pesan asisten (tool call, lalu jawaban) — transcript menampilkannya berurutan.
- opencode memuat plugin per-instance (ADR-032); chat memicu instance direktori project saat sesi dibuka.

## Batasan yang diketahui (belum dikerjakan)

- **Belum ada UI provider**: provider ditambahkan dengan mengedit `providers.json` lalu memulai ulang aplikasi.
- **Belum**: revert, regenerate, cabang sesi, lampiran file/folder, pilihan mode izin dan reasoning, tab sesi di
  atas Panel 2, part web search, tombol salin. Dari roadmap Fase 5 hanya inti yang selesai.
- **Transcript belum benar-benar divirtualisasi**: memakai `content-visibility: auto` per pesan. Sesi sangat
  panjang (ribuan pesan) perlu windowing sungguhan (kandidat: `@tanstack/react-virtual`).
- Output tool dipotong 20.000 karakter di UI.
- Hanya Windows yang diuji (ADR-027).

## Verifikasi

Core: rekaman SSE asli, reducer, normalisasi, `ChatStore` (reconnect, race muat-vs-event, model, izin, abort).
E2E Playwright dengan engine tiruan (`chat.spec.ts`): kirim/streaming, Enter vs Shift+Enter, tool call, izin
sekali/tolak, stop, tanpa provider, muat ulang riwayat. **Smoke native dengan engine dan provider nyata:** prompt →
jawaban streaming, tulis file → kartu izin → Izinkan sekali → tool call selesai; restart aplikasi → sesi tetap ada.
