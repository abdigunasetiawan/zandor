# PRD v2 — Zandor (nama kerja)

> Status: **Draf disepakati — 13-11-2026**. Menggantikan PRD "XYZ" di project lama.
> Nama "Zandor" masih menunggu cek GitHub, DJKI, dan USPTO. Domain: `zandor.web.id`.

> **Pembaruan 19-11-2026 (ADR-078):** pengembangan aktif pindah ke Rust GPUI (`apps/desktop-gpui`, lihat `docs/features/gpui.md`). Produk, prinsip, dan scope tidak berubah; yang berubah adalah teknologi UI (native Rust GPUI menggantikan Tauri + webview).

## 1. Ringkasan

Zandor adalah aplikasi desktop _agentic harness_ untuk vibecoder dan pekerja yang memakai banyak AI. Aplikasi ini bisa memakai AI apa saja (BYOK, lokal, maupun sign-in provider), bisa mengerjakan banyak project sekaligus, dan punya 5 panel. Memori dan skill creator bersifat opsional, dinyalakan per project atau per sesi. Zandor dipakai untuk ngoding sekaligus untuk pekerjaan harian lain.

## 2. Masalah

- Harness dari vendor mengunci pengguna ke satu provider.
- OpenCode terbuka, tapi UI-nya kurang nyaman untuk multi-project.
- OpenChamber fiturnya banyak, tapi UI-nya kurang disukai.
- OpenWork tampilannya bagus, tapi fitur ngodingnya kurang.
- Belum ada harness terbuka yang punya **memori jangka panjang** dan **skill creator otomatis** sebagai fitur kelas satu.

## 3. Target User

1. Vibecoder solo yang memakai lebih dari satu provider atau model, termasuk router gratis dan model lokal.
2. Developer yang bekerja di beberapa repo sekaligus.
3. Pekerja non-coding yang butuh agent harian: riset, dokumen, otomasi.

## 4. Prinsip Produk (tidak bisa ditawar)

1. **Tidak ada UI palsu.** Fitur yang belum jadi disembunyikan atau diberi label "segera hadir", tidak boleh dipalsukan dengan data dummy.
2. **Tidak ada vendor lock-in.** Semua provider setara.
3. **Local-first.** Data, memori, dan sesi ada di mesin engine milik user.
4. **Semuanya komponen.** Komponen terkecil pun reusable dan terdaftar di halaman design system. Tidak ada dialog bawaan OS (`alert`/`confirm`).
5. **Keyboard-first.** Semua aksi punya shortcut, dibagi menjadi global dan scoped.
6. **Dokumentasi tersinkron.** Setiap fitur yang selesai wajib punya test otomatis, dokumen fitur, dan entri manual.
7. **Agent bisa mengonfigurasi aplikasi.** Semua pengaturan punya skema (JSON Schema) yang terdokumentasi.
8. **Lisensi bersih.** Hanya dependensi berlisensi permisif (MIT, Apache-2.0, BSD, OFL, dan sejenisnya). Tidak boleh ada kode atau token dari sumber noncommercial.

## 5. Model Bisnis

- **Freemium.** Aplikasi inti gratis dan tidak wajib login.
- **Berbayar (nanti):** akses model yang di-host, mesin always-on yang disewakan, dan memori Hindsight terkelola.
- Versi 1 tidak butuh server lisensi. Akun bersifat opsional dan ditambahkan di fase akhir.
- Batasan legal:
  - Tidak ada OAuth langganan Anthropic (dilarang untuk pihak ketiga).
  - Login provider hanya lewat jalur yang disediakan OpenCode secara resmi.
  - Model gratis OpenCode Zen tidak dijadikan nilai jual sebelum ada izin tertulis dari OpenCode (help@anoma.ly).
  - Tema dan ikon editor diambil dari **Open VSX**, bukan VS Code Marketplace.

## 6. Platform

- Desktop: **Windows (x64) dulu** sampai aplikasi selesai. macOS (x64/arm64) dan Linux menyusul di fase port (ADR-027).
- Mobile: nanti, di luar scope PRD ini.
- Remake dengan GPUI: **dicoret.** Sebagai gantinya, engine dan logika dipisah total dari UI.

## 7. Arsitektur (ringkas)

```
┌──────────── Zandor Desktop (Tauri 2) ────────────┐
│ UI: React + TypeScript (5 panel)                 │
│ Rust core: supervisor engine, keychain, PTY,     │
│   child webview (browser), file I/O terkontrol   │
└──────────────┬───────────────────────────────────┘
               │ HTTP + SSE
┌──────────────▼──────────────┐
│ Engine: `opencode serve`    │ ← lokal (sidecar, default) / VPS / komputer lain
│ + plugin Zandor (TS):       │
│   memori, skill creator     │
└─────────────────────────────┘
```

- **Sumber mesin** (lokal, VPS, komputer lain) = ganti URL dan token server. Memori dan skill ikut tinggal di mesin engine.
- **Sign in provider** diwarisi dari mekanisme auth OpenCode, tidak dibuat ulang.
- **MemoryProvider** adalah antarmuka yang bisa ditukar: Markdown + SQLite (bawaan), Hindsight (adapter opsional), Graphify (adapter peta kode opsional).

## 8. Layout UI

**Top bar:** tombol back/next, menu (Berkas, Edit, Tampilan, Bantuan, Dev Tools), input serba guna di tengah (command palette ala VS Code), toggle cepat 5 panel, dan kontrol jendela kustom.

**Struktur 5 panel tetap.** Semua panel bisa di-resize bebas dan punya lebar minimum. Panel 1, 3, dan 4 bisa collapse, dan ada auto-collapse saat lebar chat di bawah batas minimum. Layout disimpan.

