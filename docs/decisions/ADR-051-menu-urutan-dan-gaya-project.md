# ADR-051 — Menu sesi/project, urutan seret-lepas, dan gaya project

Tanggal: 14-11-2026 · Status: diterima

## Keputusan

- **Menu ⋯ / klik kanan sesi**: Sematkan/Lepas sematan, Ganti nama, Salin ID sesi, Hapus. **Project**: Sesi baru, Sematkan, Ganti nama,
  Kustomisasi tampilan…, Muat ulang, Tampilkan di File Explorer (desktop), Lepas dari daftar.
- **Sematan dan urutan** (`SidebarPrefs`, disimpan lokal): disematkan dulu (urutan sematan), lalu urutan buatan pengguna, lalu
  bawaan. **Tahan lalu seret** (HTML5) mengurutkan sesi (per kelompok) dan project; item tersemat tetap di kelompoknya.
- **Gaya project**: logo (teks/emoji ≤ 4 karakter atau gambar kecil), latar (warna, **gradien** dari generator — jenis, sudut,
  2–5 titik warna, acak — dengan 12 **templat**, atau **gambar unggahan** yang diperkecil lewat canvas), dan warna border.
  Semuanya divalidasi ketat (`validateGradientCss` menolak url()/var()/fungsi lain; gambar hanya data URL png/jpeg/webp/gif ≤ 0,7 MB).
- Latar dipasang **pada baris project saja** (tanpa kartu pembungkus), dengan lapisan pelindung agar teks tetap terbaca.
- Dialog gaya juga dibuka dari halaman Tampilan lanjutan > Section project.

## Batas

- Sematan/urutan/gaya tersimpan di penyimpanan lokal perangkat ini.
- Bagian "Tampilkan di File Explorer" hanya tersedia di aplikasi desktop.
