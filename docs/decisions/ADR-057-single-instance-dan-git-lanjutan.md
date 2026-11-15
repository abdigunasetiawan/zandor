# ADR-057 — Single-instance dan Git lanjutan

Tanggal: 15-11-2026 · Status: diterima

## Keputusan

- **Single-instance**: `tauri-plugin-single-instance` (=2.5.5, MIT/Apache, lolos `cargo deny`) didaftarkan sebagai plugin
  pertama. Instans kedua keluar dan jendela `main` instans pertama dimunculkan/difokuskan. Alasan: database engine terkunci
  bila dua Zandor berjalan.
- **Git**: perintah baru `git_push`, `git_pull`, `git_stash_push/pop/count`, `git_discard`.
  - Push tidak pernah `--force`; pull hanya `--ff-only` (tanpa merge/rebase otomatis).
  - Operasi jaringan memakai batas waktu 180 dtk dan tanpa prompt terminal.
  - Discard destruktif: UI wajib konfirmasi; path divalidasi seperti stage/unstage (relatif, tanpa `..`).
  - `GitHost` menerima metode opsional agar host lain/tes lama tetap valid; UI menyembunyikan tombol yang tidak didukung.
- **Sematkan ke konteks**: sudah ada lewat menu "Simpan ke catatan" pada pesan; item diganti nama menjadi "Sematkan ke
  konteks sesi" agar mudah ditemukan (docs/features/memory.md).

## Batasan

- Belum dicoba di aplikasi Tauri terpaket atau ke remote sungguhan; single-instance belum diuji dengan dua proses.
