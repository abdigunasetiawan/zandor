# Konvensi Kode — Zandor

## Alat

| Area                        | Alat                                                                    | Perintah                                                     |
| --------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------ |
| Paket JS                    | Bun 1.4 workspaces                                                      | `bun install`                                                |
| Lint + format JS/TS/JSON/MD | Biome 2.6                                                               | `bun run lint`, `bun run format`                             |
| Tipe                        | TypeScript 7 (`strict`, `noUncheckedIndexedAccess`)                     | `bun run typecheck`                                          |
| Test unit/komponen          | Vitest 5 + Testing Library (jsdom per file)                             | `bun run test`                                               |
| e2e                         | Playwright + Microsoft Edge (`channel: msedge`)                         | `bun run e2e`                                                |
| Rust                        | toolchain di-pin (`rust-toolchain.toml`), rustfmt, Clippy `-D warnings` | `cargo fmt`, `cargo clippy`, `cargo test`                    |
| Lisensi                     | `scripts/license-audit.ts`, `cargo-deny`                                | `bun run licenses`, `cargo deny check licenses bans sources` |
| Token desain                | `scripts/check-design-tokens.ts`                                        | `bun run design-tokens`                                      |

## TypeScript & React

- Paket workspace mengekspor sumber `.ts` langsung (tanpa build terpisah); Vite yang membundel.
- Tidak ada `any` baru tanpa alasan tertulis. `console.log` dilarang (Biome); pakai logger `@zandor/core`.
- Semua string UI lewat `t("kunci")`. Kunci baru ditambahkan ke `locales/id.ts` dulu, lalu `en.ts`
  (compiler menolak bila kurang).
- Ukuran UI memakai `rem` (ADR-014/025); `px` hanya untuk garis 1px.
- Setiap panel dibungkus `ErrorBoundary`.
- Data sensitif tidak boleh masuk log; logger menyamarkan, tetapi jangan mengandalkannya.

## Rust

- Crate produk ada di workspace root `Cargo.toml`. `spikes/` punya workspace sendiri dan dikecualikan.
- Kode khusus Windows di bawah `#[cfg(windows)]`; padanan unix ditandai `// TODO(port)`.
- Versi crate kritis di-pin persis (`=x.y.z`): `tauri`, `tauri-build`, dan yang memakai feature `unstable`.

## Test

- Test menguji perilaku nyata, bukan detail implementasi. Fitur baru wajib punya test (roadmap
  "Definisi Selesai").
- Data dummy hanya di test dan halaman design system.
- e2e berjalan terhadap build UI (`vite preview`); engine tiruan untuk e2e dibuat di Fase 4.

## Commit

- Pesan dalam Bahasa Indonesia, baris pertama ringkas (≤ 72 karakter), badan menjelaskan "kenapa".
- Satu commit = satu perubahan logis. Jangan commit file hasil build, `node_modules`, atau binary sidecar.
