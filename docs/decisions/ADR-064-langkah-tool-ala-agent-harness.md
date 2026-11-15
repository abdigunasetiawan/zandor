# ADR-064 — Langkah tool ala agent harness; lipatan proses tahan remount; gulir mengikuti proses

Tanggal: 16-11-2026 · Status: diterima

## Konteks

Proses agent (daftar langkah di bawah "Proses · n langkah") terasa berantakan: tiap tool berupa kotak ber-lencana "Done"
dengan tombol "Buka" di baris terpisah, tanpa informasi berkas/baris/perubahan seperti agent harness pada umumnya. Selain
itu lipatan yang sudah dibuka pengguna menutup sendiri saat langkah baru datang, dan posisi gulir tidak selalu mengikuti.

Penyebab menutup sendiri (terbukti dengan tes e2e yang gagal tanpa perbaikan): pesan asisten gabungan
(`mergeAssistantTurns`) memakai ID pesan engine TERAKHIR sebagai `info.id`; engine asli memecah satu giliran menjadi
beberapa pesan, jadi `key={info.id}` berganti tiap pesan baru bergabung, giliran dipasang ulang, dan semua `useState` lipatan
hilang.

## Keputusan

- **Tampilan langkah** (`shell/ToolStep.tsx`, logika murni `shell/tool-view.ts`, bertes dengan bentuk data asli opencode 1.19.5):
  baris = ikon jenis · nama tool · target (folder redup + nama berkas) · ringkasan (rentang baris dibaca, `+n −m`, kode keluar
  bila gagal, jumlah hasil, jenis sub-agen, durasi bila ≥ 1,5 dtk) · status (centang; galat = lencana; berjalan = indikator).
  Isi yang dibuka: diff unified bernomor baris (header `Index/===/---/+++` dibuang, nomor dari hunk), isi berkas baru, baris
  yang dibaca, perintah + keluaran, hasil grep per berkas, glob sebagai daftar berkas, tugas/hasil sub-agen, daftar rencana.
  Kelas lama (`fold--tool`, `fold__head`, `fold__tool`, `data-status`) dipertahankan agar tes dan smoke tetap berlaku.
- **Ringkasan proses** menambahkan hitungan per jenis ("12 dibaca · 3 diedit +45 −12 · 8 perintah").
- **Tertutup bawaan, tidak menutup sendiri**: state buka/tutup disimpan per kunci stabil di luar komponen (`shell/open-state.ts`:
  `proc:<id pesan engine pertama>`, `tool:<id part>`, `think:<id part>`), dan kunci React giliran = pesan engine PERTAMA
  (`turnKey`). Dua lapis: kunci stabil mencegah pemasangan ulang; penyimpanan menahan state bila tetap terpasang ulang
  (virtualisasi daftar).
- **Gulir mengikuti proses**: `useStickToBottom` mengamati DOM (MutationObserver) sehingga langkah/teks baru dan gambar yang
  selesai dimuat tetap mengikuti ke dasar selama pengguna di dasar (ambang 48 px); klik pengguna (membuka lipatan) menghitung
  ulang apakah masih di dasar, dan membaca ke atas tidak direbut.

## Sub-agen (temuan)

Engine opencode punya tool `task` yang membuat sesi anak (`parentID`) dengan agent `general`/`explore`; tool ini tidak diblokir
konfigurasi Zandor, jadi model bisa menyuruh spawn sub-agen (diverifikasi di engine asli dengan model tiruan). Tampilan langkah
`task` menampilkan jenis, tugas, dan hasilnya. **Belum**: sesi anak muncul sebagai sesi biasa di daftar sidebar, dan belum ada
cara membuka/mengikuti sesi anak dari langkahnya.

## Konsekuensi

- Tool baru di engine otomatis tampil lewat cadangan generik (masukan + keluaran) sampai diberi tampilan khusus.
- Keluaran besar dipotong per blok (300 baris) agar UI tidak membeku.
