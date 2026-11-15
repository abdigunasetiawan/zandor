# ADR-056 — Akun ganda per provider; OpenWork kembali sebagai kartu Sign-in

Tanggal: 15-11-2026 · Status: diterima (menggantikan bagian "OpenWork dihapus" pada ADR-055)

## Keputusan

- **Akun ganda**: satu provider boleh punya beberapa kunci/akun. Model data: `Provider.accounts: [{id, label}]` +
  `activeAccount`. Kunci tiap akun di keychain `provider:<id>#<akun>`; kunci akun **aktif** disalin ke `provider:<id>`
  sehingga `for_engine`, ping, dan konfigurasi engine tidak berubah. Beralih akun = menyalin kunci lain + restart engine
  (tanpa memasukkan ulang kunci). Satu provider = satu id di engine (tidak ada id ganda), jadi model katalog tetap utuh.
- Akun lama (satu kunci tanpa daftar) tampil sebagai akun `main` ("Akun utama") dan dimaterialisasi saat ada akun
  ditambahkan. `provider_save` dari UI mempertahankan daftar akun. Hapus provider menghapus semua kunci akun.
- Perintah Tauri: `provider_account_add/activate/rename/remove`. UI: `AccountList` (Aktif/Gunakan/Ganti nama/Hapus/Tambah
  akun, hapus dengan konfirmasi) dipakai di tab Katalog (provider berkunci) dan kartu OpenWork.
- **OpenWork kembali**, tetap bukan popup dan tidak mendapat halaman sendiri: kartu "OpenWork Cloud" di tab Sign-in
  (device-code inline: kode, buka halaman, menunggu, status). Tiap login menambah **satu akun** pada provider `openwork`
  (endpoint kustom OpenAI-compatible, kunci gateway `ow_…` di keychain, model dari gateway). Rust `openwork.rs` dipulihkan
  (device-code RFC 8628, `resolve_gateway`, validasi URL verifikasi).

## Batasan (jujur)

- **Login OAuth lewat engine tetap satu login per provider**: opencode menyimpan satu kredensial OAuth per provider dan
  token disegarkan oleh engine; menyalin/menukar `auth.json` akan basi/rusak. Untuk beberapa akun pakai kunci API (tab
  Katalog) atau OpenWork. Catatan ini tampil di tab Sign-in.
- Akun hanya soal kunci: model/endpoint provider sama untuk semua akunnya. Hanya satu akun aktif pada satu waktu (tidak ada
  round-robin/failover otomatis).
- Di build web (non-Tauri) OpenWork hanya dijelaskan "hanya di aplikasi desktop"; akun ganda berjalan di memori saja.