**Zoom skala UI:** berbasis token `--ui-scale` (75–150%, `Ctrl +/−/0`). Zoom ini tidak ikut memperbesar halaman di browser bawaan.

| Panel            | Isi                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 — Sidebar kiri | Brand, pencarian, notifikasi, tab Chat/Files, tombol Sesi baru dan Automasi, section Project (bisa dikustom: border, logo, background), section Chat (sesi tanpa project), dan footer user (foto, nama, versi, menu ⋯: Profil, Tema, Pengaturan, Keluar)                                                                                                                                                                                                                    |
| 2 — Chat         | Tab sesi (logo model + judul); bubble user (copy, revert); bubble agent dengan proses yang terlihat (thinking, file yang dibaca, tool call, perintah terminal, web search); baris meta (model, provider, durasi, waktu dengan tooltip lengkap); aksi utama copy, regenerate, cabang, dan menu ⋯ (copy link, save as image/svg/md/txt, simpan catatan, sematkan ke konteks); badge saran prompt dinamis; composer (file, folder, izin, model + reasoning, suara, kirim/stop) |
| 3 — Dokumen      | Tab file; path; toolbar (render/raw, cari, wrap, copy isi, copy path, unduh, buka di aplikasi lain, reload, tutup); viewer penuh                                                                                                                                                                                                                                                                                                                                            |
| 4 — Tools        | Terminal, Git, Browser (default google.com), Info sesi/konteks, Log aktivitas                                                                                                                                                                                                                                                                                                                                                                                               |
| 5 — Rail kanan   | Ikon-ikon pemicu Panel 4                                                                                                                                                                                                                                                                                                                                                                                                                                                    |

**Renderer Panel 3:**

- **Penuh:** kode (Monaco), Markdown, Mermaid, KaTeX, HTML sandbox, SVG, CSV/TSV, JSON, gambar, audio, video, PDF.
- **Bagus:** DOCX, XLSX, `.ipynb`.
- **Best-effort:** PPTX.

## 9. Fitur per Modul (M = MVP, L = setelah MVP)

| Modul          | Fitur                                                                                                                            | Prioritas                   |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| Provider       | BYOK API key, custom endpoint (OpenAI-compatible), lokal (Ollama, LM Studio), sign in via OpenCode                               | M                           |
| Provider       | Model peran: chat, kecil, memori, image generator                                                                                | L                           |
| Model picker   | Autofocus ke pencarian, section Semua/Disematkan, ping model, filter cerdas, grouping nama/provider                              | M (ping & filter cerdas: L) |
| Chat           | Streaming, stop, izin tool, proses terlihat, revert, regenerate, cabang, copy                                                    | M                           |
| Chat           | Sematkan ke konteks, simpan catatan, save as, saran prompt dinamis, input suara                                                  | L                           |
| Project & sesi | Multi-project, multi-sesi, sesi tanpa project                                                                                    | M                           |
| Panel 3        | Viewer kode, Markdown, gambar, PDF                                                                                               | M                           |
| Panel 3        | DOCX, XLSX, ipynb, PPTX, buka di aplikasi lain                                                                                   | L                           |
| Panel 4        | Terminal, Git dasar, Browser, Info sesi, Log aktivitas                                                                           | M                           |
| Memori & skill | Opt-in saat membuat project/sesi, memori project + jangka panjang, skill creator otomatis/trigger dengan antrean persetujuan     | L (fase khusus)             |
| Kustomisasi    | Tema default monokrom + preset (Gemini, Claude, ChatGPT, Discord, Vercel, Raycast, dll), font per variabel, tipografi panel chat | L                           |
| Editor         | Tema editor (bawaan, impor, Open VSX), paket ikon file                                                                           | L                           |
| Shortcut       | Global + scoped, editor shortcut                                                                                                 | M (inti), L (editor)        |
| Bahasa         | Indonesia + Inggris                                                                                                              | M                           |
| Design system  | Halaman dokumentasi UI: Tokens → Atoms → Molecules → Organisms → Panels → Menus → Modals → Pages                                 | M                           |
| Automasi       | Tugas terjadwal yang benar-benar jalan                                                                                           | L                           |
| Sumber mesin   | Lokal (default), remote URL                                                                                                      | M (lokal), L (remote)       |

## 10. Tema & Tipografi Default

- **Dark:** `#121212`, `#141414`, `#1f1f1f`, monokrom tanpa warna primary.
- **Light:** padanan monokrom.
- **Font:** Plus Jakarta Sans untuk UI (OFL); font kode dan font terminal bisa diatur.
- **Ketebalan per variabel:** h1–h4, p, link, tombol, code, dan lain-lain, semua lewat token.

## 11. Non-Fungsional

- Jendela tampil < 1 detik; engine siap < 2 detik (lokal).
- Streaming tidak menurunkan frame; transcript divirtualisasi.
- Secret disimpan di keychain OS, tidak plaintext di file aplikasi.
- Izin agent tidak bisa dilewati tanpa persetujuan (kecuali mode yang dipilih user secara sadar).
- Satu error render tidak boleh memutihkan seluruh aplikasi (error boundary per panel).
- Aksesibilitas dasar: kontras, fokus terlihat, label.

## 12. Di Luar Scope

- Mobile.
- GPUI.
- Sinkronisasi cloud.
- Marketplace skill publik.
- Login Anthropic.

## 13. Kriteria Sukses MVP

1. Bisa menambah custom provider/model dan chat dengan streaming.
2. Kelima panel berfungsi nyata, tanpa data dummy.
3. Tool call (baca/tulis file, terminal, web) berjalan dan terlihat di chat, termasuk persetujuan izin.
4. Bisa membuka minimal 3 project dan 10 sesi tanpa crash.
5. Setiap fitur MVP punya test otomatis dan dokumen fitur.
