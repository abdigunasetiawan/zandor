# ADR-062 — Inspect Element di build rilis dan "Salin HTML Panel Chat"

Tanggal: 15-11-2026 · Status: diterima

## Masalah

Masalah render di Panel 2 (Chat) sulit direproduksi: isinya dibangun dari data sesi (pesan, langkah proses agent, tool call)
dan hanya sebagian yang ada di DOM (langkah proses terlipat, transkrip panjang divirtualisasi). Build rilis juga tidak punya
Inspect Element, jadi pengguna tidak bisa memeriksa elemen di salinan yang dipakai sehari-hari.

## Keputusan

- **Inspect Element/DevTools aktif di build rilis**: fitur Cargo `devtools` pada `tauri` (tanpa crate baru). F12 dan klik
  kanan → Inspect bekerja seperti di build dev; menu **Dev Tools → Buka Inspect Element** (perintah `open_devtools`,
  hanya aplikasi desktop) membukanya dari UI. Perintah itu hanya bisa dipanggil webview UI lokal (child webview remote tidak
  punya IPC, ADR-020).
- **Dev Tools → Salin HTML Panel Chat** (`dev.copyChatHtml`, juga di palet perintah): menyalin ke clipboard satu dokumen HTML
  mandiri: DOM Panel Chat apa adanya, semua CSS aplikasi (inline), atribut tema/skala, blok `zandor-dump-meta` (waktu, versi,
  sesi, ukuran) dan `zandor-dump-data` (pesan mentah sesi aktif, termasuk langkah proses yang tidak ada di DOM). Dokumen
  dapat dibuka di browser untuk melihat render yang sama (font bisa berbeda). Logika di `shell/dev-dump.ts` (bertes).

## Konsekuensi

- DevTools di rilis memperlihatkan konsol dan DOM aplikasi kepada siapa pun yang memegang komputer itu; sama dengan
  menjalankan aplikasi di mesinnya sendiri, dan tidak menambah permukaan jaringan.
- Dump memuat isi percakapan pengguna: hanya disalin atas aksi pengguna ke clipboard, tidak dikirim ke mana pun. Periksa
  isinya sebelum membagikannya (bisa memuat data pribadi atau rahasia yang pernah muncul di chat).
- Pesan demo bukan data engine, jadi untuk sesi demo hanya DOM yang disalin (tanpa `zandor-dump-data`).
