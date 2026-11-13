# ADR-022 — Spike S4: secret di keychain OS dan penyuntikan kredensial ke OpenCode

- **Tanggal:** 13-11-2026
- **Status:** Diterima (rencana penyimpanan secret untuk Fase 6)
- **Hasil spike:** ✅ Lulus di Windows 11 (Credential Manager) — rencana tertulis + PoC teruji.
  ⏸️ macOS Keychain & Linux Secret Service ditunda (tidak ada perangkat, sama seperti ADR-019/020).
- **Kode:** `spikes/s4-secrets/` (`src/lib.rs`, `plugin/zandor-env-guard.js`, `tests/secrets.rs`)
- **Memperbarui:** ADR-019 (risiko baru pada token server engine)

## Pertanyaan
Bagaimana menyimpan secret di keychain OS (crate `keyring`), dan bagaimana memberikan kredensial ke
OpenCode tanpa file plaintext dari pihak kita?

## Keputusan

### 1. Penyimpanan
- Crate **`keyring` `=4.2.0`** (MIT/Apache-2.0), mode `v1`: Windows Credential Manager, macOS
  Keychain, Linux Secret Service.
- Service `id.web.zandor`, account `provider:<id>`. Zandor tidak pernah menulis secret ke file,
  config, log, atau localStorage.
- **Secret panjang dipecah.** Credential Manager menolak nilai > 2560 byte (1280 unit UTF-16) —
  terbukti di test (token 2000 karakter ditolak). Entri utama berisi `zandor-chunked:v1:<n>`,
  potongan di `account#1..n` (≤ 1200 unit UTF-16, tidak memotong karakter). Potongan lama dibersihkan
  saat nilai diganti.

### 2. Penyuntikan ke engine (API key & custom endpoint)
- Saat spawn engine, Zandor membaca secret dari keychain dan memberikannya **hanya lewat env proses**:
  `ZANDOR_SECRET_<ID>=<secret>`.
- Config provider dikirim lewat env `OPENCODE_CONFIG_CONTENT` dengan **placeholder**
  `"apiKey": "{env:ZANDOR_SECRET_<ID>}"`. Opencode melakukan substitusi di memori. Config itu sendiri
  tidak memuat secret (dicek di test).
- Provider bawaan models.dev bisa diberi env native-nya (mis. `ANTHROPIC_API_KEY`) dengan mekanisme yang
  sama; pola placeholder dipakai untuk custom endpoint OpenAI-compatible.

### 3. Wajib: plugin `zandor-env-guard` (hook `shell.env`)
Opencode 1.19.5 menjalankan tool shell/bash dan PTY dengan env = `{...process.env, ...hook.env}`.
Tanpa guard, **semua proses anak mewarisi secret dan token server**. Plugin Zandor mengosongkan
`ZANDOR_SECRET_*`, `ZANDOR_PROXY_TOKEN`, `OPENCODE_SERVER_PASSWORD`, `OPENCODE_CONFIG_CONTENT`, dan
`OPENCODE_AUTH_CONTENT` lewat hook `shell.env`. Plugin dimuat lewat `"plugin": ["file:///…"]` di
`OPENCODE_CONFIG_CONTENT` (tanpa menyentuh config user). Plugin ini akan menjadi bagian
`packages/plugin` (Fase 1/9).

### 4. Ditolak: `OPENCODE_AUTH_CONTENT`
Env ini membuat opencode membaca semua kredensial dari env. Namun setiap operasi tulis auth (login
provider, `PUT /auth/:id`, kemungkinan juga refresh OAuth) **menulis gabungan kredensial, termasuk
yang dari env, ke `auth.json` plaintext**. Terbukti di test. Jangan dipakai.

### 5. Sign-in provider (OAuth) lewat OpenCode
Token OAuth disimpan oleh OpenCode sendiri (`auth.json` / tabel `credential` di SQLite, plaintext,
izin file 0600). Ini file milik OpenCode, bukan milik kita, jadi sesuai kriteria S4. Zandor tidak
menyalin token itu. Memindahkannya ke keychain tidak dilakukan sekarang, karena refresh token akan
menulis ulang file tersebut (lihat poin 4).

## Bukti (Windows 11, opencode 1.19.5, keychain asli)
`cargo test -p zandor-spike-s4 -- --test-threads=1` → **6/6 lulus** (2 unit + 4 integrasi):

- **Keychain:** simpan/baca/timpa/hapus, unicode, entri tidak ada → `None`, token 5000 karakter
  (dengan emoji) dipecah dan disusun ulang, potongan lama terhapus. Tidak ada entri test yang
  tertinggal (`cmdkey /list`).
