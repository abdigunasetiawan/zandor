# Kondisi Saat Ini

> Snapshot, selalu ditimpa. Terakhir diperbarui: 17-11-2026.

## Terbaru (17-11-2026)

- **Tur pengenalan pertama kali:** dialog `Onboarding` 4 langkah (nama tampilan, status model nyata + tombol hubungkan provider, tambah project / mulai di Chat, pintasan penting). Muncul sekali saat dibuka tanpa project (pengaturan `onboarding`, mati setelah Selesai/Lewati), bisa dibuka lagi dari palet ("Tur pengenalan"). Browser otomatis (Playwright, vitest jsdom) tidak dihadang kecuali tes memasang penanda `zandor.e2e.tour`. Tes: e2e `onboarding.spec.ts`.
- **Opt-in memori di dialog percayai folder:** sakelar "Aktifkan memori untuk project ini" mengikuti bawaan memori; override hanya disimpan bila berbeda. Dialog percayai kini `Dialog` biasa (role `dialog`, bukan `alertdialog`). Tes: e2e `memory.spec.ts` (5).
- **Engine remote lewat URL (ADR-076):** Pengaturan → Engine remote (`#/settings/engines`): URL + Basic Auth, status health jujur, Cek/Ubah/Hapus; pilihan Engine di dialog Tambah project; penanda status di baris project. Router EngineApi per direktori (`core/engine/remote.ts`), tanpa remote perilaku lama tidak berubah. HTTPS wajib kecuali loopback (tunnel SSH); CSP `connect-src` + `https: wss:`. Tes: core `remote.test.ts` (3), e2e `remote-engine.spec.ts` (engine tiruan kedua). Batasan: sandi tanpa enkripsi di registry, Panel 3 belum membuka berkas remote, belum diuji ke engine remote sungguhan/build native.
- **Model peran — model kecil:** Pengaturan → Provider dan model → **Model kecil** (opencode `small_model`, judul sesi/ringkasan), disimpan Rust di `model-roles.json`, masuk config saat engine dimulai; tombol mulai ulang engine. Tes Rust `small_model_dari_berkas_peran_divalidasi`, e2e `model-settings.spec.ts`. Model memori sengaja tidak dibuat (plugin memori tidak memanggil model sendiri); model gambar tetap preferensi.
- **Status uji penuh (17-11-2026):** vitest 899/899; e2e 209 lulus, 4 gagal: `panel-toggles` (tes masih mengharapkan 5 tombol, sejak tombol smart resize ADR-066 ada 6) dan `sidebar-menu` "kustomisasi tampilan" (tombol Simpan dialog tampilan project di luar viewport) — keduanya sudah gagal sebelum pekerjaan ini; `mvp` dan `design-system` zoom flaky (lulus bila dijalankan sendiri).
- **Pencarian global lintas sesi:** Ctrl+Shift+F / palet → dialog `GlobalSearch` (judul + isi pesan semua project dan Chat, hasil bertahap, sorotan, klik membuka sesi di pesannya). Logika `core/chat/session-search.ts` (cache per `time.updated`, konkurensi 4, batal saat kata berubah). Tes: core 4, e2e `global-search.spec.ts`. Belum: indeks di disk, build native.
- **Diff view Panel 3 (ADR-075):** tombol **Lihat diff** di Perubahan sesi (Panel 4) dan di langkah tool `edit` membuka `DiffViewer` (Monaco diff, inline saat sempit) dengan bilah per bagian: navigasi, Terima (menandai), Tolak (mengembalikan bagian + simpan, toast Urungkan), Terima/Tolak semua (konfirmasi), Tutup diff. Logika patch murni di `core/docs/patch.ts` (urai, balik, kembalikan satu hunk, geser). Tes: core `patch.test.ts` (5), komponen `DiffViewer.test.tsx` (4: terima tanpa tulis, tolak + urungkan, tolak semua, tanpa penulisan), e2e `panel4.spec.ts` (2). Biome kini mengecualikan `bundled-skills/**/*.md` (penyebab baris kosong di frontmatter skill). Belum: build native, pewarnaan TextMate di diff.
- **Teks PDF bisa dipilih dan dicari:** `PdfViewer` memasang `TextLayer` pdf.js di atas kanvas tiap halaman (CSS diadaptasi dari `pdf_viewer.css`, token desain); kolom cari di bilah PDF (tombol cari toolbar dokumen / Ctrl+F memfokuskannya), hitungan "i dari n", sorotan semua hasil + hasil aktif, Enter/Shift+Enter dan tombol ↑/↓, tidak peka huruf besar; PDF tanpa lapisan teks diberi catatan jujur. Bilah PDF membungkus di panel sempit. Tes: e2e `docs.spec.ts` 19/19 (termasuk posisi span sejajar glyph). Batasan: kata yang terpecah di dua potongan teks PDF tidak ditemukan; belum dicek di build native.

