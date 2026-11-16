# ADR-071 — Pewarnaan TextMate (Shiki) untuk tema editor VS Code

- **Tanggal:** 16-11-2026
- **Status:** diterima

## Konteks

Tema editor yang diimpor dari berkas atau Open VSX (ADR-038) diterapkan ke Monaco lewat `defineTheme`. Tema VS Code
memberi warna berdasarkan **scope TextMate** (mis. `entity.name.function`, `variable.parameter`,
`keyword.control.flow`), sedangkan Monaco di Zandor memakai tokenizer **Monarch** yang token-nya jauh lebih kasar
(`keyword.ts`, `identifier.ts`, `string.ts`). Hampir semua aturan warna tema tidak cocok dengan token Monarch,
sehingga kode tampil nyaris tanpa warna. Selain itu, `include` (tema induk) di paket `.vsix` diabaikan.

## Keputusan

- Tambah dependensi `shiki` (MIT) di `apps/desktop`: grammar TextMate asli VS Code + mesin regex Oniguruma (WASM).
  Semua dimuat **lazy**, hanya saat tema VS Code terpasang aktif.
- Adaptor sendiri `apps/desktop/src/shell/docs/textmate.ts` (bukan `@shikijs/monaco`, karena adaptor itu mengganti
  `monaco.editor.setTheme` secara global dan error untuk tema non-Shiki seperti `vs` atau "Otomatis"):
  - bahasa didaftarkan dengan id terpisah `tm-<bahasa>`, sehingga tokenizer Monarch bawaan tetap utuh;
  - token diberi nama dari indeks warna di metadata vscode-textmate (`t<idx>[.<fontStyle>]`), dan tema Monaco
    dibuat dari `colorMap` tema aktif. Warna akhirnya tepat seperti hasil pencocokan selektor TextMate;
  - saat tema berganti, provider didaftarkan ulang agar semua model ditokenisasi ulang.
- `CodeViewer` memakai Monarch lebih dulu, lalu beralih ke `tm-*` setelah grammar dan tema siap. Bila gagal, tetap
  memakai Monarch (dengan peringatan di konsol).
- `EditorTheme` sekarang menyimpan `tokenColors` asli yang sudah divalidasi (selektor lengkap, tanpa karakter kontrol,
  warna hex). Tema lama tanpa `tokenColors` diturunkan dari `rules` (kurang presisi; impor ulang untuk hasil terbaik).
- `include` di paket `.vsix` diresolve relatif terhadap berkas tema, maksimal 5 tingkat, tidak boleh keluar dari
  folder `extension/`. Induk digabung lebih dulu, lalu tema anak di atasnya (sama dengan VS Code).

- CSP Tauri ditambah `script-src 'self' 'wasm-unsafe-eval'` agar WebView2 boleh mengompilasi WASM Oniguruma.
  `'wasm-unsafe-eval'` hanya mengizinkan kompilasi WebAssembly, bukan `eval`/`new Function` JavaScript. Sumber skrip
  tetap hanya `'self'`.

## Batasan

- **Semantic highlighting** VS Code (`semanticTokenColors`, butuh language server) tidak didukung. Pada TS/JS di VS Code,
  sebagian warna (mis. variabel readonly, properti) berasal dari situ, jadi bisa sedikit berbeda.
- `tokenColors` berformat `.tmTheme` (plist) belum didukung dan dilaporkan sebagai peringatan.
- Ukuran bundel bertambah (WASM Oniguruma ±600 KB + grammar per bahasa), tetapi semuanya chunk lazy.

## Alternatif yang ditolak

- **Pemetaan scope → token Monarch:** tanpa dependensi, tetapi Monarch tidak bisa membedakan fungsi, variabel, properti,
  dan parameter. Hasilnya tidak akan sama dengan VS Code.
- **`@shikijs/monaco`:** lihat alasan di atas (menimpa API global Monaco).
