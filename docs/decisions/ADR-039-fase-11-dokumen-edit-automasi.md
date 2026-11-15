# ADR-039 — Fase 11: edit berkas, penampil Office/notebook, saran prompt, automasi terjadwal

- **Tanggal:** 14-11-2026
- **Status:** Diterima (sebagian Fase 11; butir yang belum dikerjakan tercantum di bawah)
- **Terkait:** ADR-013 (PPTX best-effort), ADR-026/035 (penampil), ADR-033 (chat), ADR-037/038

## Keputusan

1. **Edit berkas di Monaco** (hanya aplikasi desktop). Rust `fs_stat` + `fs_write`: hanya berkas yang **sudah ada** di project
   terdaftar, tulis **atomik** (berkas sementara di folder yang sama lalu ganti), batas 5 MB, berkas yang dapat dieksekusi
   dan read-only ditolak. Klien mengirim `expectedModified`; bila disk berubah sejak dibuka, host menolak dengan awalan
   `KONFLIK:` dan UI menanyakan "Timpa?" (tidak pernah menimpa diam-diam). Hanya UTF-8 valid yang bisa diedit (BOM UTF-8
   dipertahankan; UTF-16, biner, atau teks dengan U+FFFD ditolak dengan alasan). Ctrl+S menyimpan; menutup/muat ulang tab
   yang belum disimpan meminta konfirmasi. Di build web tombol Edit dinonaktifkan dengan alasan jujur.
2. **Penampil DOCX/XLSX/PPTX tanpa pustaka konversi pihak ketiga**: `fflate` (MIT) + `DOMParser`, parser sendiri
   (`packages/core/src/docs/office.ts`) yang menghasilkan **data polos** dirender sebagai elemen React (tidak ada HTML mentah,
   tidak ada skrip/makro). Alasan: pustaka populer (mis. mammoth) menarik dependensi berlisensi non-permisif (`argparse`),
   dan PPTX renderer yang dikaji ADR-026 berlisensi MPL-2.0 (keputusan itu **tidak perlu diambil** dengan pendekatan ini).
   - DOCX: heading, tebal/miring/garis bawah/coret, tautan http(s), daftar butir/bernomor, tabel (bersarang ≤ 3), gambar
     png/jpeg/gif/webp ≤ 2 MB (maks 30).
   - XLSX: sheet tampak, sel teks/angka/boolean/tanggal (deteksi format tanggal), rumus tanpa nilai tersimpan ditampilkan
     sebagai `=rumus`; maks 2000 baris × 100 kolom × 20 sheet.
   - PPTX: urutan slide, judul, teks, tabel, gambar; **tata letak tidak ditiru**.
   - Notebook (`.ipynb`): markdown (renderer yang sama dengan chat), kode, keluaran teks/stream/gambar/galat (ANSI dibuang);
     output HTML/JS **tidak** dirender (diberi label).
   - Format lama (`.doc/.xls/.ppt`) tetap "buka di aplikasi lain".
3. **Saran prompt dinamis** (`suggestPrompts`, murni): dari git nyata (jumlah berkas berubah), berkas yang sedang dibuka,
   dan isi akar project (README hilang, manifes ada); tanpa project → saran umum. Maks 4; hilang saat pengguna mengetik atau
   percakapan dimulai.
4. **Automasi terjadwal** (`AutomationStore`, `isDue/nextRun` murni): jadwal interval (5 mnt–7 hari), harian (jam + hari),
   sekali. Berjalan **hanya selama Zandor terbuka** (tick 30 dtk); tugas yang terlewat dijalankan **sekali** saat aplikasi
   dibuka ("kejar sekali"), tidak pernah mengejar kejadian sebelum tugas dibuat atau saat nonaktif. Pelaksana membuat sesi
   baru **tanpa merebut fokus**, mengirim prompt, memantau sesi, dan memberi notifikasi (mulai/selesai/butuh izin/gagal).
   **Tugas tidak pernah menyetujui izin sendiri**; sesi menunggu pengguna. Notifikasi sistem lewat `tauri-plugin-notification`
   (=2.8.1, MIT/Apache) dengan izin diminta lewat tindakan pengguna; selalu ada toast di dalam aplikasi.

## Bukti

- Unit: `office.test.ts` (20, terhadap berkas yang ditulis python-docx/openpyxl/python-pptx), `edit.test.ts`, `suggestions.test.ts`,
  `automation.test.ts` (16). Rust: `write_text` (konflik, atomik, read-only, executable, besar).
- E2E: `office.spec.ts`, `suggest.spec.ts`, `automation.spec.ts` (termasuk "Jalankan sekarang" ke mock engine).
- Smoke native di aplikasi asli: lihat `docs/features/documents.md` dan `docs/features/automation.md`.

## Belum dikerjakan (Fase 11)

- **Input suara**: WebView2 tidak menyediakan pengenalan suara tanpa layanan eksternal; butuh model STT (terkait "model
  peran"). Sengaja tidak ada tombol palsu.
- **Remote engine** (VPS/komputer lain) beserta UI: butuh perubahan supervisor, token, dan lokasi memori/skill (kini lokal).
- **Model peran** (memori, image generator).
- **Manual "agent mengonfigurasi Zandor"**: pengaturan UI tersimpan di WebView, belum berupa berkas yang bisa diedit agent.
- **Save as** tambahan (gambar/svg/md/txt dari pesan): awalnya hanya "Simpan salinan" berkas; kini sebagian ada lewat aksi pesan (ADR-045).
- Automasi latar belakang saat aplikasi tertutup (butuh layanan OS) — sengaja di luar cakupan; dinyatakan di UI.
- Edit untuk DOCX/XLSX/PPTX/notebook (hanya baca; notebook bisa diedit sebagai JSON di tampilan mentah).
- PPTX: tanpa tata letak, animasi, catatan pembicara; XLSX: tanpa format angka kustom selain tanggal, tanpa gambar/grafik.
