# ADR-053 — Lima tombol panel, Chat bisa disembunyikan, pegangan resize berbentuk pil

Tanggal: 14-11-2026 · Status: diterima (mengubah sebagian ADR-015: Chat tidak lagi wajib terlihat)

## Keputusan

- Top bar punya **lima** tombol tampil/sembunyi dalam satu wadah berlatar (`--z-color-segment`); yang aktif berlatar
  `--z-color-segment-active` + bayangan: Sidebar, Chat, Dokumen, Tools, dan Rel alat (P5).
- **Chat boleh disembunyikan** (prioritas lipat otomatis tertinggi, jadi dilipat paling akhir). Rel alat bukan panel resizable:
  hanya tampil/sembunyi (pilihan pengguna, ikut "ingat tata letak"; "Reset tata letak" menampilkannya lagi).
- **Jarak seragam**: tepi jendela, antarpanel, dan ke rel semuanya `--z-separator-size`.
- **Separator**: tanpa batang gelap; saat disorot/diseret/fokus muncul **pil** (ikon seret) di tengah celah.
- **Tanpa warna gelap di balik panel**: latar `.app` = `--z-color-bg` (sebelumnya `--z-color-frame` tampak di sudut membulat).
- Omnibox memakai border `--z-color-border-soft`.
- Perintah baru: `view.toggleChat`, `view.toggleRail` (menu Tampilan).

## Batas

- Bila semua panel disembunyikan, area utama kosong (pengguna bisa menampilkannya lagi dari top bar).
