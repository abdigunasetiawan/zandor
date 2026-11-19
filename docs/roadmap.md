# Roadmap Fase — Zandor

> Draf 13-11-2026. Setiap fase punya **kriteria keluar**. Fase berikutnya tidak dimulai sebelum kriteria keluar fase sebelumnya terpenuhi.
> **MVP = Fase 0–8.** Fase 9 ke atas dikerjakan setelah MVP dipakai sendiri setiap hari.

> **Pembaruan 19-11-2026 (ADR-078):** pengembangan aktif pindah ke Rust GPUI (`apps/desktop-gpui`, lihat `docs/features/gpui.md`). Fase 0–11 tercapai di versi Tauri/React (dibekukan). Pekerjaan berikutnya adalah paritas 1:1 dan penyelesaian di Rust GPUI; kriteria "selesai" di bawah tetap berlaku dengan padanan `cargo test`/`cargo build --release` di `apps/desktop-gpui` (lihat `docs/agents.md`). Langkah "build Tauri" dan e2e Playwright hanya untuk versi React.

## Definisi "Selesai" untuk setiap task (berlaku di semua fase)

1. Kode lulus lint, typecheck, dan test (`bun test` / `vitest`, `cargo test`).
2. Ada test otomatis baru yang menguji perilaku nyata: unit dan komponen; e2e untuk alur utama.
3. Komponen UI baru terdaftar di halaman design system.
4. Dokumen fitur `docs/features/<fitur>.md` diperbarui (spek, status, cara test).
5. Pengaturan baru masuk JSON Schema dan `docs/manual/`.
6. String UI tersedia dalam ID dan EN.
7. Tidak ada data dummy di build biasa (hanya boleh di halaman design system dan di test).

---

## Fase 0 — Spike Risiko (sebelum menulis fitur)

Tujuan: membuktikan hal yang paling mungkin gagal. Hasil setiap spike dicatat sebagai ADR.

> **Diubah 13-11-2026 (ADR-027):** semua spike dinilai di **Windows** saja; macOS/Linux ditunda sampai
> aplikasi selesai. S9 dibatalkan.

| Spike | Pertanyaan                                                                                                            | Lulus jika                                                                 | Hasil (Windows)                                              |
| ----- | --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------ |
| S1    | Apakah binary `opencode` bisa dibundel sebagai sidecar lalu dijalankan, dipantau, dan dimatikan dengan bersih?        | Tidak ada proses yatim; port loopback + token; versi di-pin                | ✅ ADR-019                                                   |
| S2    | Event SSE `opencode serve`: bisakah semua part (thinking, tool, file, patch, permission) dipetakan?                   | Contoh transcript lengkap ter-render dari event nyata                      | ✅ ADR-021                                                   |
| S3    | Child webview untuk browser bawaan                                                                                    | Bisa membuka google.com dan login Google; posisinya mengikuti resize panel | ✅ ADR-020 (persistensi sesi Google belum konsisten)         |
| S4    | Secret: keychain OS (crate `keyring`) dan cara memberikan kredensial ke OpenCode tanpa file plaintext dari pihak kita | Rencana penyimpanan secret tertulis + PoC                                  | ✅ ADR-022                                                   |
| S5    | Renderer Panel 3: pdf.js, docx-preview, SheetJS, kandidat PPTX — cek lisensi dan performa                             | Tabel lisensi + demo file 10 MB                                            | ✅ ADR-026 (PPTX: keputusan MPL-2.0 tertunda sampai Fase 11) |
| S6    | Plugin OpenCode untuk memori: hook untuk system prompt, compaction, dan tool kustom                                   | Plugin bisa menyuntik blok memori dan menambah tool                        | ✅ ADR-023                                                   |
| S7    | Terminal: xterm.js + PTY                                                                                              | Shell interaktif berjalan, resize benar                                    | ✅ ADR-024 (dipilih `/pty` engine, bukan `portable-pty`)     |
| S8    | Layout: `react-resizable-panels` + child webview + zoom `--ui-scale`                                                  | Lima panel bisa di-resize tanpa jank; zoom tidak merusak layout            | ✅ ADR-025                                                   |
| S9    | ~~Kirim email ke OpenCode soal izin model gratis Zen~~                                                                | —                                                                          | ❌ Dibatalkan (ADR-027); ADR-008 tetap berlaku               |

