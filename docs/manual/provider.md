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

## Masuk dengan OpenWork

Di aplikasi desktop, klik **Masuk dengan OpenWork** di halaman Provider. Zandor menampilkan **kode perangkat** dan membuka
halaman OpenWork; masuk di browser, pastikan kodenya sama, lalu setujui. Setelah itu provider **OpenWork Cloud** dan model
dari AI Gateway akun Anda ditambahkan otomatis. Token dan kunci disimpan di keychain dan tidak ditampilkan. Bila organisasi
Anda belum punya kredensial AI Gateway, muncul pesan galat (hubungi admin organisasi).
