# Tampilan & Kustomisasi (Fase 10)

- **Status:** selesai di lokal dengan batasan (di bawah) · **ADR:** [ADR-038](../decisions/ADR-038-kustomisasi-tampilan.md)
- **Manual:** [tampilan.md](../manual/tampilan.md) · Halaman: `#/appearance` (palet "Tampilan & tema")

## Spek

| Bagian          | Isi                                                                                                                                                             |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tema            | 7 preset (Zandor, Gemini, Claude, ChatGPT, Discord, Vercel, Raycast), tema kustom (editor warna 18 token × gelap/terang, peringatan kontras), impor/ekspor JSON |
| Tipografi       | 10 variabel × (ukuran, ketebalan, tinggi baris, jarak huruf) + font UI/kode/terminal                                                                            |
| Panel chat      | opasitas, ketebalan, ukuran, jarak antarpesan, font                                                                                                             |
| Section project | warna border, warna latar, logo (teks/emoji) di sidebar                                                                                                         |
| Editor          | tema Monaco: otomatis, bawaan, impor berkas VS Code, cari & pasang dari Open VSX                                                                                |
| Pintasan        | ubah combo tiap pintasan (deteksi bentrok), kembali ke bawaan, palet menampilkan combo efektif                                                                  |

## Cara test

| Lapisan                                                                         | Perintah                                                                                |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Unit (validator, preset+kontras, store, konversi tema, vsix, override shortcut) | `bun x vitest run packages/core/src/theme packages/core/src/shortcuts-override.test.ts` |
| Komponen (tab Editor/Open VSX tiruan, Pintasan)                                 | `bun x vitest run apps/desktop/src/shell/AppearancePage.test.tsx`                       |
| Rust (Open VSX: URL, kueri, parsing)                                            | `cargo test --lib openvsx`                                                              |
| E2E (Edge)                                                                      | `bun x playwright test e2e/appearance.spec.ts`                                          |

## Batasan jujur

- Paket ikon file Open VSX **belum ada** (ikon file tetap Material Icon Theme).
- Open VSX: klien Rust diuji ke jaringan nyata (`cargo test --lib openvsx -- --ignored`); di build web tab itu menjelaskan bahwa fitur hanya ada di desktop.
- Logo project hanya teks/emoji (maks 4 karakter), bukan gambar.
- Kontras tema kustom hanya diperingatkan (teks/latar), tidak dipaksakan.
- Tema editor mengubah penampil kode saja (panel dokumen: latar, teks, nomor baris; blok kode chat).
- **Tema editor bawaan = "Theme Darker"** (tal7aouy, MIT; `assets/themes/theme-darker.json`, lisensi di samping berkasnya), dipakai
  saat pengguna belum memilih. Warna token diselesaikan dengan scope gaya TextMate (awalan segmen, selektor turunan,
  aturan paling spesifik/belakangan menang) terhadap `tokenColors`, seperti VS Code (`core/editor_theme.rs::Resolver`).
  Perbedaan jujur: highlighter (`ui/highlight.rs`) lexer per baris, bukan grammar TextMate penuh, jadi scope yang dihasilkan
  hanya perkiraan (mis. parameter fungsi/properti objek tidak dibedakan) dan warna bisa berbeda di kasus tepi.
