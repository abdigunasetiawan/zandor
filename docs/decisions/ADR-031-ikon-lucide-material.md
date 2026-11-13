# ADR-031 — Ikon: Lucide untuk UI, Material Icon Theme untuk tipe file

- **Tanggal:** 13-11-2026
- **Status:** Diterima (keputusan pemilik)
- **Terkait:** ADR-010 (tema/ikon editor dari Open VSX), ADR-029 (design system)

## Keputusan

1. **Ikon UI = `lucide-react` 1.72.0 (ISC)**, di-pin. `packages/ui/src/icons.tsx` membungkus ikon Lucide dengan
   nama ekspor yang sama seperti sebelumnya (`PlusIcon`, `CloseIcon`, …), ketebalan garis seragam 1,5, ukuran
   dari token (`--z-icon-*`), dekoratif (`aria-hidden`, `focusable="false"`). Ikon baru ditambahkan dengan
   pola yang sama; ikon gambar-sendiri hanya bila tidak ada di Lucide.
2. **Ikon tipe file/folder = Material Icon Theme** (PKief). Dipakai lewat paket npm `material-icon-theme` 5.40.0
   (MIT; proyek yang sama dengan ekstensi VS Code-nya, tersedia juga di Open VSX), bukan lewat unduhan `.vsix`:
   paket menyertakan manifest (`fileExtensions`, `fileNames`, `folderNames`) dan SVG, sehingga bisa di-bundel
   dan dicari tanpa jaringan. Ini tetap sejalan dengan ADR-010 (sumber ikon: ekosistem Open VSX/Material).
   Paket ikon lain (pilihan pengguna) tetap direncanakan di Fase 10.

## Konsekuensi

- Audit lisensi menandai kedua paket sebagai permisif (ISC, MIT).
- Beberapa ikon di Material Icon Theme meniru logo merek (bahasa/framework). **Sebelum rilis publik** tinjau
  berkas NOTICE/lisensi paket dan cantumkan di Third-Party Notices (Fase 12).
- Pohon file (Fase 4) memakai ikon ini; tipe file tak dikenal jatuh ke ikon berkas generik.
