# Kondisi Saat Ini

> Snapshot, selalu ditimpa. Terakhir diperbarui: 23-11-2026.

## Fokus: Rust GPUI (19-11-2026, ADR-078)

Pengembangan aktif pindah ke **`apps/desktop-gpui`** (branch `zandor/rewrite-gpui`, worktree `Zandor-gpui`). Tauri/React dan
`packages/*` dibekukan sebagai referensi paritas 1:1. Panduan: [`features/gpui.md`](../features/gpui.md). Bagian
"Terbaru/Sebelumnya" di bawah mendeskripsikan perilaku versi React yang menjadi acuan port.

- **Agent CLI lewat ACP (23-11-2026, branch `acp`, worktree `Zandor-acp`, belum di-commit, ADR-083):** Claude Code,
  Codex, dan Gemini (Antigravity) bisa dipakai dengan langganan pengguna lewat Agent Client Protocol (`core/acp/`).
  - UI: Pengaturan > Model > Agent CLI dan langkah provider di Onboarding (`shell/acp_section.rs`, i18n ID+EN).
  - Model muncul di pemilih sebagai provider `acp-*`, dengan logo, ping lewat agent, dan pemilih model dimuat ulang saat
    daftar berubah.
  - OpenCode tetap jalur bawaan; sesi tetap miliknya.
  - **Uji langsung:** ketiga agent lulus (berkas dibuat lewat izin, konteks giliran kedua benar).
  - **Diuji di jendela (build release, data asli):** pasang adaptor, aktifkan, pemilih model, satu sesi berganti
    Claude → Codex → Gemini dengan konteks dioper, judul sesi, dan Onboarding.
  - **Belum:** unrevert untuk pesan ACP, urutan fork sesi campuran, dan kartu izin di jendela (izin hanya teruji lewat
    tes langsung).
  - Catatan alat: worktree baru tidak punya `apps/desktop/src-tauri/binaries/opencode-*.exe` dan
    `resources/zandor-plugin.js` (diabaikan git); salin dari worktree utama atau engine akan "bermasalah".