## Sebelumnya (16-11-2026)
- **Trajectory sesi (ADR-073):** alat baru di Panel 4 (ikon Rel): buku besar per giliran (pengguna, konteks, panggilan model + token/biaya, berpikir, tool call input/output/durasi, langkah, galat), ikhtisar waktu 3 jalur, pencarian, filter jenis, inspektur Input/Output/Raw, salin JSON. Unit `trajectory.test.ts` lulus. Belum: system prompt/daftar tool (tak tersedia dari engine), subcall `task`, virtualisasi.

- **Background job agent (ADR-072):** tool plugin `job_start/job_output/job_list/job_stop/job_restart` (proses anak env tersaring, izin lewat `ctx.ask` bash, deteksi port, blok `<zandor-jobs>` tiap giliran), status di `<app_data>/jobs`, perintah Tauri `jobs_list/job_log/job_control/job_forget`, alat **Background jobs** di Panel 4 (status, log, buka port di Browser, Stop/Jalankan ulang/Hapus). Tes: plugin 7, komponen 4, Rust 2, e2e shell/tools/panel4 hijau. **Engine asli teruji** (`packages/plugin/e2e/jobs.e2e.test.ts`, opencode 1.19.5 + model tiruan: izin bash diminta, server HTTP sungguhan hidup, port terdeteksi, blok `<zandor-jobs>`, cwd luar project ditolak, stop dari berkas kontrol mematikan server) 5/5; e2e engine lama 20/20. Belum: klik panel di aplikasi Tauri hasil build (jalur Rust→berkas hanya teruji unit).
- **Skill bawaan dokumen (ADR-070):** `zandor-docx/xlsx/pptx/pdf` + dipasang ke folder skill global engine saat start. Pemeriksa kini subperintah biner `Zandor office-check` (ADR-074, Rust) jadi membaca/verifikasi tanpa Python; mengedit masih butuh Python (tidak dibundel). Tes Rust 6 lulus, uji fixture nyata valid, biner rilis (subsystem windows) mengeluarkan JSON + kode keluar benar lewat bash dan PowerShell. Belum diuji dengan model nyata.

- Composer: perintah slash (`/btw`, `/new`, `/compact`, `/undo`, `/redo`, `/fork`, `/copy`, `/mode`, `/stop` + perintah engine) dan mention `@berkas` (ADR-069). Unit + e2e `composer-slash` lulus.
- Bilah aksi composer kembali membungkus saat panel sempit (aturan `flex-wrap: nowrap` membuat tombol suara menimpa pil model); suara + kirim tetap di kanan. E2E `chat`, `chat2`, `composer-*`, `draft` lulus (39/39).

## Fase

