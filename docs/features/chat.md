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

## Saran prompt dinamis (Fase 11)

Di sesi kosong, composer menampilkan hingga 4 chip saran yang dihitung dari konteks nyata: jumlah berkas yang berubah di git
("Tinjau N perubahan", "Tulis pesan commit"), berkas yang sedang dibuka ("Jelaskan x", "Tulis tes untuk x"), isi akar project
("Buat README" bila README hilang, "Jalankan tes" bila ada manifes), dan saran umum bila tanpa project. Klik mengisi kotak
pesan (tidak langsung terkirim); chip hilang saat mulai mengetik. Tes: `suggestions.test.ts`, `e2e/suggest.spec.ts`.

## Render kaya, pertanyaan agent, info pemakaian (Fase 12, ADR-040)

- **Render respons:** Markdown GFM, KaTeX, blok kode berwarna (label bahasa, Salin, ciut > 30 baris), Mermaid (setelah jawaban
  selesai), tabel yang bisa digulir. HTML mentah/skrip/gambar luar tidak aktif.
- **Form pertanyaan agent** (tool `question`): kartu dengan pilihan tunggal/jamak dan jawaban sendiri; Kirim/Lewati. **Rencana
  agent** (todo) tampil di atas composer.
- **Info pemakaian** di composer: model, konteks terpakai vs batas (peringatan hampir penuh), token, biaya, rincian.
- **Batas:** kuota/limit tarif provider tidak tersedia dari engine; gambar dalam jawaban tidak dimuat; kepatuhan model pada
  tool `question` bergantung model.
- **Tes:** `bun x vitest run packages/core/src/chat packages/core/src/tools`, `bun x playwright test e2e/rich e2e/questions
  e2e/composer-info`, smoke native (render dari model nyata, info composer, form pertanyaan bila model memakai tool).
