# Fitur: Chat inti (Panel 2)

- **Status:** sebagian — inti selesai (kriteria keluar Fase 5 terpenuhi), fitur lanjutan belum. Keputusan: ADR-033.
- **Kode:** `packages/core/src/chat/` (`types.ts`, `reducer.ts`, `store.ts`), `packages/core/src/engine/api.ts`,
  `apps/desktop/src/shell/{ChatPanel,Markdown}.tsx`, `apps/desktop/src-tauri/src/providers.rs`.

## Spek (yang sudah ada)

- Transcript dari engine secara streaming (SSE): pesan pengguna, jawaban Markdown (GFM), blok "Berpikir"
  (reasoning) yang bisa dilipat, kartu **tool call** (nama, ringkasan, status, masukan/keluaran yang bisa dibuka),
  baris meta (model, durasi, token/waktu lengkap di tooltip), galat pesan/sesi, status "bekerja"/"mencoba ulang"/
  "menyambung ulang".
- **Composer:** Enter kirim, Shift+Enter baris baru, tombol Kirim/Hentikan, pemilih model, pesan jelas bila tidak ada
  model atau engine belum siap. Teks dikembalikan ke kotak bila pengiriman gagal.
- **Kartu izin** (`ask` untuk edit/shell/webfetch/di luar folder): Izinkan sekali / Selalu izinkan / Tolak, dengan
  pola path dan diff.
- **Stop** membatalkan giliran yang berjalan. Auto-scroll hanya bila pengguna berada di dekat dasar.
- **Provider:** `app_data/providers.json` (lihat bawah).

## Menambah provider (sementara, manual)

Buat `%APPDATA%\id.web.zandor.app\providers.json` (di luar repo), lalu mulai ulang Zandor:

```json
{
  "version": 1,
  "providers": [
    {
      "id": "antigravity",
      "name": "Antigravity Tools",
      "baseUrl": "http://localhost:8045/v1",
      "apiKey": "<kunci Anda>",
      "models": [{ "id": "claude-sonnet-5-5", "name": "Claude Sonnet 5.5" }]
    }
  ]
}
```

`id` huruf kecil/angka/`-`. Kunci hanya dikirim lewat environment engine, tidak masuk konfigurasi atau log.

## Belum ada

Revert, regenerate, cabang sesi, lampiran, mode izin & reasoning, tab sesi, web search, salin, virtualisasi
sungguhan, UI provider.

## Cara test

- Otomatis: `bun run test`, `bun x playwright test` (`chat.spec.ts`), `cargo test --workspace`, dan smoke native
  (`bun e2e-native/smoke.ts ../../target/debug/zandor-desktop.exe`; fase chat nyata hanya jalan bila ada provider).
- Manual: tambah project, buat sesi, minta "buat file x.txt berisi halo" → harus muncul kartu izin dan file baru
  muncul **setelah** Izinkan sekali; coba Hentikan saat jawaban panjang; matikan engine lalu hidupkan lagi.