- **Fase 0 — Spike Risiko:** lulus untuk Windows (ADR-019–027).
- **Fase 2 — Design System:** lulus 13-11-2026 (CI run 43427766836).
- **Fase 1 — Fondasi Repo, Docs, dan CI:** lulus 13-11-2026 (CI GitHub Actions run 43412491664, 14m50s).
- **Fase 3 — App Shell:** lulus (CI hijau).
- **Fase 4 — Engine & Project:** lulus (CI hijau).
- **Fase 5 — Chat Inti:** lulus (CI hijau), pelengkap selesai.
- **Fase 6 — Provider & Model:** selesai dengan batasan (CI hijau).
- **Fase 7 — Panel 3 Viewer Dokumen:** selesai (CI hijau).
- **Fase 8 — Panel 4 Tools:** selesai (CI hijau).
- **Fase 9 — Memori & Skill Creator:** selesai di lokal (belum di-push); ADR-037, `docs/features/memory.md`. Opt-in di dialog Tambah project ada (17-11-2026); adapter Hindsight/Graphify ditunda.
- **Pengaturan satu halaman (15-11-2026):** Tampilan lanjutan digabung ke kategori Tampilan; Memori dan skill serta Automasi kini kategori di Pengaturan (ADR-059); rute lama jadi alias. Bug aliran event (batas 6 koneksi WebView) diperbaiki di `ChatStore` (maks 4 aliran, lihat `docs/features/tools.md`); smoke native 50/50.
- **Toggle memori/skill creator di composer (15-11-2026):** ADR-060; sesi project berbagi pengaturan project, sesi Chat per sesi; skill creator kini per project/sesi (plugin + inti).
- **Panel 3 auto-tutup (16-11-2026):** menutup tab dokumen terakhir melipat Panel 3 (`DocPanel`, hanya peralihan ada → kosong; membuka panel manual tanpa dokumen tetap menampilkan keadaan kosong). Membuka berkas memunculkannya lagi. Tes e2e `docs.spec.ts` ("tertutup otomatis").
- **HTML → live server (16-11-2026):** bilah opsi di pratinjau HTML (saklar "Izinkan skrip…", teks sandbox, tombol) dihapus; tombol **Buka di browser** pindah ke toolbar dokumen (ikon globe) dan menjalankan live server Rust (`liveserver.rs`, perintah `doc_live_server`) → Browser Panel 4 dengan auto-reload. Skema `zpreview` dan `doc_preview_register` dihapus (CSP `frame-src` dirapikan). Tes: `cargo test liveserver` (3), e2e `doc-tabs`/`docs` hijau. Belum diuji di build Tauri sungguhan.
- **Tema editor VS Code = pewarnaan TextMate (16-11-2026):** ADR-071. Tema terpasang kini memakai grammar TextMate asli (Shiki, lazy) lewat bahasa Monaco `tm-*` (`shell/docs/textmate.ts`, `CodeViewer.syncHighlighting`); `EditorTheme.tokenColors` menyimpan aturan asli; `include` di .vsix diresolve; impor berkas menerima `.vsix`. Tes: core `vscode-theme.test.ts`, e2e `docs.spec.ts` ("tema VS Code terpasang"). Belum: semantic highlighting, `.tmTheme`. e2e `appearance.spec.ts` "tipografi, font" sudah diperbarui ke FontPicker (17-11-2026, 7/7 lulus).
- **Kustomisasi Panel 3 (16-11-2026):** tombol Kustomisasi di `PanelTabBar` Dokumen (kiri tombol tutup) → popover `DocStyleControls` yang **dinamis per jenis penampil** (`groupsForViewer`): kode/mentah (tema editor, font kode, ukuran px, tinggi baris, nomor baris, minimap, ligatur, spasi), dokumen render Markdown/Word/PowerPoint/notebook (tema baca Ikut aplikasi/Kertas/Sepia/Malam, font, ukuran, tinggi baris, lebar kolom), tabel/JSON (ukuran huruf), gambar (latar kotak-kotak/terang/gelap); reset per kelompok. Semua kelompok juga di Pengaturan → **Dokumen** (`/settings/documents`). Tersimpan di `appearance.docs` (`setDoc`/`resetDocs`, token `--z-doc-*`). **Ctrl + roda mouse** di teks mentah/kode memperbesar/memperkecil ukuran huruf editor (8–40px, tersimpan), bukan skala UI. Tes: core `theme.test.ts`, e2e `docs.spec.ts` ("kustomisasi Panel 3"). HTML/PDF/media belum punya opsi (popover menjelaskan).
- **Kustomisasi Panel 2 (16-11-2026):** tombol Kustomisasi (ikon slider) di `PanelTabBar` Chat, kiri tombol tutup → popover `ChatStyleControls`: kerapatan (Rapat/Normal/Lega), jenis huruf, ketebalan, lebar kolom isi, lebar composer (× kolom), ukuran, transparansi, tinggi baris, jarak huruf, jarak antarpesan, ukuran kode, reset. Kontrol yang sama di Pengaturan → Chat (`/settings/chat`) dan Tampilan → Chat; tersimpan di `appearance.chat` (token baru `--z-chat-line/-tracking/-code-size/-composer-ratio`). Slider lebar menimpa pengaturan `chatWidth` (sempit/normal/lebar) selama diubah. Tes core + vitest desktop hijau; e2e `chat-style.spec.ts` (2, 17-11-2026): kerapatan/ketebalan/slider/font → token, dipakai pesan, bertahan setelah muat ulang, reset, sinkron dengan Pengaturan → Chat. Tema terang dan build native belum diperiksa.
- **Tombol tutup panel (15-11-2026):** Panel Chat, Dokumen, dan Tools memakai satu wrapper `PanelTabBar` (`.ptb`): baris paling atas panel, tinggi seragam (`--z-control-lg`), tampilan tab tetap seperti semula (tanpa latar atau warna baru), pemisah `--z-color-border`, tombol tutup di ujung kanan (di luar area tab yang menggulir). Sebelumnya tombol Chat melayang di tengah karena `.stabs--empty` mewarisi `flex: 1`. e2e (`shell.spec.ts`) memeriksa posisi paling atas, ujung kanan, sejajar, dan tinggi seragam.
- **Menu tautan di respons model (16-11-2026):** klik tautan → pilih browser Zandor (Panel 4) / browser eksternal / salin (`rich/ChatLink.tsx`, lihat `docs/features/chat.md`); `openTool` tidak lagi menutup Browser yang sudah aktif (`showBrowserTool`). Belum diuji di build native Tauri.
- **Pintasan panel + smart resize (16-11-2026):** `Ctrl+1..5` tampil/sembunyi panel 1–5; panel utama terakhir (Chat/Dokumen/Tools) tidak bisa ditutup (toast); smart resize `Ctrl+Alt+0` + tombol pertama di grup toggle title bar: sidebar 16rem (maks 22%), 2 panel utama 50/50, 3 panel utama Chat 40 / 30 / 30 (ADR-066, `smartLayout` di `layout-plan.ts`). Tes core + e2e `shell.spec.ts` 17/17. Belum dicoba di build native.
- **Mode izin bawaan "Otomatis, kecuali aksi penting" (16-11-2026):** ADR-068; `permissionRisk` (core, bertes) memilah aksi berisiko, kartu izin menyebut alasannya; popup mode izin menampilkan penjelasan tiap mode (`SelectItem.description`), pil mode selebar labelnya. Pengguna yang pernah memilih mode lain tetap memakai pilihannya. Belum diuji dengan engine asli/build native.
- **Profil pengguna (16-11-2026):** Pengaturan → Profil: nama tampilan + avatar (inisial / unggah / ilustrasi DiceBear lokal), dipakai footer sidebar dan sapaan (ADR-067, `docs/features/profile.md`). Menutup "belum: nama pengguna di pengaturan" dari layout shell. Tema gelap belum diperiksa.
- **Sidebar & dialog project (16-11-2026):** hanya isi Workspace yang menggulir (tombol aksi tetap), scrollbar tipis menempel tepi dan tampil saat hover dengan ruang tetap; tombol tutup semua/kembalikan folder project; ikon kanan sejajar satu kolom; tanpa lapisan putih di latar kustom project; dialog tampilan project dirapikan; halaman utama tanpa overscroll (`overscroll-behavior` di root, `contain` di Workspace); `dragDropEnabled: false` di Tauri agar seret-lepas HTML5 (urutan project/sesi) jalan di Windows.
- **Logo aplikasi (16-11-2026):** varian "Tiga Panel — Tajam" dipasang — atom `AppLogo` di header sidebar, favicon SVG, ikon Tauri (ADR-065); ikon di build native belum diperiksa.
- **Gelembung pesan pengguna (15-11-2026):** token `--z-color-bubble` (campuran `fg` 8% di atas `bg`, ikut tema kustom) agar gelembung pesan pengguna terlihat di tema gelap maupun terang; e2e memeriksa latar berbeda dari panel.
- **Penyempurnaan chat (permintaan pemilik 14-11-2026):** render kaya, form pertanyaan agent, info pemakaian composer — ADR-040 (Masuk dengan OpenWork dihapus oleh ADR-055 lalu dikembalikan sebagai kartu Sign-in + akun ganda oleh ADR-056, belum diuji ke akun OpenWork sungguhan); belum: kuota provider (engine tidak menyediakannya, lihat chat.md).
- **Fase 11 — Panel 3 lanjutan & fitur ekstra:** sebagian selesai di lokal (ADR-039, ADR-045, ADR-050): edit berkas, DOCX/XLSX/PPTX/ipynb, saran prompt, automasi + notifikasi, input suara (dikte, Web Speech), save-as pesan (teks/gambar/SVG). Engine remote (ADR-076) dan model kecil selesai 17-11-2026. Belum: generate gambar (model gambar masih preferensi), skill bawaan "agent mengonfigurasi Zandor".
- **Fase 10 — Kustomisasi:** selesai di lokal; ADR-038, `docs/features/appearance.md`. Belum: paket ikon file Open VSX.
- **Layout shell baru (permintaan pemilik 14-11-2026):** panel mengambang, sidebar bertingkat, sapaan sesi baru — ADR-041; notifikasi sudah ada (ADR-050); nama pengguna di pengaturan kini ada (ADR-067).
- **Pengaturan Provider & model (permintaan pemilik 14-11-2026):** model bawaan, provider (protokol/id/kunci/ambil model), pengelola model (visibilitas, kemampuan, grup), logo kustom + otomatis — ADR-054; belum: fitur generate gambar, deteksi kemampuan dari endpoint.
- **Status MVP:** audit PRD §13 selesai — kelima kriteria terpenuhi (`docs/features/mvp.md`); menunggu keputusan rilis `0.1.0`.

