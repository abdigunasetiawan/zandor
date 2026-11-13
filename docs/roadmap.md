# Roadmap Fase — Zandor

> Draf 13-11-2026. Setiap fase punya **kriteria keluar**. Fase berikutnya tidak dimulai sebelum kriteria keluar fase sebelumnya terpenuhi.
> **MVP = Fase 0–8.** Fase 9 ke atas dikerjakan setelah MVP dipakai sendiri setiap hari.

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

| Spike | Pertanyaan | Lulus jika |
|---|---|---|
| S1 | Apakah binary `opencode` bisa dibundel sebagai sidecar di Win/mac/Linux dan dijalankan, dipantau, lalu dimatikan dengan bersih? | Tidak ada proses yatim; port loopback + token; versi di-pin |
| S2 | Event SSE `opencode serve`: bisakah semua part (thinking, tool, file, patch, permission) dipetakan? | Contoh transcript lengkap ter-render dari event nyata |
| S3 | Child webview untuk browser bawaan di Win/mac/Linux X11; fallback di Wayland | Bisa membuka google.com dan login Google; posisinya mengikuti resize panel |
| S4 | Secret: keychain OS (crate `keyring`) dan cara memberikan kredensial ke OpenCode tanpa file plaintext dari pihak kita | Rencana penyimpanan secret tertulis + PoC |
| S5 | Renderer Panel 3: pdf.js, docx-preview, SheetJS, kandidat PPTX — cek lisensi dan performa | Tabel lisensi + demo file 10 MB |
| S6 | Plugin OpenCode untuk memori: hook untuk system prompt, compaction, dan tool kustom | Plugin bisa menyuntik blok memori dan menambah tool |
| S7 | Terminal: xterm.js + PTY (`portable-pty`) di tiga OS | Shell interaktif berjalan, resize benar |
| S8 | Layout: `react-resizable-panels` + child webview + zoom `--ui-scale` | Lima panel bisa di-resize tanpa jank; zoom tidak merusak layout |
| S9 | Kirim email ke OpenCode soal izin model gratis Zen | Jawaban tercatat (boleh berjalan paralel) |

**Kriteria keluar:** semua spike punya ADR. Kalau ada yang gagal, PRD direvisi dulu.

## Fase 1 — Fondasi Repo, Docs, dan CI
- Monorepo (Bun workspaces): `apps/desktop` (Tauri + React), `packages/ui`, `packages/core` (client engine dan state), `packages/plugin` (plugin OpenCode), `packages/i18n`.
- Struktur docs ramping (lihat lampiran) + `AGENTS.md`.
- Lint dan format (Biome atau ESLint + Prettier), TypeScript strict, Clippy.
- Test: Vitest + Testing Library; e2e Playwright terhadap UI dengan engine tiruan untuk test; `cargo test`.
- CI GitHub Actions: lint, test, dan build untuk ketiga OS.
- Infrastruktur i18n (ID/EN, key bertipe), error boundary, logger, dan nomor build dari jumlah commit.

**Kriteria keluar:** aplikasi kosong bisa di-build di 3 OS lewat CI, dan test berjalan.

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

## Fase 4 — Engine & Project
- Supervisor engine di Rust (start, health check, restart, mati bersama aplikasi).
- Connection manager: lokal (default) + struktur untuk remote URL.
- Project: tambah folder, daftar, hapus, kustomisasi dasar. Sesi: buat, rename, hapus, switch; sesi tanpa project.
- Panel 1 berfungsi penuh: daftar project, daftar chat, pencarian, footer user + versi.
- Tab Files: pohon file nyata dari project aktif.

**Kriteria keluar:** project dan sesi persisten di engine; restart aplikasi tidak menghilangkan data.

## Fase 5 — Chat Inti
- Streaming via SSE; transcript divirtualisasi.
- Part ter-render: teks/markdown, thinking, tool call, file yang dibaca/diubah (diff), perintah terminal + output, web search.
- Stop/abort, kartu izin (sekali / selalu / tolak), revert, regenerate, cabang sesi, copy.
- Baris meta: model, provider, durasi, waktu (tooltip lengkap).
- Composer: lampirkan file/folder, mode izin, pilih model + reasoning (versi dasar), kirim/stop.
- Tab sesi di atas Panel 2.

