# ADR-044 — Giliran asisten: proses ringkas, baris info, dan kartu dokumen

Tanggal: 14-11-2026. Status: diterima.

## Keputusan

- **Proses vs jawaban:** part pesan asisten dipisah: proses = berpikir + tool call (dan narasi di antaranya) sampai part proses
  terakhir; jawaban = sisanya. Logika murni di `process-label.ts` (dengan tes).
- **Belum disajikan:** tidak lagi menumpuk semua kartu. Tampil "Bekerja selama {n}d" (durasi hidup, shimmer), langkah terakhir
  (label per jenis tool: Berpikir, Menggunakan skill, Mengambil/Menyimpan memori, Mencari di internet, Membaca halaman web,
  Membaca/Mengedit/Menulis berkas, Menjalankan perintah, …) yang bisa dibuka-lipat untuk melihat semua langkah, dan baris model
  dengan logo berkedip (berhenti saat jawaban disajikan/dihentikan; hormati `prefers-reduced-motion`).
- **Sudah disajikan:** ringkasan "Proses · n langkah · durasi" (terlipat), jawaban, lalu baris info `Provider • model • durasi •
  tanggal` dan aksi (salin, cabangkan, ulangi). Aksi di gambar acuan yang belum punya fitur (salin tautan, simpan sebagai gambar,
  simpan ke memori, sematkan pesan) tidak ditampilkan.
- **Kartu dokumen:** satu baris [ikon jenis][nama][Buka][Unduh], tanpa teks tambahan. Hanya untuk dokumen yang DIBUAT agent (tool
  `write` selesai dengan ekstensi dokumen) dan HANYA di sesi tanpa project (Chat) — di sesi project berkasnya sudah ada di pohon berkas; part berkas non-gambar mengikuti aturan yang
  sama (hanya di sesi tanpa project); gambar data: URI tetap pratinjau. Buka → Panel Dokumen; Unduh → `DocHost.saveCopy` (dialog simpan di Tauri, unduhan Blob di
  web). Hook `useDocActions` (resolve/open/download) dibagi dengan `useOpenDoc`.
- Streaming hanya berlaku untuk pesan TERAKHIR percakapan saat engine sibuk (sebelumnya jawaban turn sebelumnya bisa ikut berstatus
  "bekerja" saat prompt baru dikirim).

## Belum

Unduh belum diuji di aplikasi Tauri nyata (memakai jalur `saveCopy` yang sama dengan Panel Dokumen).

## Pembaruan: satu giliran = satu tampilan

Engine asli memecah satu giliran menjadi beberapa pesan asisten berurutan (pesan pertama hanya tool call, berikutnya jawaban), sehingga baris info/aksi muncul dua kali dan dokumen yang dibuat berada di pesan yang berbeda dari jawabannya. `mergeAssistantTurns` (core, bertes) menggabungkan pesan asisten berurutan (tanpa pesan pengguna di antaranya) menjadi satu giliran: satu blok proses, satu jawaban, satu baris info/aksi; waktu mulai dari pesan pertama, selesai/model dari pesan terakhir; id sumber disimpan agar tautan dan sematan lama tetap ditemukan. Mock engine e2e kini meniru pemecahan ini.

## Koreksi aturan kartu dokumen

Aturan sebelumnya terbalik. Yang benar: kartu dokumen tampil di sesi **tanpa project** (Chat) dan disembunyikan di sesi project. Sesi demo diperlakukan sebagai sesi tanpa project agar kartunya terlihat.

## Dok izin/pertanyaan/rencana

Kartu izin, formulir pertanyaan agent, dan rencana tugas dipindah dari dalam transkrip ke dok di atas composer (`PromptDock`), selebar composer dan selalu terlihat sampai dijawab. Sesi demo memakai dok yang sama dengan state lokal.
