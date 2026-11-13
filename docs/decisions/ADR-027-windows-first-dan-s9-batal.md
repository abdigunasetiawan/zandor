# ADR-027 — Windows dulu sampai aplikasi selesai; spike S9 dibatalkan

- **Tanggal:** 13-11-2026
- **Status:** Diterima (keputusan pemilik)
- **Mengubah:** ADR-005 (target OS), roadmap Fase 0 (kriteria S1/S3 tiga OS), roadmap S9

## Konteks
- Spike S1, S3, S4, S7, dan S8 lulus di Windows 11, tetapi belum pernah dijalankan di macOS/Linux
  (ADR-019–025). Pemilik tidak punya perangkat macOS/Linux.
- Kriteria Fase 0 di roadmap menuntut tiga OS, sehingga Fase 0 tidak akan pernah bisa lulus.
- S9 (meminta izin tertulis OpenCode untuk model gratis Zen) adalah tugas pemilik, bukan teknis.

## Keputusan
1. **Fokus Windows (x64) sampai aplikasi benar-benar selesai** (MVP Fase 0–8 dan fitur
   setelahnya yang direncanakan). macOS dan Linux **ditunda**, bukan dibatalkan.
2. Kode tetap ditulis lintas platform bila tidak menambah biaya berarti (Rust `cfg(unix)`, path
   dengan `std::path`, tanpa API Windows di lapisan UI), tetapi **tidak diuji dan tidak dijanjikan**.
   Kode khusus Windows (Job Object, `SetConsoleCtrlHandler`, Credential Manager) boleh ditulis
   langsung, dengan padanan unix ditandai `TODO(port)`.
3. CI Fase 1 hanya menjalankan build dan test di **Windows**.
4. Port macOS/Linux menjadi fase tersendiri setelah aplikasi selesai, dengan daftar risiko yang
   sudah diketahui dari spike (ADR-019 proses yatim di macOS, ADR-020 WKWebView/Wayland, ADR-022
   Secret Service di Linux headless, ADR-024 PTY, ADR-025 HiDPI).
5. **S9 dibatalkan.** Tidak ada email ke OpenCode. Konsekuensinya, ADR-008 tetap berlaku penuh:
   model gratis OpenCode Zen **tidak** dijadikan nilai jual, tidak dipromosikan, dan tidak menjadi
   default di Zandor. Kalau user sudah login atau memakai Zen lewat mekanisme OpenCode sendiri,
   Zandor memperlakukannya seperti provider lain.

## Konsekuensi
- Fase 0 dinilai lulus berdasarkan hasil Windows (lihat roadmap).
- PRD §6 dan roadmap diperbarui.
- Risiko spike yang khusus macOS/Linux dipindahkan ke fase port dan tidak menghalangi fase lain.
