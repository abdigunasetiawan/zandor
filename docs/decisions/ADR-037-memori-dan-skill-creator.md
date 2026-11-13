# ADR-037 — Memori & Skill Creator (Fase 9)

- **Tanggal:** 14-11-2026
- **Status:** Diterima
- **Terkait:** ADR-011 (MemoryProvider), ADR-022 (env guard), ADR-023 (spike S6), ADR-033/034

## Keputusan

1. **Satu plugin Zandor** (`packages/plugin`, dibundel ke `zandor-plugin.js`) memuat guard env **dan** memori/skill
   creator. Opsi `memoryDir` (= `<app_data>/memory`) dikirim Rust lewat `OPENCODE_CONFIG_CONTENT`. Tanpa `memoryDir`
   plugin hanya menjadi guard (perilaku Fase 4 tidak berubah).
2. **Opt-in** (PRD): bawaan **mati**. Urutan keputusan: **override sesi → override project → bawaan**
   (`settings.json`: `defaultEnabled`, `projects{key}`, `sessions{id}`, `skillCreator`). Skill creator punya saklar sendiri.
   Pengaturan dibaca ulang setiap giliran, jadi perubahan dari UI berlaku tanpa restart engine.
3. **Berkas memori** (format = kontrak UI ↔ plugin ↔ Rust, diuji paritasnya): `global.md`, `projects/<key>.md`,
   `pins/<sessionID>.md`, `meta.json` (key → path, ditulis plugin), `skill-queue.jsonl`, `index.sqlite` (FTS5 via
   `bun:sqlite`; bila tak tersedia, cadangan pemindaian teks). `key` = sha256 16 heks dari path project yang dinormalisasi
   (huruf kecil, `/`, tanpa akhiran `/`); dihitung identik di plugin (Node crypto) dan UI (Web Crypto).
4. **Penyuntikan**: blok `<zandor-memory>` (maks 6000 karakter) dan `<zandor-pinned>` lewat
   `experimental.chat.system.transform`; sematan juga masuk prompt kompaksi (`experimental.session.compacting`).
   Tool `memory_write` / `memory_search`. **Penulisan memori menolak teks yang tampak memuat secret** (pemindai).
5. **Skill creator** (antrean persetujuan): pemicu `chat.message` (koreksi pengguna, perintah `/reflect`) dan
   `tool.execute.after` (≥ 5 tool call dalam satu sesi) mencatat **kandidat** (`candidate`) dan menyuntik petunjuk satu
   kali ke giliran itu agar model memanggil tool **`skill_propose`**. Tool memvalidasi nama (kebab-case), panjang, dan
   **memindai secret**; draf valid menjadi `pending`. Skill **tidak aktif** sampai pengguna menyetujuinya di UI
   (Setujui → `SKILL.md` ditulis; Tolak; Hapus). Draf bisa diedit sebelum disetujui; validasi dan pemindaian diulang.
6. **Lokasi skill** (terbukti ditemukan engine 1.19.5): global `engine/config/opencode/skills/<nama>/SKILL.md`
   (XDG config terisolasi), project `<project>/.opencode/skills/<nama>/SKILL.md`.
7. **Rust** (`memory.rs`): perintah `memory_*` hanya menerima berkas dalam **daftar putih** (tanpa traversal),
   `meta.json` hanya-baca, batas 200 KB; `skill_*` memvalidasi nama, menolak menimpa tanpa `overwrite`, batas 16 KB,
   skill project hanya di project terdaftar, dan hapus skill hanya menghapus `SKILL.md` (berkas pengguna lain aman).
8. **UI**: halaman `/memory` (Pengaturan, Memori, Antrean skill, Skill terpasang), tombol di footer sidebar, perintah
   palet, dan opt-in per sesi di tab Info sesi. Di luar Tauri host = pratinjau di memori proses **berlabel jujur**.

## Bukti

- Plugin vs engine asli (`bun run e2e:engine` di `packages/plugin`, model mock): 9/9 — opt-in, penyuntikan, tulis →
  langsung terbaca, secret ditolak, override sesi, pemicu koreksi → kandidat → petunjuk → `skill_propose` → pending,
  draf berisi secret ditolak, skill project dan global ditemukan engine, sematan masuk kompaksi.
- Paritas UI ↔ plugin (kunci project, pemindai secret, validasi, `SKILL.md`, resolusi opt-in): `memory.test.ts`.
- Smoke native di aplikasi asli: lihat `docs/features/memory.md`.

## Batas / risiko

- Hook `experimental.*` bisa berubah antarversi opencode: jalankan ulang `e2e:engine` sebelum menaikkan versi pin.
- Kualitas isi skill dan kapan model memanggil `skill_propose` bergantung pada model; yang terbukti hanya mekanismenya.
- Antrean ditulis ulang utuh oleh UI (dibaca ulang tepat sebelum menulis); jendela balapan dengan plugin sangat kecil
  tetapi tidak nol.
- Adapter Hindsight/Graphify (PRD Fase 9, opsional) **belum dibuat**; antarmuka `Indexer` di plugin adalah titik sambungnya.
- Opt-in saat **membuat** project belum ada di dialog "Tambah project" (hanya lewat halaman Memori / Info sesi).
- Memori di mesin remote belum relevan (remote engine = Fase 11).
