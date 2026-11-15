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

## Dua jenis sesi (ADR-046)

|                            | Sesi **dalam project**                            | Sesi **tanpa project** ("Chat")                                                                      |
| -------------------------- | ------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Folder kerja               | folder project (dibagi semua sesi project itu)    | **folder sendiri per sesi**: `chats/chat-YYYYMMDD-HHMMSS-xxxxxx/`                                    |
| Pohon berkas / Git         | ya                                                | tidak                                                                                                |
| Kartu dokumen buatan agent | tidak ditampilkan (berkasnya ada di pohon berkas) | ditampilkan (Buka + Unduh)                                                                           |
| Terminal                   | di folder project                                 | di folder sesi itu                                                                                   |
| Hapus sesi                 | sesi dihapus; folder project tidak disentuh       | sesi dihapus; foldernya ikut dihapus **hanya bila kosong** (berkas hasil kerja tidak pernah dihapus) |

Sesi Chat lama (dibuat sebelum ADR-046) yang berbagi folder `chats/` tetap tampil dan tetap berfungsi. Daftar sesi Chat
mengumpulkan sesi dari folder `chats/` dan semua folder sesi di bawahnya. Tooltip baris sesi menampilkan folder kerjanya.
Diurutkan terbaru dulu. Judul bawaan engine tampil sebagai "Sesi tanpa judul". Pilihan terakhir diingat.

- **Pencarian** menyaring judul sesi di semua grup.
- **Footer:** status engine (titik + teks + versi), tombol "Coba lagi" saat error, versi aplikasi.
- **Tab Files:** pohon berkas project terpilih, dimuat malas per folder; berkas yang di-ignore diredupkan; ikon
  tipe file dari Material Icon Theme.
- **Panel Chat** menampilkan sesi terpilih (percakapan hadir di Fase 5).
- **Persistensi:** daftar project di `projects.json` (atomik); sesi di database engine (`app_data/engine`).

## Batasan (jujur)

- UI gaya/warna project ada (ADR-051). Remote engine belum (Fase 11). Single-instance: instans kedua langsung keluar dan memunculkan jendela yang sudah berjalan (`tauri-plugin-single-instance`, ADR-057); belum diuji di aplikasi terpaket. Hanya Windows.

## Cara test

- Otomatis: `bun run test` (core: path, registry rusak, dedupe, respons basi, galat engine), `bun x playwright test`
  (`projects.spec.ts`), `cargo test --workspace` (+ `ZANDOR_REQUIRE_ENGINE=1`), dan
  `bun run tauri build --debug --no-bundle --ci` lalu `bun e2e-native/smoke.ts ../../target/debug/zandor-desktop.exe`.
- Manual: tambah folder sungguhan → buat sesi → tutup dan buka aplikasi → sesi masih ada; coba folder berisi spasi.

## Tab Files (pohon berkas)

Header: nama project (menu **Ganti project**), **berkas baru**, **folder baru**, **tutup semua**, **buka semua** (maks 150 folder, folder yang diabaikan git dilewati), **muat ulang**. Di bawahnya kolom cari (menyaring daftar yang termuat; saat mulai mengetik semua folder dibuka bertahap) lalu pohon dalam kartu; garis panduan vertikal tampak saat hover. Klik berkas membukanya di Panel 3. Berkas/folder baru dibuat lewat perintah Rust `fs_create` (hanya di dalam project terdaftar, nama satu segmen, tidak pernah menimpa, ekstensi eksekusi ditolak); di build web tombolnya dinonaktifkan. Tes: `e2e/files-tree.spec.ts`, `files::tests::create_entry_*`.

## Menu, urutan, dan gaya project (ADR-051)

Menu ⋯/klik kanan: sematkan, ganti nama, salin ID, hapus (sesi); sesi baru, sematkan, ganti nama, kustomisasi tampilan, muat ulang, File Explorer, lepas (project). Tahan lalu seret untuk mengurutkan sesi dan project. Kustomisasi: logo teks/gambar, latar warna/gradien (generator + 12 templat)/gambar unggahan, border. Tes: `sidebar-prefs.test.ts`, `gradient.test.ts`, `e2e/sidebar-menu.spec.ts`.
