---
name: zandor-app
description: "Mengoperasikan dan mengonfigurasi aplikasi Zandor tempat agent berjalan: membuka/menutup panel, membuka berkas di Panel 3 atau URL di browser Panel 4, membuka alat (terminal, git, jobs), menutup terminal, mengubah pengaturan (tema, bahasa, zoom, notifikasi, …), menjalankan perintah palet, dan membuat gambar. Pakai bila user meminta sesuatu tentang tampilan atau pengaturan Zandor, atau meminta dibuatkan gambar."
---

# Mengoperasikan Zandor

Skill bawaan Zandor. Kamu berjalan di dalam aplikasi desktop Zandor. Dua tool plugin mengendalikan aplikasinya
(bukan sistem berkas):

- `zandor_app` — tampilan dan pengaturan.
- `zandor_image` — membuat gambar dengan model gambar bawaan pengguna.

Bila tool menjawab "Aplikasi Zandor tidak merespons" atau "dimatikan pengguna", sampaikan apa adanya. Jangan mencoba
mengubah pengaturan `agentControl`: itu izin milik pengguna dan akan ditolak.

## Tata letak

| Panel | Isi |
| --- | --- |
| `sidebar` (Panel 1) | project dan sesi |
| `chat` (Panel 2) | percakapan |
| `docs` (Panel 3) | berkas/dokumen yang dibuka |
| `tools` (Panel 4) | alat: `terminal`, `jobs`, `git`, `changes`, `files`, `browser`, `info`, `session`, `trajectory`, `log` |
| `rail` | deret ikon alat |

## Resep

1. **Mulai dengan `status`** bila perlu tahu panel mana yang terbuka, alat aktif, project, dan dokumen yang terbuka.
2. Panel: `{action: "panel", panel: "docs", state: "open"}` (`open`/`close`/`toggle`).
3. Alat Panel 4: `{action: "open_tool", tool: "terminal"}`.
4. Pratinjau web: jalankan server dengan `job_start`, lalu `{action: "open_url", url: "http://localhost:5173"}`.
5. Tampilkan berkas ke user: `{action: "open_file", path: "src/App.tsx"}` (relatif ke folder project).
6. Terminal: `list_terminals` lalu `{action: "close_terminal", id: "…"}`. Untuk proses latar milikmu sendiri pakai
   `job_stop`, bukan menutup terminal.
7. Pengaturan: `get_settings` untuk melihat kunci dan nilai saat ini, lalu `{action: "set_setting", key: "theme",
   value: "light"}`. Nilai divalidasi skema; galat validasi dikembalikan apa adanya. Contoh kunci: `theme`
   (`dark|light|system`), `language` (`id|en`), `uiScale`, `notifySound`, `terminalFontSize`.
8. Halaman: `{action: "navigate", page: "settings/models"}` (juga `settings`, `settings/chat`, `settings/documents`,
   `settings/engines`, `settings/profile`, `memory`, `automation`, `appearance`, `home`).
9. Perintah lain: `list_commands` lalu `{action: "run_command", id: "view.smartResize"}`.

## Gambar

`zandor_image {prompt, size?, path?}` membuat gambar dengan model gambar bawaan (Pengaturan → Provider dan model →
Model generate gambar bawaan), menyimpannya di folder project (bawaan `images/<nama>-<waktu>.png`), lalu membukanya
di Panel 3. Tulis prompt yang deskriptif (subjek, gaya, komposisi, warna). Bila model belum dipilih atau provider tidak
mendukung gambar, sampaikan galatnya dan arahkan user ke pengaturan tersebut — jangan membuat gambar palsu (mis. SVG)
kecuali user memintanya.

## Batas

- Hanya aksi tampilan/aplikasi. Membaca/menulis berkas dan menjalankan perintah tetap lewat tool engine biasa (yang
  meminta izin user).
- Setiap aksi yang mengubah tampilan memunculkan pemberitahuan singkat ke user. Lakukan hanya yang diminta.
