# ADR-012 — Browser bawaan memakai child webview Tauri

- **Tanggal:** 13-11-2026
- **Status:** Diterima

## Keputusan

Browser bawaan memakai child webview Tauri (feature `unstable`, versi di-pin). Fallback ke browser sistem kalau tidak didukung (misalnya Linux Wayland)

## Alasan

Iframe diblokir oleh X-Frame-Options

> Dipecah dari tabel `docs/decisions.md` saat Fase 1 tanpa perubahan isi. Perubahan berikutnya dicatat
> sebagai ADR baru yang merujuk ADR ini.
