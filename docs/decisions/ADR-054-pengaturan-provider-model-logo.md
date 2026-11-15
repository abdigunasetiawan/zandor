# ADR-054 — Pengaturan Provider & model: model bawaan, protokol, kemampuan, visibilitas, logo kustom

Tanggal: 14-11-2026 · Status: diterima (melengkapi ADR-022/034/040)

## Keputusan

- Kategori baru **Provider dan model** di Pengaturan (halaman lebar): **model bawaan** (chat, generate gambar),
  **daftar provider** (katalog opencode, lokal Ollama/LM Studio, endpoint kustom), dan **pengelola model** per provider.
- Endpoint kustom: **id** (bisa diisi, terkunci setelah dibuat), nama tampilan, **protokol API**
  (`openai-compatible` default, `openai`, `anthropic`, `google` → adapter `@ai-sdk/*` di config engine), base URL,
  kunci API (tetap hanya di keychain), ambil daftar model dari endpoint.
- Model kustom punya **kemampuan** opsional (jendela konteks, maks keluaran, reasoning, tool call, lampiran) yang ditulis
  ke `providers.json` dan diteruskan ke config engine (`limit`, `reasoning`, `tool_call`, `attachment`). Validasi di Rust
  (protokol dikenal, batas > 0).
- Kemampuan semua model (katalog maupun kustom) dibaca dari `capabilities`/`limit` engine ke `ModelOption`.
- **Visibilitas model** (`ModelPrefs.hidden`): pilih semua / batalkan semua / per model; pemilih model di composer
  menyembunyikannya (model yang sedang dipakai tetap tampil). Pengelompokan: tanpa, keluarga model, kemampuan.
- **Model chat bawaan** (`ModelPrefs.defaultChat`): urutan pilih model = sesi ini → bawaan → pilihan terakhir → pertama.
- **Logo**: deteksi otomatis dari id model, id provider, lalu **nama tampilan** (nama produk ≥5 huruf lebih dulu;
  `ollama` ≠ `llama`). Logo kustom provider/model dari berkas pengguna (PNG/JPEG/WebP/GIF/SVG), raster dikecilkan ke
  128px, disimpan sebagai data URL tervalidasi (maks ~96k karakter) di penyimpanan lokal; `ModelLogoOverrides` (konteks)
  menimpa logo di seluruh aplikasi.
- Lapisan popup (`--z-layer-popup`) dinaikkan di atas modal agar Select di dalam Dialog bisa dipakai.

## Batas

- **Model generate gambar bawaan hanya preferensi tersimpan**; belum ada fitur gambar (diberi label jujur di UI).
- Preferensi model/logo disimpan di localStorage perangkat ini (bukan keychain; bukan rahasia), tidak ikut ekspor pengaturan.
- Kemampuan model kustom diisi manual; tidak ada deteksi otomatis dari endpoint.