## Checklist Fase 1

- [x] Monorepo Bun workspaces: `apps/desktop`, `packages/{core,ui,i18n,plugin}`
- [x] Biome, TypeScript 7 strict, Clippy `-D warnings`, rustfmt, toolchain di-pin
- [x] Vitest + Testing Library; Playwright + Edge; `cargo test`
- [x] i18n ID/EN bertipe, error boundary per panel, logger dengan redaksi secret, nomor build `0.0.<commit>`
- [x] Audit lisensi JS (`bun run licenses`) + konfigurasi `cargo-deny`
- [x] Workflow CI GitHub Actions (Windows)
- [x] Struktur docs ramping + ADR 001–018 dipecah ke file
- [x] CI lulus di GitHub (`windows-latest`), artefak installer `zandor-windows`

## Pekerjaan terbuka / keputusan tertunda

- Tinjau ADR-028: Tauri membawa 5 crate MPL-2.0 (dikecualikan per crate agar CI lulus).
- PPTX: lisensi MPL-2.0 di `@aiden0z/pptx-renderer` (ADR-026) — diputuskan sebelum Fase 11.
- Persistensi sesi Google di browser bawaan belum konsisten (ADR-020) — diuji ulang di Fase 8.
- Lisensi Zandor sendiri belum ditentukan (paket ditandai `UNLICENSED`/private).
- Ikon aplikasi masih placeholder dari spike.

