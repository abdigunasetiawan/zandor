# ADR-075 — Tinjau perubahan agent di Panel 3 (diff view per bagian)

- **Tanggal:** 17-11-2026
- **Status:** diterima

## Konteks

Perubahan berkas oleh agent hanya bisa dilihat sebagai teks diff di Panel 4 (Perubahan sesi) atau di kartu tool chat.
Pemilik ingin meninjau perubahan di Panel 3 berdampingan dengan isi berkas, lalu menerima atau menolak per bagian.

## Keputusan

- Pintu masuk: tombol **Lihat diff** di daftar Perubahan sesi (Panel 4, patch snapshot sesi dari engine) dan di langkah
  tool `edit` yang sudah selesai di chat (patch metadata tool). Keduanya memanggil `useDocActions().openDiff`.
- `DocStore.openDiff/setDiff` menyimpan patch di tab (`DocTab.diff`, **tidak dipersistenkan**). `viewerFor` memilih
  `DiffViewer` untuk berkas teks yang punya diff dan tidak sedang diedit.
- `DiffViewer` = Monaco diff editor (kiri sebelum, kanan isi berkas saat ini; inline otomatis di panel sempit) + bilah
  "Bagian i dari n", sebelumnya/berikutnya, **Terima**, **Tolak**, Terima semua, Tolak semua, Tutup diff.
- Sisi "sebelum" **dihitung dengan membalik patch terhadap isi berkas saat ini** (`reverseApply` di
  `packages/core/src/docs/patch.ts`), tanpa isi karangan. Hunk dicocokkan dengan isi (dekat posisi yang diharapkan,
  lalu satu kecocokan unik di seluruh berkas); bila tidak cocok, penampil mengatakannya dan tidak menebak.
- **Terima** hanya menandai bagian selesai (berkas tidak ditulis). **Tolak** mengembalikan bagian itu dan langsung
  menyimpan berkas (`DocStore.save`, deteksi konflik tetap berlaku); toast menawarkan **Urungkan** yang menulis ulang isi
  sebelumnya dan memulihkan bagian itu. **Tolak semua** meminta konfirmasi.
- Tanpa penulisan berkas (mode web) atau isi yang tidak aman ditulis (`editBlock`), Tolak nonaktif dengan alasan.

## Batasan

- Patch snapshot sesi bisa usang bila berkas diubah di luar agent; penampil lalu menolak bagian yang tidak cocok.
- Pewarnaan memakai tema Monarch (belum TextMate seperti `CodeViewer`).
- Belum diuji di build native dengan penulisan berkas sungguhan; alur tulis diuji di tes komponen dengan sumber tiruan.
