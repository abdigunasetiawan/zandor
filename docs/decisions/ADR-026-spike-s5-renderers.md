# ADR-026 — Spike S5: renderer Panel 3 (lisensi & performa)

- **Tanggal:** 13-11-2026
- **Status:** Diterima, dengan **satu keputusan pemilik tertunda** (PPTX / MPL-2.0)
- **Hasil spike:** ✅ Lulus untuk PDF, DOCX, XLSX · ⚠️ PPTX: kandidat yang berfungsi membawa kode MPL-2.0
- **Kode:** `spikes/s5-renderers/` (`tools/licenses.ts`, `tools/make-fixtures.ts`, `src/bench.ts`, `src/xlsx-worker.ts`)

## Pertanyaan
Renderer Panel 3 — pdf.js, docx-preview, SheetJS, kandidat PPTX: bagaimana lisensinya (termasuk
dependensi transitif) dan performanya dengan file ±10 MB?

## Audit lisensi (seluruh pohon dependensi produksi, `tools/licenses.ts`)

| Paket | Versi | Lisensi | Dep. transitif | Temuan |
|---|---|---|---|---|
| `pdfjs-dist` | 6.5.438 | Apache-2.0 | 0 | bersih |
| `docx-preview` | 0.4.1 | Apache-2.0 | 13 | bersih; `jszip` = "MIT OR GPL-3.0" → pakai MIT |
| `mammoth` | 1.13.0 | BSD-2-Clause | 23 | `duck@0.1.12` hanya berlabel "BSD" (tanpa varian) |
| `xlsx` (SheetJS CE, **dari cdn.sheetjs.com**) | 0.20.3 | Apache-2.0 | 0 | bersih; lihat catatan distribusi |
| `exceljs` | 4.4.0 | MIT | **174** | `buffers` **tanpa lisensi**; tidak aktif sejak 12-2024 → **ditolak** |
| `@aiden0z/pptx-renderer` | 1.4.0 | Apache-2.0 | 17 | **`mtx-decompressor` MPL-2.0**, ikut dibundel di build browser (font tertanam) |
| `pptx-preview` | 1.0.7 | ISC (klaim) | 19 | **tanpa file LICENSE, tanpa repo publik**, kode hanya minified → **ditolak** |

Paket lain untuk format "Penuh" baru dicek lisensi paket utamanya (pohonnya belum diaudit):
`monaco-editor` 0.57 MIT, `markdown-it` 15 MIT, `mermaid` 12.3 MIT, `katex` 0.19 MIT, `shiki` 4.6 MIT,
`papaparse` 5.7 MIT, `dompurify` 3.4 (MPL-2.0 **OR** Apache-2.0 → pakai Apache-2.0).

**Catatan SheetJS:** paket `xlsx` di registry npm macet di 0.18.5 dan punya celah keamanan yang sudah
diperbaiki di versi CDN. Pemasangan dari URL tarball CDN (di-pin per versi) memang resmi menurut
SheetJS, tetapi berada di luar registry npm. Produk wajib mem-pin URL + hash di lockfile.

## Performa (file uji buatan sendiri: teks banyak + gambar noise tak-terkompresi)
File: PDF 9,35 MB (400 hlm, 27 gambar) · DOCX 8,43 MB (1.500 paragraf, 27 gambar, tabel) ·
XLSX 10,28 MB (62.001 baris × 8 kolom, 2 sheet) · PPTX 8,86 MB (60 slide, 27 gambar, tabel).
Lingkungan: Chromium di browser pane Claude Desktop (mesin yang sama dengan WebView2 di Windows),
tab terlihat, cache hangat, 2 run konsisten.

| Kasus | Total | Long task terpanjang | Jeda frame maks | Heap +MB | Cek isi |
|---|---|---|---|---|---|
| PDF · pdf.js (worker, per halaman) | 122–126 ms | 0 ms | 9 ms | −8 | teks hlm 1 ✓, 400 hlm, hlm 200 render 24 ms |
| DOCX · docx-preview (tata letak halaman) | 302–316 ms | 114–117 ms | 115–118 ms | 29–33 | teks ✓, 27 gambar, tabel ✓ |
| DOCX · mammoth (HTML semantik) | 454 ms | 172 ms | 225 ms | 84 | teks ✓, 27 gambar |
| XLSX · SheetJS di main thread | 1.326 ms | **1.320 ms** (UI beku) | 1.321 ms | 87 | 62.001 baris ✓ |
| XLSX · SheetJS di **Web Worker** | 1.408 ms | **0 ms** | 16 ms | −10 | 62.001 baris ✓, parse 1,36 dtk di worker |
| PPTX · @aiden0z/pptx-renderer (lazy) | 663–673 ms | 0 ms | 44–47 ms | 30 | 60 slide ✓, teks slide 1 ✓ |
| PPTX · pptx-preview | — | — | — | — | **gagal: tidak merender apa pun (DOM 1 node)** |

