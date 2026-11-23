# ADR-083 — Langganan Claude, ChatGPT, dan Gemini lewat agent CLI (ACP)

Tanggal: 23-11-2026 · Status: diterima (teruji langsung dan di jendela aplikasi build release) · Menambah: ADR-054/055 (provider)

## Konteks

Pemilik ingin memakai langganan Claude (Pro/Max), ChatGPT (Plus/Pro), dan Gemini di Zandor, seperti t3code
(`reference/t3code`). Langganan ini tidak bisa dipakai lewat kunci API, dan mengambil token OAuth sendiri melanggar
ketentuan layanan serta rapuh. Cara t3code: aplikasi tidak memegang token. Ia menjalankan CLI resmi yang sudah login di
mesin pengguna dan berbicara dengannya lewat protokol terstruktur (Agent Client Protocol, codex app-server, Claude Agent
SDK). CLI itu yang mengautentikasi dan menagih kuota langganan.

## Keputusan

1. **ACP sebagai satu protokol** untuk ketiga agent: JSON-RPC 2.0 NDJSON lewat stdio (`core/acp/rpc.rs`).
   - Claude Code: adaptor resmi `@agentclientprotocol/claude-agent-acp@0.109.1`.
   - Codex: adaptor resmi `@agentclientprotocol/codex-acp@2.9.0`. Versi 0.16 milik zed-industries terlalu tua untuk
     model akun, jadi tidak dipakai.
   - Gemini: Antigravity `agy acp`, server ACP bawaan CLI.

   Versi adaptor dikunci di `core/acp/agents.rs`. Adaptor dipasang ke `<data>/acp/tools/<id>@<versi>` lewat
   `npm install` hanya bila pengguna menekan **Pasang adaptor**. Adaptor global (`npm i -g`) dan perintah kustom
   (`acp/agents.json`) juga diterima.
2. **Tanpa kredensial di Zandor.** Lingkungan proses agent dibersihkan dari kunci ambien, misalnya `ANTHROPIC_API_KEY`,
   `OPENAI_API_KEY`, dan `GEMINI_API_KEY` yang mungkin terwarisi dari konfigurasi OpenCode. Tujuannya agar agent memakai
   login langganan. Galat autentikasi (-32000) diterjemahkan menjadi petunjuk perintah login (`claude auth login`,
   `codex login`, `agy`).
3. **Host ACP di balik `EngineApi`** (`core/acp/host.rs`). OpenCode tetap pemilik sesi: sidebar, judul, dan hapus.
   - Model agent muncul di pemilih model sebagai provider `acp-claude`, `acp-codex`, dan `acp-antigravity`. Daftar model
     diambil dari `configOptions` (kategori model) atau `models.availableModels`, lalu di-cache di `acp/models.json`.
   - Giliran dengan model ACP dikirim ke agent. Transkripnya disimpan lokal dalam bentuk pesan opencode di
     `<data>/acp/sessions/<id>.json` dan digabung saat sesi dimuat. ID pesan menaik bergaya opencode, sehingga urutan
     campuran tetap benar.
   - Pembaruan ACP dipetakan ke bagian yang sudah ada (`core/acp/convert.rs`): teks, reasoning, alat (read/edit/bash/
     grep/webfetch, diff berkas, keluaran terminal), plan → daftar tugas, dan usage → token.
   - `session/request_permission` dijawab lewat kartu izin yang sudah ada. Mode izin Zandor dipetakan ke mode agent.
   - Varian reasoning dikirim sebagai `thought_level`.
   - Batal = `session/cancel`. Ringkas = `/compact`. Perintah slash diteruskan.
4. **Satu proses per (agent, folder kerja).** Antigravity menulis berkas relatif ke folder kerja prosesnya, bukan ke `cwd`
   sesi. Proses dibuat malas dan dimatikan saat agent dinonaktifkan.
5. **Ganti model di tengah sesi.** Riwayat yang belum dilihat model baru dioper sebagai konteks: blok
   `<riwayat_percakapan>` ke agent ACP, atau catatan sintetis ke OpenCode.
6. **UI** (`shell/acp_section.rs`) ada di Pengaturan > Model (di atas Provider) dan di langkah provider Onboarding.
   - Isinya: status jujur per agent (CLI tidak ditemukan, adaptor belum dipasang, Node.js tidak ada, perlu login, galat,
     siap + jumlah model), sakelar aktif (bawaan mati), Pasang adaptor, Uji koneksi, dan Salin perintah login.
   - Mengaktifkan agent mengambil daftar model, lalu memberi tahu pemilih model lewat `EngineStatus::models_changed`.
   - Ping model ACP = satu giliran pendek lewat agent.
   - Label provider di pemilih model dan Kelola model: "Agent CLI (langganan)".
7. **Judul sesi.** OpenCode tidak melihat giliran ACP, jadi tidak membuat judul. Pesan pertama ACP memberi judul
   sementara (rename di OpenCode, hanya bila judul masih bawaan). Event `session.updated` menandainya sebagai judul
   otomatis, sehingga penamaan ulang Zandor (autotitle) tetap bisa memperbaikinya.

## Konsekuensi dan batasan

- Penggunaan memotong kuota langganan masing-masing layanan, sesuai ketentuan CLI resmi. Ping dan uji koneksi juga
  memakai kuota, walau kecil.
- Node.js dan npm dibutuhkan untuk Claude Code dan Codex.
- Unrevert tidak berlaku untuk pesan ACP. Rewind atau hapus memotong transkrip lokal dan membuka sesi agent baru dengan
  konteks dioper ulang. Urutan fork di sesi campuran (OpenCode + ACP) belum sempurna.
- Kompaksi otomatis Zandor dilewati untuk model ACP, karena agent mengelola konteksnya sendiri.
- Agent memakai alat bawaannya sendiri. `clientCapabilities.fs` dan `terminal` sengaja `false`.

## Verifikasi

Uji `live_acp` (`#[ignore]`, `ZANDOR_ACP_LIVE=<agent>`) dengan agent sungguhan, akun pemilik, 23-11-2026. Ketiga agent
lulus: membuat berkas lewat izin di folder kerja, lalu menjawab benar di giliran kedua (konteks tersambung).

| Agent | Model | Waktu |
| --- | --- | --- |
| Claude Code | sonnet, 14 model | 29 dtk |
| Codex | gpt-5.6-luna, 3 model | 16 dtk |
| Antigravity | – | lulus |

**Di jendela aplikasi** (build release, data asli pemilik, 23-11-2026):

- Pengaturan > Model > Agent CLI: Pasang adaptor Claude Code dan Codex lewat tombol berhasil (status "Memasang…" lalu
  "Belum diuji"). Sakelar aktif mengambil model: Claude 14, Codex 3, Antigravity 9.
- Pemilih model menampilkan ketiga provider dengan logo.
- Satu sesi berganti model tiga kali (Claude Haiku 5.5 → Codex 5.6 Luna → Gemini 3.8 Flash). Tiap model menjawab
  benar tentang model sebelumnya, jadi konteks dioper.
- Setelah aplikasi dibuka ulang, agent tetap aktif, dan langkah provider di Onboarding menampilkan bagian agent.
- Proses agent berhenti saat aplikasi ditutup.
- Diperbaiki setelah uji jendela: teks judul bagian tidak terbungkus (tombol "Periksa ulang" terdorong keluar, juga
  "Tambah koneksi" di bagian Provider); judul sesi ACP tetap "Sesi tanpa judul"; label provider "Provider katalog".
