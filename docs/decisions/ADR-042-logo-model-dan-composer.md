# ADR-042 — Logo model/provider dan composer berbentuk kartu

Tanggal: 14-11-2026. Status: diterima.

## Keputusan

- **Logo model:** 147 logo (106 berwarna merek, sisanya monokrom `currentColor`) dari paket `@lobehub/icons-static-svg` (MIT) disalin sebagai data path
  (`packages/ui/src/model-logos.generated.ts`, dihasilkan `bun scripts/gen-model-logos.ts <folder icons>`; paketnya bukan
  dependensi runtime). Dirender sebagai `<svg>` biasa dengan `currentColor` (tanpa `dangerouslySetInnerHTML`).
- **Resolusi:** keluarga model dahulu (mis. `gemini-*` → Gemini, termasuk lewat provider perantara seperti Antigravity/OpenRouter),
  lalu provider (dengan alias), lalu fallback tile inisial (`Avatar`). Logo tampil di tab sesi (model pesan asisten terakhir,
  atau model terpilih), pemilih model, dan pemicu model di composer.
- **Merek dagang:** logo milik pemiliknya masing-masing dan dipakai hanya untuk mengenali model/provider (nominative use);
  lisensi MIT paket mencakup berkas ikonnya, bukan hak merek. Bila ada keberatan dari pemilik merek, hapus kuncinya dari
  daftar di skrip generator dan jalankan ulang.
- **Composer:** kartu berisi textarea dan bilah aksi: tombol + (lampir berkas; menu berkas/folder di desktop), pil mode izin
  (ikon perisai; "Setujui semua" berwarna peringatan), pil model (logo + nama monospace) + varian, tombol kirim/hentikan bulat.
  Info pemakaian di bawah kartu, chip saran di bawahnya. Input suara (mikrofon) tidak ditampilkan karena belum ada.

## Konsekuensi

Bundel membesar sekitar 200 KB (data path) — bisa dimuat malas bila perlu.

## Pembaruan (14-11-2026)

- Varian berwarna merek dirender sebagai `<img src="data:image/svg+xml,...">`; CSP sudah mengizinkan `img-src data:`. Aset dikecualikan dari penjaga `design-tokens`.
- Kolom isi chat selebar tetap `--z-chat-width` (46rem); composer 120% darinya.

## Input suara (14-11-2026)

Pil [mikrofon | chevron] di composer: dikte lewat Web Speech API milik WebView (hasil final ditambahkan ke kotak pesan), menu chevron memilih perangkat input nyata (`enumerateDevices`, tersimpan di localStorage) dan meneruskannya lewat `SpeechRecognition.start(track)` bila engine mendukung (Chromium baru); selain itu memakai perangkat bawaan sistem. Bila Web Speech API tidak ada, tombol nonaktif dengan alasan. **Belum diuji dengan suara nyata di aplikasi Tauri** (ketersediaan dan layanan jaringan pengenalan suara di WebView2 perlu dicek manual).

## Pemilih model sebagai popover (14-11-2026)

Modal diganti popover di atas pemicu: fokus awal ke pencarian, "Terakhir digunakan" (maks 3; disimpan di prefs `zandor.recentModels`, diperbarui saat model dipilih atau prompt dikirim), model yang disematkan, lalu semua model per provider yang bisa dilipat, dengan ping per model dan "Uji semua" untuk koneksi kustom/lokal.

## Pemilih model bertab (14-11-2026)

Popover model dibagi tab: Model (pencarian, Terakhir digunakan, Disematkan, provider — semua bisa dilipat), Uji/ping (semua, per provider, per model; status, waktu respons, HTTP, jumlah model terdaftar, waktu uji, galat; urut dan kelompok; logika murni di `model-ping.ts` dengan tes), dan satu tab per provider. Uji hanya untuk koneksi kustom/lokal milik pengguna (provider katalog tidak bisa diuji dari klien). Kolom pencarian (`SearchField`) kini satu permukaan tanpa border/ring ganda.

## Penyempurnaan pemilik model dan scrollbar (14-11-2026)

Hover baris model satu permukaan dengan state terpilih; header bagian tanpa latar hover; pin aktif berupa ikon terisi. Tab Uji menampilkan semua model: model provider katalog (mis. opencode) berstatus "Tidak bisa diuji" dengan alasan, karena uji koneksi hanya tersedia untuk koneksi kustom/lokal milik pengguna (klien tidak punya endpoint/kunci provider katalog yang dikelola engine). Scrollbar global memakai batang tipis tanpa tombol panah (`::-webkit-scrollbar`, token `--z-color-scrollbar*`).

## Uji (ping) provider katalog (14-11-2026)

Koreksi: sebelumnya tab Uji menyebut provider katalog "tidak bisa diuji dari klien" — itu keliru. Engine opencode menyebutkan per model `api.url`, adapter (`api.npm`), dan sering kunci (`GET /config/providers`). Perintah Rust `provider_ping` kini, untuk provider tanpa base URL di registry, mengambil metadata itu sendiri (Basic auth engine; **kredensial engine dan kunci tidak pernah dikirim ke JavaScript**) lalu mengirim satu permintaan 1 token: format OpenAI-compatible (`/chat/completions`, termasuk opencode zen, OpenRouter, xAI, Groq, DeepSeek, dst.) atau Anthropic (`/messages`). Adapter lain (Google, Bedrock, Azure, dst.) dilaporkan jujur sebagai "belum didukung" (status `Tidak valid` + pesan), bukan disembunyikan. Di build web (tanpa Rust) hanya koneksi kustom/lokal yang bisa diuji. Tes: derivasi target, base64, header Anthropic, Basic auth, adapter tak dikenal (Rust) dan sort/group (TS). **Belum diuji terhadap opencode zen nyata dari aplikasi Tauri** (perlu dicek manual: kunci publik zen dan adapter per model).

Juga: popover model bertinggi tetap (`min(28rem, 50vh)`), border dropdown/popup dilembutkan.

Catatan uji: ping adalah permintaan nyata 1 token sehingga menghabiskan kuota; opencode zen menjawab HTTP 429 `FreeUsageLimitError` bila kuota model gratis habis (respons asli provider, bukan galat aplikasi). Tab Uji kini menampilkan peringatan biaya; hasil panjang dipotong satu baris (lengkap di tooltip).
