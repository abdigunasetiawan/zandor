# ADR-070 — Skill bawaan dokumen: DOCX, XLSX, PPTX, PDF

- **Tanggal:** 16-11-2026
- **Status:** diterima

## Konteks

Panel 3 hanya **menampilkan** DOCX/XLSX/PPTX/PDF (ADR-035, ADR-039). Bila user meminta agent mengubah dokumen
semacam itu, model harus menebak sendiri caranya: skrip ad-hoc, pustaka yang mungkin tidak terpasang, dan tanpa
verifikasi. Akibatnya berkas bisa rusak diam-diam (misalnya XML DOCX ditulis seperti teks biasa) atau rumus Excel
hilang. Aplikasi agent lain menyelesaikan ini dengan skill per format plus pemeriksa. Catatan referensi
`reference/deepseek-harness/.agents/notes/archived/feature/2026-09-15-bundled-office-skills.md` memakai pola yang sama.
Itu hanya dibaca sebagai ide; tidak ada kode yang disalin.

## Keputusan

- Empat skill bawaan dengan nama berawalan `zandor-`: `zandor-docx`, `zandor-xlsx`, `zandor-pptx`, `zandor-pdf`.
  Skill dipisah per format supaya model hanya memuat panduan yang relevan.
- Sumber ada di `apps/desktop/src-tauri/bundled-skills/` dan ditanam ke biner (`include_str!`). Modul `bundled_skills.rs`
  menyinkronkannya ke folder skill global engine (`<engine_root>/config/opencode/skills/`) **setiap kali engine dimulai**.
  Berkas hanya ditulis bila isinya berbeda. `{{SKILL_DIR}}` diganti dengan path nyata, dan CRLF dinormalisasi.
- Pemeriksa bersama `office_check.py` hanya memakai pustaka standar Python. Isinya: ZIP/XML/relasi internal OOXML,
  ringkasan struktur, ekstraksi teks (termasuk header/footer, shared strings), assertion `--expect`, serta header/trailer
  dan perkiraan halaman PDF. Hasilnya berupa laporan JSON dengan kode keluar 0/1.
- Isi skill mewajibkan: cadangan sebelum menimpa, edit lewat python-docx/openpyxl/python-pptx/pypdf, verifikasi dengan
  pemeriksa sebelum diserahkan, dan laporan jujur (tampilan visual dan hitung ulang rumus tidak diperiksa kecuali
  LibreOffice tersedia).
- Python **tidak dibundel**. Skill mendeteksi `python`/`py -3`/`python3` dan meminta izin sebelum `pip install --user`.

## Alternatif yang ditolak

- **Membundel Python + pustaka** (±40–80 MB): ditunda. Ini bisa menjadi tahap berikutnya bila ternyata banyak user
  tidak punya Python.
- **Tool engine khusus (TypeScript) untuk mengedit Office**: biaya pemeliharaan besar dan kurang lentur dibanding
  pustaka Python yang matang.
- **Satu skill gabungan**: menambah konteks yang tidak relevan untuk setiap tugas.

## Konsekuensi

- Skill tampil di "Skill terpasang". Menghapus atau mengubahnya tidak permanen karena akan dipulihkan saat engine
  berikutnya dimulai.
- Kualitas hasil tetap bergantung pada model dan ketersediaan Python di mesin user.
- Belum ada pratinjau sebelum/sesudah atau muat ulang otomatis Panel 3 setelah agent mengubah berkas Office.
