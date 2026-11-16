# ADR-067 — Profil pengguna dengan avatar DiceBear lokal

Tanggal: 16-11-2026 · Status: diterima

## Konteks

Pemilik ingin pengaturan profil: nama dan avatar. Avatar bisa diunggah sendiri atau dipilih dari ilustrasi yang
disediakan. Sebelumnya nama selalu diambil dari akun OS dan avatar hanya inisial. Skema `SETTINGS_SCHEMA` hanya mendukung
nilai enum/angka/boolean, jadi teks bebas dan gambar tidak cocok di sana.

Penyedia ilustrasi yang dipertimbangkan: DiceBear (library MIT, 30+ gaya, bisa lokal atau API HTTP), Boring Avatars (MIT,
pola abstrak, bukan karakter), Multiavatar (lisensi khusus berbatas), aset Open Peeps manual (CC0, variasi terbatas).
Pemilik memilih **DiceBear lokal**.

## Keputusan

- Profil disimpan sebagai `Appearance.profile` (`name?`, `avatar?`) di `AppearanceStore`, yang sudah menangani validasi
  dan data URL gambar. `resetAll()` tampilan **mempertahankan** profil.
- Avatar: `{ kind: "image", src }` (data URL raster ≤ 200 KB) atau `{ kind: "illustration", style, seed }`. Ilustrasi
  dirender **di perangkat** dengan `@dicebear/core` saat ditampilkan (di-cache), sehingga tidak ada nama/seed yang dikirim
  ke server dan avatar tetap ada tanpa internet.
- Hanya paket gaya yang dipakai yang dipasang (bukan `@dicebear/collection`): 5 gaya CC0 + 3 gaya CC BY 4.0. Gaya
  berlisensi "Free for personal and commercial use" (Avataaars, Bottts) tidak dipakai karena tidak berlisensi standar.
  Paket CC BY 4.0 tercatat di `licenses-allow.json`; atribusi tampil di halaman Profil dari metadata paket.
- SVG ilustrasi ditampilkan lewat `<img>` (skrip SVG tidak berjalan); CSP `img-src data:` sudah mengizinkannya.
- UI: kategori **Profil** paling atas di Pengaturan, pakai atom yang ada (`Avatar`, `Field`, `Input`, `Tabs` berikon,
  `Select`, `Button`); ikon baru `UserIcon` dan `ShuffleIcon`.

## Akibat

- Nama efektif = nama profil → nama akun OS → "Pengguna" (`useProfile`), dipakai footer sidebar dan sapaan.
- `@dicebear/*` versi dikunci sama (9.4.2): `@dicebear/core` 10 tidak kompatibel dengan paket gaya 9.
- Belum: pemotong gambar dan avatar di gelembung pesan pengguna.
