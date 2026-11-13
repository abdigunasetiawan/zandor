# Keputusan (ADR ringkas) — Zandor

> Satu baris per keputusan. Setelah repo dibuat, setiap keputusan dipecah menjadi `docs/decisions/ADR-NNN-*.md`.

| # | Tanggal | Keputusan | Alasan |
|---|---|---|---|
| 001 | 13-11-2026 | Proyek dibuat ulang dari nol. Project lama hanya dipakai sebagai referensi. | Kualitas project lama tidak dipercaya: ada UI palsu, god component, dan dokumentasi yang membengkak |
| 002 | 13-11-2026 | Shell aplikasi memakai Tauri 2 + React + TypeScript | Lintas OS, ringan, berlisensi MIT/Apache |
| 003 | 13-11-2026 | GPUI dicoret. Engine dan logika dipisah total dari UI | GPUI tidak punya webview, jadi remake sama dengan menulis ulang semuanya |
| 004 | 13-11-2026 | Engine memakai `opencode serve` (sidecar lokal atau remote URL) + plugin Zandor | Sumber mesin jadi gratis, sign-in provider diwarisi, update engine terpisah dari aplikasi |
| 005 | 13-11-2026 | Target OS: Windows, macOS, Linux. Mobile nanti | Permintaan pemilik |
| 006 | 13-11-2026 | Model bisnis freemium. Inti gratis tanpa login. Berbayar nanti: model hosted dan mesin always-on | Versi 1 tidak butuh server lisensi |
| 007 | 13-11-2026 | Tidak ada OAuth langganan Anthropic. Login provider hanya lewat jalur resmi OpenCode | Anthropic melarang OAuth langganan di pihak ketiga (Feb 2026) |
| 008 | 13-11-2026 | Model gratis OpenCode Zen tidak dijadikan nilai jual sebelum ada izin tertulis | ToS OpenCode: layanan hanya untuk penggunaan internal |
| 009 | 13-11-2026 | Kode, token, dan komponen OpenBot dilarang (PolyForm Noncommercial). Folder `/ee` OpenWork juga dilarang | Aplikasi akan berbayar |
| 010 | 13-11-2026 | Tema dan ikon editor diambil dari Open VSX, bukan VS Code Marketplace | Ketentuan Marketplace membatasi penggunaan ke produk Microsoft |
| 011 | 13-11-2026 | Memori berlapis lewat antarmuka `MemoryProvider`: Markdown + SQLite FTS bawaan; Hindsight (MIT) dan Graphify (Apache-2.0) sebagai adapter opsional | Versi gratis tetap ringan, tanpa Python/Postgres wajib |
| 012 | 13-11-2026 | Browser bawaan memakai child webview Tauri (feature `unstable`, versi di-pin). Fallback ke browser sistem kalau tidak didukung (misalnya Linux Wayland) | Iframe diblokir oleh X-Frame-Options |
| 013 | 13-11-2026 | PPTX dirender best-effort | Tidak ada renderer open-source dengan fidelitas penuh |
| 014 | 13-11-2026 | Zoom UI berbasis token `--ui-scale`, bukan zoom webview | Tidak mengganggu browser bawaan; konsisten dengan token |
| 015 | 13-11-2026 | Struktur 5 panel tetap; resize bebas (`react-resizable-panels`); ada lebar minimum dan auto-collapse | Permintaan pemilik + layar laptop 1366px |
| 016 | 13-11-2026 | Versi: semver untuk rilis; build number = jumlah commit git, ditampilkan di dev sebagai `0.0.<commit>` | Auto-updater butuh semver |
| 017 | 13-11-2026 | AI pengerja: Opus 5.5 untuk rencana, arsitektur, dan review; Sonnet untuk implementasi task | Pembagian biaya dan kualitas |
| 018 | 13-11-2026 | Aturan "tidak ada UI palsu": fitur belum jadi disembunyikan atau berlabel "segera hadir" | Pelajaran dari project lama (temuan K1, K2) |
| 019 | 13-11-2026 | Spike S1: `opencode` 1.19.5 dibundel via `externalBin`, dijalankan supervisor Rust sendiri (loopback, port acak, token Basic per start, versi di-pin, Job Object di Windows). Detail: `decisions/ADR-019-spike-s1-sidecar-opencode.md` | Lulus di Windows; macOS/Linux ditunda (tidak ada perangkat); risiko yatim di macOS saat crash; binary 211 MB |
| 020 | 13-11-2026 | Spike S3: browser = child webview `browser` (Tauri `=2.12.5` + `unstable`), bounds disinkron dari UI via ResizeObserver; IPC hanya untuk webview `ui`. Detail: `decisions/ADR-020-spike-s3-child-webview-browser.md` | Lulus di Windows (otomatis + login Google manual berhasil); persistensi sesi Google belum konsisten; mac/Linux ditunda |
| 021 | 13-11-2026 | Spike S2: event SSE dipetakan dengan reducer "part utuh menang + delta untuk streaming", tipe dari SDK resmi `@opencode-ai/sdk@1.19.5`; 89 jenis event diklasifikasikan. Detail: `decisions/ADR-021-spike-s2-sse-event-mapping.md` | Lulus: reduce event nyata == `GET /session/:id/message`; transcript lengkap ter-render. Part subtask/retry/compaction & question belum terekam |
| 022 | 13-11-2026 | Spike S4: secret di keychain OS (`keyring =4.2.0`, dipecah bila > batas Windows), disuntik ke engine hanya lewat env + placeholder `{env:…}` di `OPENCODE_CONFIG_CONTENT`; plugin `zandor-env-guard` (hook `shell.env`) wajib; `OPENCODE_AUTH_CONTENT` ditolak. Detail: `decisions/ADR-022-spike-s4-secret-keychain.md` | Lulus di Windows: tanpa guard secret bocor ke model & DB lewat tool bash, dengan guard bersih; secret masih terlihat via API engine; mac/Linux ditunda |
| 023 | 13-11-2026 | Spike S6: satu plugin Zandor (bundel satu file, dimuat via `OPENCODE_CONFIG_CONTENT` + opsi) untuk memori (Markdown + FTS5 `bun:sqlite`), sematan tahan kompaksi, tool `memory_write/search`, dan pemicu skill. Detail: `decisions/ADR-023-spike-s6-memory-plugin.md` | Lulus 10/10 di opencode 1.19.5 asli; bergantung pada hook `experimental.*` (risiko upgrade); model masih mock |
