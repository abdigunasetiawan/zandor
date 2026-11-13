# ADR-038 — Kustomisasi tampilan (Fase 10)

- **Tanggal:** 14-11-2026
- **Status:** Diterima
- **Terkait:** ADR-014 (zoom token), ADR-029 (design system), ADR-031 (ikon), ADR-035 (Monaco), ADR-037

## Keputusan

1. **Satu `AppearanceStore`** (core, murni; disimpan di `localStorage["zandor.appearance"]`) memegang: preset/tema kustom,
   tipografi per variabel, font, gaya panel chat, gaya section project, tema editor terpasang, dan override shortcut.
   Data tersimpan **dibersihkan saat dimuat** (`sanitizeAppearance`): bagian rusak dibuang, sisanya dipertahankan.
2. **Semua nilai divalidasi sebelum menyentuh CSS** (`theme/tokens.ts`): warna hanya heks atau `rgb/hsl(...)` numerik;
   ukuran dalam rem/em dengan batas; ketebalan 100–900; nama font hanya huruf/angka/spasi/koma/kutip. Tema impor yang
   memuat `url(...)`, `;`, `var(...)` atau token tak dikenal **ditolak seluruhnya** (tidak diperbaiki diam-diam).
3. **Penerapan** lewat gaya inline di `<html>` (`applyVars`, hanya properti `--z-*`), dihitung untuk **mode efektif**
   (gelap/terang/sistem, mengikuti `prefers-color-scheme`). Tanpa kustomisasi tidak ada gaya inline sama sekali, jadi
   `tokens.css` tetap satu-satunya sumber nilai mentah dan penjaga token desain tetap berlaku.
4. **Preset** (Zandor monokrom, Gemini, Claude, ChatGPT, Discord, Vercel, Raycast): palet gelap + terang, hanya warna
   (tanpa aset/merek). Uji `theme.test.ts` menjaga **kontras WCAG**: teks/latar ≥ 7:1, teks redup/halus ≥ 4.5:1, teks di
   atas aksen ≥ 4.5:1, aksen/latar ≥ 3:1 (dua preset disesuaikan karena awalnya di bawah batas).
5. **Panel chat**: token baru `--z-chat-{opacity,weight,size,gap,font}` (bawaan mengikuti token tipografi).
   **Section project**: border/latar (warna tervalidasi) dan logo teks/emoji ≤ 4 karakter tanpa karakter markup
   (gambar sengaja tidak didukung: ukuran dan risiko).
6. **Tema editor (Monaco)**: bawaan Monaco, "otomatis" (mengikuti token), atau tema VS Code yang diimpor — dari berkas
   JSON/JSONC lokal atau **Open VSX**. Hanya berkas JSON tema di dalam `.vsix` yang dibaca (tanpa menjalankan kode
   ekstensi); `tokenColors` dikonversi ke aturan Monaco, warna disaring. Maks 20 tema terpasang, 400 KB per tema.
7. **Open VSX lewat Rust** (`openvsx.rs`): hanya `https://open-vsx.org/`, pencarian ≤ 2 MB, unduhan ≤ 15 MB, kueri
   di-percent-encode; WebView tidak membuka koneksi (CSP tidak diubah). `fflate` (MIT) membuka zip di JS.
8. **Editor shortcut**: `ShortcutRegistry.setOverride/applyOverrides` (bentrok di scope yang sama ditolak; override untuk
   binding yang belum terdaftar disimpan dan berlaku saat ia terdaftar). UI menangkap tombol (wajib Ctrl/Alt/Meta atau
   tombol F), menampilkan combo bawaan, dan palet menampilkan combo efektif.

## Bukti

- `theme.test.ts`, `vscode-theme.test.ts`, `shortcuts-override.test.ts` (unit), `AppearancePage.test.tsx` (komponen,
  Open VSX dengan host tiruan), `e2e/appearance.spec.ts` (7 tes di Edge), Rust `openvsx` (3 tes).
- e2e menangkap satu bug nyata: id shortcut `cmd:…` ditolak sanitizer karena regex tanpa `:`.
- Smoke native menangkap bug nyata kedua: ureq (fitur `native-tls` saja) memakai Rustls secara bawaan dan **panik pada https**.
  Penyedia TLS kini dipilih eksplisit (`NativeTls`) di `openvsx.rs` dan **`netprobe.rs`** — ini juga memperbaiki bug laten
  uji/ping provider cloud ber-https sejak Fase 6 (endpoint http lokal tidak terpengaruh). Tes jaringan nyata (`--ignored`)
  untuk pencarian + unduhan Open VSX dan probe https lulus.

## Batas / belum

- **Paket ikon file dari Open VSX belum dibuat** (format `iconDefinitions` + SVG; butuh pemetaan ekstensi → ikon dan
  render aman). Ikon file tetap Material Icon Theme (ADR-031).
- Open VSX diuji ke jaringan nyata di level Rust (`cargo test --lib openvsx -- --ignored`); UI-nya diuji dengan host tiruan
  dan dicoba ulang di smoke native. Lisensi tiap tema milik pihak ketiga; UI hanya memberi peringatan.
- Tema editor diterapkan ke penampil kode (Panel 3). Terminal dan Monaco-lain mengikuti token biasa.
- Konversi `tokenColors` pendekatan terbaik (scope TextMate → token Monaco): hasil warna bisa berbeda dari VS Code asli.
- Gambar sebagai logo/latar project belum didukung.
