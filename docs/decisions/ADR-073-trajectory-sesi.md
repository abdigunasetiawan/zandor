# ADR-073 — Trajectory sesi di Panel 4

- **Tanggal:** 16-11-2026
- **Status:** diterima

## Konteks

Pengguna ingin melihat proses di balik layar agent secara transparan dan rinci: setiap giliran, setiap panggilan
model, berpikir, tool call beserta input/output, durasi, dan token. Panel "Aktivitas sesi" yang ada hanya garis waktu
ringkas. DeepSeek Harness (MIT, `reference/deepseek-harness/packages/client/ui-trajectory`) punya tampilan *Trajectory*:
buku besar per giliran + ikhtisar waktu tiga jalur + inspektur rekaman. Konsepnya dipakai; kodenya ditulis ulang
(tidak disalin) di atas model data Zandor.

## Keputusan

- Alat baru **Trajectory** (`tools/TrajectoryPanel.tsx`) di Panel 4, ikon di Rel (Panel 5).
- Logika murni `shell/trajectory.ts` memetakan `ChatMessage[]` → giliran → rekaman berjenis: Pengguna, Konteks (teks
  `synthetic` dan lampiran), Model (satu per pesan assistant: provider/model, agent, finish, token, biaya, durasi),
  Berpikir, Asisten, Tool (input JSON, keluaran/galat, waktu dari `state.time`), Langkah (`step-finish` + token),
  Galat, dan part `other` (ditampilkan dengan `rawType` apa adanya).
- **Tanpa data karangan** (ADR-018): rekaman tanpa stempel waktu (berpikir, jawaban) tidak diberi durasi dan tidak
  muncul di ikhtisar mode Durasi; mode Urutan menampilkan semua rekaman berurutan.
- Bilah alat: lipat semua giliran, tampil/sembunyi Konteks/Berpikir/Tool/Langkah, mode ikhtisar, salin seluruh
  trajectory sebagai JSON, pencarian (ringkasan, nama, input, output).
- Inspektur: tab Ringkasan (jenis, status, mulai/selesai, durasi, token, biaya, id), Input, Output, Raw (JSON part).
- Daftar mengikuti ekor selama sesi berjalan, berhenti mengikuti bila pengguna menggulir ke atas.

## Batasan

- System prompt dan daftar tool yang dikirim engine ke provider **tidak** tersedia lewat API pesan opencode, jadi
  tidak ditampilkan (berbeda dengan DeepSeek Harness). Subcall dari tool `task` (sesi anak) belum digabungkan.
- Belum ada virtualisasi; sesi sangat panjang bisa berat.
