# ADR-020 — Spike S3: browser bawaan via child webview Tauri

- **Tanggal:** 13-11-2026
- **Status:** Diterima untuk Windows · macOS, Linux X11, dan Wayland **ditunda** (keputusan pemilik
  13-11-2026: belum ada perangkat; diverifikasi lewat CI runner atau perangkat pinjaman nanti)
- **Hasil spike:** ✅ Lulus di Windows 11 (WebView2 157): test otomatis + **login Google manual
  berhasil** (oleh pemilik, 13-11-2026) · ⚠️ persistensi sesi: hasil bertentangan (lihat temuan 2) ·
  ⏸️ macOS/Linux ditunda
- **Kode:** `spikes/s3-webview/`
- **Memperjelas:** ADR-012

## Pertanyaan

Apakah child webview bisa dipakai untuk browser bawaan di Win/mac/Linux X11 (fallback di Wayland)?
Lulus jika: bisa membuka google.com dan login Google; posisinya mengikuti resize panel.

## Keputusan

1. **Tauri `=2.12.5` dengan feature `unstable`** (multiwebview), versi di-pin persis.
2. Satu `Window`, dua webview anak:
   - `ui` — UI lokal, `auto_resize()` memenuhi jendela, satu-satunya yang punya IPC.
   - `browser` — konten remote (default `https://www.google.com/`), ditambahkan **setelah** `ui`
     sehingga berada di atasnya.
3. **Sinkronisasi posisi:** UI memasang `ResizeObserver` pada elemen panel host (+ `resize` jendela),
   lalu memanggil `browser_set_bounds(x, y, w, h)` dalam piksel **logis** (CSS px) per animation frame.
   Rust memanggil `set_position`/`set_size` (logical). Bounds juga disinkron ulang tiap kali halaman
   browser mulai dimuat (mitigasi race cold-start, lihat bukti).
4. **Navigasi:** `navigate(url)` (hanya `http/https`), back/forward/reload lewat `eval` `history.*`,
   sembunyikan/tampilkan lewat `hide()/show()`. URL aktif dilaporkan ke UI lewat `on_page_load`.
5. **Isolasi keamanan:** capability `ui-only` hanya untuk webview `ui` (URL lokal). Webview
   `browser` tidak tercantum di capability mana pun dan **tidak boleh** diberi `remote.urls`.
6. **Fallback:** Linux dengan `WAYLAND_DISPLAY` dan tanpa `GDK_BACKEND=x11` → child webview tidak
   dibuat; panel menampilkan keterangan jujur dan tautan dibuka di browser sistem (`xdg-open`).

## Bukti (Windows 11, skala layar 100%)

Self-test otomatis (`ZANDOR_SELFTEST=<file>`), dijalankan 5x: 4x lulus penuh, 1x (run pertama,
cold-start) `bounds_initial` gagal — lihat temuan 1. Setelah mitigasi + 3 run ulang: lulus semua.

| Cek                                                                                | Hasil                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| google.com termuat                                                                 | 0,6–1,2 dtk, `document.title = "Google"` (tidak diblokir X-Frame-Options)                                                                                                                    |
| bounds awal / splitter 300px / splitter 800px / jendela 1000×650                   | selisih panel ↔ webview **0 px**                                                                                                                                                             |
| navigasi ke example.com lalu back                                                  | ✓                                                                                                                                                                                            |
| sembunyikan / tampilkan                                                            | ✓                                                                                                                                                                                            |
| **login Google (manual, pemilik)**                                                 | **berhasil** — tidak diblokir "browser may not be secure"                                                                                                                                    |
| sesi Google setelah app ditutup & dibuka lagi                                      | pemilik: **tidak bertahan** pada buka-ulang pertama · self-test berikutnya (4 run): **masih login** (`google_session_state.signedIn = true`, `accounts.google.com` → `myaccount.google.com`) |
| halaman login `accounts.google.com`                                                | termuat (`/v3/signin/identifier`), teks "Sign in · Use your Google Account · Email or phone"; tidak ada pesan "browser may not be secure" di tahap ini                                       |
| global IPC di halaman remote                                                       | `__TAURI_INTERNALS__`, `__TAURI__`, `ipc` **ada** (diinjeksi Tauri)                                                                                                                          |
| `invoke` dari google.com: perintah app (`get_browser_mode`, `browser_set_visible`) | **ditolak**: "not allowed. Plugin not found"                                                                                                                                                 |
| `invoke` dari google.com: `plugin:event\|emit`, `plugin:webview\|get_all_webviews` | **ditolak**: "not allowed on … webview \"browser\" … allowed on: [webviews: \"ui\", URL: local]"                                                                                             |