- **Rewind (22-11-2026, belum di-commit; ketiga mode terverifikasi di app):** dialog "Rewind ke pesan ini?" (`shell/rewind_dialog.rs`, logika + 7 tes di `core/rewind.rs`) menggantikan konfirmasi revert; 3 mode (Kode dan percakapan = engine revert; Hanya percakapan = hapus pesan; Hanya kode = balikkan patch di disk, gagal jujur bila berkas berubah). Detail di `features/chat.md`. `cargo check` lulus; **belum dicoba di aplikasi/engine nyata** (asumsi `diff?messageID=` belum terbukti).
- **Simulasi perangkat di Panel 4 Browser (22-11-2026):** bilah kedua di bawah bilah alamat: pemilih templat (Responsif, Android Galaxy S24/Redmi Note 13/Reno 11, iPhone SE/15/15 Pro Max, Pixel 8/Fold, iPad Mini/Pro 11/Galaxy Tab S9, Laptop 13"/14"/MacBook Air/15", Desktop FHD/QHD, Kustom), kolom lebar × tinggi (Enter), tombol putar, dan skala (Pas/50–150%). Cara kerja: webview diubah ukurannya ke viewport × skala dan diberi zoom WebView2 = skala, jadi viewport CSS persis ukuran perangkat; UA seluler + emulasi sentuh lewat CDP (halaman dimuat ulang saat UA berubah). Data di `shell/browser_devices.rs` (2 tes). **Bingkai standar** per jenis (ponsel/tablet/laptop/desktop; bezel digambar di canvas, sudut layar dibulatkan lewat `SetWindowRgn` pada jendela anak webview); saat webview disembunyikan menu/dropdown, bingkai + tangkapan terakhir tetap tampil (tangkapan diambil saat bilah simulasi ditekan). **Batasan jujur:** DPR/piksel fisik tidak disimulasikan, tanpa notch/tombol; simulasi berlaku untuk semua tab. `cargo test --offline` 297 lulus; **belum dicoba di jendela aplikasi** (webview native tidak bisa diuji dari browser pane) dan belum build rilis.
- **Pin chat dan project (20-11-2026):** menu ⋯ sesi dan project punya "Sematkan/Lepas sematan"; item tersemat naik ke atas
  daftarnya (urutan relatif lain tetap) dan diberi ikon pin kecil. Disimpan di pref `sidebarPinned` (`p:<id>` / `s:<id>`),
  global (bukan per project). Tes `pin_tests` lulus; belum dicoba visual di build release.
- **Urutan seret-lepas sidebar (20-11-2026):** baris project dan baris sesi (di Chats dan di dalam project) bisa diseret;
  jatuh di atas baris lain memindahkan item ke posisi itu (garis fokus penanda). Hanya dalam kelompok yang sama; pin tetap
  di atas. Urutan disimpan di pref `sidebarOrder` (`projects` / id project / `CHAT_KEY`); sesi baru tampil di atas.
  Tes `pin_tests` (4) lulus. Belum dicoba manual di build berjalan; belum ada jalur keyboard (Alt+↑/↓).
- **Selesai (sesi ini):** paritas UI (sidebar, tab, popup model/permission, lingkaran konteks); performa (profil build,
  view ter-cache, daftar virtual untuk chat, model picker, tab Uji); caret halus ala monkeytype; reduced motion (menu,
  dialog, dock, cincin ping, shimmer); pengaturan chat (sendKey, pasteLongText, confirmDelete, showDemos); tiga sesi demo
  dari sumber React; nama mode izin diringkas (Tanya dulu / Auto edit / Otomatis / Akses penuh) di GPUI dan React;
  hover pilihan reasoning menyatu dengan pil model; ikon aplikasi di exe; reset demo mereset kartu pertanyaan.
- **Uji:** `cargo test --offline` 216 lulus; e2e React `chat`+`chat2` 34/34.
- **Diuji lewat skrip desktop (21-11-2026, build debug `dev.cmd`, data asli):** scroll tab Uji (843 model) lulus; kirim Ctrl+Enter
  lulus (Enter polos tidak mengirim); hapus sesi tanpa konfirmasi lulus (tanpa dialog); palet Ctrl+K (3 bagian, filter fuzzy)
  lulus; resize 760/900/1000/1300 px lulus. **Diperbaiki:** chip `Ctrl+K` di bar judul menimpa toolbar di lebar sempit
  (`titlebar.rs`: chip disembunyikan di bawah ~864 px + `overflow_hidden`; belum di-commit).
- **Placeholder composer (21-11-2026, terverifikasi di jendela, belum di-commit):** ikut bahasa dan `sendKey`
  (`chat.placeholder.ctrl`, id+en; `chat_panel.rs` `apply`): "Enter to send" / "Ctrl+Enter to send" berganti langsung.
- **Escape pada popup model (21-11-2026, terverifikasi di jendela, belum di-commit):** percobaan 1
  (`on_action(InputEscape)` pada popover) gagal; yang berhasil: `mp_search.intercept = true` + `chat_panel.rs` menutup popup
  pada `InputNav::Escape` (`close_picker` jadi `pub(super)`). Catatan: link inkremental pernah rusak (LNK1120) karena
  edit saat build; diatasi `cargo clean -p zandor-gpui`.
- **Belum terverifikasi / temuan:** kelancaran (fps) resize/popup belum diukur, seret tepi dengan mouse belum; crash
  0xc0000409 satu kali saat klik pertama (14:53, tidak terulang, penyebab tidak diketahui). Catatan alat: proses yang
  diluncurkan dari app Claude desktop memakai `%APPDATA%` terpaket (data berbeda dari sesi asli).
- **Menunggu keputusan pemilik:** push/PR `zandor/rewrite-gpui` (belum di-push). Perubahan `syscheck.rs` (sidecar opencode
  bawaan lebih dulu) sudah dikomit di branch `zandor/background-jobs-dan-lainnya` (a42a1ef).

- **Desain ulang halaman "Memori dan skill" GPUI (22-11-2026, belum di-commit):** `src/shell/memory_page.rs` + i18n ID/EN.
  Kartu `raised` tanpa border, tab beri ikon; Settings dengan header kolom per project; Memory dengan hitung karakter dan
  penanda "belum disimpan" (dibandingkan ke `note_base`, bug penanda palsu saat load diperbaiki); Skill queue: draf pending
  di atas sebagai kartu, sisanya baris ringkas + "Bersihkan"; Installed skills dengan deskripsi dari SKILL.md dan ikon
  edit/hapus. Terverifikasi visual di jendela (lebar penuh, 4 tab); lebar sempit dan tema terang belum dicek.
- **Mode "Pasang skill otomatis" ala Hermes (22-11-2026, belum di-commit, bawaan MATI):** sakelar baru di Pengaturan > Memori dan skill (`skillAutoInstall` di `settings.json`). Bila aktif, `skill_propose` memvalidasi (nama, panjang, pemindai secret) lalu langsung menulis `SKILL.md` (project: `<project>/.opencode/skills`, sesi Chat: skill global), tidak menimpa skill yang sudah ada, dan mencatat entri `approved` di antrean. Ikon tab/skill diganti `file_text`; kartu dalam kartu di tab Memory dihapus. Tes plugin 50 lulus, cargo test 295 lulus; belum diuji di sesi nyata (perlu restart engine agar bundel plugin baru dimuat).
- **Panel 2: info sesi menggantikan tab sesi (22-11-2026, belum diuji di app)**: bilah atas Panel 2 kini menampilkan `Nama project · branch git · Nama sesi` (sesi di project; branch hanya bila folder repo git, dimuat di latar) atau hanya `Nama sesi` (Chat tanpa project). Tab per sesi dan menu klik-kanannya dihapus; sesi dipilih dari sidebar. Kode: `shell/chat_tabs.rs`.

- **Copywriting kartu langkah berikutnya (22-11-2026, belum di-commit):** uji 5 skenario nyata menunjukkan ringkasan yang melaporkan ulang isi jawaban ("sudah dibuat..."), pertanyaan kartu yang menyalin pertanyaan di akhir jawaban, serta label tombol berupa fragmen ("Versi Word", "Versi hemat"). Prompt di `core/next_steps.rs` ditulis ulang: gaya teman bicara ("kamu"), ringkasan hanya menambah nilai, tanya `-` bila sudah ada pertanyaan, label berawalan kata kerja 2-4 kata dengan contoh bagus/buruk; `MAX_LABEL` 24→32. Tes cargo next_steps lulus. **Diuji ulang di app (4 skenario):** ringkasan jadi bernilai ("Bagian dalam [ ] masih perlu kamu isi", "Tenggat Sari masih minggu depan"), label jadi kata kerja ("Bagi jatah mingguan", "Simpan sebagai Word"), pertanyaan duplikat hilang. Cacat baru: model kecil melihat ekor jawaban saja sehingga sekali menulis "jawabanmu kepotong"; ditambah aturan larangan menilai kelengkapan (belum diuji ulang).
- **Pembuatan skill otomatis (22-11-2026, belum di-commit):** saat sakelar pasang otomatis aktif, plugin menyuntik `AUTO_SKILL_HINT` tiap giliran (tanpa menunggu pemicu koreksi/`/reflect`/5 tool call) agar model sendiri menilai dan memanggil `skill_propose` setelah tugas yang layak; dibatasi `AUTO_SKILL_LIMIT`=3 skill per sesi. Pemicu lama tetap jadi dorongan tambahan. Tes plugin 51 lulus. **Diuji di sesi nyata (DeepSeek Flash):** instruksi awal terlalu lemah (2 tugas, tidak ada skill); setelah diganti kriteria konkret + "WAJIB panggil skill_propose", skill global `ringkasan-rapat` terbuat sendiri dan terpasang (antrean `approved`, memberi tahu user di balasan). Tugas sepele (CSV) tidak diuji ulang. Skill uji itu masih terpasang.
- **Uji Memory + Skill Creator, sesi nyata (22-11-2026, build debug, DeepSeek V4.1 Flash Free via OpenCode, kontrol desktop):**
  - Lulus: `memory_write` (scope project) menulis `- Kode uji memori project: JERUK-4417` (terlihat di Pengaturan > Memory);
    pemanggilan lintas sesi dalam project yang sama menjawab benar tanpa langkah tool; isolasi: chat tanpa project menjawab
    "tidak tahu"; koreksi pengguna memicu `skill_propose`, draf muncul "Awaiting approval", kartu "Setujui skill / Cek draf"
    tampil di chat, "Approve & install" ke Project: tes memasang `jawaban-kode-tolong`, tampil di Installed skills, dan
    Delete skill menghapusnya. Data uji (skill, berkas memori project, entri antrean, sesi chat tanpa project) sudah dibersihkan.
  - **Diperbaiki (22-11-2026, belum di-commit, tes plugin 26 lulus; terverifikasi di app setelah restart + build ulang `zandor-plugin.js`: satu koreksi menghasilkan tepat satu entri, draf diringkas di balasan tanpa baca folder data/izin; entri uji sudah dihapus):**
    `packages/plugin/src/skills.ts` `INTERNAL_PROMPT` + `index.ts` `chat.message` melewati prompt judul/ringkasan; `skillHint`
    menyuruh model meringkas draf di balasan dan tidak membaca folder data. Tombol "Cek draf" ternyata dibuat model
    (bukan kode aplikasi). Entri lama 21-11 di antrean harus dihapus manual.
  - **Temuan awal (bug kandidat):** antrean skill terisi entri "Your correction / Waiting for a draft" yang
    excerpt-nya adalah prompt judul otomatis ("Anda memberi judul pada sebuah percakapan...") dan prompt ringkasan
    ("Anda membantu pengguna melanjutkan percakapan..."), padahal pengguna tidak mengoreksi apa pun (juga pada entri lama
    21-11). Deteksi pemicu koreksi kemungkinan ikut memproses sesi internal judul/ringkasan.
  - **Temuan:** tombol "Cek draf" hanya mengirim pesan biasa; model lalu mencari berkas antrean di folder data aplikasi
    (di luar project) dan memicu permintaan izin `external_directory` (ditolak). Perlu jalur tampilkan-draf tanpa membaca berkas.
  - Catatan: draf komposer bersama sempat tertimpa saat uji; sesi uji di project `tes` ("Menyimpan kode...", "Kode uji
    memori...") belum dihapus.

## Terbaru (22-11-2026, GPUI) — Audit performa (branch `zandor/perf-gpui`)

- Streaming chat: render digabung ~30 Hz (`delta_pending` di `ChatStore`), part dicari dari belakang, log aktivitas pakai `EngineEvent::name()`.
- Render chat: `ChatState::render_view` (salinan tanpa riwayat pesan + turn tergabung, satu clone per frame), `ml.remeasure()` hanya saat streaming/kunci ukuran berubah, `has_pinned` murah.
- `mtime` dokumen aktif dicek di thread latar; `prefs.json` di-cache di memori dan ditulis atomik di thread latar; profil `dist` (fat LTO) untuk rilis.
- Cache turn tampilan per (sesi, `ChatState::rev`, revert): ketikan composer tidak lagi meng-clone/menggabung riwayat. `window.refresh()` di mouse-down root hanya bila webview tampil.
- Pengukuran: `ZANDOR_PERF=1` mencetak `[perf] chat.render`/`chat.item` yang melewati anggaran (`src/perf.rs`).
- Sengaja belum: cache parse markdown (butuh desain ulang, ukur dulu dengan `ZANDOR_PERF`), `Arc<Theme>` (clone ≈ <2 KB, dampak kecil), virtualisasi file tree (sudah dibatasi MAX_EXPAND=150), polling Mermaid/agent (murah saat idle; interval lebih panjang menambah latensi perintah agent).

## Terbaru (22-11-2026, GPUI)

- **Daftar tugas (todowrite)**: (1) Copy "Rencana" diganti **"Tugas"/"Tasks"** di i18n (`todo.*`, `dock.label`, `step.plan`, dll)
  dan teks resume. (2) Kartu tugas di dock **terlipat otomatis** saat semua tugas selesai/batal dan **terbuka lagi** saat ada
  daftar baru yang belum selesai (`prompt_dock.rs::auto_closed`; tes `lipat_otomatis_...`). Pengguna tetap bisa melipat/membuka manual.
  (3) Akar masalah "agent hanya membuat plan di awal": system prompt engine tidak punya panduan `todowrite`. Plugin kini menyuntik
  blok `<zandor-tasks>` (`packages/plugin/src/index.ts`, `TASK_HINT`) yang meminta daftar baru tiap permintaan multi-langkah,
  termasuk permintaan lanjutan. **Bundel plugin harus dibangun ulang** (`bun scripts/prepare-engine.ts --plugin`).
  Diuji di app GPUI (release, sesi Chat, gpt-6-luna): giliran 1 dan giliran "Perbaiki..." sama-sama membuat daftar baru, lalu
  terlipat saat selesai. Belum diuji: sesi Project nyata; perilaku model bersifat probabilistik (hint bukan jaminan).

- **Auto-refresh panel Dokumen**: `DocsPanel` memeriksa mtime berkas aktif tiap 0,8 dtk dan memuat ulang bila berubah di disk
  (mis. diedit agent). Dilewati saat berkas sedang diedit (mode edit) atau menampilkan diff, agar perubahan pengguna tidak hilang.
  Hanya tab aktif yang dipantau. Diuji di app: file `.txt` diubah dari luar, panel berganti isi dalam ≤3 dtk. Belum diuji untuk
  PDF/gambar/spreadsheet (memakai jalur muat ulang yang sama).

## Terbaru (21-11-2026, GPUI)

- **Gaya permukaan Solid/Kaca** (pengaturan `surface`, Pengaturan > Tampilan). Kaca (hanya Windows): jendela memakai
  `WindowBackgroundAppearance::Blurred` dan `Colors::glassify` menurunkan alpha latar (frame/bg/surface/raised/segment/composer/bubble);
  teks dan warna status tidak berubah. **Ditunda (21-11-2026): belum berfungsi.** Pengaturan dan panggilan DWM sukses
  (HRESULT 0), alpha panel benar, tetapi di komputer pemilik (Windows 11 build 26200) latar tetap putih/solid di semua mode
  (Kaca, Mica, Transparan) — penyebab belum diketahui. Mode lain: `mica`, `clear`; ada log diagnostik `[zandor] kaca:` dan
  `%TEMP%/zandor-kaca.log`. Default tetap Solid, jadi tidak memengaruhi pemakaian biasa.

## Terbaru (20-11-2026, GPUI)

- **Dikte realtime dengan Whisper lokal (ADR-080):** tombol mic di composer kini merekam sendiri (WASAPI, mikrofon pilihan pengguna; `host/voice_capture.rs`), VAD + resampler 16 kHz (`voice_dsp.rs`), pipeline STT (`voice_stt.rs`: `whisper-rs`, Greedy untuk teks sementara, BeamSearch untuk final, filter halusinasi, kamus koreksi), sesi di thread (`voice_session.rs`) dan penyisipan di posisi kursor (`compose_text`). Pengaturan Chat: bahasa dikte (id/en/auto, bawaan `id`), model (base/small/medium), kosakata dan kamus koreksi (`shell/voice_section.rs`). Win+H dihapus. Tambahan lepas: `ToastTone::Info` (tampil sama dengan `Neutral`) karena `chat_store.rs` sudah memakainya. **Tes:** `cargo test --offline` 284 lulus (termasuk DSP, pipeline, koreksi, `compose_text`, skema, i18n); uji model `small` sungguhan pada TTS Inggris 7,6 dtk benar kecuali nama proyek ("Xander"). **Skill `zandor-code` + aturan skill (ADR-081) 21-11-2026:** diuji dengan engine opencode bawaan dan model `deepseek-v4.1-flash-free` (3 percobaan; model lain belum): antislop tidak lagi memaksa, pertanyaan skill desain dan uji otomatis muncul, pnpm dipakai setelah aturannya dipindah ke blok `<zandor-skill-policy>`; **ditandai lulus oleh pemilik**. Belum teruji: alur uji otomatis di browser Panel 4, model lain. **Build rilis 21-11-2026:** `cargo build --release --offline` berhasil (2m13s, exe 47 MB); perilaku dan performa di build rilis belum dicoba. **Pembaruan 21-11-2026 (ADR-082):** diuji di mesin pengembangan — model terdeteksi, mic menangkap suara (perlu `AUTOCONVERTPCM`; mic laptop sangat pelan → penguat otomatis), teks masuk composer; ditambah model `large-v3-turbo-q5_0`, transkripsi ulang rekaman utuh saat Stop, dan bungkus lunak composer (295 tes lulus). **Diuji manual oleh pemilik (21-11-2026): lulus** — dikte (model turbo, transkripsi ulang saat Stop) dan bungkus lunak composer. **Belum teruji:** mikrofon lain, tampilan/perilaku UI di jendela aplikasi, latensi streaming, model base/medium, build release. **Unduh model dari aplikasi:** tombol Unduh/Batal + progres di Pengaturan → Chat (`host/voice_models.rs`, 8 tes dengan server HTTP lokal); unduhan nyata dari Hugging Face lewat tombol dan tampilannya di jendela aplikasi belum dicoba (model di mesin pengembangan diunduh manual ke `<data>/models/ggml-<ukuran>.bin`). **Build:** butuh CMake, Build Tools C++, LLVM; `bindgen` perlu `LIBCLANG_PATH`, `BINDGEN_EXTRA_CLANG_ARGS` (header clang + MSVC) dan `CMAKE_GENERATOR=NMake Makefiles` (belum dipasang permanen di proyek).
- **Satu instance (`single_instance.rs`):** mutex bernama `Local\Zandor-GPUI-single-instance` di awal `main`; instance kedua menunggu ≤ 2 dtk (peluncuran ulang dev-watch), lalu memunculkan jendela "Zandor" yang sudah ada dan keluar. Hanya Windows (`TODO(port)`). **Diuji manual di aplikasi berjalan (21-11-2026): lulus.**
- **Kartu saran aksi menetap di riwayat:** kartu kesimpulan tersimpan per pesan pengguna (`ChatStore::next_steps`: daftar per sesi, maks 50) dan tetap tampil di bawah jawabannya. Tombol aktif hanya di jawaban terakhir; setelah dipilih (`choose_next_step`) kartu menjadi catatan: kesimpulan, pertanyaan tebal, dan pilihan sebagai teks di bawahnya, lalu pesan terkirim sebagai pesan pengguna. Kartu lama tanpa pilihan tampil tanpa tombol. Hanya di memori: hilang saat aplikasi ditutup.

- **Kesimpulan + saran aksi di akhir jawaban (`core/next_steps.rs`, `shell/next_steps_card.rs`):** setelah giliran selesai (status Idle, tanpa izin/pertanyaan/antrean tertunda, jawaban ≥ 160 karakter), `ChatStore::maybe_next_steps` memanggil model kecil (`model-roles.json`, atau model sesi) lewat `complete_once` dengan permintaan + jawaban. **Model yang menjawab tidak diminta menulisnya**, jadi seragam di model murah/mahal, pintar/bodoh. Format balasan berbasis baris (`RINGKAS:`/`TANYA:`/`AKSI: label | pesan`), diurai longgar (huruf besar-kecil, bullet, markdown, JSON/pagar kode) dan divalidasi ketat (ringkasan ≤ 280, maks 3 aksi, label ≤ 24, tanpa duplikat). Bila panggilan gagal/tak terbaca: cadangan = paragraf penutup jawaban ≥ 600 karakter sebagai ringkasan, tanpa tombol (aksi tidak dikarang tanpa model). Kartu tampil di bawah jawaban terakhir; tombol mengirim pesannya; hilang saat ada pesan baru atau ditutup. **Tidak tampil di semua jawaban:** hanya giliran yang memakai tool dengan teks akhir ≥ 80 karakter, atau jawaban obrolan ≥ 500 karakter; lalu model kecil boleh membalas `TIDAK PERLU` (penjelasan/obrolan/jawaban lengkap tanpa tindak lanjut) dan kartu dilewati tanpa cadangan. **Aksi harus nyambung:** prompt melarang topik baru/tebakan/nama yang tak tertulis dan mengutamakan 1-2 aksi pasti; lalu `ground()` membuang aksi yang tidak berbagi kata isi dengan permintaan pengguna atau ujung jawaban (penyaring leksikal kasar), dan pertanyaan ikut dibuang bila semua aksi gugur. Pengaturan Chat → "Kesimpulan dan saran aksi" (`nextSteps`, bawaan aktif; biaya: satu panggilan singkat tambahan per jawaban). Tes: 8 unit `next_steps` (termasuk balasan model lemah), `cargo test --offline` 257 lulus. **Belum:** dicoba di aplikasi berjalan dan dengan model nyata (mutu saran, latensi, tampilan kartu), build release; panggilan memakai sesi sementara di engine sehingga plugin/memori ikut berjalan di dalamnya.

- **Saran buat project di sesi Chat (`core/project_intent.rs`, `shell/project_suggest.rs`):** pesan pertama di Chat (tanpa project) dinilai dengan skor sinyal ID+EN tanpa model: instal dependensi/`npm|pip|cargo install`, "buat aplikasi/web/bot/API", framework, repo, banyak berkas, data (csv/dataset/scraping), riset mendalam, dokumen panjang (skripsi/buku/N halaman), media massal (ffmpeg/subtitle), otomasi (cron/jadwal), daftar langkah, lampiran; pertanyaan, terjemahan/ringkasan, dan potongan kecil (satu fungsi/puisi) mengurangi skor. Lolos ambang → pesan ditahan dan kartu di atas composer menawarkan **Buat project** (folder baru `Documents\Zandor Projects\<slug>` yang unik, didaftarkan sebagai project, sesi baru, pesan terkirim di sana) atau **Tetap di Chat** (Enter lagi sama; tidak bertanya lagi di sesi itu). Pengaturan Chat → "Sarankan membuat project" (`suggestProject`, bawaan aktif). Tes: 5 unit `project_intent`; `cargo test --offline` 249 lulus. **Saran dari agent di tengah percakapan:** plugin menyisipkan `<zandor-chat-scope>` ke system prompt hanya untuk sesi Chat (folder sesi di dalam opsi `chatsDir`, dikirim `host/engine.rs`) dan menyediakan tool `zandor_suggest_project {reason, summary, name?}` (aksi `suggest_project`). Aplikasi menolaknya bila sesi sudah di project atau `suggestProject` mati; bila diterima, kartu yang sama tampil di sesi itu. **Buat project** → folder baru + sesi baru dengan `summary` (ringkasan mandiri dari agent) sebagai pesan pertama; **Tetap di Chat** → mengirim pesan "tetap di sini" ke agent. Tes: plugin vitest 48 lulus (tool + `isInside`), Rust 249 lulus (termasuk `chatsDir` di config engine). **Belum:** dicoba di aplikasi berjalan (kartu, tool dengan model nyata — apakah model patuh pada petunjuk belum teruji; e2e engine plugin tidak dijalankan), build release; pesan pengguna hanya dinilai pada pesan pertama sesi; nama project belum bisa diubah di kartu (ganti nama lewat sidebar); sesi Chat lama tidak ikut dipindahkan, hanya ringkasan agent yang dibawa. Perubahan menyentuh `packages/plugin` (dipakai bersama GPUI).

- **Command palette pintar (Ctrl+K):** saat kueri kosong, palet dibagi 3 bagian: **Terakhir digunakan** (3 perintah terakhir yang dipicu pengguna lewat palet/menu/pintasan; disimpan di `<app_data>/recent-commands.json`; perintah dari agent tidak dihitung), **Relevan untuk panel {X}** (panel yang terakhir diklik atau dibuka lewat pintasan; peta di `commands::relevant_for`), dan **Semua perintah** (sisanya, tanpa duplikat dari dua bagian atas), lalu Chat terbaru. Saat mengetik, hasil tetap satu daftar fuzzy tanpa judul bagian. Panel aktif dilacak lewat `capture_any_mouse_down` di `layout.rs` (`AppState::active_panel`); sebelum ada klik pertama, bagian "Relevan" tidak tampil. Tes: `terakhir_dipakai_*`, `perintah_relevan_semuanya_terdaftar`. **Belum dilihat di aplikasi berjalan** (tampilan judul bagian, urutan, pelacakan klik panel).
- **Tema editor bawaan = Theme Darker (tal7aouy, MIT) + pemetaan warna ala VS Code:** default pertama kali (`DEFAULT_EDITOR_THEME`, `assets/themes/theme-darker.json`), muncul di pemilih tema editor. `ui/highlight.rs` kini memberi scope TextMate per token (keyword/storage/konstanta/fungsi/tipe/variabel/properti/operator/tag/atribut/CSS) dan `editor_theme::Resolver` menyelesaikannya terhadap `tokenColors` (selektor turunan, paling spesifik menang). Panel dokumen sekarang memakai `editor.background`/`editor.foreground`/`editorLineNumber.foreground` (sebelumnya hanya blok kode chat yang memakai latar tema). Tes `tema_bawaan_seperti_vscode` lulus. **Belum dilihat di aplikasi berjalan**; highlighter tetap lexer per baris, bukan grammar TextMate penuh.

- **Giliran terputus + Lanjutkan (`core/resume.rs`):** pesan asisten tanpa `completed`/galat saat engine idle dianggap terputus (aplikasi/engine mati di tengah giliran). Durasi "Proses · n langkah · t" berhenti di aktivitas tool terakhir (dulu terus menghitung sampai sekarang). Di bawah giliran tampil kartu "Proses terputus"; pada giliran terakhir ada tombol **Lanjutkan** yang mengirim pesan pendek + part teks sintetis `<zandor-resume>` berisi permintaan awal pengguna, ringkasan tiap tool call (selesai / galat / terputus-setengah-jadi, maks 40 terbaru), rencana, dan teks terakhir asisten. Lanjutan berulang tetap merujuk permintaan asli. Isi reprompt tampil sebagai langkah pertama "Melanjutkan proses terputus" di detail proses giliran berikutnya. Tes: 4 unit `resume`.
- **Status sesi dari engine:** saat riwayat dimuat, status jalan diambil dari `GET /session/status` (dulu ditebak dari pesan terakhir sehingga sesi terputus tampak "bekerja" selamanya). Tes: `status_engine_mengalahkan_pesan_yatim`.
- **System prompt di Trajectory:** plugin merekam isi akhir hook `experimental.chat.system.transform` ke `<app_data>/system-prompts/<sessionID>.jsonl` (hanya saat berubah, secret disamarkan; `packages/plugin/src/system-trace.ts`, opsi plugin `systemDir`); Trajectory menampilkannya sebagai Konteks "system" di awal tiap giliran (`core/system_prompts.rs`, `trajectory::attach_system`). Giliran sebelum fitur ini tanpa system prompt; daftar tool tetap tidak terlihat. Catatan: ini menyentuh `packages/plugin` (plugin engine dipakai bersama GPUI), bukan UI React.
- **Tombol jendela:** Minimize dan Close kini tombol area klien (`minimize_window` / `remove_window`), bukan `WindowControlArea::Min/Close`; area non-klien membuat Windows memunculkan tooltip bawaan "Close" yang bisa tersangkut di layar. Maximize tetap `WindowControlArea::Max` (Snap Layouts).
- **dev-watch:** mode jeda bawaan (build tetap jalan, restart setelah Enter; `-Auto` = perilaku lama); aplikasi dijalankan dari salinan `target\run\` agar cargo bisa menimpa exe saat aplikasi terbuka.
- **Uji:** `cargo test` 237 lulus; plugin vitest 46 lulus + typecheck + Biome. **Diuji manual (21-11-2026): lulus** — kartu terputus/Lanjutkan, tombol jendela baru, resize/popup. **Belum diuji:** entri system prompt di Trajectory, dev-watch mode jeda.

## Terbaru (19-11-2026)

- **Browser-use agent, tahap F1 (baca saja) — ADR-079, `docs/features/browser-agent.md`:** tool plugin `zandor_browser` (`navigate`, `snapshot`, `screenshot`, `tabs`, `back`, `forward`, `reload`) lewat saluran kontrol ADR-077 (`browser_<aksi>`), dieksekusi di `shell/browser_agent.rs`. Agent memakai satu tab **Agent** di Panel 4 dengan profil WebView2 terpisah (`<app_data>/browser-agent/profile`, tanpa login pengguna). Pengaman: saklar `browserAgent` (**bawaan mati**, dilindungi dari `set_setting`); **izin per domain** lewat dialog (Sesi ini / Selalu / Tolak; "selalu" disimpan di `<app_data>/browser-agent/domains.json`); situs keuangan/pembayaran diblokir; navigasi (termasuk redirect) ditegakkan di kode lewat navigation handler WebView2; unduhan ditolak; isi halaman dibungkus `<halaman_tidak_tepercaya>` + peringatan pola injeksi; nilai password disamarkan di snapshot; API snapshot dikunci agar tak bisa ditimpa halaman; tombol **Hentikan/Izinkan lagi** di Panel 4; polling kontrol 100 ms hanya saat agent aktif. Screenshot lewat CDP (`Page.captureScreenshot`) disimpan di `browser-shots/` folder sesi; gagal cepat (5 dtk) bila Panel Browser tak tampil. Tes: `cargo test browser` (5 unit baru: host/pola/blokir/navigasi/snapshot), vitest `control.test.ts`. **Belum:** klik/ketik/isi form (F2), konfirmasi aksi berisiko (F2), uji native end-to-end di jendela GPUI (kartu izin, tab Agent, snapshot nyata), `cargo build --release` (disk penuh saat sesi ini), skill `zandor-browser` (F3). Pilihan "Sekali" di kartu izin dihapus (hanya Sesi ini / Selalu / Tolak). Hanya Windows.

## Terbaru (17-11-2026)

- **Agent mengoperasikan Zandor + generate gambar (ADR-077):** tool plugin `zandor_app` (status, perintah palet, panel, alat Panel 4, URL di browser, berkas di Panel 3, halaman, terminal, pengaturan) dan `zandor_image`, lewat antrean berkas `<app_data>/control` (Rust `control_take/control_reply`, UI `AgentControlDriver` + `agent-control.ts` memvalidasi tiap aksi, toast "Agent: …"). Pengaturan `agentControl` (Umum, bawaan aktif; agent tidak bisa mengubahnya). Generate gambar di Rust `images.rs` (OpenAI Images, Gemini, OpenRouter; kunci dari keychain), disimpan di `images/` project lalu dibuka di Panel 3; dialog **Generate gambar…** di palet. Skill bawaan `zandor-app`. Tes: plugin `control.test.ts` (2), Rust `images`/`control` (6), unit `agent-control.test.ts` (8), e2e `image-generate.spec.ts`. **Belum diuji** ke engine/model sungguhan, API gambar sungguhan, maupun build native.
- **Model cadangan:** `fallbackModel` (`packages/core/src/chat/store.ts`) — bila belum ada pilihan model dan semua model dari provider `opencode`, pakai model DeepSeek OpenCode (engine 1.19.5: `deepseek-v4.1-flash-free`); selain itu model pertama. Tes: `fallback-model.test.ts`.
- **Halaman onboarding (`#/welcome`, `OnboardingPage.tsx` + `onboarding-steps.tsx`):** urutan Sambutan (logo, ringkasan setup, Lanjut) → Profil → Provider → Pemeriksaan sistem → Memori & skill (Otomatis / Tentukan sendiri / Mati beserta kelebihan-kekurangan; Otomatis = `defaultEnabled`/`skillCreator` true, dua lainnya menyimpan false) → Impor memori (prompt baku `- kunci: nilai` untuk ChatGPT/Gemini/Claude/DeepSeek/Grok/…, balasan bisa dari beberapa AI; `packages/core/src/memory/import.ts` menggabungkan dengan `global.md`, kunci bernilai tunggal (`SINGLE_KEYS`: nama, panggilan, pekerjaan, perusahaan, lokasi, zona waktu, gaya jawaban; ID+EN) dengan nilai berbeda jadi konflik radio yang wajib dipilih atau "Tulis sendiri" (satu nilai); kunci lain (proyek, alat, minat, kunci buatan AI) bernilai banyak: digabung, semua tercentang bawaan, bisa dibuang dan ditambah nilai sendiri, baris mirip secret dibuang, pratinjau sebelum simpan) → Terima kasih. Profil: placeholder nama generik, spasi bisa diketik (draf lokal; dulu `trim` per ketikan menghapusnya), tab avatar bawaan Ilustrasi. Fokus penuh satu kolom tanpa sidebar dan tanpa header aplikasi (TitleBar hanya area seret + kontrol jendela), bilah progres langkah di atas, tombol panah Kembali/Berikutnya. Pemeriksaan sistem dikelompokkan: Engine dan model (engine, model OpenCode, router, OpenCode CLI), Python (python, pip, pustaka dokumen, uv), Alat pengembangan (Git, GitHub CLI, Node.js, npm, Bun), Dokumen (LibreOffice), Pemasang paket (winget); hasil nyata di mesin bisa dicetak dengan `cargo test syscheck::live -- --ignored --nocapture`. Layar penuh, jadi fokus utama saat pengaturan `onboarding` masih true (dibuka otomatis, tanpa syarat belum ada project). 4 langkah: Profil (editor `ProfileSettings`: nama + avatar inisial/unggah/ilustrasi), Provider dan model (jumlah model nyata per provider, tambah koneksi inline `ConnectionPanel`), Pemeriksaan sistem (engine OpenCode, model bawaan OpenCode, router/endpoint lokal, lalu Python, pustaka dokumen Python, Git, Node.js, winget lewat perintah Rust `system_check` di `syscheck.rs` — hanya membaca versi, tidak memasang; yang belum ada diberi perintah winget/pip yang bisa disalin + tautan situs; di browser dikatakan tidak bisa diperiksa), Mulai (tambah project / Chat, pintasan). Selesai/Lewati mematikan `onboarding`; bisa dibuka lagi dari palet ("Tur pengenalan"). Dialog `Onboarding.tsx` lama dihapus. Browser otomatis (Playwright, vitest jsdom) tidak dihadang kecuali tes memasang penanda `zandor.e2e.tour`. Tes: e2e `onboarding.spec.ts`.
- **Opt-in memori di dialog percayai folder:** sakelar "Aktifkan memori untuk project ini" mengikuti bawaan memori; override hanya disimpan bila berbeda. Dialog percayai kini `Dialog` biasa (role `dialog`, bukan `alertdialog`). Tes: e2e `memory.spec.ts` (5).
- **Engine remote lewat URL (ADR-076):** Pengaturan → Engine remote (`#/settings/engines`): URL + Basic Auth, status health jujur, Cek/Ubah/Hapus; pilihan Engine di dialog Tambah project; penanda status di baris project. Router EngineApi per direktori (`core/engine/remote.ts`), tanpa remote perilaku lama tidak berubah. HTTPS wajib kecuali loopback (tunnel SSH); CSP `connect-src` + `https: wss:`. Tes: core `remote.test.ts` (3), e2e `remote-engine.spec.ts` (engine tiruan kedua). Batasan: sandi tanpa enkripsi di registry, Panel 3 belum membuka berkas remote, belum diuji ke engine remote sungguhan/build native.
- **Model peran — model kecil:** Pengaturan → Provider dan model → **Model kecil** (opencode `small_model`, judul sesi/ringkasan), disimpan Rust di `model-roles.json`, masuk config saat engine dimulai; tombol mulai ulang engine. Tes Rust `small_model_dari_berkas_peran_divalidasi`, e2e `model-settings.spec.ts`. Model memori sengaja tidak dibuat (plugin memori tidak memanggil model sendiri); model gambar tetap preferensi.
- **Status uji penuh (17-11-2026):** vitest 899/899; e2e 214/214 (dua putaran suite penuh setelah perbaikan flaky). Penyebab flaky: (1) bug aplikasi — fokus otomatis composer menganggap toast (`role="dialog"` non-modal) sebagai dialog terbuka sehingga tidak memfokus; kini hanya overlay modal/menu yang menahan; (2) `mvp` melewati batas 30 dtk per tes saat paralel (kini 90 dtk); (3) tes membaca nilai sekali tanpa menunggu (zoom → `expect.poll`), menunggu penanda yang sudah terlihat (`mvp` buat sesi), atau mengirim sebelum composer siap (`composer-info`); (4) batas tunggu asersi 10 dtk (startup di bawah beban). Mock engine kini mengabaikan galat `route.fulfill` untuk permintaan yang dibatalkan muat ulang.
- **Pencarian global lintas sesi:** Ctrl+Shift+F / palet → dialog `GlobalSearch` (judul + isi pesan semua project dan Chat, hasil bertahap, sorotan, klik membuka sesi di pesannya). Logika `core/chat/session-search.ts` (cache per `time.updated`, konkurensi 4, batal saat kata berubah). Tes: core 4, e2e `global-search.spec.ts`. Belum: indeks di disk, build native.
- **Diff view Panel 3 (ADR-075):** tombol **Lihat diff** di Perubahan sesi (Panel 4) dan di langkah tool `edit` membuka `DiffViewer` (Monaco diff, inline saat sempit) dengan bilah per bagian: navigasi, Terima (menandai), Tolak (mengembalikan bagian + simpan, toast Urungkan), Terima/Tolak semua (konfirmasi), Tutup diff. Logika patch murni di `core/docs/patch.ts` (urai, balik, kembalikan satu hunk, geser). Tes: core `patch.test.ts` (5), komponen `DiffViewer.test.tsx` (4: terima tanpa tulis, tolak + urungkan, tolak semua, tanpa penulisan), e2e `panel4.spec.ts` (2). Biome kini mengecualikan `bundled-skills/**/*.md` (penyebab baris kosong di frontmatter skill). Belum: build native, pewarnaan TextMate di diff.
- **Teks PDF bisa dipilih dan dicari:** `PdfViewer` memasang `TextLayer` pdf.js di atas kanvas tiap halaman (CSS diadaptasi dari `pdf_viewer.css`, token desain); kolom cari di bilah PDF (tombol cari toolbar dokumen / Ctrl+F memfokuskannya), hitungan "i dari n", sorotan semua hasil + hasil aktif, Enter/Shift+Enter dan tombol ↑/↓, tidak peka huruf besar; PDF tanpa lapisan teks diberi catatan jujur. Bilah PDF membungkus di panel sempit. Tes: e2e `docs.spec.ts` 19/19 (termasuk posisi span sejajar glyph). Batasan: kata yang terpecah di dua potongan teks PDF tidak ditemukan; belum dicek di build native.

## Sebelumnya (16-11-2026)
- **Trajectory sesi (ADR-073):** alat baru di Panel 4 (ikon Rel): buku besar per giliran (pengguna, konteks, panggilan model + token/biaya, berpikir, tool call input/output/durasi, langkah, galat), ikhtisar waktu 3 jalur, pencarian, filter jenis, inspektur Input/Output/Raw, salin JSON. Unit `trajectory.test.ts` lulus. Belum: daftar tool (tak tersedia dari engine), subcall `task`, virtualisasi. (System prompt kini ada di GPUI, lihat 20-11-2026.)

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
- **Fase 11 — Panel 3 lanjutan & fitur ekstra:** sebagian selesai di lokal (ADR-039, ADR-045, ADR-050): edit berkas, DOCX/XLSX/PPTX/ipynb, saran prompt, automasi + notifikasi, input suara (dikte, Web Speech), save-as pesan (teks/gambar/SVG). Engine remote (ADR-076) dan model kecil selesai 17-11-2026. Generate gambar dan skill bawaan "agent mengoperasikan Zandor" (ADR-077) selesai 17-11-2026 (belum diuji dengan model/API sungguhan).
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

## Rilis beta (2026-11-18)

- `Buat-Installer-Beta.cmd` (sekali klik) → `scripts/build-beta.ps1`: bun install, unduh+verifikasi opencode
  (`prepare-engine --fetch`), `tauri build --bundles nsis`, salin installer ke `rilis-beta/` (di-gitignore).
  opencode ikut dibundel sebagai sidecar (`externalBin`), jadi pengguna tidak perlu memasangnya.
- Build rilis menampilkan badge **BETA** (build dev tetap "Dev"). Fitur Cargo `devtools` dicabut (menggantikan ADR-062):
  rilis tanpa Inspect/F12, menu Dev disembunyikan, `browser-guard.ts` meredam klik kanan bawaan WebView2, pintasan
  browser (F5/Ctrl+R/Ctrl+P/Ctrl+U/Ctrl+Shift+I…), dan seleksi teks bebas (kecuali isi chat, kode, kolom isian, editor).
  Catatan: salinan stabil (`Perbarui-Zandor.cmd`) juga kehilangan DevTools; pakai build dev untuk debugging.
