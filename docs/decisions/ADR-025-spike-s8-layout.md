# ADR-025 — Spike S8: layout 5 panel + child webview + zoom `--ui-scale`

- **Tanggal:** 13-11-2026
- **Status:** Diterima untuk Windows (skala layar 100%) · macOS/Linux dan HiDPI ditunda
- **Hasil spike:** ✅ Lulus — 15/15 cek, 3 run berturut-turut
- **Kode:** `spikes/s8-layout/` (React 19 + `react-resizable-panels` 4.22, app Tauri turunan S3)
- **Memperjelas:** ADR-014 (zoom via token), ADR-015 (5 panel), ADR-020 (child webview)

## Pertanyaan

Bisakah lima panel di-resize tanpa jank bersama child webview browser, dan apakah zoom `--ui-scale`
tidak merusak layout?

## Keputusan

1. **`react-resizable-panels` `4.22.0`** (MIT). API v4: `Group` / `Panel` / `Separator`,
   `groupRef`/`panelRef` imperatif, `useDefaultLayout` untuk persistensi. Catatan: ini **berbeda dari
   API v2/v3** (`PanelGroup`/`PanelResizeHandle`) yang banyak dicontohkan di internet.
2. **Zoom UI = token `--ui-scale`** pada `html { font-size: calc(16px * var(--ui-scale)) }`, dan **semua
   ukuran UI memakai `rem`**, termasuk `minSize` panel (v4 menerima `"12rem"`). Hasilnya, batas
   minimum panel ikut berskala tanpa kode tambahan. Rentang 75–150%, `Ctrl +/−/0`, tersimpan.
3. **Struktur:** top bar `2.5rem`; Group horizontal P1 sidebar (collapsible, min 12rem) · P2 chat
   (**tidak** collapsible, min 22rem) · P3 dokumen (collapsible, min 16rem) · P4 tools (collapsible,
   min 18rem); P5 rail `3rem` di luar Group.
4. **Browser (child webview) di P4:** bounds disinkron dari `ResizeObserver` host per animation frame
   (pola S3). Browser **disembunyikan** saat P4 terlipat atau saat ada overlay UI (menu/popover/modal)
   yang menimpanya. Ini menjawab masalah airspace dari ADR-020.
5. **Auto-collapse** saat ruang kurang untuk minimum semua panel. Prioritas yang diinginkan:
   P3 → P1 → P4; chat tidak pernah dilipat. Lihat temuan 2.

## Bukti (Windows 11, skala layar 100%, debug build; run 2–4 setelah perbaikan test)

| Cek                                                              | Hasil                                                                                                       |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| 1366×768 @100%: kelima panel terbuka, chat ≥ min, tanpa overflow | ✓ P1 234 · P2 442 · P3 286 · P4 338 px                                                                      |
| Simulasi drag 60 langkah (layout berubah tiap frame)             | jeda frame maks **7–9 ms**, tanpa long task, **0 langkah** bounds browser tertinggal > 2 px setelah 1 frame |
| Drag pointer sungguhan di separator (PointerEvent, 20 langkah)   | P4 tepat +100 px, jeda frame maks 7–11 ms                                                                   |
| Lipat/buka P1, P3, P4                                            | bounds browser selisih ≤ 0,45 px; saat P4 terlipat browser disembunyikan                                    |
| Overlay menu menimpa P4                                          | browser disembunyikan, lalu tampil kembali dengan bounds benar                                              |
| Zoom 75 / 87,5 / 100 / 112,5 / 125 / 137,5 / 150%                | tanpa overflow; tinggi top bar = 2,5 rem × skala; tidak ada panel terbuka di bawah min                      |
| Konten browser saat zoom                                         | `innerWidth` browser = lebar host, `devicePixelRatio` tetap 1 → **halaman web tidak ikut ter-zoom**         |
| Jendela 1000×650 @150%                                           | chat 909 px ≥ min; panel lain terlipat; tanpa overflow                                                      |
| Persistensi: reload webview UI                                   | layout (selisih 0) dan skala 125% kembali persis                                                            |

## Temuan & risiko (jujur)

1. **Layar 1366 px tidak muat lima panel di atas 112,5%.** Jumlah lebar minimum (68 rem + separator)
   melebihi lebar layar: di 125% P3 terlipat, di 150% P1 dan P3 terlipat. Ini konsekuensi angka
   minimum, bukan bug. PRD perlu menerima ini, atau minimum diturunkan.
2. **Urutan auto-collapse belum deterministik.** Saat zoom berubah, library melipat panel collapsible
   sendiri (yang terlipat P1 dulu), sebelum logika prioritas kita (P3 dulu) berjalan. Saat jendela
   dikecilkan, logika kita yang jalan duluan. Produk harus menghitung panel yang dilipat **sebelum**
   menerapkan skala atau ukuran baru, bukan bereaksi setelahnya.
3. Drag di run pertama sempat mencatat jeda frame 40 ms (pemanasan, debug build). Run berikutnya
   7–11 ms. Ambang test 50 ms. Belum diukur di release build atau mesin lebih lemah.
4. **Kesalahan proses di spike ini (bukan temuan produk):** Tauri menanam aset frontend ke binary saat
   `cargo build`. Setelah mengubah UI saya lupa build ulang, jadi beberapa run memakai UI lama dan
   sempat salah didiagnosis. Pipeline produk: build UI → build Tauri, selalu berurutan (Fase 1 CI).
5. Belum diuji: **HiDPI** (skala layar 125/150% — hanya 100% di mesin ini), macOS/Linux, drag dengan
   mouse fisik (yang diuji adalah PointerEvent sintetis), shortcut `Ctrl +/−/0` (kodenya ada, tetapi
   test memanggil `setScale` langsung), resize separator dengan keyboard, aksesibilitas separator,
   jendela frameless/kontrol kustom (Fase 3).

## Konsekuensi

- Fase 2: semua token ukuran dalam `rem`; tidak ada `px` hard-coded kecuali garis 1px/separator.
- Fase 3: shell memakai pola ini; komponen overlay mendaftar ke "overlay manager" yang
  menyembunyikan browser bila menimpa P4; logika auto-collapse dengan prioritas eksplisit.
- PRD §8: tambahkan catatan bahwa di 1366 px dengan zoom > 112,5%, minimal satu panel terlipat otomatis.

## Cara mengulang

```
cd spikes/s8-layout && bun install && bun run build && cd ..
cargo build -p zandor-spike-s8          # WAJIB setelah build UI (aset ditanam ke binary)
ZANDOR_SELFTEST=$TEMP/s8.json ./target/debug/zandor-spike-s8
./target/debug/zandor-spike-s8          # uji manual: geser panel, Ctrl +/−/0, tombol P1/P3/P4, Menu
```