Screenshot manual: webview Google pas di panel kanan, di bawah header panel, tanpa celah.

## Temuan & risiko (jujur)

1. **Race bounds saat cold-start** (1 dari 5 run): sinkron pertama dari UI tidak berefek, webview
   tetap di posisi awal dari Rust. Mitigasi: sinkron ulang di setiap event load browser, setelah itu
   4 run lulus. Di produk: Rust sebaiknya mengirim event `browser-ready` dan UI selalu sinkron ulang.
2. **Login Google berhasil di Windows** (diuji manual oleh pemilik). **Persistensi sesi belum
   tuntas:** pada buka-ulang pertama pemilik melihat Google dalam keadaan belum login. Pemeriksaan
   setelahnya:
   - Cookie sesi Google (`SID`, `HSID`, `SSID`, `__Secure-1PSID`, …) **tersimpan persisten** di
     `%LOCALAPPDATA%/id.web.zandor.spike.s3/EBWebView/Default/Network/Cookies`, kedaluwarsa 2027.
   - Selama buka-ulang itu Google menulis ulang `SIDCC` dan `__Secure-1PSIDTS` (rotasi sesi).
   - 4 run self-test setelahnya semuanya mendeteksi **masih login**.

   Hipotesis (belum terbukti): render pertama setelah buka-ulang memakai cookie yang sedang dirotasi
   Google sehingga tampil "Login"; setelah rotasi selesai sesi kembali valid. Profil WebView2 juga
   punya berkas `Device Bound Sessions` (DBSC) yang bisa ikut berperan. Langkah lanjut: pemilik
   menutup-buka app beberapa kali lagi; bila tampil "Login", tekan reload sekali dan catat apakah
   sesi muncul. Kalau masalah berulang, produk perlu reload otomatis atau profil khusus via
   `data_directory`. Belum dikonfirmasi pula apakah Google memblokir di WKWebView (macOS) /
   WebKitGTK (Linux).
3. **Objek IPC tetap diinjeksi ke halaman remote.** Saat ini aman karena ACL capability menolak
   semua perintah. Aturan wajib ke depan: jangan pernah menambah capability tanpa `webviews: ["ui"]`,
   jangan pakai `remote.urls`, dan tambahkan test regresi "remote invoke ditolak" ke CI.
4. **Airspace / z-order:** child webview native selalu di atas webview UI. Menu, popover, toast,
   command palette, atau modal UI yang menimpa area Panel 4 akan **tertutup** browser. Konsekuensi
   desain: sembunyikan browser (atau ganti dengan screenshot statis) saat overlay terbuka di atasnya.
   Belum diuji visual, tetapi ini sifat bawaan webview native.
5. **Hanya diuji di skala 100%.** HiDPI 125/150% dan interaksi dengan `--ui-scale` → masuk S8.
6. **Ditunda: macOS (WKWebView), Linux X11 (WebKitGTK), dan Wayland belum dijalankan** karena
   pemilik belum punya perangkatnya. Jalur fallback Wayland hanya ada sebagai kode.
7. **Profil/cookie** browser memakai data directory WebView2 yang sama dengan UI. Login Google
   tersimpan di profil app. Bila perlu dipisah (mode privat, hapus data browser), gunakan
   `data_directory` terpisah — belum diuji.
8. Shortcut keyboard global tidak diterima UI saat fokus ada di webview browser — perlu
   ditangani di sistem shortcut (Fase 2/8). Belum diuji.
9. `unstable` berarti API bisa berubah di minor Tauri berikutnya; pin `=2.12.5` wajib dan upgrade
   harus melalui test S3.

## Konsekuensi

- Fase 8 (Browser) memakai pola ini. Komponen panel browser wajib mengirim bounds lewat satu
  jalur (ResizeObserver) dan menyembunyikan webview saat panel collapse atau ada overlay.
- Design system (Fase 2) harus punya aturan "overlay di atas Panel 4 → browser disembunyikan".

## Cara mengulang

```
cd spikes
cargo build -p zandor-spike-s3
ZANDOR_SELFTEST=$TEMP/s3.json ./target/debug/zandor-spike-s3   # laporan JSON
cargo run -p zandor-spike-s3                                    # uji manual + login Google
```
