# ADR-060 — Toggle memori dan skill creator di composer

Tanggal: 15-11-2026 · Status: diterima

## Keputusan

- Composer punya dua toggle: **Ingat memori** dan **Skill creator** (`ComposerMemory`).
- Cakupan: sesi **dalam project** berbagi pengaturan project (satu sakelar untuk semua sesinya; toggle menulis pengaturan
  project dan membuang override sesi lama agar tidak bertentangan). Sesi **tanpa project** (Chat) diatur **per sesi**.
  Di halaman sesi-baru, pilihan untuk Chat ditahan dulu dan diterapkan saat sesi dibuat.
- Skill creator kini punya resolusi yang sama dengan memori: sesi → project → bawaan (`skillCreatorSessions`,
  `skillCreatorProjects` di `settings.json`; plugin `skillCreatorOn(sessionID)` dan inti `skillCreatorEnabled` identik,
  dijaga tes paritas). Berkas lama tetap valid (kunci baru opsional).
- Info sesi: untuk sesi dalam project, pemilih per-sesi diganti keterangan "berbagi pengaturan project"; halaman Memori &
  skill menambah pilihan skill creator per project.

## Konsekuensi

- Pengguna bisa mengaktifkan memori/skill creator kapan saja dari tempat mengetik, tanpa membuka pengaturan.
- Override per-sesi di dalam project tidak lagi dibuat dari UI (masih dihormati engine bila ada di berkas).
