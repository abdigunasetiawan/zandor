---
name: zandor-xlsx
description: "Membuat, membaca, atau mengubah spreadsheet Excel (.xlsx/.xlsm) dan CSV: isi sel, rumus, lembar, format, membersihkan data, merangkum tabel. Pakai setiap kali user menyebut Excel, .xlsx, spreadsheet, atau lembar kerja."
---

# Spreadsheet Excel (.xlsx)

Skill bawaan Zandor. Folder skill: `{{SKILL_DIR}}`.

## Prinsip

- **Cadangan dulu**: salin ke `<nama>.bak.xlsx` sebelum menimpa, atau tulis ke berkas baru.
- **Pakai rumus Excel, bukan angka hasil hitungan Python**, untuk nilai turunan (total, rata-rata, persen) agar
  spreadsheet tetap hidup. Hitung di Python hanya untuk memeriksa.
- `openpyxl` **tidak menghitung rumus**. Nilai cache rumus bisa usang sampai berkas dibuka di Excel/LibreOffice;
  sampaikan ini kepada user.
- Jangan pernah memuat dengan `data_only=True` lalu menyimpan: semua rumus akan hilang menjadi nilai.
- `.xlsm`: muat dengan `keep_vba=True` agar makro tidak hilang.

## Lingkungan

Membaca struktur/isi dan memverifikasi hasil **tidak butuh Python**: pakai pemeriksa bawaan Zandor
`{{OFFICE_CHECK}} <berkas> [--text] [--expect "teks"]` (laporan JSON). Python hanya dibutuhkan untuk mengubah atau
membuat berkas; bila tugasnya hanya membaca, lewati langkah di bawah.

1. Cari Python: `python --version`, `py -3 --version`, `python3 --version`. Bila tidak ada, berhenti dan katakan jujur bahwa
   mengubah berkas butuh Python 3 (membaca dan verifikasi tetap bisa).
2. `python -c "import openpyxl"`; bila gagal, minta izin lalu `python -m pip install --user openpyxl`
   (tambahkan `pandas` hanya bila perlu analisis besar).

## Alur

1. **Baca dulu**: `{{OFFICE_CHECK}} data.xlsx --text` (lembar, jumlah sel, jumlah rumus).
   Untuk melihat isi terstruktur:

   ```python
   from openpyxl import load_workbook
   wb = load_workbook("data.xlsx")
   ws = wb["Sheet1"]
   for row in ws.iter_rows(min_row=1, max_row=10, values_only=True): print(row)
   ```
2. **Ubah** (skrip sementara di folder temp):

   ```python
   ws["D2"] = "=B2*C2"            # rumus, bukan angka
   ws.append(["Baru", 3, 1500])   # baris baru
   ws["D2"].number_format = "#,##0"
   wb.save("data.xlsx")
   ```

   Saat menambah baris, perbarui juga rentang rumus agregat (mis. `=SUM(D2:D10)` → `D2:D11`).
3. **Verifikasi wajib**: `{{OFFICE_CHECK}} data.xlsx --expect "Baru"` → `valid: true`.
   Bila LibreOffice tersedia, hitung ulang untuk memeriksa galat rumus:
   `soffice --headless --convert-to xlsx --outdir <temp> data.xlsx`, lalu muat hasilnya dengan `data_only=True`
   dan pastikan tidak ada `#REF!`, `#DIV/0!`, `#NAME?`, `#VALUE!`.
4. **Laporkan**: berkas hasil, sel/lembar yang berubah, cadangan, dan apakah rumus sudah dihitung ulang atau belum.

## CSV

Untuk `.csv`/`.tsv`, pakai modul `csv` bawaan (pertahankan delimiter dan encoding asli, cek BOM UTF-8).
