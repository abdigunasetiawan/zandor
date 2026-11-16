# Tampilan & Tema

Buka lewat palet perintah ("Kustomisasi tampilan") atau `#/appearance`. Mode gelap/terang/sistem tetap diatur di Pengaturan;
tema di sini berlaku untuk **kedua mode**. Perubahan langsung berlaku dan tersimpan di perangkat ini.

## Tema

- **Pakai** preset atau tema kustom. **Kembalikan semua ke default** menghapus semua kustomisasi.
- **Buat tema:** isi nama, pilih "Mulai dari" (opsional), isi warna per token (`#rrggbb`, `rgb(...)`, `hsl(...)`). Kolom
  kosong = pakai nilai bawaan. Kontras teks/latar di bawah 4.5:1 diberi peringatan.
- **Ekspor/Impor:** berkas JSON; impor ditolak seluruhnya bila ada nilai tidak valid.

Format berkas tema:

```json
{
  "id": "senja",
  "name": "Senja",
  "dark": { "bg": "#101820", "fg": "#f2e9dc", "accent": "#ff9966" },
  "light": { "bg": "#fff8f0", "fg": "#2b2118" }
}
```

Token: `bg surface raised hover active border border-strong fg fg-muted fg-subtle accent on-accent focus backdrop danger success warning shadow-color`.
`id` kebab-case (maks 40) dan tidak boleh sama dengan preset bawaan (diberi akhiran `-impor` saat impor).

## Tipografi, panel chat, section project

- **Tipografi:** ukuran dalam `rem` (0.5–4), ketebalan 100–900, tinggi baris 1–3, jarak huruf dalam `em` (−0.1–0.5).
  Font: daftar nama dipisah koma (font harus terpasang). Kosongkan kolom untuk kembali ke bawaan.
- **Panel chat:** opasitas teks 0.3–1, ketebalan, ukuran, jarak antarpesan, font.
- **Section project:** warna border, warna latar, dan logo (teks/emoji, maks 4 karakter) untuk tiap project di sidebar.

## Editor (tema kode)

Pilih "Otomatis" (mengikuti tema aplikasi), tema bawaan Monaco, atau tema VS Code yang diimpor:

- **Impor berkas** tema VS Code: `.json`/`.jsonc`, atau paket ekstensi `.vsix` (semua tema di dalamnya ikut dipasang).
- Tema VS Code mewarnai kode dengan grammar TextMate yang sama seperti VS Code, jadi warna fungsi, parameter, properti, dan
  kata kunci ditempatkan sama. Pengecualian: warna dari *semantic highlighting* VS Code tidak didukung. Tema yang
  dipasang sebelum 16-11-2026 sebaiknya diimpor ulang agar aturan warnanya lengkap.
- **Open VSX** (aplikasi desktop): cari, lalu **Pasang**. Hanya berkas tema yang dibaca dari paket; tidak ada kode yang dijalankan.
  Tema milik pihak ketiga dengan lisensinya sendiri — periksa sebelum memakai.

## Pintasan

Klik **Ubah**, tekan kombinasi baru (wajib Ctrl/Alt atau tombol F; Esc membatalkan). Bentrok di scope yang sama ditolak.
**Bawaan** mengembalikan combo asli.