- **End-to-end:** keychain → env → `opencode serve` → mock provider OpenAI-compatible lokal. Mock
  menerima `Authorization: Bearer <secret dari keychain>` di semua panggilan. Model dan small model
  diarahkan ke mock, jadi tidak ada panggilan keluar.
- **Agent sungguhan:** mock menyuruh agent memanggil tool `bash`
  (`node -p "JSON.stringify(process.env)"`), lalu output tool diperiksa. Data opencode diisolasi
  dengan `XDG_*_HOME` sementara (terbukti dihormati di Windows).

| Pemeriksaan | Tanpa guard (kontrol) | Dengan `zandor-env-guard` |
|---|---|---|
| PTY mewarisi secret | **ya** | tidak |
| PTY mewarisi `OPENCODE_SERVER_PASSWORD` | **ya** | tidak |
| PTY mewarisi `OPENCODE_CONFIG_CONTENT` | **ya** | tidak |
| Output tool bash memuat secret | **ya** | tidak |
| Output tool bash memuat password server | **ya** | tidak |
| Secret ikut terkirim ke model | **ya** | tidak |
| File berisi secret (workspace + data/config/cache opencode) | **`opencode.db-wal`** (output tool) | tidak ada |
| Secret terlihat lewat API engine | `/config`, `/config/providers`, `/provider` | **masih terlihat** |

- `OPENCODE_AUTH_CONTENT` + `PUT /auth/openai` → secret dari env muncul di `data/opencode/auth.json`.

## Risiko & yang belum terbukti (jujur)
1. **Secret terlihat lewat API engine** (`GET /config`, `/config/providers`, `/provider`) bagi
   siapa pun yang memegang token server. Di mode lokal pemegangnya hanya UI Zandor, jadi diterima.
   Aturannya: UI tidak pernah menampilkan atau me-log respons mentah endpoint itu. Untuk engine
   remote (Fase 11), opsi yang lebih ketat adalah **proxy kredensial lokal**: engine hanya memegang
   token proxy, dan Zandor menukarnya dengan key asli. Opsi ini **belum dibuat atau diuji**.
2. **Guard bergantung pada plugin termuat.** Kalau plugin gagal dimuat, perlindungan hilang tanpa
   suara. Produk wajib memverifikasi saat start (mis. probe PTY env setelah engine siap) dan menolak
   lanjut kalau gagal. Belum diimplementasikan.
3. **Temuan untuk ADR-019:** tanpa guard, proses yang dijalankan agent bisa membaca
   `OPENCODE_SERVER_PASSWORD`, memanggil API engine, dan **menyetujui izinnya sendiri**. Guard ini
   wajib sejak Fase 4, bukan menunggu Fase 6.
4. Secret tetap ada di memori proses engine dan env-nya. Proses lain milik user yang sama bisa
   membacanya, sama seperti keychain yang juga terbuka bagi user yang sama tanpa prompt di Windows.
   Ancaman dari malware di akun user berada di luar cakupan.
5. **macOS & Linux belum diuji.** Linux butuh daemon Secret Service (gnome-keyring/KWallet). Di
   Linux headless atau VPS tidak ada, sehingga perlu fallback (mis. file terenkripsi dengan
   passphrase) — belum diputuskan. macOS mungkin memunculkan prompt izin Keychain untuk build yang
   belum ditandatangani.
6. Belum diuji untuk provider bawaan dengan env native (mis. `ANTHROPIC_API_KEY`). Mekanismenya sama,
   tetapi tidak ada key asli di spike ini.
7. Sumber `reference/opencode` berada di versi 2.2.7 dan berbeda dari binary 1.19.5. Perilaku
   di atas diambil dari **binary 1.19.5 yang diuji langsung**, bukan dari membaca source. Upgrade
   engine wajib menjalankan ulang test S4.

## Konsekuensi
- Fase 4: supervisor menerima env secret dan selalu memuat `zandor-env-guard` (+ verifikasi saat start).
- Fase 6: modul provider menyimpan key lewat `SecretStore` (dengan pemecahan) dan tidak pernah
  memakai `OPENCODE_AUTH_CONTENT` maupun `PUT /auth` untuk key milik Zandor.
- `packages/plugin` dimulai dengan hook `shell.env` sebelum fitur memori (Fase 9).

## Cara mengulang
```
cd spikes
cargo build -p zandor-spike-s1                       # menyalin sidecar opencode ke target/debug
cargo test -p zandor-spike-s4 -- --test-threads=1    # laporan: target/s4-report.json
```
