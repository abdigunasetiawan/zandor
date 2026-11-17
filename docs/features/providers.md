# Fitur: Provider & model

- **Status:** selesai untuk lingkup Fase 6 dengan batasan jujur di bawah. Keputusan: ADR-034.
- **Kode:** `apps/desktop/src-tauri/src/{secrets,providers,netprobe}.rs`, `packages/core/src/providers/store.ts`,
  `apps/desktop/src/shell/{ProvidersPage,ModelPicker,provider-host}.ts(x)`.
- **Buka:** tombol roda gigi di footer Sidebar, command palette ("Kelola provider & model"), atau "Kelola provider…"
  di pemilih model.

## Spek

- **Koneksi Anda**: kartu per koneksi dengan jenis, base URL, lencana _kunci tersimpan_, lencana _tersambung ke engine_
  (dari engine), daftar model dengan tombol **Uji** (hasil: OK + latensi, atau kategori + pesan server), ubah, hapus
  (dengan konfirmasi; kunci di keychain ikut dihapus).
- **Tambah koneksi** (dialog bertab):
  - _Endpoint kustom_: nama, base URL OpenAI-compatible, kunci; **Ambil daftar model** (`GET /models`), centang model,
    tambah model manual.
  - _Lokal_: **Deteksi Ollama / LM Studio** (port 11434/1234) → satu klik tambah; atau preset yang mengisi base URL.
  - _Provider katalog_: cari dari katalog engine (236 provider), hubungkan dengan kunci API.
  - _Sign-in_: provider dengan login akun (OAuth) menurut engine; mendukung metode otomatis (menunggu browser) dan metode
    kode.
- **Pemilih model**: pencarian fokus otomatis, grup Disematkan lalu per provider, sematkan, uji model, "Kelola provider…".
- Simpan/hapus memulai ulang engine sebentar agar konfigurasi terbaca.

## Keamanan

Kunci hanya di keychain OS dan di environment proses engine; tidak di berkas, konfigurasi, log, state UI, atau repo.
Plugin guard membersihkan `ZANDOR_SECRET_*` dari proses anak engine. `providers.json` lama yang berisi kunci dimigrasikan
otomatis.

## Belum / terbatas

Provider katalog yang butuh data tambahan (Account ID dsb.), login OAuth sungguhan belum diuji manusia, model lokal
sungguhan belum diuji (tak ada Ollama/LM Studio di mesin pengembang), macOS/Linux.

## Cara test

- Otomatis: `cargo test -p zandor-desktop` (keychain nyata, probe/ping), `bun run test`, `bun x playwright test providers`,
  smoke native.
- Manual: tambah endpoint (mis. `http://localhost:8045/v1`) → Ambil daftar model → Simpan → Uji model → buka
  Windows "Credential Manager" dan lihat entri `id.web.zandor`; pastikan `providers.json` tidak berisi kunci.

## Tambah koneksi (ADR-055)

Tambah/ubah koneksi adalah **tampilan inline** (bukan popup) di Pengaturan › Provider dan model dan di halaman Provider:
tab Endpoint kustom, Lokal, Katalog provider, dan Masuk (OAuth lewat engine; plus kartu OpenWork, lihat di bawah).

## Akun ganda dan OpenWork (ADR-056)

- Satu provider bisa punya beberapa akun/kunci (tab Katalog untuk provider berkunci; kartu OpenWork). Daftar akun:
  **Aktif** / **Gunakan** / ganti nama / hapus (konfirmasi) / **Tambah akun**. Kunci per akun di keychain
  (`provider:<id>#<akun>`); kunci akun aktif disalin ke `provider:<id>` yang dipakai engine; beralih akun memulai ulang
  engine. `providers.json` hanya menyimpan id+label akun (tanpa kunci).
- **OpenWork** = kartu di tab Sign-in (device-code inline, bukan popup). Tiap login menambah satu akun pada provider
  `openwork` (endpoint kustom, kunci gateway `ow_…`, model dari gateway). Rust: `openwork.rs`, perintah `openwork_*`.
  Hanya di aplikasi desktop (build web menjelaskannya).
- **Batasan**: OAuth lewat engine tetap satu login per provider (engine menyimpan satu kredensial); untuk banyak akun
  pakai kunci API atau OpenWork. Hanya satu akun aktif sekaligus (tanpa failover otomatis).
- Tes: cargo `multiple_accounts_keep_keys_apart…` + tes `openwork`; vitest `accounts.test.ts`, `openwork.test.ts`,
  `OpenWorkSignIn.test.tsx`; e2e "akun ganda pada provider katalog" dan "tab Sign-in menampilkan OpenWork".
  Belum diuji ke OpenWork sungguhan (butuh akun) — hanya kode perangkat via tes `--ignored`.

## Pengaturan › Provider dan model (ADR-054)

- Model bawaan chat dan generate gambar (gambar: preferensi saja, fiturnya belum ada).
- **Model peran — model kecil** (17-11-2026): opencode `small_model` untuk tugas ringan (judul sesi, ringkasan).
  Disimpan Rust di `<app_data>/model-roles.json` (`{"small":"provider/model"}`, divalidasi), dimasukkan ke
  `OPENCODE_CONFIG_CONTENT` saat engine dimulai; UI menawarkan **Mulai ulang engine sekarang** setelah diubah. Otomatis =
  kunci tidak ditulis (engine memilih sendiri). Model tersimpan yang hilang dari daftar ditampilkan "(tidak tersedia)".
  Di build web kontrolnya nonaktif dengan alasan. Tidak memakai `PATCH /config` engine karena itu menulis berkas config
  ke folder project pengguna.
- **Model memori: tidak dibuat.** Plugin memori tidak memanggil model sendiri; catatan memori dan draf skill ditulis oleh
  agent sesi dengan model aktifnya, jadi pengaturan "model memori" tidak akan berpengaruh (UI palsu, ADR-018). Baru masuk
  akal bila kelak ada ekstraksi memori otomatis di latar.
- Provider: id, nama tampilan, protokol API, base URL, kunci (keychain), ambil model; edit/hapus.
- Pengelola model: pencarian, kelompokkan (tanpa/keluarga/kemampuan), pilih semua/batalkan semua, saklar tampil di
  pemilih composer, lencana kemampuan (konteks, keluaran, reasoning, alat, lampiran, varian), ubah kemampuan model kustom.
- Logo provider/model: otomatis dari nama, atau unggah sendiri (dikecilkan, disimpan lokal).
- Test: `packages/core/src/models/prefs.test.ts`, `providers.test.ts`, `chat.test.ts`, `ModelLogo.test.tsx`,
  `logo-file.test.ts`, cargo `providers::tests`, e2e `model-settings.spec.ts`.

## Info model hasil fetch

`GET {base}/models` standar hanya memberi `id`. Bila endpoint menambah metadata (OpenRouter: `context_length`,
`top_provider.max_completion_tokens`, `supported_parameters`, `architecture.input_modalities`; LM Studio:
`max_context_length`, `capabilities`, `type`; vLLM: `max_model_len`), Zandor menampilkannya (nama, konteks, keluaran,
reasoning, alat, lampiran) dan menyimpannya bersama koneksi. Ollama (`/v1/models`) tidak memberi metadata; isi manual.
Tes: `model-info.test.ts`, cargo `netprobe::model_info_reads_openrouter_and_lmstudio_fields`, e2e `providers.spec.ts`.
Pilih semua / Batalkan semua tersedia di daftar hasil fetch (mengikuti filter pencarian).
