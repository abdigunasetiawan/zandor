# ADR-030 — App shell: jendela frameless, perencana panel, perintah, dan pengaturan berskema

- **Tanggal:** 13-11-2026
- **Status:** Diterima
- **Terkait:** ADR-015/025 (lima panel), ADR-024 (Ctrl+C), ADR-029 (design system), PRD §4.7

## Keputusan

1. **Jendela frameless** (`decorations: false`, `shadow: true`) dengan kontrol jendela kustom
   (minimalkan/maksimalkan-pulihkan/tutup) di top bar. Izin Tauri minimal, hanya untuk webview UI:
   `core:window:allow-{minimize,toggle-maximize,maximize,unmaximize,close,is-maximized,start-dragging}`.
   Area seret memakai `data-tauri-drag-region`. Di luar Tauri (browser/e2e) kontrol jendela **tidak
   ditampilkan** (tidak ada tombol palsu).
2. **Auto-collapse = fungsi murni** (`desiredCollapsed` di `@zandor/core`). Set panel yang dilipat dihitung
   ulang dari nol dari (lebar tersedia, minimum dalam rem × ukuran root, pilihan pengguna). Prioritas lipat:
   Dokumen → Sidebar → Tools; Chat tidak bisa dilipat; panel yang baru dibuka pengguna dilindungi. Panel yang
   dilipat otomatis terbuka lagi sendiri saat ruang cukup; yang dilipat pengguna tidak pernah dibuka otomatis.
   UI menerapkan hasilnya setelah setiap perubahan ukuran grup **dan** setiap perubahan tata letak (idempoten,
   dengan batas 6 penerapan-ulang berturut-turut agar tidak berputar bila pustaka menolak).
3. **Sumber tunggal perintah:** `CommandRegistry` (`@zandor/core`) memberi makan menu, command palette, dan
   pintasan keyboard. Palette memakai pencarian fuzzy (`searchCommands`) dan komponen `Palette` (design system).
4. **Pengaturan berskema:** `SETTINGS_SCHEMA` (JSON Schema 2020-12, `additionalProperties: false`) adalah satu-satunya
   definisi. UI Pengaturan dirender dari skema; `SettingsStore` memvalidasi tiap tulis/baca dan menyimpan hanya
   nilai eksplisit; `docs/manual/settings.schema.json` dan `settings.md` dihasilkan oleh
   `bun scripts/gen-settings-docs.ts` dan dijaga test agar tidak basi.
5. **Navigasi in-app:** `NavigationHistory` untuk tombol back/next (rute: beranda, Pengaturan, design system),
   disinkronkan ke hash URL tanpa menambah riwayat browser.
6. **Menu hanya berisi aksi nyata.** Berkas (Pengaturan, Tutup jendela di Tauri), Tampilan (panel, zoom, tema,
   bahasa), Bantuan (palette, Tentang), Dev Tools (design system). Menu **Edit ditunda** sampai ada fitur yang
   bisa diedit (Fase 5+), bukan diisi item palsu. Item panel hanya muncul saat tata letak panel terpasang.
7. **Tata letak tersimpan** (ukuran + panel yang dilipat pengguna) bila pengaturan `rememberLayout` aktif.
8. Pintasan baru: `Ctrl+K`/`Ctrl+Shift+P` palette, `Ctrl+,` Pengaturan, `Alt+←/→` back/next, `Ctrl+B`/`Ctrl+Alt+B`/`Ctrl+J`
   panel, `Ctrl +/−/0` zoom (juga `Ctrl+Shift++`), `Ctrl+Alt+T` tema.

## Bukti

- `bun run check`: 264 test unit/komponen (perencana, perintah+fuzzy, skema+store+validator, navigasi, Palette,
  katalog=demo) + lint + typecheck + audit lisensi + anti-hardcode.
- **e2e Edge: 26 skenario** — lima panel dan empty state, seret separator + persistensi, auto-collapse saat jendela
  1000 px dan 150% zoom (urutan benar, chat ≥ minimum, panel terbuka lagi saat melebar), toggle panel + persistensi,
  rel alat, palette, menu, Tentang, Pengaturan (bahasa/tema/skala/ingat tata letak/reset), reset tata letak.
- **Smoke native 8/8** (`apps/desktop/e2e-native/smoke.ts`): aplikasi Tauri sungguhan + Playwright via CDP:
  IPC tersedia, tombol kustom memaksimalkan/memulihkan/meminimalkan/menutup jendela Windows asli, aplikasi keluar bersih.
- Tangkapan jendela nyata (`PrintWindow`) menunjukkan tidak ada title bar sistem.

## Temuan

- **Pustaka panel melipat panel sendiri** saat jendela menyempit dan itu _tidak_ mengubah ukuran grup, jadi
  `ResizeObserver` tidak terpicu: hasil awal melipat Sidebar padahal hanya Dokumen yang perlu. Perbaikan: terapkan
  ulang rencana pada setiap perubahan tata letak (lihat keputusan 2). Ditemukan oleh e2e, bukan oleh unit test.
- `cargo build` biasa memuat `devUrl` (localhost:1420), bukan UI tertanam. Untuk menjalankan UI tertanam gunakan
  `bun run tauri build --debug --no-bundle`.
- Bit `WS_CAPTION` tetap menyala pada jendela tanpa dekorasi yang memakai bayangan; indikator yang benar adalah
  tangkapan visual, bukan flag gaya.
- Tombol back/next berlaku untuk rute aplikasi saja; belum ada riwayat sesi/dokumen (Fase 4+).

## Belum teruji / batas

- **Seret jendela lewat area `data-tauri-drag-region`, klik dua kali untuk maksimalkan, dan snap layout Windows 11**
  tidak teruji otomatis (butuh input mouse sistem nyata). Perlu dicoba manual.
- Resize jendela dari tepi pada jendela frameless (hit-test border) dicoba hanya lewat gaya jendela, bukan input nyata.
- HiDPI/monitor ganda, macOS/Linux (ditunda, ADR-027).
- Pemindahan fokus keyboard antar panel (`F6`) dan editor shortcut belum ada.
- Panel masih kosong (empty state "Segera hadir"); isi tiap panel dikerjakan Fase 4–8.
- Smoke native belum dijalankan di CI GitHub (ditambahkan sebagai langkah non-blocking dulu).
