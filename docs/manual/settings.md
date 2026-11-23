# Referensi pengaturan

> Dihasilkan dari `packages/core/src/settings.ts` oleh `bun scripts/gen-settings-docs.ts`. Jangan diedit manual.
> Skema lengkap (JSON Schema 2020-12): [`settings.schema.json`](settings.schema.json).

Pengaturan disimpan sebagai satu objek JSON. Properti yang tidak ada memakai default; properti tak dikenal ditolak.

| Kunci | Tipe | Nilai yang diizinkan | Default | Keterangan |
| --- | --- | --- | --- | --- |
| `language` | string | `id`, `en` | `"id"` | Bahasa antarmuka. Pada pemakaian pertama mengikuti bahasa sistem bila didukung. |
| `onboarding` | boolean | boolean | `true` | Tampilkan tur pengenalan (nama, model, project, pintasan) saat aplikasi dibuka tanpa project. Mati otomatis setelah tur selesai atau dilewati; bisa dibuka lagi dari palet perintah. |
| `agentControl` | boolean | boolean | `true` | Izinkan agent memakai tool `zandor_app` dan `zandor_image`: membuka/menutup panel dan alat, membuka URL di browser Panel 4 dan berkas di Panel 3, menutup terminal, mengubah pengaturan (kecuali pengaturan ini), dan membuat gambar dengan model gambar bawaan. Agent tidak bisa mengubah pengaturan ini sendiri. |
| `browserAgent` | boolean | boolean | `false` | Izinkan agent memakai tool `zandor_browser`: membuka halaman, membaca isi halaman (snapshot), dan mengambil screenshot di tab Agent browser Panel 4 (profil terpisah, tanpa login Anda). Tiap domain butuh izin Anda saat pertama dibuka; situs keuangan/pembayaran diblokir. Mati secara bawaan; agent tidak bisa mengubah pengaturan ini sendiri. |
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
| `terminalFontSize` | number | `11`, `12`, `13`, `14`, `16` | `13` | Ukuran huruf terminal (satuan titik). |

## Contoh

```json
{ "language": "en", "theme": "dark", "uiScale": 1.25, "rememberLayout": true }
```
