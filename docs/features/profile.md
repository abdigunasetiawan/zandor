# Profil Pengguna (nama tampilan + avatar)

- **Status:** selesai di lokal · **ADR:** [ADR-067](../decisions/ADR-067-profil-pengguna-avatar-dicebear.md)
- Halaman: Pengaturan → **Profil** (`#/settings/profile`, alias `#/profile`; menu ⋯ footer sidebar → "Profil…")

## Spek

| Bagian        | Isi                                                                                                                                                      |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Nama tampilan | Teks bebas maks 40 karakter, tersimpan saat diketik. Kosong → nama akun OS (`host.osUser()`), lalu teks bawaan "Pengguna"                                |
| Avatar        | Tiga pilihan (tab): **Inisial** (bawaan), **Gambar** (unggah png/jpeg/webp/gif, diperkecil ke 256 px JPEG, maks ~200 KB), **Ilustrasi** (DiceBear lokal) |
| Ilustrasi     | 8 gaya: Lorelei, Notionists, Open Peeps, Pixel Art, Thumbs (CC0), Adventurer, Big Smile, Fun Emoji (CC BY 4.0). 12 variasi per gaya + "Acak lagi"        |
| Atribusi      | Judul, pembuat, dan lisensi gaya dibaca dari metadata paket DiceBear dan ditampilkan di bawah pilihan                                                    |
| Dipakai di    | Footer sidebar (avatar + nama), sapaan sesi baru di Panel Chat                                                                                           |
| Penyimpanan   | `Appearance.profile` di `AppearanceStore` (lokal). Ilustrasi disimpan sebagai `{ style, seed }`, bukan gambar. "Atur ulang tampilan" tidak menghapusnya  |

Keamanan: DiceBear dirender di perangkat (tanpa jaringan); SVG ditampilkan lewat `<img>` sehingga skrip tidak berjalan;
gambar unggahan melewati canvas (SVG ditolak); semua nilai divalidasi saat disimpan dan saat dimuat (data rusak dibuang).

## Cara test

| Lapisan                                                     | Perintah                                                                    |
| ----------------------------------------------------------- | --------------------------------------------------------------------------- |
| Unit (validasi, simpan/muat, reset tampilan menjaga profil) | `bun x vitest run packages/core/src/theme/theme.test.ts`                    |
| E2E (Edge): nama, ilustrasi, sidebar, muat ulang, kembali   | `cd apps/desktop && bun x playwright test e2e/profile.spec.ts`              |
| Manual                                                      | Unggah gambar sungguhan; cek tema gelap/terang dan avatar di footer sidebar |

## Batasan jujur

- Tidak ada pemotong (crop) gambar: gambar diperkecil utuh lalu ditampilkan `object-fit: cover` dalam lingkaran.
- Avatar belum dipakai di gelembung pesan pengguna (gelembung saat ini tanpa avatar).
- Tampilan tema gelap belum diperiksa lewat tangkapan layar.