## Fase 2 — Design System: LULUS (CI hijau, run 43427766836)

## Fase 3 — App Shell: selesai di lokal (menunggu CI hijau)

- [x] Jendela frameless + kontrol kustom (terbukti di jendela asli via smoke native 8/8)
- [x] Top bar: back/next, menu, omnibox/command palette, toggle panel, Pengaturan
- [x] Lima panel: resize, minimum, collapse, auto-collapse deterministik, layout tersimpan; rel memicu Tools
- [x] Zoom `Ctrl +/-/0`
- [x] Pengaturan berbasis JSON Schema + dokumen skema yang dihasilkan
- [x] Test: 264 unit/komponen, 26 e2e (Edge), smoke native
- [ ] CI GitHub hijau untuk commit Fase 3

## Pekerjaan terbuka / keputusan tertunda

- Uji manual: seret jendela lewat top bar, klik dua kali, resize dari tepi, snap layout Windows 11 (ADR-030).
- Tinjau ADR-028: Tauri membawa 5 crate MPL-2.0 (dikecualikan per crate agar CI lulus).
- PPTX: lisensi MPL-2.0 di `@aiden0z/pptx-renderer` (ADR-026) — diputuskan sebelum Fase 11.
- Persistensi sesi Google di browser bawaan belum konsisten (ADR-020) — diuji ulang di Fase 8.
- Lisensi Zandor sendiri belum ditentukan (paket ditandai `UNLICENSED`/private).
- Ikon aplikasi masih placeholder dari spike.
- Menu Edit ditunda sampai ada fitur yang bisa diedit.

