# ADR-034 — Provider & model (Fase 6) dan pelengkap chat inti

- **Tanggal:** 13-11-2026
- **Status:** Diterima
- **Terkait:** ADR-022 (secret & env guard), ADR-032, ADR-033

## Keputusan

1. **Kunci API di keychain OS** (Windows Credential Manager, crate `keyring` 4.2.0; service `id.web.zandor`,
   akun `provider:<id>`; secret panjang dipecah per 1200 unit UTF-16). Daftar koneksi di
   `app_data/providers.json` **versi 2 tanpa kunci**. Berkas versi 1 (kunci di berkas, dari Fase 5) dimigrasikan
   otomatis saat engine dimulai: kunci dipindah ke keychain lalu berkas ditulis ulang; bila keychain gagal, berkas
   lama **tidak diubah** dan peringatan dicatat. Terbukti pada mesin pengembang (kunci pindah, berkas bersih).
2. **Tiga jenis koneksi:** `custom` (endpoint OpenAI-compatible), `local` (Ollama/LM Studio, kunci opsional), dan
   `builtin` (provider katalog engine; hanya kunci yang kita simpan). Konfigurasi engine tetap tanpa nilai secret:
   `options.apiKey = "{env:ZANDOR_SECRET_PROVIDER_<ID>}"`. Terbukti pada engine asli: provider builtin
   (`openrouter`) dengan `options.apiKey` menjadi `connected` dengan 371 model; provider lokal tanpa kunci juga
   `connected`.
3. **Probe/ping dari Rust**, bukan JS (`netprobe.rs`, `ureq` + `native-tls`/schannel): ambil `GET {base}/models`, ping
   satu model (`POST /chat/completions`, `max_tokens: 1`), dan deteksi Ollama (11434)/LM Studio (1234). Kunci dari
   keychain tidak pernah menyentuh JavaScript setelah dimasukkan. Waktu tunggu selalu terbatas (probe 8 dtk, ping 20 dtk,
   deteksi 0,8 dtk, ditambah penjaga 30 dtk di UI) sehingga **tidak ada spinner tanpa akhir**.
4. **Status yang jujur:** hasil dikelompokkan `ok / auth / notfound / ratelimit / server / network / invalid` dengan
   pesan asli dari server (dipotong 300 karakter). Badge "Tersambung ke engine" berasal dari engine (`GET /provider`
   → `connected`), bukan tebakan kita. Rate limit saat chat tampil lewat status `retry` engine.
5. **Sign-in lewat OpenCode:** `GET /provider/auth` memberi metode per provider; `POST /provider/:id/oauth/authorize`
   mengembalikan `{url, method: auto|code}`. Zandor membuka URL (hanya `https`, lewat `rundll32 url.dll,
   FileProtocolHandler`, bukan `cmd /c start`) lalu memanggil `oauth/callback` (menunggu untuk `auto`, memakai kode untuk
   `code`; batas 5 menit). Token disimpan **oleh engine** (ADR-022 butir 5), bukan oleh Zandor.
6. **Provider katalog "sederhana"** = tepat satu env var dan metode API tanpa prompt tambahan. Yang butuh data
   tambahan (mis. Cloudflare: Account ID) ditandai "perlu konfigurasi tambahan" dan belum didukung lewat kunci.
7. **Pemilih model penuh:** dialog dengan pencarian yang langsung fokus, grup "Disematkan" lalu per provider, sematkan
   (disimpan di `localStorage`), pilih, dan uji model milik koneksi kustom/lokal Anda.
8. **Pelengkap chat inti (sisa Fase 5):** mode izin sisi-klien (`ask` / setujui edit otomatis / setujui semua),
   varian reasoning dari `model.variants`, lampiran (dialog native atau `<input>`; berkas dan folder),
   regenerate (revert ke pesan pengguna terakhir lalu kirim ulang), revert/unrevert, fork sesi, tab sesi (maks 12,
   tersimpan), salin, kartu web search, tampilan diff untuk edit, dan transcript tervirtualisasi (`@tanstack/react-virtual`
   bila ≥ 40 pesan).

## Temuan

- **Bash di lingkungan pengembangan menggandakan-hilangkan backslash di heredoc** — skrip edit yang berisi `\n`/`\r`
  harus ditulis lewat berkas, bukan heredoc (hanya catatan proses).
- Engine katalog: **236 provider** (`connected` bawaan hanya `opencode`); metode auth OAuth ada untuk `openai`,
  `github-copilot`, `gitlab`, `poe`, `digitalocean`, `snowflake-cortex`, `xai`; sebagian punya `prompts` (mis. URL
  instance GitLab, tipe deployment GitHub) yang diteruskan sebagai `inputs`.
- Mode izin otomatis dijalankan **di klien**: bekerja selama Zandor terbuka; izin yang menunggu saat aplikasi tertutup tetap
  menunggu.

## Batasan yang diketahui (jujur)

- **Chat nyata setelah migrasi keychain belum bisa dibuktikan ulang pada mesin pengembang**: upstream proxy Antigravity
  sedang mengembalikan HTTP 500 ("Connection failed") saat pengujian; ping Zandor melaporkannya apa adanya. Jalur kunci
  keychain → env → engine diuji unit (`for_engine`) dan jalur env → engine sudah terbukti di Fase 5.
- **Ollama/LM Studio tidak terpasang di mesin pengembang**: deteksi lokal diuji dengan server tiruan (Rust) dan e2e;
  "chat dengan model lokal" belum diuji dengan server sungguhan.
- **Sign-in OAuth** diuji sampai `authorize` mengembalikan URL pada engine asli (OpenAI) dan alur UI dengan engine
  tiruan; **login akun sungguhan belum dilakukan** (butuh pengguna di browser).
- Provider katalog yang butuh data tambahan, ubah nama model builtin, dan sinkron lintas perangkat belum ada.
- Mode dev (di luar Tauri) menyimpan kunci hanya di memori halaman; UI menjelaskannya.
- Hanya Windows (ADR-027); `open_external` belum ada untuk macOS/Linux.

## Verifikasi

Rust: 29 tes (keychain nyata dengan secret 3000 karakter + emoji, migrasi v1→v2, kegagalan migrasi, probe/ping
terhadap server tiruan termasuk 401/404/429/5xx dan koneksi gagal). Core: `ProviderStore` (validasi, kunci tak masuk
state, ping selalu berakhir, OAuth menolak URL non-https), pin model, revert/regenerate/fork/mode izin/varian,
tab sesi. E2E (60): halaman provider, pemilih model, OAuth otomatis/kode, tab sesi, revert/ulang/fork, lampiran,
virtualisasi 600 pesan. Smoke native 16/16 (keychain, migrasi, engine mengenali provider, ping nyata berakhir jujur).
