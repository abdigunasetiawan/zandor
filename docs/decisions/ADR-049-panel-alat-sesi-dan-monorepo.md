# ADR-049 — Panel alat: alat khusus sesi, git monorepo, terminal

Tanggal: 14-11-2026 · Status: diterima (git monorepo belum diuji di aplikasi Tauri sungguhan)

## Konteks

Panel 4 (alat) dipilih lewat rel di Panel 5. Git hanya memeriksa akar project (monorepo dengan repo di anak folder tampak "bukan
repo"), Info sesi hanya bekerja untuk sesi nyata yang dipilih, tidak ada tampilan aktivitas per sesi maupun perubahan berkas per
sesi, dan tata letak terminal kurang rapi.

## Keputusan

- **Rel alat**: Terminal · Git · **Perubahan sesi** · **Berkas sesi** · Browser · Info sesi · **Aktivitas sesi** · Log aktivitas aplikasi.
- **Git monorepo**: bila akar project bukan repo, Rust (`git::find_repos`, maks 3 tingkat/4000 folder, melewati `node_modules`,
  `target`, dst., berhenti di repo yang ditemukan) mencari repo di anak folder; panel Git menampilkan pemilih repo dan memakai
  repo aktif untuk status/diff/commit/cabang. Perintah git menerima akar terdaftar **atau anak foldernya** (`resolve_dir_within`).
- **Perubahan sesi**: `GET /session/:id/diff` dari engine (snapshot, bukan git) → berkas dengan status Baru/Diubah/Dihapus, +/−, dan
  diff berwarna; bekerja juga di folder Chat tanpa repo; dimuat ulang saat giliran selesai.
- **Berkas sesi**: berkas yang disentuh tool read/edit/write sesi ini, klik membuka di Panel 3.
- **Aktivitas sesi**: garis waktu seluruh kejadian sesi (pesan, berpikir, tool + status + durasi, jawaban, galat, izin/pertanyaan
  menunggu) dari pesan sesi; bisa disaring, dirinci, dan melompat ke pesan. **Log aktivitas aplikasi** tetap untuk seluruh aplikasi.
- **Info sesi** bekerja juga untuk sesi demo (berlabel) dan menjelaskan keadaan tanpa sesi; baris baru: status, menunggu Anda,
  berkas disentuh, rencana, mode izin.
- **Terminal**: tab berikon, bilah tanpa scrollbar, jalur folder berikon, keadaan kosong di tengah.

## Batas

- Perubahan sesi bergantung pada engine yang mengembalikan snapshot; engine lama tanpa endpoint menampilkan galat apa adanya.
- Aktivitas sesi dibangun dari pesan yang dimuat; tidak ada riwayat di luar itu.
- Pemilihan repo monorepo diingat hanya selama aplikasi berjalan.
