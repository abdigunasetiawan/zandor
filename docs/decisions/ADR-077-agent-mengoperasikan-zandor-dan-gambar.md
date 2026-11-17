# ADR-077 — Agent mengoperasikan Zandor + generate gambar

Tanggal: 17-11-2026 · Status: diterima

## Konteks

Roadmap Fase 11 meminta (1) skill bawaan agar agent bisa mengonfigurasi dan mengoperasikan Zandor (membuka panel,
browser, menutup terminal, mengubah pengaturan), dan (2) fitur generate gambar memakai "model gambar bawaan" yang
selama ini hanya tersimpan sebagai preferensi.

Agent berjalan di engine opencode (proses terpisah), sedangkan tampilan dan pengaturan hidup di UI (webview). Kunci
API provider hanya ada di keychain (Rust) dan tidak boleh sampai ke UI maupun model.

## Keputusan

**Satu saluran kontrol berbasis berkas**, meniru pola background job (ADR-072):

1. Plugin Zandor mendapat opsi `controlDir` (`<app_data>/control`) dan dua tool:
   - `zandor_app {action, …}`: `status`, `list_commands`, `run_command`, `panel`, `open_tool`, `open_url`, `open_file`,
     `navigate`, `list_terminals`, `close_terminal`, `get_settings`, `set_setting`.
   - `zandor_image {prompt, size?, path?}`.
   Tool menulis `requests/<id>.json` secara atomik lalu menunggu `responses/<id>.json` (20 detik; gambar 180 detik).
   Tanpa jawaban → pesan jujur "aplikasi tidak merespons". Engine remote tidak mendapat opsi ini (tool tidak ada).
2. Rust: `control_take` (ambil + hapus, maks 16, buang yang > 5 menit atau > 64 KB, id divalidasi) dan `control_reply`
   (tulis atomik).
3. UI: `AgentControlDriver` mengambil antrean tiap 700 ms dan menjalankan `executeControl` (`shell/agent-control.ts`).
   Semua argumen dari model divalidasi: panel/alat dari daftar tetap, URL hanya http/https tanpa kredensial, berkas hanya
   di dalam project/Chat yang dikenal, halaman lewat `parseRoute`, pengaturan lewat skema (`validateSetting`).
   Dilarang: mengubah `agentControl`, menjalankan `window.close`/`dev.*`. Aksi yang mengubah tampilan memunculkan toast
   "Agent: …" agar pengguna tahu.
4. Pengaturan baru `agentControl` (bawaan aktif) di Umum — pengguna bisa mematikan seluruh kontrol agent.
5. **Generate gambar** di Rust (`images.rs`, perintah `image_generate`): kunci dari keychain, gaya API
   OpenAI Images (`/images/generations`), Google Gemini (`generateContent` + `responseModalities`), dan OpenRouter
   (`chat/completions` + `modalities`). Provider lain (Anthropic) → galat jujur. Hasil disimpan di folder project
   (bawaan `images/<slug>-<waktu>.png`, ekstensi mengikuti magic bytes, tidak menimpa berkas lama), lalu dibuka di
   Panel 3. Dipakai oleh dialog **Generate gambar…** (palet perintah) dan tool `zandor_image`.
6. Skill bawaan `zandor-app` (ADR-070 mekanismenya) menjelaskan tata letak, resep, dan batasnya.

## Alternatif yang ditolak

- **Bereaksi pada bagian tool di stream SSE** (UI mengeksekusi saat melihat tool part selesai): tidak bisa
  mengembalikan hasil (mis. daftar terminal) ke agent, dan berisiko mengeksekusi ulang saat riwayat dimuat.
- **Server HTTP lokal di Rust**: port tambahan + autentikasi; antrean berkas sudah terbukti untuk job.
- **Generate gambar di plugin**: plugin tidak boleh memegang kunci (guard env ADR-022).

## Akibat

- Tidak ada izin per aksi: aksi hanya tampilan/pengaturan non-keamanan; akses berkas/shell tetap lewat tool engine yang
  meminta izin. Generate gambar memakai kuota provider tanpa konfirmasi — pengguna bisa mematikan `agentControl`.
- Belum diuji ke API gambar sungguhan maupun build native; e2e hanya memverifikasi dialog di browser (tanpa Tauri).
- Ukuran hanya tiga pilihan (1024², 1536×1024, 1024×1536); Gemini/OpenRouter mengabaikan ukuran.
