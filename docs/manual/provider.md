# Provider dan model

Buka lewat ikon roda gigi di footer sidebar, palet perintah ("Kelola provider & model"), atau "Kelola provider…" di pemilih model.

- **Endpoint kustom (OpenAI-compatible):** isi nama, Base URL (mis. `http://localhost:8045/v1`), kunci → **Ambil daftar model**
  → centang → Simpan. Engine dimulai ulang sebentar.
- **Lokal:** **Deteksi Ollama / LM Studio** (port 11434/1234) atau pakai preset.
- **Provider katalog:** cari provider dari katalog engine, isi kunci API. Provider yang butuh data tambahan (mis. Account ID)
  ditandai belum didukung.
- **Sign-in:** provider dengan login akun (OAuth) menurut engine; ikuti langkah di browser.
- **Uji model:** tombol petir mengirim satu permintaan kecil dan menampilkan OK + latensi, atau penyebab gagal apa adanya
  (kunci ditolak, rate limit, server bermasalah, tidak terjangkau).
- **Keamanan:** kunci disimpan di keychain sistem (Windows Credential Manager), bukan di berkas atau konfigurasi.

## Model bawaan bila belum memilih

Urutan pemilihan model untuk sesi baru: model yang dipilih di sesi → **Model chat bawaan** (Pengaturan) → model terakhir
dipakai → cadangan. Cadangannya: bila satu-satunya koneksi adalah **OpenCode bawaan** (belum menambah provider lain),
Zandor memakai model **DeepSeek dari OpenCode** (mis. `opencode/deepseek-v4.1-flash-free`); bila ada koneksi lain, model
pertama dalam daftar. Pilihan Anda sendiri selalu didahulukan.

## Tambah koneksi

Di **Pengaturan › Provider dan model**, klik **Tambah koneksi**: formulir tampil di halaman yang sama (bukan popup) dengan tab Endpoint kustom (id, nama, protokol API, base URL, kunci, ambil model), Lokal (Ollama/LM Studio), Katalog provider, dan Masuk. Semua provider diperlakukan sama.

**Beberapa akun**: di tab Katalog, provider yang sudah berkunci menampilkan daftar **Akun** — **Tambah akun** (nama + kunci), **Gunakan** untuk mengganti akun aktif, ganti nama, atau hapus. Di tab **Sign-in** ada kartu **OpenWork Cloud**: masuk lewat kode perangkat (setiap login menambah satu akun). Login OAuth lewat engine hanya satu per provider.

## Generate gambar

Pilih **Model generate gambar bawaan** di Pengaturan › Provider dan model (mis. `openai/gpt-image-1`,
`google/gemini-2.5-flash-image`, atau model gambar di OpenRouter). Lalu buka palet perintah → **Generate gambar…**, tulis
deskripsi, pilih ukuran, **Generate**. Gambar disimpan di `images/` folder project/sesi aktif (tidak menimpa berkas lama)
dan dibuka di Panel 3. Agent juga bisa membuat gambar lewat tool `zandor_image`. Didukung: OpenAI, Google Gemini,
OpenRouter, dan endpoint kompatibel OpenAI yang kuncinya tersimpan di Zandor; provider tanpa API gambar (Anthropic)
menampilkan galat. Setiap gambar memakai kuota provider Anda.

## Agent mengoperasikan Zandor

Agent bisa membuka/menutup panel, membuka alat Panel 4, membuka URL di browser dan berkas di Panel 3, menutup terminal,
menjalankan perintah palet, dan mengubah pengaturan (skill bawaan `zandor-app`). Setiap aksi yang mengubah tampilan
memunculkan pemberitahuan "Agent: …". Matikan seluruhnya di **Pengaturan › Umum › Agent boleh mengoperasikan Zandor**
(agent tidak bisa mengubah pengaturan ini).

## Model dari server

Setelah **Ambil daftar model**, tiap model menampilkan info yang disediakan server (konteks, keluaran, reasoning, alat, lampiran) bila ada; gunakan **Pilih semua / Batalkan semua** untuk memilih sekaligus. Info yang tidak disediakan server bisa diisi lewat **Kelola model › Ubah model**. Di pemilih model, **Kelola provider…** membuka Pengaturan › Provider dan model.