## Fase 4 — Engine & Project: selesai di lokal (menunggu CI hijau)

- [x] Supervisor engine produksi (start, health, restart terbatas, mati bersama app), engine di-pin + sha256
- [x] Plugin guard wajib + penanda diverifikasi (temuan: plugin dimuat per-instance; path `\?\` merusak URL)
- [x] Connection manager lokal + engine remote lewat URL per project (ADR-076)
- [x] Project (tambah/daftar/ganti nama/lepas), sesi (buat/ganti nama/hapus/pilih), sesi tanpa project, pencarian
- [x] Dialog "Percayai folder ini?" (16-11-2026) sebelum folder baru ditambahkan sebagai project; folder yang sudah jadi
      project tidak ditanya lagi; batal/Esc = tidak ditambahkan (e2e `projects.spec.ts`)
- [x] Tab Files: pohon file nyata, ikon Material Icon Theme; ikon UI Lucide
- [x] Terbukti end-to-end: smoke native 12/12 termasuk restart aplikasi → sesi tetap ada
- [ ] Belum: UI warna project, single-instance (DB terkunci bila dua Zandor). Remote engine ada (ADR-076)
- Catatan CI: langkah baru `bun run prepare:engine` dan `ZANDOR_REQUIRE_ENGINE=1`; belum pernah dijalankan di runner.

## Fase 5 — Chat Inti: inti selesai di lokal (belum di-push)

- [x] SSE per direktori + reducer murni + `ChatStore`, diuji dengan rekaman SSE asli
- [x] Transcript Markdown, reasoning, kartu tool call, meta, composer, pilih model, Stop
- [x] Izin bawaan `ask` (temuan: opencode bawaan tidak meminta izin) + kartu izin dengan diff
- [x] Provider kustom OpenAI-compatible lewat `providers.json` (kunci hanya via env; bukan di repo)
- [x] Terbukti nyata: smoke native 15/15 (jawaban streaming, tulis file → izin → selesai, restart → sesi tetap)
- [ ] Belum: UI provider, revert/regenerate/cabang sesi, lampiran, mode izin/reasoning, tab sesi, web search, salin, virtualisasi sungguhan
- Catatan: `providers.json` milik pengguna ada di `%APPDATA%\id.web.zandor.app\` (berisi kunci; jangan di-commit). Fase chat nyata di smoke dilewati di CI (tak ada provider).

## Fase 5 (pelengkap) dan Fase 6 — Provider & Model: selesai di lokal (belum di-push)

- [x] Tab sesi, salin, mode izin, reasoning, web search, diff, lampiran, regenerate, revert, fork, virtualisasi transcript
- [x] Kunci API di keychain OS; `providers.json` v2 tanpa kunci; migrasi otomatis (terbukti pada mesin pengembang)
- [x] Endpoint kustom, lokal (deteksi Ollama/LM Studio), provider katalog (kunci), sign-in OAuth lewat engine
- [x] Probe/ping dari Rust dengan status jujur dan batas waktu; pemilih model penuh (cari, sematkan, uji)
- [x] Tes: 401 unit, 60 e2e, 29 Rust, smoke native 16/16
- [ ] Belum terbukti: chat nyata setelah migrasi keychain (upstream proxy Antigravity sedang HTTP 500), model lokal sungguhan, login OAuth sungguhan
- [ ] Belum: provider katalog yang butuh data tambahan, macOS/Linux
- Catatan: kunci Anda sekarang di Windows Credential Manager (`id.web.zandor`); `providers.json` tidak lagi memuatnya.

## Fase 7 — Panel 3 Viewer Dokumen: selesai di lokal (belum di-push)

- [x] Tab dokumen, path, toolbar lengkap (render/mentah, cari, bungkus, salin isi/path, simpan salinan, buka di aplikasi lain, muat ulang, tutup)
- [x] Monaco, Markdown + KaTeX + Mermaid, SVG/gambar, PDF (pdf.js), CSV/TSV, JSON, HTML sandbox, audio, video
- [x] Klik berkas di Files atau kartu tool di chat membuka Panel 3
- [x] Rust `fs_read` byte-persis dengan akses terbatas (traversal/absolut/root tak terdaftar ditolak; terbukti di aplikasi asli)
- [x] Tes: 439 unit, 75 e2e (15 dokumen + snapshot ARIA), 36 Rust, smoke native 22/22
- [ ] Belum: edit berkas, teks/seleksi PDF, DOCX/XLSX/PPTX/ipynb (Fase 11), pewarnaan kode di Markdown
- Catatan: `elkjs` (EPL/GPL) di-stub; Mermaid memakai dagre. `dist` ±17 MB.

## Fase 8 — Panel 4 Tools: selesai di lokal (belum di-push)

- [x] Terminal xterm + PTY engine, multi-tab per project, dipulihkan setelah muat ulang, sambung ulang
- [x] Git (status, diff, stage, commit, cabang, log) via git CLI aman di Rust
- [x] Browser child webview (http/https, bounds mengikuti panel, sembunyi saat overlay, tanpa IPC untuk konten remote)
- [x] Info sesi (token, konteks, biaya bila dilaporkan, tool call) dan Log aktivitas (ring buffer, filter)
- [x] Tes: 466 unit, 95 e2e, 45 Rust, smoke native 30/30 (terminal & git nyata, browser, isolasi IPC)
- [ ] Belum: airspace diuji otomatis, Git Bash di terminal (Git push/pull/stash/discard sudah ada, ADR-057)
- [x] Audit PRD §13: 5/5 terpenuhi; smoke native 40/40 dengan model nyata (chat, tulis file, bash, webfetch + izin; 3 project/10 sesi)
- Temuan: perintah Tauri yang membuat webview wajib `async` (jika tidak, child webview macet di about:blank).
- [x] Izin auto-approve tidak lagi berkedip: permintaan yang dijawab otomatis tidak pernah masuk state chat (tanpa kartu, tanpa geser layout, tanpa notifikasi "butuh aksi"); bila jawaban gagal, kartu muncul untuk dijawab manual (`ChatStore.maybeAutoReply`, tes di `chat-extra.test.ts`).
- [x] Toast tidak lagi menyembunyikan webview browser Panel 4 (`BrowserPanel.OVERLAY`): sebelumnya tiap notifikasi membuat halaman browser hilang 6–10 detik. Konsekuensi: toast yang beririsan dengan webview tertutup webview (notifikasi sistem dan suara tetap jalan).
- [x] DevTools (ADR-062): Inspect Element/F12 aktif di build rilis (fitur Cargo `devtools`); menu **Dev Tools** → "Buka Inspect Element" dan "Salin HTML Panel Chat" (dokumen HTML mandiri: DOM, CSS, tema, meta, dan data pesan sesi aktif termasuk langkah proses; `shell/dev-dump.ts`, bertes). Belum dicoba di build native.
- [x] Uji memori/skill end-to-end (ADR-063): 20 skenario engine asli + model tiruan (isolasi project, lintas sesi, memory_search, skill per project/sesi, /reflect); 3 cacat nyata diperbaiki (skill baru tak terlihat engine → dispose instance; indeks FTS basi; duplikat `memory_write`). Smoke native model nyata `e2e-native/memory-smoke.ts` ada dan lulus 20/20 pada satu run (Gemini 3.8 Flash High via Omni Route); run lain kadang macet karena upstream. Dispose instance terbukti tidak merusak sesi (project A tetap bisa chat, skill terbaca).
- [x] Redesain proses agent (ADR-064): langkah tool ala agent harness (`ToolStep.tsx`, `tool-view.ts`: berkas+rentang baris, +/−, diff bernomor, perintah+kode keluar, hasil grep, sub-agen), lipatan tahan remount (`open-state.ts`, `turnKey`; penyebab tertutup sendiri = key berganti saat pesan engine baru bergabung), gulir mengikuti proses. Tes: unit `tool-view.test.ts`, e2e `turn.spec.ts` (3 baru, terbukti gagal tanpa perbaikan). Belum: sesi anak sub-agen masih tampil di sidebar dan belum bisa dibuka dari langkahnya; belum dicoba di build native.
