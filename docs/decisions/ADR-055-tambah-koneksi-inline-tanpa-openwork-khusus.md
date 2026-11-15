# ADR-055 — Tambah koneksi inline; OpenWork tidak diperlakukan khusus

Tanggal: 14-11-2026 · Status: diterima (menggantikan bagian "Masuk dengan OpenWork" pada ADR-040)

> Bagian "alur khusus OpenWork dihapus" digantikan oleh ADR-056 (OpenWork kembali sebagai kartu Sign-in + akun ganda).

## Keputusan

- "Tambah/Ubah koneksi" bukan lagi popup: `ConnectionPanel` menggantikan isi halaman (Pengaturan › Provider dan model dan
  halaman Provider) dengan tombol Kembali.
- Alur khusus OpenWork dihapus: Rust `openwork.rs` + perintah `openwork_*`, `OpenWorkHost`/status di `ProviderStore`,
  `OpenWorkLogin`, tombol "Masuk dengan OpenWork", teks `ow.*`, dan tesnya. OpenWork diperlakukan seperti provider
  opencode lain.

## Dampak

- Provider `openwork` yang sudah tersimpan tetap berfungsi sebagai endpoint kustom (kunci di keychain).
- Dialog kecil lain (ubah model, logo) tetap dialog karena berupa penyuntingan satu item.

## Tambahan (hari yang sama)

- Halaman `/providers` dihapus sebagai halaman terpisah: `/providers` dan `/settings/models` membuka Pengaturan di
  kategori "Provider dan model". Semua jalan masuk (tombol "Kelola provider…" di pemilih model, palet perintah, sidebar,
  info chat) menuju ke sana. Uji (ping) per model pindah ke pengelola model.
- **Hasil ambil model membawa info**: `GET /models` hanya wajib memberi `id`, tetapi metadata yang disediakan endpoint
  (nama, konteks, keluaran, reasoning, alat, lampiran — gaya OpenRouter/LM Studio/vLLM) dibaca (`parseModelInfo` di TS dan
  `netprobe::parse_model_info` di Rust), ditampilkan sebagai lencana di daftar, dan ikut tersimpan. Yang tidak disediakan
  endpoint dibiarkan kosong (tidak ditebak) dan bisa diisi manual lewat "Ubah model".
