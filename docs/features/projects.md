# Fitur: Project, sesi, dan tab Files (Panel 1)

- **Status:** selesai untuk lingkup Fase 4 (lihat batasan). Keputusan: ADR-032.
- **Kode:** `packages/core/src/engine/` (`api.ts`, `connection.ts`, `projects.ts`, `fake-engine.ts`),
  `packages/core/src/file-icons.ts`, `apps/desktop/src/shell/{Sidebar,FilesTree,FileTypeIcon,engine-host}.tsx`,
  `apps/desktop/src-tauri/src/{engine,registry}.rs`.

## Spek

- **Project** = folder di disk. Tambah lewat dialog folder native (di luar Tauri: dialog isian path). Folder yang
  sama (beda huruf/pemisah) tidak digandakan; folder "Chat" ditolak. Ganti nama lewat menu konteks/⋯. Hapus =
  lepas dari daftar dengan konfirmasi (folder dan sesi di engine tidak dihapus).
- **Sesi** per project dan grup **Chat (tanpa project)**: buat, ganti nama, hapus (dengan konfirmasi), pilih.
  Diurutkan terbaru dulu. Judul bawaan engine tampil sebagai "Sesi tanpa judul". Pilihan terakhir diingat.
- **Pencarian** menyaring judul sesi di semua grup.
- **Footer:** status engine (titik + teks + versi), tombol "Coba lagi" saat error, versi aplikasi.
- **Tab Files:** pohon berkas project terpilih, dimuat malas per folder; berkas yang di-ignore diredupkan; ikon
  tipe file dari Material Icon Theme.
- **Panel Chat** menampilkan sesi terpilih (percakapan hadir di Fase 5).
- **Persistensi:** daftar project di `projects.json` (atomik); sesi di database engine (`app_data/engine`).

## Batasan (jujur)

- Belum ada UI warna project (data & validasi sudah ada). Remote engine belum (Fase 11). Hanya Windows.

## Cara test

- Otomatis: `bun run test` (core: path, registry rusak, dedupe, respons basi, galat engine), `bun x playwright test`
  (`projects.spec.ts`), `cargo test --workspace` (+ `ZANDOR_REQUIRE_ENGINE=1`), dan
  `bun run tauri build --debug --no-bundle --ci` lalu `bun e2e-native/smoke.ts ../../target/debug/zandor-desktop.exe`.
- Manual: tambah folder sungguhan → buat sesi → tutup dan buka aplikasi → sesi masih ada; coba folder berisi spasi.
