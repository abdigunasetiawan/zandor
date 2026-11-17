# Fitur: Chat inti (Panel 2)

- **Status:** selesai (inti ADR-033; pelengkap ADR-034).
- **Kode:** `packages/core/src/chat/` (`types.ts`, `reducer.ts`, `store.ts`), `packages/core/src/engine/api.ts`,
  `apps/desktop/src/shell/{ChatPanel,Markdown}.tsx`, `apps/desktop/src-tauri/src/providers.rs`.

## Spek (yang sudah ada)

- Transcript dari engine secara streaming (SSE): pesan pengguna (gelembung berlatar `--z-color-bubble`, terlihat di tema
  gelap dan terang), jawaban Markdown (GFM), blok "Berpikir"
  (reasoning) yang bisa dilipat, kartu **tool call** (nama, ringkasan, status, masukan/keluaran yang bisa dibuka),
  baris meta (model, durasi, token/waktu lengkap di tooltip), galat pesan/sesi, status "bekerja"/"mencoba ulang"/
  "menyambung ulang".
- **Composer:** Enter kirim, Shift+Enter baris baru, tombol Kirim/Hentikan, pemilih model, pesan jelas bila tidak ada
  model atau engine belum siap. Teks dikembalikan ke kotak bila pengiriman gagal.
- **Kartu izin** (`ask` untuk edit/shell/webfetch/di luar folder): Izinkan sekali / Selalu izinkan / Tolak, dengan
  pola path dan diff.
- **Stop** membatalkan giliran yang berjalan. Auto-scroll hanya bila pengguna berada di dekat dasar.
- **Provider:** lihat `providers.md` (keychain; halaman Provider & model).

## Pelengkap (ADR-034)

Tab sesi (maks 12, tersimpan), Salin pesan, mode izin (tanya / setujui edit otomatis / otomatis kecuali berisiko — bawaan, ADR-068 / setujui semua — sisi klien),
varian reasoning (dari `model.variants`), lampiran berkas/folder, Ulangi jawaban (revert + kirim ulang), Kembalikan ke
sebelum pesan ini (revert) dan Batalkan revert, Cabangkan dari sini (fork), kartu web search, diff untuk edit/izin, dan
transcript tervirtualisasi untuk sesi ≥ 40 pesan. Provider ditambahkan lewat halaman Provider (lihat `providers.md`).

## Terbatas

Mode izin otomatis hanya bekerja selama Zandor terbuka. Deteksi risiko mode bawaan berbasis pola (`permissionRisk`), bukan sandbox: perintah yang dikaburkan bisa lolos — pakai mode Tanya bila butuh jaminan. Lampiran browser/clipboard/drop dibaca sebagai data URL tanpa batas ukuran
buatan di Zandor (dibatasi kapasitas memori dan batas engine/provider); lampiran
native dari pemilih berkas memakai URL `file://`. Lampiran berbasis data URL membuka modal viewer sesuai tipe (gambar, PDF,
teks/kode, Markdown, tabel, JSON, media, Office best-effort, atau keterangan untuk biner); HTML tetap sandbox tanpa skrip.
Gambar PNG/JPEG/GIF/WebP punya thumbnail dengan zoom dan pan.
Output tool dipotong 20.000 karakter di UI.

## Cara test

- Otomatis: `bun run test`, `bun x playwright test` (`chat.spec.ts`), `cargo test --workspace`, dan smoke native
  (`bun e2e-native/smoke.ts ../../target/debug/zandor-desktop.exe`; fase chat nyata hanya jalan bila ada provider).
- Manual: tambah project, buat sesi, minta "buat file x.txt berisi halo" → harus muncul kartu izin dan file baru
  muncul **setelah** Izinkan sekali; coba Hentikan saat jawaban panjang; matikan engine lalu hidupkan lagi.

## Saran prompt dinamis (Fase 11)

Di sesi kosong, composer menampilkan hingga 4 chip saran yang dihitung dari konteks nyata: jumlah berkas yang berubah di git
("Tinjau N perubahan", "Tulis pesan commit"), berkas yang sedang dibuka ("Jelaskan x", "Tulis tes untuk x"), isi akar project
("Buat README" bila README hilang, "Jalankan tes" bila ada manifes), dan saran umum bila tanpa project. Klik mengisi kotak
pesan (tidak langsung terkirim); chip hilang saat mulai mengetik. Tes: `suggestions.test.ts`, `e2e/suggest.spec.ts`.

## Render kaya, pertanyaan agent, info pemakaian (Fase 12, ADR-040)

- **Render respons:** Markdown GFM, KaTeX, blok kode berwarna (label bahasa, Salin, ciut > 30 baris), Mermaid (setelah jawaban
  selesai), tabel yang bisa digulir. HTML mentah/skrip/gambar luar tidak aktif.
