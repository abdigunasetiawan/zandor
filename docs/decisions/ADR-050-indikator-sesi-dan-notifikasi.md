# ADR-050 — Indikator sesi dan sistem notifikasi

Tanggal: 14-11-2026 · Status: diterima

## Keputusan

- **Status sesi** diturunkan dari state chat (`AttentionTracker.indicator`): idle, bekerja, butuh aksi (izin/pertanyaan), selesai
  (belum dilihat), gagal, dan batas penyedia (rate limit/kuota/kredit/konteks penuh, dikenali dari pesan galat). Abort pengguna
  bukan kegagalan.
- **Titik di sidebar** (`SessionDot`) menggantikan bullet: titik redup (idle), indikator **ripple** (bekerja; atom `Working`,
  bukan spinner biasa), cincin berdenyut (butuh aksi), titik terisi (selesai), titik bahaya (gagal), titik bahaya bercincin
  putus (batas). Tab sesi menampilkan titik hanya saat ada yang perlu diperhatikan.
- **Atom `Working`** (`wave` / `ripple` / `orbit`): murni CSS, `currentColor`, diam saat gerak dikurangi.
- **Pemberitahuan** (`NotificationDriver`): peralihan busy→idle dinilai setelah mengendap 300 ms (event galat bisa tiba sesaat setelah
  status idle), lalu toast (dengan tombol "Buka sesi"), notifikasi sistem (hanya saat jendela tidak fokus; izin lewat tombol di
  Pengaturan), dan suara. Default: tidak memberi tahu untuk sesi yang sedang dilihat.
- **Suara**: nada bawaan disintesis dengan Web Audio (tanpa aset): denting, pop, lonceng, lembut, peringatan, rendah; atau berkas
  suara pengguna per peristiwa (data URL, maks 1 MB). Volume dan pilihan per peristiwa ada di Pengaturan.
- Sesi di **tab terbuka tetap dipantau di latar belakang** (sebelumnya sesi dilepas saat tidak dipilih).

## Batas

- Hanya sesi yang pernah dibuka di sesi aplikasi ini (tab) yang dipantau; sesi yang belum pernah dibuka tidak punya status.
- Suara kustom disimpan di penyimpanan lokal (bukan sinkron antar perangkat). Notifikasi sistem dan pemutaran suara belum diuji
  di aplikasi Tauri sungguhan.
