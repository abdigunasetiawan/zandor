# ADR-066 — Pintasan panel bernomor dan smart resize

Tanggal: 16-11-2026 · Status: diterima

## Konteks

Pemilik ingin setiap panel bisa ditampilkan/disembunyikan dengan kombinasi angka 1–5, plus satu pintasan dan tombol
"smart resize" yang membagi ulang lebar panel yang sedang aktif. Sebelumnya pintasan panel tidak seragam (`Ctrl+B`,
`Ctrl+Alt+B`, `Ctrl+J`; Chat dan Rel tanpa pintasan), dan pengguna bisa menutup semua panel utama sehingga hanya sidebar
(atau sidebar + rel) yang tersisa.

## Keputusan

- **Penomoran**: 1 Sidebar, 2 Chat, 3 Dokumen, 4 Tools, 5 Rel. Pintasan `Ctrl+1` … `Ctrl+5` menggantikan
  `Ctrl+B`/`Ctrl+Alt+B`/`Ctrl+J` (satu perintah = satu pintasan; tetap bisa diubah di Pengaturan → pintasan).
- **Aturan visibilitas** (`canHidePanel` di `@zandor/core`): panel 2/3/4 adalah _panel utama_; minimal satu harus terlihat.
  Menyembunyikan panel utama terakhir ditolak dengan toast. Akibatnya kombinasi "hanya 1", "hanya 5", atau "hanya 1 dan 5"
  tidak pernah terjadi; hanya panel 2/3/4 yang boleh penuh sendirian. `apply()` di `ShellLayout` juga menjaga aturan ini
  (mis. state tersimpan lama yang melipat semua panel utama → Chat dibuka).
- **Smart resize** (`smartLayout`, fungsi murni bertes) — `Ctrl+Alt+0`, tombol paling awal di grup toggle panel title bar,
  menu Tampilan, dan palet perintah:
  - Sidebar mendapat lebar "secukupnya": 16rem, dibatasi 22% lebar grup, tidak kurang dari minimumnya (12rem).
  - 1 panel utama → seluruh sisa lebar; 2 panel utama → 50/50; 3 panel utama → Chat lebih besar (4:3:3 ≈ 40/30/30),
    Dokumen dan Tools sama besar.
  - Minimum lebar tiap panel dihormati: kekurangan diambil dari panel utama lain yang masih di atas minimumnya.
  - Rel (5) tidak resizable, jadi tidak ikut dibagi. Smart resize tidak membuka/menutup panel; hanya membagi lebar.
- Ikon tombol: `SmartLayoutIcon` (lucide `Columns3Cog`) di `packages/ui/src/icons.tsx`.

## Akibat

- `Ctrl+B`, `Ctrl+Alt+B`, `Ctrl+J` kini bebas (mis. untuk editor). Pengguna lama perlu membiasakan diri dengan angka.
- Smart resize dijalankan atas permintaan, bukan otomatis setiap toggle, agar ukuran hasil seret pengguna tidak hilang
  diam-diam.
- Tes: `packages/core/src/layout-plan.test.ts` (aturan + pembagian), e2e `apps/desktop/e2e/shell.spec.ts`
  (pintasan 1–5, penolakan panel utama terakhir, urutan tombol, pembagian lebar nyata).
