# Referensi pengaturan

> Dihasilkan dari `packages/core/src/settings.ts` oleh `bun scripts/gen-settings-docs.ts`. Jangan diedit manual.
> Skema lengkap (JSON Schema 2020-12): [`settings.schema.json`](settings.schema.json).

Pengaturan disimpan sebagai satu objek JSON. Properti yang tidak ada memakai default; properti tak dikenal ditolak.

| Kunci | Tipe | Nilai yang diizinkan | Default | Keterangan |
| --- | --- | --- | --- | --- |
| `language` | string | `id`, `en` | `"id"` | Bahasa antarmuka. Pada pemakaian pertama mengikuti bahasa sistem bila didukung. |
| `theme` | string | `dark`, `light`, `system` | `"system"` | Tema warna. `system` mengikuti preferensi sistem operasi. |
| `uiScale` | number | `0.75`, `0.875`, `1`, `1.125`, `1.25`, `1.375`, `1.5` | `1` | Skala antarmuka (token --ui-scale). Tidak memengaruhi isi browser bawaan. |
| `rememberLayout` | boolean | boolean | `true` | Menyimpan ukuran dan keadaan lipat panel antar sesi aplikasi. |

## Contoh

```json
{ "language": "en", "theme": "dark", "uiScale": 1.25, "rememberLayout": true }
```
