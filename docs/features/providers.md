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

## Masuk dengan OpenWork (Fase 12, ADR-040)

Tombol **Masuk dengan OpenWork** di halaman Provider (aplikasi desktop): OAuth device flow → kunci AI Gateway di keychain →
provider `openwork` dengan model dari gateway. Token/kunci tidak pernah sampai ke UI. Tes: `cargo test --lib openwork`
(+ `-- --ignored` untuk kode perangkat dari server nyata), `OpenWorkLogin.test.tsx`, `openwork.test.ts`. **Belum diuji:**
login penuh dengan akun sungguhan (butuh persetujuan manusia); akun tanpa kredensial gateway dilaporkan sebagai galat jelas.