**Kriteria keluar:** semua spike punya ADR. Kalau ada yang gagal, PRD direvisi dulu.
**Status: LULUS (13-11-2026)** untuk Windows.

**Bawaan wajib dari Fase 0 ke fase berikutnya:**

- Plugin `zandor-env-guard` (hook `shell.env`) selalu dimuat sejak engine pertama kali jalan (ADR-022).
- `SetConsoleCtrlHandler(NULL, FALSE)` saat app start di Windows (ADR-024).
- Suite test kontrak S2, S4, S6, dan S7 wajib dijalankan setiap kali versi opencode di-pin ulang.
- Build UI → build Tauri selalu berurutan; aset frontend ditanam ke binary (ADR-025).

## Fase 1 — Fondasi Repo, Docs, dan CI

- Monorepo (Bun workspaces): `apps/desktop` (Tauri + React), `packages/ui`, `packages/core` (client engine dan state), `packages/plugin` (plugin OpenCode), `packages/i18n`.
- Struktur docs ramping (lihat lampiran) + `AGENTS.md`.
- Lint dan format (Biome atau ESLint + Prettier), TypeScript strict, Clippy.
- Test: Vitest + Testing Library; e2e Playwright terhadap UI dengan engine tiruan untuk test; `cargo test`.
- CI GitHub Actions: lint, test, audit lisensi, dan build untuk **Windows** (ADR-027).
- Infrastruktur i18n (ID/EN, key bertipe), error boundary, logger, dan nomor build dari jumlah commit.

**Kriteria keluar:** aplikasi kosong bisa di-build di Windows lewat CI, dan test berjalan.
**Status: LULUS (13-11-2026)** — CI Windows hijau (lint, typecheck, Vitest, e2e Edge, Clippy, cargo test, audit lisensi JS+Rust, build NSIS).

## Fase 2 — Design System

- Token (warna, tipografi per variabel, spacing, radius, shadow, `--ui-scale`). Default monokrom dark + light.
- Plus Jakarta Sans + font kode + font terminal.
- Atoms: Button, IconButton, Input, Textarea, Select, Switch, Checkbox, Slider, Badge, Avatar, Kbd, Tooltip, Spinner, Skeleton.
- Molecules: Menu, ContextMenu, Popover, Tabs, Toast, Dialog/Confirm kustom, Combobox, SearchField.
- Sistem shortcut: registry global + scoped berdasarkan fokus panel, popup, atau view.
- Halaman design system: Tokens → Atoms → Molecules → Organisms → Panels → Menus → Modals → Pages.

**Kriteria keluar:** semua komponen ada di halaman design system dengan test, dan tidak ada warna atau ukuran yang di-hardcode.

## Fase 3 — App Shell

- Jendela frameless + kontrol jendela kustom; top bar (back/next, menu, input serba guna/command palette, toggle panel).
- Lima panel: resize, lebar minimum, collapse, auto-collapse, layout tersimpan.
- Zoom UI `Ctrl +/−/0`.
- Panel 5 (rail) memicu Panel 4.
- Kerangka Pengaturan (dialog/halaman) berbasis JSON Schema.

**Kriteria keluar:** shell lengkap tanpa fitur palsu; panel yang belum punya isi menampilkan empty state yang jujur.
**Status: selesai (13-11-2026)** — lihat ADR-030 dan `docs/features/app-shell.md`; menunggu CI hijau.

## Fase 4 — Engine & Project

- Supervisor engine di Rust (start, health check, restart, mati bersama aplikasi).
- Connection manager: lokal (default) + struktur untuk remote URL.
- Project: tambah folder, daftar, hapus, kustomisasi dasar. Sesi: buat, rename, hapus, switch; sesi tanpa project.
- Panel 1 berfungsi penuh: daftar project, daftar chat, pencarian, footer user + versi.
- Tab Files: pohon file nyata dari project aktif.

**Kriteria keluar:** project dan sesi persisten di engine; restart aplikasi tidak menghilangkan data.
**Status: selesai di lokal (13-11-2026)** — terbukti di smoke native (restart aplikasi → sesi tetap); lihat ADR-032 dan `docs/features/projects.md`. Menunggu CI hijau.

## Fase 5 — Chat Inti

