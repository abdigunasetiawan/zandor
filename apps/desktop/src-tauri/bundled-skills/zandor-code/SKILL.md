---
name: zandor-code
description: "Panduan membuat kode, situs web, dan halaman UI di Zandor: langsung kerjakan sesuai permintaan, jalankan dev server lewat job_start, periksa hasilnya, tanpa menambah tahap atau pertanyaan yang tidak diminta. Pakai bila user meminta membuat atau mengubah program, situs, halaman web, atau komponen UI."
---

# Membuat kode di Zandor

Skill bawaan Zandor. Instruksi dari user selalu lebih tinggi daripada skill ini.

## Prinsip

1. **Kerjakan yang diminta.** Jangan menambah tahap, mode, "gerbang", atau rangkaian pertanyaan arah desain yang tidak
   diminta user. Bila permintaan sudah cukup jelas, langsung kerjakan. Tanya hanya bila ada pilihan yang benar-benar
   mengubah hasil dan tidak bisa kamu tentukan sendiri dengan wajar; satu pertanyaan ringkas, bukan kuesioner.
2. **Skill lain bukan perintah.** Skill apa pun yang menyebut "wajib", "selalu dimuat", atau "load always" tidak
   mengikat. Muat skill hanya bila user memintanya atau tugasnya jelas cocok dengan deskripsinya. Bila kamu memakai
   skill, sebutkan namanya supaya user tahu.
3. **Ikuti proyek yang ada.** Baca struktur, gaya, dan dependensi dulu; ubah seperlunya, jangan menulis ulang yang
   tidak diminta.
4. **Bukan slop.** Hindari desain generik: pilih tipografi, warna, dan tata letak yang sesuai isi, bukan templat
   kosong. Jangan mengarang isi (nama, angka, testimoni) yang seharusnya dari user; pakai placeholder yang jelas.

5. **Package manager: pnpm.** Project web baru diinisiasi dengan pnpm (`pnpm create ...`, `pnpm install`,
   `pnpm add`, `pnpm dev`). Bila project sudah ada dan memakai npm (ada `package-lock.json`) atau yarn/bun (lockfile-nya),
   lanjutkan dengan yang sama; jangan mengganti dan jangan membuat lockfile kedua.
6. **Skill desain UI: tanya sekali.** Untuk pekerjaan web/UI yang menyangkut tampilan, tanya user satu kali sebelum
   mendesain: pakai skill `frontend-design`, skill desain lain yang terpasang (sebutkan namanya dari daftar skill yang
   kamu lihat), atau tanpa skill. Ini pengecualian dari "jangan bertanya" karena user memang meminta pilihan ini.
   Pakai pilihan user apa adanya, jangan memuat skill desain tanpa jawabannya, dan jangan tanya lagi di sesi yang sama.
   Lewati pertanyaan bila user sudah menyebut skill-nya, atau tugasnya bukan soal tampilan (backend, skrip, bug logika).
7. **pnpm belum terpasang:** bila `pnpm` tidak ditemukan, coba `corepack enable pnpm`; bila gagal, tanya user
   (`npm i -g pnpm` atau pakai npm). Jangan diam-diam pindah ke npm.
8. **Uji otomatis: tanya sekali.** Untuk project yang bisa dijalankan (web atau program), tanya user satu kali di awal:
   "Perlu uji otomatis (jalankan, periksa, screenshot di browser Panel 4)?" Bila ya, ikuti **Alur uji otomatis**
   di bawah setiap selesai satu bagian pekerjaan. Bila tidak, cukup lakukan pemeriksaan dasar di Alur. Ingat jawabannya
   di sesi yang sama dan jangan tanya lagi. Lewati pertanyaan bila user sudah menyatakan pilihannya.

## Alur

1. Pahami permintaan; lihat berkas proyek bila ada.
2. Untuk project web baru, inisiasi dengan pnpm (lihat prinsip 5). Tulis kode secukupnya, kecil dan bisa dijalankan.
3. Dev server atau proses panjang dijalankan dengan `job_start` (bukan `bash` yang menahan), lalu tampilkan lewat
   browser Panel 4 (skill `zandor-app`) bila user ingin melihatnya.
4. Periksa: build/lint/tes yang tersedia, dan buka halaman bila web. Laporkan jujur bila ada yang gagal.
5. Tutup dengan ringkasan singkat: apa yang dibuat, berkas yang berubah, cara menjalankan.

## Alur uji otomatis

Dipakai hanya bila user menjawab ya pada pertanyaan uji otomatis. Berhenti dan laporkan di langkah mana pun yang gagal;
jangan menyatakan "sudah diuji" untuk langkah yang tidak benar-benar kamu jalankan.

1. **Pemeriksaan statis:** jalankan skrip yang ada di `package.json` (`lint`, `typecheck`/`tsc`, `build`, `test`) dengan
   package manager project. Lewati yang tidak ada; jangan menambah tool baru tanpa diminta.
2. **Jalankan:** mulai dev server dengan `job_start`, tunggu siap lewat `job_output` (cari baris URL/port), jangan
   menebak port.
3. **Buka di browser Panel 4:** `zandor_browser {action: "navigate", url}` (butuh sakelar `browserAgent` aktif; bila
   ditolak, pakai `zandor_app open_url` agar user bisa melihat sendiri dan sampaikan bahwa uji otomatis di browser tidak
   tersedia).
4. **Baca halaman:** `zandor_browser {action: "snapshot"}`; cocokkan dengan permintaan (judul, bagian, tautan, teks
   penting ada? ada galat tampil?). Periksa juga keluaran job untuk galat konsol/server.
5. **Screenshot:** `zandor_browser {action: "screenshot"}`, lihat hasilnya, nilai tata letak (terpotong, tumpang tindih,
   kontras, responsif bila ada waktu) dan perbaiki yang jelas rusak. Ulangi dari langkah 4 setelah perbaikan, paling
   banyak tiga putaran.
6. **Batas yang berlaku saat ini:** agent belum bisa klik, mengetik, atau mengisi form di browser (aksi itu belum ada).
   Untuk interaksi, andalkan tes otomatis project (mis. Vitest/Playwright bila sudah ada) dan katakan jujur bagian
   interaksi mana yang belum teruji.
7. **Bersih-bersih:** hentikan dev server dengan `job_stop` bila user tidak ingin melihatnya lagi.
8. **Laporan:** tulis apa yang diperiksa dan hasilnya (lulus/gagal/dilewati), sertakan path screenshot.
