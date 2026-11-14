# ADR-046 — Dua jenis sesi: dalam project dan tanpa project dengan folder sendiri

Tanggal: 14-11-2026. Status: diterima.

## Konteks

Sesi tanpa project ("Chat") sebelumnya semuanya memakai satu folder `chats/`. Berkas yang dibuat agent di berbagai sesi bercampur,
dan tidak ada batas antar sesi.

## Keputusan

- **Sesi dalam project:** tetap memakai folder project.
- **Sesi tanpa project:** setiap sesi baru mendapat **folder kerja sendiri** `chats/chat-YYYYMMDD-HHMMSS-xxxxxx/` (dibuat Rust:
  `chat_folder_new`, nama divalidasi: 1–64 karakter aman, tidak diawali titik). Sesi engine dibuat dengan folder itu sebagai
  direktorinya.
- **Daftar sesi Chat** = sesi di folder `chats/` (sesi lama yang berbagi) + semua folder sesi (`chat_folders`), digabung tanpa duplikat;
  folder yang gagal dibaca diabaikan selama ada yang berhasil. Ganti nama/hapus memakai folder milik sesi itu.
- **Hapus sesi Chat:** folder sesi dihapus hanya bila KOSONG dan hanya anak langsung `chats/` (`chat_folder_remove_if_empty`);
  berkas hasil kerja tidak pernah dihapus. Fork tetap di folder sesi induknya (satu percakapan, satu folder).
- Panel Dokumen tetap memakai `chats/` sebagai akar yang terdaftar (berkas folder sesi diakses sebagai jalur relatif di bawahnya);
  terminal Chat mengikuti folder sesi terpilih. Tooltip baris sesi menampilkan folder kerjanya.
- Di web (tanpa Rust) daftar folder disimpan di localStorage (`zandor.chatFolders`) sehingga sesi tetap terdaftar.

## Belum

Perilaku opencode untuk folder sesi di luar repo git (sesi per folder) perlu dipantau di aplikasi nyata; migrasi sesi lama ke folder
masing-masing tidak dilakukan (sesi lama tetap di folder bersama).
