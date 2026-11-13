# Fitur: Chat inti (Panel 2)

- **Status:** selesai (inti ADR-033; pelengkap ADR-034).
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
- **Provider:** lihat `providers.md` (keychain; halaman Provider & model).

## Pelengkap (ADR-034)

Tab sesi (maks 12, tersimpan), Salin pesan, mode izin (tanya / setujui edit otomatis / setujui semua — sisi klien),
varian reasoning (dari `model.variants`), lampiran berkas/folder, Ulangi jawaban (revert + kirim ulang), Kembalikan ke
sebelum pesan ini (revert) dan Batalkan revert, Cabangkan dari sini (fork), kartu web search, diff untuk edit/izin, dan
transcript tervirtualisasi untuk sesi ≥ 40 pesan. Provider ditambahkan lewat halaman Provider (lihat `providers.md`).

## Terbatas

Mode izin otomatis hanya bekerja selama Zandor terbuka. Lampiran dari browser dibatasi 5 MB (data URL); lampiran
native memakai URL `file://`. Output tool dipotong 20.000 karakter di UI.

## Cara test

- Otomatis: `bun run test`, `bun x playwright test` (`chat.spec.ts`), `cargo test --workspace`, dan smoke native
  (`bun e2e-native/smoke.ts ../../target/debug/zandor-desktop.exe`; fase chat nyata hanya jalan bila ada provider).
- Manual: tambah project, buat sesi, minta "buat file x.txt berisi halo" → harus muncul kartu izin dan file baru
  muncul **setelah** Izinkan sekali; coba Hentikan saat jawaban panjang; matikan engine lalu hidupkan lagi.
