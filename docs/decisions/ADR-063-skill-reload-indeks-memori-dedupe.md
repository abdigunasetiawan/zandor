# ADR-063 — Muat ulang skill lewat dispose instance, indeks memori turunan berkas, dedupe tulis

Tanggal: 15-11-2026 · Status: diterima

## Konteks

Tes end-to-end terhadap engine asli (model tiruan) menemukan tiga cacat nyata:

1. Skill yang baru disetujui/disimpan tidak terlihat oleh engine: opencode menyimpan hasil penemuan skill per instance.
2. Indeks pencarian FTS5 hanya ditambah (append-only): edit/hapus lewat UI membuat hasil `memory_search` basi atau ganda.
3. Model sering menulis ulang fakta yang sama lewat `memory_write`, sehingga berkas memori berisi duplikat.

## Keputusan

- `EngineApi.disposeInstance(directory)` memanggil `POST /instance/dispose`. `MemoryStore` menerima observer
  `onSkillsChanged` (setelah approve, simpan, hapus skill); shell meneruskannya ke `ChatStore.reloadInstances`, yang
  men-dispose instance yang sedang idle dan menunda yang sibuk sampai selesai. Kegagalan dispose tidak menjatuhkan aksi.
- Indeks FTS5 menjadi turunan berkas Markdown: `sync()` membangun ulang tabel bila tanda (mtime/ukuran) berubah, dipanggil
  sebelum setiap pencarian. Berkas tetap sumber kebenaran (`memoryFacts`).
- `write()` tidak menambah fakta identik (abaikan huruf besar/kecil).

## Konsekuensi

- Skill baru langsung terlihat tanpa restart engine; diverifikasi di aplikasi native dengan model nyata (`memory-smoke.ts`): sesi tetap bisa chat dan skill terbaca setelah dispose.
- Pencarian selalu mengikuti isi berkas, dengan biaya membaca dua berkas kecil per pencarian.
