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
| `notifyToast` | boolean | boolean | `true` | Tampilkan toast saat sesi selesai, gagal, atau butuh aksi. |
| `notifySystem` | boolean | boolean | `true` | Kirim notifikasi sistem operasi (butuh izin dari Anda). |
| `notifySound` | boolean | boolean | `true` | Putar suara untuk peristiwa sesi. |
| `notifyOnlyAway` | boolean | boolean | `true` | Lewati pemberitahuan untuk sesi yang sedang Anda lihat (jendela fokus). |
| `notifyDone` | boolean | boolean | `true` | Beritahu saat agent selesai bekerja. |
| `notifyFailed` | boolean | boolean | `true` | Beritahu saat sesi gagal (termasuk rate limit/kuota). |
| `notifyAction` | boolean | boolean | `true` | Beritahu saat agent meminta izin atau mengajukan pertanyaan. |
| `notifyVolume` | number | `0.2`, `0.4`, `0.6`, `0.8`, `1` | `0.6` | Volume suara pemberitahuan. |
| `soundDone` | string | `chime`, `pop`, `bell`, `soft`, `alert`, `low`, `none`, `custom` | `"chime"` | Suara saat sesi selesai. |
| `soundFailed` | string | `chime`, `pop`, `bell`, `soft`, `alert`, `low`, `none`, `custom` | `"low"` | Suara saat sesi gagal atau terkena batas. |
| `soundAction` | string | `chime`, `pop`, `bell`, `soft`, `alert`, `low`, `none`, `custom` | `"bell"` | Suara saat izin atau pertanyaan menunggu. |
| `sendKey` | string | `enter`, `ctrl-enter` | `"enter"` | Enter mengirim (Shift+Enter baris baru), atau Ctrl+Enter mengirim (Enter baris baru). |
| `pasteLongText` | string | `attach`, `inline` | `"attach"` | Teks lebih dari 30 baris atau 2.000 karakter dijadikan lampiran teks agar composer tetap rapi. Ctrl+Shift+V selalu menempel apa adanya. |
| `chatWidth` | string | `narrow`, `normal`, `wide` | `"normal"` | Lebar kolom pesan di Panel Chat. |
| `confirmDelete` | boolean | boolean | `true` | Tanyakan dulu sebelum menghapus sesi dari sidebar. |
| `showDemos` | boolean | boolean | `true` | Tampilkan tiga sesi demo bawaan di daftar Chat. |
| `terminalFontSize` | number | `11`, `12`, `13`, `14`, `16` | `13` | Ukuran huruf terminal (satuan titik). |

## Contoh

```json
{ "language": "en", "theme": "dark", "uiScale": 1.25, "rememberLayout": true }
```