- **Aksi blok render kaya** (`rich/RichActions.tsx`, `rich/rich-export.ts`): bilah mengambang saat hover/fokus per jenis blok.
  Kode: bungkus baris, salin, salin sebagai blok Markdown, simpan berkas (ekstensi dari bahasa), salin/simpan gambar. Tabel:
  salin (HTML + TSV, tempel ke Word/Excel tetap tabel), Markdown, TSV, CSV, HTML, simpan CSV/Markdown, gambar PNG/SVG. Rumus
  blok KaTeX: salin LaTeX, LaTeX dengan `$$`, MathML, gambar PNG/SVG, simpan `.tex`. Mermaid: salin sumber, salin SVG, simpan
  PNG/SVG/`.mmd`. Gambar `data:`: salin dan simpan. Rumus sebaris belum punya aksi. Tes: `rich-export.test.ts`.
- **Warna blok kode = tema editor aktif** (`core/theme/code-colors.ts`): `computeVars` menimpa `--z-code-*` (+ `--z-code-bg`,
  `--z-code-fg`) dari tema editor terpilih — tema VS Code terpasang lewat scope TextMate (`keyword`, `string`,
  `constant.numeric`, `comment`, `entity.name.function`, `entity.name.type`, `entity.other.attribute-name`, `markup.*`) dan
  `editor.background/foreground`; tema bawaan Monaco memakai palet bawaannya; "Otomatis" memakai token Zandor, dan Monaco
  "Otomatis" kini juga memakai `--z-code-*` sehingga editor dan chat identik. Jenis yang tak didefinisikan tema jatuh ke token
  bawaan. Tes: `code-colors.test.ts`.
- **Tautan di respons** (16-11-2026): klik/Enter pada tautan http(s) tidak langsung membuka halaman, tetapi menu kecil yang
  menempel pada tautan (`ChatLink`, `Menu nativeButton={false}`): **Buka di browser Zandor** (Panel 4 → alat Browser, tab aktif),
  **Buka di browser eksternal** (perintah Rust `open_external`, browser bawaan sistem), **Salin tautan**. Keduanya menerima `http` dan
  `https` (mis. `http://localhost:3000`); `open_external` mewajibkan skema eksplisit (`browser::external_url`). Di versi web
  browser Zandor dinonaktifkan dengan label "hanya di aplikasi desktop". `mailto:` dan jangkar `#` tetap tautan biasa. Tes: `bun x vitest run apps/desktop/src/shell/rich`.
- **Form pertanyaan agent** (tool `question`): kartu dengan pilihan tunggal/jamak dan jawaban sendiri; Kirim/Lewati. **Rencana
  agent** (todo) tampil di atas composer.
- **Info pemakaian** di composer: model, konteks terpakai vs batas (peringatan hampir penuh), token, biaya, rincian.
- **Dok rencana agent**: kepala satu baris (Rencana · n/N · langkah yang sedang dikerjakan · chevron) + bilah progres tipis, daftar
  langkah bisa dilipat; langkah aktif digulir ke tampilan. Progres **realtime**: diturunkan dari tool part `todowrite` begitu
  argumennya diterima (reducer `todosFromPart`), tidak menunggu event `todo.updated` yang baru datang setelah tool selesai.
- **Batas:** kuota/limit tarif provider tidak tersedia dari engine; gambar dalam jawaban tidak dimuat; kepatuhan model pada
  tool `question` bergantung model.
- **Tes:** `bun x vitest run packages/core/src/chat packages/core/src/tools`, `bun x playwright test e2e/rich e2e/questions
  e2e/composer-info`, smoke native (render dari model nyata, info composer, form pertanyaan bila model memakai tool).

## Antrean pesan (ADR-047)

Saat agent bekerja, pesan yang dikirim masuk **antrean** (dok di atas composer): ubah urutan dengan seret-lepas atau Alt+↑/↓ pada gagang, edit, hapus, atau **kirim langsung** (menghentikan giliran berjalan lalu mengirim item itu). Antrean dikirim berurutan saat agent selesai; **Stop menjeda** antrean (tombol Lanjutkan). Hanya di memori (hilang saat aplikasi ditutup). Tes: `e2e/queue.spec.ts`, `chat-extra.test.ts`.

## Indikator sesi, notifikasi, dan suara (ADR-050)

