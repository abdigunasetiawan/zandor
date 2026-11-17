# ADR-076 — Engine remote lewat URL, dipilih per project

- **Tanggal:** 17-11-2026
- **Status:** diterima

## Konteks

Roadmap Fase 11 menyebut "remote engine (VPS, komputer lain)". Pemilik memilih bentuk paling sederhana: **sambung ke URL
engine opencode yang sudah berjalan** (URL + kata sandi Basic Auth), dipilih per project, dengan status jujur. Zandor
tidak memasang atau menjalankan engine lewat SSH.

## Keputusan

- **Data:** `RemoteEngine {id, name, url, username, password}` disimpan di registry project (`remotes`, opsional; versi
  registry tetap 1). `Project.engine` = id engine remote; tidak ada = engine lokal. Path project remote adalah path di
  mesin engine itu (diketik, bukan pemilih folder lokal).
- **Router, bukan banyak store:** `createRoutedApi` (`packages/core/src/engine/remote.ts`) membungkus EngineApi lokal dan
  remote menjadi satu. Setiap panggilan yang membawa `directory` diarahkan ke engine milik project yang memuat direktori
  itu (`ProjectStore.routeOf`, path terpanjang menang); panggilan tanpa direktori (model, katalog provider, login OAuth,
  `pty.shells`) selalu ke engine lokal. Daftar metode dicek tipe agar metode EngineApi baru tidak lupa dirutekan.
  Engine yang tidak tersedia → Promise ditolak dengan pesan jelas (bukan diam-diam ke engine lain); `subscribe` → tanpa
  aliran. Tanpa engine remote, klien gabungan = klien lokal apa adanya (perilaku lama tidak berubah).
- **Pengkabelan:** `ProjectStore` memiliki engine remote (`addRemote/updateRemote/removeRemote/checkRemote`) dan
  meneruskan klien gabungan lewat `onApi` ke `ChatStore` dan `TerminalStore`. `ProviderStore` tetap memakai engine lokal.
- **Status jujur:** health check nyata saat dimuat/disimpan dan lewat tombol **Cek**; galat menampilkan sebab aslinya.
  Penanda globe di baris project (warna status + teks untuk pembaca layar); tombol sesi baru mengikuti status engine
  project itu. Engine yang masih dipakai project tidak bisa dihapus.
- **Keamanan:** URL ke mesin lain **wajib HTTPS** (kata sandi Basic Auth tidak boleh lewat jaringan terbuka); HTTP hanya
  untuk loopback (tunnel SSH `ssh -L 4096:127.0.0.1:4096 server`). URL berisi kredensial/query ditolak. CSP
  `connect-src` diperluas dengan `https: wss:` dan `http(s)://localhost:*`; HTTP ke host lain tetap diblokir CSP.
- **UI:** Pengaturan → **Engine remote** (`#/settings/engines`): daftar, status, Cek/Ubah/Hapus, formulir dengan validasi
  langsung. Dialog Tambah project menampilkan pilihan **Engine** bila ada engine remote (di Tauri tetap ada tombol Pilih
  folder untuk engine lokal). Dialog percayai folder untuk project remote menyembunyikan opsi memori.

## Batasan (diketahui, jujur di UI/manual)

- Kata sandi disimpan **tanpa enkripsi** di registry project di komputer ini (belum memakai keychain OS).
- Panel 3 (dokumen) membaca dari disk komputer ini: berkas project remote belum bisa dibuka di sana. Pohon file,
  perubahan sesi, chat, dan terminal berjalan lewat engine remote.
- Memori/skill Zandor dan folder Chat hanya untuk engine lokal; model & provider berasal dari engine lokal.
- Melebarkan `connect-src` ke `https:` memperbesar dampak bila ada XSS; mitigasinya tetap: konten model dirender tanpa
  HTML mentah (ADR-018, aturan zandor-ui).
- Pilihan engine ditetapkan saat menambah project (ubah = lepas lalu tambah lagi).
