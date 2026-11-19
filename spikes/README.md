# Spikes Fase 0

Kode sekali pakai untuk membuktikan risiko. **Bukan kode produk** — jangan diimpor ke `apps/` atau
`packages/`. Hasil setiap spike ada di `docs/decisions/ADR-*.md`.

| Spike | Folder | ADR | Status |
|---|---|---|---|
| S1 sidecar `opencode serve` | `s1-sidecar/` | ADR-019 | Windows lulus · mac/Linux ditunda (tidak ada perangkat) |
| S2 pemetaan event SSE | `s2-events/` | ADR-021 | Lulus (rekaman nyata, 12/12 test) · beberapa jenis part belum terekam |
| S4 secret keychain + injeksi | `s4-secrets/` | ADR-022 | Windows lulus (6/6) · mac/Linux ditunda |
| S6 plugin memori | `s6-memory-plugin/` | ADR-023 | Lulus (10/10, opencode asli + mock model) |
| S7 terminal xterm.js + PTY | `s7-terminal/` | ADR-024 | Windows lulus (A & B) · mac/Linux ditunda |
| S8 layout 5 panel + zoom | `s8-layout/` | ADR-025 | Windows 100% lulus (15/15 ×3) · HiDPI, mac/Linux belum |
| S5 renderer Panel 3 | `s5-renderers/` | ADR-026 | PDF/DOCX/XLSX lulus · PPTX menunggu keputusan MPL-2.0 |
| S3 child webview browser | `s3-webview/` | ADR-020 | Windows lulus (termasuk login Google manual) · persistensi sesi belum konsisten · mac/Linux ditunda |
| S9 browser-use agent (CDP, dua profil, snapshot) | `s9-browser-agent/` | ADR-079 | Windows lulus (15/15) · crate mandiri: `cargo build` di foldernya lalu jalankan `zandor-spike-s9.exe` |

## Prasyarat
Rust (stable), Node ≥ 20, npm. Windows butuh WebView2 runtime.

## Menjalankan
```
node tools/fetch-opencode.mjs          # unduh opencode versi pin → s1-sidecar/src-tauri/binaries/
cargo test -p zandor-engine-supervisor -- --test-threads=1
cargo build -p zandor-spike-s1 -p zandor-spike-s3
cargo test -p zandor-spike-s4 -- --test-threads=1   # keychain asli + engine + mock provider
ZANDOR_SELFTEST=/tmp/s1.json ./target/debug/zandor-spike-s1
ZANDOR_SELFTEST=/tmp/s3.json ./target/debug/zandor-spike-s3
```
Tanpa `ZANDOR_SELFTEST`, app terbuka untuk uji manual.

S7 (butuh Bun untuk aset xterm.js):
```
cd s7-terminal && bun install && node tools/vendor.mjs && cd ..
cargo build -p zandor-spike-s7 && ZANDOR_SELFTEST=/tmp/s7.json ./target/debug/zandor-spike-s7
```

S8 (butuh Bun; UI ditanam ke binary, jadi build UI dulu):
```
cd s8-layout && bun install && bun run build && cd ..
cargo build -p zandor-spike-s8 && ZANDOR_SELFTEST=/tmp/s8.json ./target/debug/zandor-spike-s8
```

S6 (butuh Bun + `cargo build -p zandor-spike-s1` lebih dulu):
```
cd s6-memory-plugin && bun install && bun test && bun x tsc --noEmit
```

S2 (butuh Bun):
```
cd s2-events && bun install && bun test && bun x tsc --noEmit
node tools/record.mjs --model opencode/big-pickle   # rekam ulang fixture (memakai provider user)
```

`tools/make-icon.mjs` hanya membuat ikon placeholder untuk build spike.
