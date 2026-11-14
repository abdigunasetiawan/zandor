# Automasi Terjadwal (Fase 11)

- **Status:** selesai di lokal dengan batasan (di bawah) · **ADR:** [ADR-039](../decisions/ADR-039-fase-11-dokumen-edit-automasi.md)
- **Manual:** [automasi.md](../manual/automasi.md) · Halaman: `#/automation` (ikon jam di footer sidebar, palet "Automasi terjadwal")

## Spek

- Tugas = nama + project (atau tanpa project) + prompt + jadwal: **interval** (5 mnt–7 hari), **harian** (jam + hari terpilih),
  atau **sekali**. Maks 50 tugas; disimpan di perangkat ini.
- Penjadwal berdetak tiap 30 detik **selama Zandor terbuka**. Tugas yang terlewat saat aplikasi tertutup dijalankan **sekali**
  ketika dibuka; kejadian sebelum tugas dibuat atau saat tugas nonaktif tidak dikejar.
- Saat jalan: sesi baru di project (tanpa memindahkan fokus Anda), prompt dikirim dengan model terpilih, lalu dipantau.
- **Notifikasi** (toast di aplikasi + notifikasi sistem via plugin Tauri bila diizinkan): mulai, selesai, **menunggu persetujuan
  Anda**, gagal. Tugas **tidak pernah menyetujui izin sendiri**.
- "Jalankan sekarang", ubah, aktif/nonaktif, hapus, dan "Buka sesi terakhir". Galat terakhir ditampilkan.

## Cara test

| Lapisan                                            | Perintah                                                  |
| -------------------------------------------------- | --------------------------------------------------------- |
| Unit (jadwal, store, pelaksana tiruan)             | `bun x vitest run packages/core/src/automation`           |
| E2E (form, validasi, persisten, Jalankan sekarang) | `bun x playwright test e2e/automation.spec.ts`            |
| Native (model nyata, plugin notifikasi)            | `ZANDOR_SMOKE_FORCE_CHAT=1 bun e2e-native/smoke.ts <exe>` |

## Batasan jujur

- **Tidak berjalan saat aplikasi tertutup** (butuh layanan OS/daemon; di luar cakupan, dinyatakan di UI).
- Tampilan notifikasi sistem (toast Windows) **tidak diverifikasi secara visual**; yang diuji: plugin tersedia dan
  izin dapat ditanyakan. Toast in-app selalu muncul.
- Tugas yang butuh izin akan menunggu sampai Anda membuka sesinya; tidak ada batas waktu otomatis pada izin.
- Zona waktu mengikuti jam sistem; perubahan jam sistem/DST tidak ditangani khusus.
