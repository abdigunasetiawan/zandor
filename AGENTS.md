# AGENTS.md — Zandor

**Fokus pengembangan: Rust GPUI (`apps/desktop-gpui`), bukan Tauri/React lagi** (ADR-078).

Baca **[`docs/agents.md`](docs/agents.md)** lebih dulu: aturan, urutan baca, isolasi development, dan cek wajib sebelum
menyerahkan pekerjaan. Kondisi terkini ada di [`docs/context/current.md`](docs/context/current.md).

- **Isolasi total dari Zandor terpasang:** JANGAN PERNAH menyentuh/menghentikan/mengubah aplikasi Zandor produksi (`Zandor.exe`, `%LOCALAPPDATA%\Programs\Zandor`, `%APPDATA%\Zandor`). Pengembangan hanya di **Zandor Dev** (`target\run\zandor-gpui.exe` + `%APPDATA%\ZandorDev`).
- **Build & laporan versi dev (wajib):** saat selesai mengerjakan fitur/perbaikan, jalankan `bun scripts/bump-dev-build.ts`, biarkan `dev.cmd` melakukan rebuild inkremental cepat (`lld-link` + `CARGO_INCREMENTAL=1`), dan **wajib sertakan versi keluaran di akhir pesan** (`Versi Dev: v... · dev.N`) agar pemilik bisa mencocokkannya dengan label di bawah username pada panel Sidebar. Saat build rilis (`--release` / `bun scripts/rilis.ts`), label ini otomatis hilang.

**Pekerjaan UI** (membangun/mengubah/memperbaiki komponen, halaman, CSS, token, ikon, teks i18n, tema) **wajib memakai skill
`zandor-ui`** (`.claude/skills/zandor-ui/SKILL.md`) secara otomatis: cek dulu desain sistem (elemen sudah ada? token sesuai?)
sebelum menulis kode.

Jawab dan dokumentasikan dalam Bahasa Indonesia. Jujur bila ada yang gagal atau rencana kurang tepat.