Run pertama (cold, saat tab sempat belum aktif) lebih lambat: render halaman pertama PDF 1,0 dtk
(start worker + dekode gambar), docx-preview 1,6 dtk.

## Keputusan
1. **PDF:** `pdfjs-dist` 6.5 dengan worker, render per halaman (virtualisasi halaman di Fase 7).
2. **DOCX:** `docx-preview` sebagai default (tata letak halaman); `mammoth` tidak dipakai.
3. **XLSX:** SheetJS CE 0.20.3 dari CDN resmi, **wajib di Web Worker**; tabel tervirtualisasi
   (hanya baris terlihat yang dirender). `exceljs` ditolak.
4. **PPTX (best-effort, ADR-013):** satu-satunya kandidat yang berfungsi adalah
   `@aiden0z/pptx-renderer`, dan ia **membawa `mtx-decompressor` (MPL-2.0)** di dalam bundel browsernya.
   MPL-2.0 adalah copyleft lemah per file: boleh dipakai dalam produk komersial selama file MPL itu
   tetap tersedia sumbernya, tetapi **bukan lisensi permisif** sesuai PRD §4.8. Pilihan:
   - (a) Terima MPL-2.0 sebagai pengecualian tertulis (file tidak diubah, sumber dicantumkan di
     Third-Party Notices).
   - (b) Pakai build non-browser paket ini dan ganti `mtx-decompressor` dengan stub. Font tertanam
     (MTX) tidak didukung, sehingga tampilan jatuh ke font sistem. **Belum diuji.**
   - (c) Tunda PPTX (fitur Fase 11), dan sementara pakai "buka di aplikasi lain".

   **Keputusan pemilik dibutuhkan sebelum Fase 11.** Rekomendasi saya: (c) sekarang, lalu (b) diuji
   saat Fase 11.

## Risiko & yang belum terbukti (jujur)
1. **Fidelitas visual belum dinilai.** Test hanya memastikan teks dan jumlah elemen ter-render, bukan
   kemiripan dengan Word/Excel/PowerPoint. File uji dibuat oleh generator, bukan dokumen nyata dari
   Office. Perlu korpus dokumen nyata di Fase 7/11.
2. Diukur di Chromium browser pane, bukan di WebView2 di dalam Tauri, dan bukan di WKWebView/WebKitGTK
   (macOS/Linux). Safari/WebKit diketahui berbeda untuk OffscreenCanvas/worker modul pdf.js.
3. docx-preview me-render seluruh dokumen sekaligus (5.600+ node DOM untuk 8 MB). Dokumen yang jauh
   lebih besar bisa berat. Belum ada virtualisasi.
4. Heap SheetJS +87 MB untuk file 10 MB. File XLSX 50–100 MB berisiko kehabisan memori. Perlu batas
   ukuran + pesan jujur ("terlalu besar untuk pratinjau").
5. Pohon dependensi monaco, mermaid, katex, dan shiki belum diaudit (baru paket utamanya).
6. `pptx-preview` dikirim tanpa sumber. Klaim ISC tidak bisa diverifikasi, dan ia tidak berfungsi di
   uji ini (mungkin butuh konfigurasi lain). Karena tidak bisa diaudit, tidak dikejar lebih jauh.

## Konsekuensi
- Fase 1: script audit lisensi (pola `tools/licenses.ts`) masuk CI dan menggagalkan build bila ada
  lisensi non-permisif yang tidak ada di daftar pengecualian tertulis.
- Fase 7: pdf.js (worker) + docx-preview; XLSX/CSV dengan Web Worker + tabel tervirtualisasi;
  batas ukuran pratinjau.
- PRD §8 "Best-effort: PPTX" bergantung pada keputusan MPL-2.0 di atas.

## Cara mengulang
```
cd spikes/s5-renderers
bun install
bun tools/licenses.ts                 # audit lisensi
bun tools/make-fixtures.ts            # file uji ±10 MB → public/fixtures/
bun run build && bun tools/serve.ts   # buka http://127.0.0.1:5175/ ; hasil → spikes/target/s5-report.json
```