- Streaming via SSE; transcript divirtualisasi.
- Part ter-render: teks/markdown, thinking, tool call, file yang dibaca/diubah (diff), perintah terminal + output, web search.
- Stop/abort, kartu izin (sekali / selalu / tolak), revert, regenerate, cabang sesi, copy.
- Baris meta: model, provider, durasi, waktu (tooltip lengkap).
- Composer: lampirkan file/folder, mode izin, pilih model + reasoning (versi dasar), kirim/stop.
- Tab sesi di atas Panel 2.

**Kriteria keluar:** tool call nyata (baca/tulis file, terminal) berjalan dari chat dengan persetujuan izin, dan ada test e2e-nya.
**Status: selesai (13-11-2026)** — inti (ADR-033) dan pelengkapnya (revert, regenerate, cabang sesi, lampiran, mode izin/reasoning, tab sesi, web search, salin, virtualisasi; ADR-034). CI Fase 5 hijau.

## Fase 6 — Provider & Model

- Tambah koneksi: API key (daftar provider dari OpenCode), sign-in via OpenCode, custom endpoint OpenAI-compatible, lokal (Ollama, LM Studio, auto-detect).
- Secret disimpan sesuai hasil S4.
- Model picker penuh: autofocus ke pencarian, Semua provider, Disematkan, grouping nama/provider, ping model.
- Status provider yang jujur (rate limit, error, kuota), tidak ada spinner tanpa akhir.

**Kriteria keluar:** bisa chat dengan model lokal, model custom endpoint, dan model hasil sign-in.
**Status: selesai di lokal dengan batasan (13-11-2026)** — koneksi, keychain, katalog, OAuth, pemilih model, dan status jujur selesai. Chat dengan custom endpoint terbukti (Fase 5); model lokal dan sign-in akun sungguhan belum diuji di mesin pengembang. Lihat ADR-034 dan `docs/features/providers.md`.

## Fase 7 — Panel 3: Viewer Dokumen

- Tab dokumen, path, toolbar lengkap (render/raw, cari, wrap, copy isi/path, unduh, buka di aplikasi lain, reload, tutup).
- Monaco (read-only dulu; edit menyusul sebagai task terpisah), Markdown + Mermaid + KaTeX, gambar, PDF, CSV/JSON, HTML sandbox.
- Klik file di chat atau di Files akan membuka file itu di Panel 3.

**Kriteria keluar:** semua format "Penuh" di PRD ter-render dan ada test snapshot-nya.
**Status: selesai di lokal (14-11-2026)** — semua format "Penuh" ter-render dan teruji (snapshot ARIA + verifikasi piksel/DOM, e2e, smoke native); belum: edit, DOCX/XLSX/PPTX. (Teks PDF bisa dipilih/dicari sejak 17-11-2026.) Lihat ADR-035 dan `docs/features/documents.md`.

## Fase 8 — Panel 4: Tools

- Terminal (xterm + PTY, multi-tab, mengikuti project aktif).
- Git: status, diff, stage, commit, branch.
- Browser: child webview, default google.com, navigasi; fallback ke browser sistem.
- Info sesi: penggunaan konteks/token, model, biaya kalau tersedia.
- Log aktivitas: event agent dan event aplikasi.

**Kriteria keluar: MVP selesai.** Kriteria sukses MVP di PRD §13 terpenuhi → versi `0.1.0` (alpha pribadi).
**Status: fitur Fase 8 selesai di lokal (14-11-2026)** — terminal, Git, browser, info sesi, log aktivitas terbukti di aplikasi asli (smoke 30/30). **Audit PRD §13 selesai (14-11-2026): kelima kriteria terpenuhi** (lihat `docs/features/mvp.md`); versi `0.1.0` menunggu keputusan pemilik. Lihat ADR-036 dan `docs/features/tools.md`.

---

## Fase 9 — Memori & Skill Creator

- Opt-in saat membuat project atau sesi, dan bisa diubah di pengaturan.
- `MemoryProvider` bawaan: blok Markdown (project + global) + pencarian SQLite FTS5 lewat plugin OpenCode.
- Sematkan ke konteks (tahan kompaksi), simpan sebagai catatan.
- Skill creator: pemicu otomatis (koreksi user, jumlah tool call, `/reflect`), status draft, antrean persetujuan, pemindai secret.
- UI: kelola memori dan skill (lihat, edit, hapus).
- Adapter opsional: Hindsight (MCP/REST), Graphify (`graph.json`).

