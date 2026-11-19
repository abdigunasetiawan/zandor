# AGENTS.md — Zandor

**Fokus pengembangan: Rust GPUI (`apps/desktop-gpui`), bukan Tauri/React lagi** (ADR-078).

Baca **[`docs/agents.md`](docs/agents.md)** lebih dulu: aturan, urutan baca, dan cek wajib sebelum
menyerahkan pekerjaan. Kondisi terkini ada di [`docs/context/current.md`](docs/context/current.md).

**Pekerjaan UI** (membangun/mengubah/memperbaiki komponen, halaman, CSS, token, ikon, teks i18n, tema) **wajib memakai skill
`zandor-ui`** (`.claude/skills/zandor-ui/SKILL.md`) secara otomatis: cek dulu desain sistem (elemen sudah ada? token sesuai?)
sebelum menulis kode.

Jawab dan dokumentasikan dalam Bahasa Indonesia. Jujur bila ada yang gagal atau rencana kurang tepat.