**Kriteria keluar:** tool call nyata (baca/tulis file, terminal) berjalan dari chat dengan persetujuan izin, dan ada test e2e-nya.

## Fase 6 — Provider & Model
- Tambah koneksi: API key (daftar provider dari OpenCode), sign-in via OpenCode, custom endpoint OpenAI-compatible, lokal (Ollama, LM Studio, auto-detect).
- Secret disimpan sesuai hasil S4.
- Model picker penuh: autofocus ke pencarian, Semua provider, Disematkan, grouping nama/provider, ping model.
- Status provider yang jujur (rate limit, error, kuota), tidak ada spinner tanpa akhir.

**Kriteria keluar:** bisa chat dengan model lokal, model custom endpoint, dan model hasil sign-in.

## Fase 7 — Panel 3: Viewer Dokumen
- Tab dokumen, path, toolbar lengkap (render/raw, cari, wrap, copy isi/path, unduh, buka di aplikasi lain, reload, tutup).
- Monaco (read-only dulu; edit menyusul sebagai task terpisah), Markdown + Mermaid + KaTeX, gambar, PDF, CSV/JSON, HTML sandbox.
- Klik file di chat atau di Files akan membuka file itu di Panel 3.

**Kriteria keluar:** semua format "Penuh" di PRD ter-render dan ada test snapshot-nya.

## Fase 8 — Panel 4: Tools
- Terminal (xterm + PTY, multi-tab, mengikuti project aktif).
- Git: status, diff, stage, commit, branch.
- Browser: child webview, default google.com, navigasi; fallback ke browser sistem.
- Info sesi: penggunaan konteks/token, model, biaya kalau tersedia.
- Log aktivitas: event agent dan event aplikasi.

**Kriteria keluar: MVP selesai.** Kriteria sukses MVP di PRD §13 terpenuhi → versi `0.1.0` (alpha pribadi).

---

## Fase 9 — Memori & Skill Creator
- Opt-in saat membuat project atau sesi, dan bisa diubah di pengaturan.
- `MemoryProvider` bawaan: blok Markdown (project + global) + pencarian SQLite FTS5 lewat plugin OpenCode.
- Sematkan ke konteks (tahan kompaksi), simpan sebagai catatan.
- Skill creator: pemicu otomatis (koreksi user, jumlah tool call, `/reflect`), status draft, antrean persetujuan, pemindai secret.
- UI: kelola memori dan skill (lihat, edit, hapus).
- Adapter opsional: Hindsight (MCP/REST), Graphify (`graph.json`).

## Fase 10 — Kustomisasi
- Engine tema + preset (Gemini, Claude, ChatGPT, Discord, Vercel, Raycast, dll); impor/ekspor tema.
- Tipografi per variabel; kustomisasi panel chat (opasitas, ketebalan, ukuran, spasi, font).
- Kustomisasi section project di sidebar (border, logo, background).
- Tema editor + paket ikon dari Open VSX; editor shortcut.

## Fase 11 — Panel 3 Lanjutan & Fitur Ekstra
- DOCX, XLSX, ipynb, PPTX (best-effort); edit file di Monaco.
- Saran prompt dinamis, save as (gambar/svg/md/txt), input suara.
- Automasi terjadwal yang nyata, notifikasi.
- Remote engine (VPS, komputer lain) beserta UI pengelolanya.
- Model peran (memori, image generator).
- Manual "agent bisa mengonfigurasi Zandor": skema pengaturan + skill bawaan.

## Fase 12 — Rilis Publik
- Code signing Windows, notarisasi macOS, paket Linux (AppImage/deb/rpm).
- Auto-updater (semver), Third-Party Notices otomatis, crash report opt-in.
- Website `zandor.web.id`, akun opsional, persiapan paket berbayar.

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
