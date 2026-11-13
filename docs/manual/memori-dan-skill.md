# Memori dan Skill

Buka lewat ikon otak di footer sidebar, palet perintah ("Kelola memori & skill"), atau `#/memory`.

## Opt-in

Memori dan skill creator **mati secara bawaan**. Urutan penentuan: **sesi → project → bawaan**.

- **Bawaan / per project:** halaman Memori & Skill → tab **Pengaturan**.
- **Per sesi:** panel Tools → **Info sesi** → "Memori sesi ini". Baris "Sekarang" menunjukkan hasil akhirnya.
- Perubahan berlaku di giliran chat berikutnya (tanpa restart engine).

## Memori

Memori = fakta singkat yang disuntikkan ke setiap giliran chat (maks ±6000 karakter): **global** (semua project) dan
**project**. Agent bisa menulisnya sendiri lewat tool `memory_write` dan mencarinya lewat `memory_search`.

- Tab **Memori**: pilih berkas (global, per project, sematan sesi), edit, **Simpan**, atau **Hapus berkas**.
- Format: satu fakta per baris diawali `- `.
- **Sematan sesi** (`pins/<id>.md`) ikut dipertahankan saat riwayat dipadatkan (kompaksi).
- Teks yang tampak memuat secret (kunci API, token, kunci privat, URL berkredensial, dsb.) **ditolak** saat disimpan.

## Skill creator

1. Saat aktif, Zandor mencatat **pemicu**: koreksi Anda ("salah", "bukan begitu", …), perintah `/reflect`, atau ≥ 5 tool
   call dalam satu sesi.
2. Model diminta mengusulkan skill lewat tool `skill_propose` (nama kebab-case, deskripsi, langkah). Draf yang memuat
   secret ditolak otomatis.
3. Draf muncul di tab **Antrean skill** dengan status "Menunggu persetujuan". Anda bisa mengubah nama/deskripsi/isi,
   memilih tujuan (**Global** atau **project**), lalu **Setujui & pasang**, **Tolak**, atau **Hapus entri**.
4. Skill yang disetujui menjadi `SKILL.md` yang ditemukan engine; tidak ada skill aktif sebelum Anda setujui.
5. Tab **Skill terpasang**: lihat, **Ubah** (divalidasi ulang), atau **Hapus skill**.

## Lokasi data

| Data                        | Lokasi                                                                                              |
| --------------------------- | --------------------------------------------------------------------------------------------------- |
| Memori, pengaturan, antrean | `<data aplikasi>/memory/` (`global.md`, `projects/`, `pins/`, `settings.json`, `skill-queue.jsonl`) |
| Skill global                | `<data aplikasi>/engine/config/opencode/skills/<nama>/SKILL.md`                                     |
| Skill project               | `<folder project>/.opencode/skills/<nama>/SKILL.md`                                                 |

`settings.json`: `{ "defaultEnabled": false, "projects": { "<key>": true }, "sessions": { "<id>": false }, "skillCreator": false }`
(`key` = sha256 16 heks dari path project yang dinormalisasi; `meta.json` memetakan key → path).

Semua data ada di **mesin engine** (lokal-first). Di build web/pratinjau, halaman ini menampilkan label bahwa data tidak
dibaca engine.
