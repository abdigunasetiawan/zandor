# Arsitektur — Zandor

> Status: Fase 1. Bagian bertanda _(rencana)_ belum diimplementasikan di kode produk; buktinya ada di spike
> dan ADR yang dirujuk.

> **Pembaruan 19-11-2026 (ADR-078):** pengembangan aktif pindah ke Rust GPUI (`apps/desktop-gpui`, lihat `docs/features/gpui.md`). Diagram dan bagian bertanda Tauri/React/webview di bawah menggambarkan versi React yang kini dibekukan sebagai referensi paritas; kontrak dengan engine opencode, plugin, dan keychain tidak berubah. Di GPUI tidak ada webview: dokumen dan UI dirender native.

## Gambaran

```
┌──────────────── Zandor Desktop (Tauri 2.12.5, Windows) ────────────────┐
│ apps/desktop/src        UI React 19 + Vite (webview "main")            │
│   packages/ui           komponen + error boundary per panel            │
│   packages/i18n         ID/EN, kunci bertipe                           │
│   packages/core         logger (redaksi secret), build info,           │
│                         client engine & state (rencana, Fase 4–5)      │
│ apps/desktop/src-tauri  Rust: supervisor engine, keychain, child       │
│                         webview browser (rencana, Fase 4/6/8)          │
└───────────────┬────────────────────────────────────────────────────────┘
                │ HTTP + SSE (fetch stream) + WebSocket PTY, loopback + token
┌───────────────▼──────────────────────────┐
│ opencode serve 1.19.5 (sidecar, di-pin)  │
│ + packages/plugin (dimuat lewat           │
│   OPENCODE_CONFIG_CONTENT)                │
└──────────────────────────────────────────┘
```

## Struktur repo

| Path              | Isi                                                                                   |
| ----------------- | ------------------------------------------------------------------------------------- |
| `apps/desktop`    | Aplikasi Tauri: `src/` (UI), `src-tauri/` (Rust), `e2e/` (Playwright)                 |
| `packages/core`   | Logika tanpa UI: logger, build info, shortcut, perintah, layout, navigasi, pengaturan |
| `packages/ui`     | Komponen React. Sekarang: `ErrorBoundary`                                             |
| `packages/i18n`   | Katalog ID (sumber kebenaran) + EN, `createTranslator`, `I18nProvider`                |
| `packages/plugin` | Plugin opencode Zandor. Sekarang: guard `shell.env`                                   |
| `scripts/`        | Audit lisensi, utilitas repo                                                          |
| `spikes/`         | Kode spike Fase 0 (referensi, tidak dipakai produk)                                   |
| `docs/`           | Dokumentasi (lihat `docs/README.md`)                                                  |

## Kontrak engine (dari Fase 0)

- **Proses** (ADR-019): sidecar `opencode serve --hostname 127.0.0.1 --port 0`, token acak per start lewat
  `OPENCODE_SERVER_PASSWORD` (HTTP Basic `opencode:<token>`), `OPENCODE_DISABLE_AUTOUPDATE=1`, Job Object
  `KILL_ON_JOB_CLOSE`, versi diverifikasi lewat `/global/health`. _(rencana, Fase 4)_
- **Event** (ADR-021): `GET /event?directory=…` via `fetch` stream (bukan `EventSource`), tipe dari
  `@opencode-ai/sdk@1.19.5`, reducer "part utuh menang + delta untuk streaming". _(rencana, Fase 5)_
- **Secret** (ADR-022): keychain OS → env `ZANDOR_SECRET_<ID>` + placeholder `{env:…}` di
  `OPENCODE_CONFIG_CONTENT`. Plugin guard `shell.env` **wajib**. Jangan memakai `OPENCODE_AUTH_CONTENT`.
- **Plugin** (ADR-023): satu bundel `packages/plugin`, dimuat dengan opsi lewat config. _(guard: ada;
  memori: Fase 9)_
- **Terminal** (ADR-024): API `/pty` engine (WebSocket + tiket sekali pakai). _(rencana, Fase 8)_
- **Browser** (ADR-020/025): child webview `browser` tanpa IPC; disembunyikan saat P4 terlipat atau
  tertimpa overlay. _(rencana, Fase 8)_

## Keamanan IPC

Capability Tauri hanya untuk webview `main` (UI lokal). Tidak boleh ada `remote.urls`. Halaman remote
(browser bawaan) tidak punya akses perintah (dibuktikan di ADR-020).

## Versi & build

- Semver di `apps/desktop/package.json` (dipakai juga oleh `tauri.conf.json`). Build dev menampilkan
  `0.0.<jumlah commit>` (ADR-016).
- Aset UI ditanam ke binary saat `cargo build`/`tauri build`; selalu build UI dulu (ADR-025).
