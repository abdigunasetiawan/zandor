# ADR-069 — Perintah slash dan mention berkas di composer

- Status: diterima
- Tanggal: 16-11-2026

## Konteks

Composer belum punya perintah `/` (mis. `/btw`) maupun mention `@berkas`. Engine opencode yang di-pin (SDK 1.19.5)
menyediakan `GET /find/file`, `GET /command`, `POST /session/:id/command`, dan `POST /session/:id/summarize`.

## Keputusan

- Logika murni di `packages/core/src/chat/composer-input.ts` (token di kursor, `parseSlash`, daftar bawaan, lampiran
  mention); UI di `ComposerSuggest.tsx` (popup listbox, keyboard tetap di textarea) + hook `composer-assist.tsx`.
- **Perintah bawaan aplikasi** (menang atas perintah engine bernama sama): `/btw <pertanyaan>` (fork seluruh riwayat lalu
  bertanya di cabang, boleh saat agent bekerja — sesi utama tak terganggu), `/new`, `/compact` (summarize engine),
  `/undo` (revert pesan pengguna terakhir + teksnya kembali ke composer), `/redo`, `/fork`, `/copy`, `/mode <mode>`, `/stop`.
- **Perintah engine** (bawaan opencode seperti `/init`, `.opencode/command`, prompt MCP, skill) didaftar dari `GET /command`
  dan dijalankan lewat `session.command` dengan model/varian terpilih. Perintah tak dikenal tidak dikirim (toast);
  `//teks` mengirim garis miring apa adanya.
- **Mention `@`**: cari berkas lewat engine (jeda 120 ms), pilih → teks `@jalur` + lampiran `file://` absolut (MIME
  gambar/PDF dikenali, selain itu `text/plain`) sehingga engine membaca isinya.
- ↑ di composer kosong memanggil ulang pesan pengguna terakhir.

## Konsekuensi

- Perintah engine dan `/compact` berjalan sinkron di engine; selama itu sesi berstatus mengirim (Stop tersedia).
- Mention butuh folder (project atau folder sesi Chat); di halaman bawaan tanpa project, popup menjelaskannya.
