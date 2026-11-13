# Audit kriteria sukses MVP (PRD §13)

- **Tanggal audit:** 14-11-2026 · **Hasil:** kelima kriteria terpenuhi dengan bukti di aplikasi asli; batasan jujur di bawah.
- **Bukti utama:** smoke native 40/40 (`apps/desktop/e2e-native/smoke.ts` dengan `ZANDOR_SMOKE_FORCE_CHAT=1`), 466+ tes unit,
  96 e2e, 45 tes Rust, CI hijau sampai Fase 8.

| #   | Kriteria                                                                                            | Status              | Bukti                                                                                                                                                                                                                                                                                                              |
| --- | --------------------------------------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Menambah custom provider/model dan chat dengan streaming                                            | ✅                  | Endpoint kustom OpenAI-compatible (Antigravity) dengan kunci di keychain Windows; model **Gemini 3.8 Flash (High)** dipilih lewat pemilih model; jawaban streaming dari model nyata (smoke: "chat nyata: prompt → jawaban streaming"). UI tambah/ubah/hapus koneksi diuji e2e; migrasi kunci ke keychain terbukti. |
| 2   | Kelima panel berfungsi nyata, tanpa data dummy                                                      | ✅                  | P1 project/sesi/Files nyata; P2 chat nyata; P3 penampil dokumen nyata; P4 terminal PTY, Git, browser, info sesi, log; P5 rel memicu P4. Teks "Segera hadir"/"hadir di Fase N" yang usang dihapus (audit ini). Di luar Tauri fitur native menampilkan pesan jujur, bukan data palsu.                                |
| 3   | Tool call (baca/tulis file, terminal, web) berjalan dan terlihat di chat, termasuk persetujuan izin | ✅                  | Engine + model nyata: tulis file → kartu izin → file ada **setelah** izin; **bash** → kartu izin → dijalankan; **webfetch** ke server lokal → kartu izin → selesai; baca file (glob/read) terlihat sebagai kartu tool. Kartu menampilkan diff, status, masukan/keluaran.                                           |
| 4   | Minimal 3 project dan 10 sesi tanpa crash                                                           | ✅                  | Smoke native (engine nyata): 3 project + 10 sesi dibuat, dilewati dua putaran, **tanpa galat halaman**, bertahan setelah restart aplikasi, dibersihkan. E2E (`mvp.spec.ts`): 3 project, 12 sesi, kirim pesan, pindah-pindah, muat ulang, tanpa galat konsol.                                                       |
| 5   | Setiap fitur MVP punya test otomatis dan dokumen fitur                                              | ✅ (dengan catatan) | Dokumen fitur: `app-shell`, `design-system`, `projects`, `chat`, `providers`, `documents`, `tools`, plus manual pengguna di `docs/manual/`. Tiap fitur punya tes unit/e2e/native (lihat "Cara test" di dokumen masing-masing).                                                                                     |

## Batasan jujur (tidak menggugurkan kriteria, tetapi perlu diketahui)

- **Upstream proxy Antigravity tidak stabil** (bergantian 200/500 per permintaan). Ping model di UI melaporkannya apa adanya;
  bukti chat nyata di atas diperoleh dengan `ZANDOR_SMOKE_FORCE_CHAT=1`. Smoke bawaan melewati fase chat bila ping gagal.
- **Smoke native bukan tes terisolasi**: ia memakai data aplikasi asli (registry, engine, keychain). Pemisahan fase belum dikerjakan.
- **Belum diuji manusia:** fidelitas visual dokumen, login Google di browser tertanam, sembunyi-saat-overlay (airspace),
  model lokal (Ollama/LM Studio tidak ada di mesin pengembang), login OAuth provider sungguhan, Git Bash di terminal.
- Hanya Windows (ADR-027). macOS/Linux ditunda.
- Belum ada: lisensi Zandor sendiri, ikon aplikasi final, installer yang dirilis/ditandatangani, pembaruan otomatis.
- **Versi `0.1.0` belum dirilis**: roadmap mengaitkannya dengan "MVP selesai"; menaikkan versi dan menandai rilis adalah keputusan pemilik.