Titik di sidebar menunjukkan status sesi (bekerja = ripple, butuh aksi, selesai belum dilihat, gagal, batas penyedia). Saat sesi yang tidak dilihat selesai/gagal/butuh aksi: toast (tombol Buka sesi), notifikasi sistem (jendela tidak fokus), dan suara — diatur di Pengaturan > Notifikasi dan suara (nada bawaan atau berkas sendiri per peristiwa). Tes: `attention.test.ts`, `e2e/notify.spec.ts`. Belum diuji di Tauri sungguhan: notifikasi sistem dan pemutaran suara.

## Langkah tool ala agent harness (ADR-064)

Di dalam "Proses · n langkah" tiap tool tampil sebagai satu baris: ikon jenis · nama tool · target (folder redup + nama berkas,
perintah, kueri) · ringkasan (rentang baris yang dibaca, `+n −m`, jumlah hasil, jenis sub-agen, kode keluar bila gagal, durasi
≥ 1,5 dtk) · status. Dibuka menampilkan: **diff unified bernomor baris** (edit), isi berkas baru (tulis), **baris yang dibaca**
(read), **perintah + keluaran** (bash), **hasil grep per berkas** dan daftar berkas (glob), **tugas + hasil sub-agen** (`task`),
dan daftar rencana (todowrite). Tool lain memakai cadangan generik (masukan + keluaran). Ringkasan proses menambahkan hitungan
("12 dibaca · 3 diedit +45 −12 · 8 perintah").

- **Tertutup bawaan; yang dibuka pengguna tidak menutup sendiri** saat langkah/pesan engine baru bergabung (state lipatan di
  `open-state.ts`, kunci giliran `turnKey`). **Gulir mengikuti** langkah baru selama pengguna di dasar; membaca ke atas tidak direbut.
- **Sub-agen:** engine mendukung tool `task` (agent `general`/`explore`, sesi anak ber-`parentID`); model bisa menyuruh spawn.
  Belum: sesi anak tampil sebagai sesi biasa di sidebar dan belum bisa dibuka dari langkahnya.
- **Tes:** `bun x vitest run apps/desktop/src/shell/tool-view.test.ts`; e2e `bun x playwright test e2e/turn.spec.ts` (langkah,
  tidak menutup sendiri, gulir). Bentuk data tool diambil dari engine asli opencode 1.19.5.

## Kustomisasi tampilan Panel 2

Tombol **Kustomisasi** (ikon slider) di bilah atas Panel 2, di kiri tombol tutup, membuka popover `ChatStyleControls`
(`apps/desktop/src/shell/ChatStyleControls.tsx`). Isinya: kerapatan (Rapat/Normal/Lega = jarak antarpesan + tinggi baris),
jenis huruf, ketebalan, lebar kolom isi (28–120rem), lebar composer (1–1,6 × kolom), ukuran huruf, transparansi teks,
tinggi baris, jarak antarhuruf, jarak antarpesan, ukuran huruf kode, dan Kembalikan bawaan. Kontrol yang sama ada di
Pengaturan → Chat (`#/settings/chat`) dan Tampilan → Chat; semuanya menulis `appearance.chat` (divalidasi
`validateChat`, diterapkan sebagai variabel `--z-chat-*`). Cara uji: `bun x vitest run packages/core/src/theme`.

## Perintah slash dan mention (ADR-069)

Popup `ComposerSuggest` di atas kartu composer. `/` = perintah bawaan aplikasi + perintah engine (`GET /command`,
dijalankan via `session.command`); `@` = cari berkas (`GET /find/file`), dilampirkan sebagai `file://`. `/btw` memakai
`ChatStore.aside` (fork tanpa `messageID` + kirim di cabang). ↑ di composer kosong memanggil ulang pesan terakhir.
Tes: `composer-input.test.ts` (unit + store), `e2e/composer-slash.spec.ts`.

## Pencarian global lintas sesi (17-11-2026)

**Ctrl+Shift+F** atau palet → "Cari di semua sesi" membuka dialog `GlobalSearch`: mencari judul (judul bawaan engine
diabaikan) dan teks pesan pengguna/asisten (bukan berpikir/tool/teks sintetis) di semua project dan Chat. Pesan dimuat
dari engine per sesi (4 paralel) lewat `SessionSearch` (`packages/core/src/chat/session-search.ts`) dan di-cache selama
`time.updated` sesi sama; hasil tampil bertahap ("Mencari… n dari N sesi"), sesi yang gagal dimuat disebut. Urutan:
judul cocok, jumlah kecocokan, terbaru. Klik hasil (atau Enter = hasil pertama) membuka sesi dan menggulir ke pesannya.
Tes: core `session-search.test.ts`, e2e `global-search.spec.ts`. Batasan: tanpa indeks di disk (sesi banyak = pemuatan
pertama lebih lama); daftar sesi project yang belum dibuka dimuat saat dialog dibuka.
