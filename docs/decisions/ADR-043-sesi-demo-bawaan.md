# ADR-043 — Sesi demo bawaan (disematkan, permanen, hanya-baca)

Tanggal: 14-11-2026. Status: diterima.

## Konteks

Pemilik ingin melihat "apa yang bisa ditampilkan aplikasi" — trajectory agent (berpikir, tool call, pencarian web, memori,
skill) dan render kaya (teks, gambar, diagram, chart, tabel, kode, kartu berkas, izin, pertanyaan) — tanpa harus memancing
model. Aturan proyek melarang UI/data palsu (ADR-018); demo yang jelas berlabel dan diminta pemilik adalah pratinjau, bukan
data palsu yang menyamar sebagai kenyataan.

## Keputusan

- Tiga sesi demo **statis** di sidebar, bagian "Demo (disematkan)" (permanen: tanpa menu ganti nama/hapus): **Trajectory &
  Tools**, **Render Kaya**, **Izin & Pertanyaan**. Definisi di `demo-sessions.ts`; aset gambar di `demo-assets.ts`.
- Bukan sesi engine: memakai kunci "project" khusus (`DEMO_KEY`), tidak muncul di riwayat engine, tidak dikirim ke engine, tidak
  menampilkan composer. Dirender dengan komponen transkrip yang sama (`Transcript`) agar tampilan identik dengan sesi nyata.
- Berlabel "Demo" (banner + lencana + ikon sematan di tab). Kartu izin/pertanyaan bisa dijawab; jawabannya hanya ditampilkan
  lewat toast ("tidak dikirim ke engine"), dan tombol "Atur ulang demo" memulihkannya.
- Fitur render baru yang ikut dibuat (nyata, bukan khusus demo): **gambar** di respons (Markdown `![](data:image/...)` dan part
  berkas gambar) dan **kartu berkas** untuk part `file` (ikon menurut ekstensi; berkas `file://` di dalam project bisa dibuka).
  Gambar **hanya** dimuat dari data: URI bertipe gambar (png/jpeg/gif/webp/svg); URL jaringan tidak diambil (cegah eksfiltrasi).
- Chart memakai Mermaid (`pie`, `xychart-beta`).

## Konsekuensi

Sesi demo menambah ±30 KB kode dan tidak mengubah data pengguna. Dapat disembunyikan lewat pengaturan di rilis berikutnya bila
mengganggu (belum ada). Dokumen: `docs/features/chat.md`.