**Status: fitur inti Fase 9 selesai di lokal (14-11-2026)** — plugin memori/skill, antrean persetujuan, UI, opt-in per sesi/project; terbukti di engine asli dan aplikasi asli. Belum: adapter Hindsight/Graphify, opt-in di dialog Tambah project. (Sematkan ke konteks sudah ada lewat menu catatan pesan.) Lihat ADR-037 dan `docs/features/memory.md`.

## Fase 10 — Kustomisasi

- Engine tema + preset (Gemini, Claude, ChatGPT, Discord, Vercel, Raycast, dll); impor/ekspor tema.
- Tipografi per variabel; kustomisasi panel chat (opasitas, ketebalan, ukuran, spasi, font).
- Kustomisasi section project di sidebar (border, logo, background).
- Tema editor + paket ikon dari Open VSX; editor shortcut.

**Status: fitur Fase 10 selesai di lokal (14-11-2026)** — engine tema + 7 preset, impor/ekspor, tipografi, panel chat, section project, tema editor (berkas + Open VSX), editor shortcut. Belum: paket ikon file Open VSX. Lihat ADR-038 dan `docs/features/appearance.md`.

## Fase 11 — Panel 3 Lanjutan & Fitur Ekstra

- DOCX, XLSX, ipynb, PPTX (best-effort); edit file di Monaco.
- Saran prompt dinamis, save as (gambar/svg/md/txt), input suara.
- Automasi terjadwal yang nyata, notifikasi.
- Remote engine (VPS, komputer lain) beserta UI pengelolanya.
- Model peran (memori, image generator).
- Manual "agent bisa mengonfigurasi Zandor": skema pengaturan + skill bawaan.

**Status: Fase 11 sebagian selesai di lokal (14-11-2026)** — edit file di Monaco, DOCX/XLSX/PPTX/ipynb, saran prompt dinamis, automasi terjadwal + notifikasi. Input suara (dikte), notifikasi, dan save-as pesan menyusul (ADR-045, ADR-050). Engine remote lewat URL per project (ADR-076) dan model kecil (`small_model`) selesai 17-11-2026. Generate gambar (model gambar bawaan, dialog + tool `zandor_image`) dan skill bawaan `zandor-app` + tool `zandor_app` agar agent mengoperasikan/mengonfigurasi Zandor (ADR-077) selesai 17-11-2026. **Belum:** model memori (belum ada pemanggilan model oleh plugin memori); uji fitur ADR-076/077 dengan model, API, dan build native sungguhan.

## Fase 12 — Rilis Publik (Windows)

- Code signing Windows (aplikasi + sidecar opencode).
- Auto-updater (semver), Third-Party Notices otomatis, crash report opt-in.
- Website `zandor.web.id`, akun opsional, persiapan paket berbayar.

## Fase 13 — Port macOS & Linux (setelah aplikasi selesai, ADR-027)

- Jalankan ulang suite spike S1, S3, S4, S7, dan S8 di macOS dan Linux; tutup risiko yang sudah diketahui:
  proses yatim saat crash di macOS (ADR-019), WKWebView/WebKitGTK + fallback Wayland (ADR-020),
  Secret Service di Linux headless (ADR-022), PTY (ADR-024), HiDPI (ADR-025).
- Notarisasi macOS, paket Linux (AppImage/deb/rpm), CI matrix tiga OS.

---

## Lampiran — Struktur docs yang lebih ramping

```
AGENTS.md                 → pointer ke docs/agents.md
docs/
  README.md               peta dokumen
  agents.md               aturan agent + urutan baca
  prd.md                  produk
  roadmap.md              fase + checklist aktif (fase selesai diringkas)
  architecture.md         arsitektur + kontrak engine
  rules.md                konvensi kode
  design.md               preferensi visual
  decisions/ADR-NNN-*.md  satu keputusan per file
  features/<fitur>.md     spek + status + cara test per fitur
  manual/                 manual user + referensi skema pengaturan (bisa dibaca agent)
  context/current.md      snapshot kondisi saat ini (selalu ditimpa)
  changelog.md            dihasilkan dari commit
```
Aturan: setiap file maksimal ±30 KB. Kalau lebih, pecah atau arsipkan. Agent cukup membaca `agents.md → context/current.md → roadmap.md` di awal sesi.
