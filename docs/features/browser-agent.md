# Browser-use: agent mengendalikan browser Panel 4 (RENCANA)

**Status:** rencana; **F0 (spike) lulus 15/15, lihat [ADR-079](../decisions/ADR-079-spike-f0-browser-agent.md)**; **F1 (baca saja) terimplementasi, belum diuji native end-to-end**; F2–F4 belum · Basis: ADR-077 (kontrol agent), ADR-058 (tab browser), ADR-020 (child webview)

## 1. Tujuan dan non-tujuan

**Tujuan.** Agent dapat mengoperasikan browser Panel 4: pindah halaman, membaca isi halaman, klik, mengetik/mengisi form,
scroll, menunggu, ganti tab, dan mengambil screenshot. Pengguna melihat semuanya langsung di panel dan bisa menghentikan kapan saja.

**Non-tujuan (v1).** Tanpa multi-agent paralel di browser; tanpa mengunduh berkas; tanpa unggah berkas; tanpa otomasi
halaman bank/pembayaran (diblokir); tanpa ekstensi/Chrome eksternal (itu jalur Claude in Chrome, terpisah).

## 2. Kondisi sekarang (hasil baca kode)

| Hal | Fakta | Berkas |
|---|---|---|
| Browser | `wry` 0.57 child webview (WebView2), maks 8 tab, hanya tab aktif yang tampil; webview dibuat malas oleh `canvas` saat render | `shell/browser.rs` |
| Hanya JS satu arah | `evaluate_script` tanpa hasil; hanya dipakai back/forward/reload | `browser.rs:183` |
| Konten remote tanpa IPC ke app | by design (ADR-020) | `browser.rs` header |
| Saluran agent | tool plugin `zandor_app` menulis `requests/<id>.json`, app polling tiap 700 ms, jawab ke `responses/` | `packages/plugin/src/control.ts`, `shell/agent_control.rs` |
| Aksi browser untuk agent | hanya `open_tool browser` dan `open_url` | `agent_control.rs:231-248` |
| Pengaman yang sudah ada | saklar `agentControl`, `PROTECTED_SETTINGS`, `BLOCKED_COMMANDS`, validasi URL (http/https, tanpa kredensial), semua permintaan dianggap tidak tepercaya | `agent_control.rs` |
| Profil/cookie | memakai data directory WebView2 yang sama dengan UI; login Google bisa persisten (ADR-020 temuan 2, 7) | ADR-020 |
| Dependensi | `wry` expose `evaluate_script_with_callback`, `with_ipc_handler`, `with_navigation_handler`, `with_download_started_handler`, `with_incognito`, dan di Windows `WebViewExtWindows::controller()` (→ `ICoreWebView2Controller`) | wry 0.57.2 `lib.rs` |

Temuan penting: `controller()` membuka jalan ke **CDP lewat WebView2** (`CallDevToolsProtocolMethod`), jadi klik/ketik
bisa berupa event input **tepercaya** (`isTrusted=true`) dan screenshot tersedia tanpa membuat browser terpisah.
Konsekuensi: perlu dependensi langsung `webview2-com` (versi sama dengan yang ditarik wry, 0.39) + `windows`.
Hanya Windows (sejalan ADR-027, Windows-first).

## 3. Riset: bagaimana aplikasi serupa melakukannya

1. **Snapshot aksesibilitas + ref (Playwright MCP).** Agent menerima pohon teks halaman (role, nama, state) dengan *ref*
   per elemen; aksi memakai ref, bukan selector; loop: snapshot → pikir → aksi → snapshot. Murah token (jauh lebih kecil
   dari dump DOM/screenshot), deterministik, tidak butuh model vision. Kelemahan: ref basi setelah navigasi/render ulang,
   Shadow DOM/iframe lintas-origin bisa tak terlihat. ([Playwright MCP snapshots](https://playwright.dev/mcp/snapshots))
2. **DOM berindeks + screenshot opsional (browser-use).** Elemen interaktif yang terlihat diberi nomor; screenshot
   dipakai hanya bila perlu; navigasi dibatasi `allowed_domains` **di kode**, bukan di prompt; kredensial lewat
   `sensitive_data` (model hanya melihat placeholder, nilai asli disuntik di browser, dan screenshot menjadi jalur bocor
   sehingga vision dimatikan saat memakai rahasia). ([docs browser-use](https://docs.browser-use.com/open-source/examples/templates/sensitive-data))
3. **Claude in Chrome.** Izin per-domain, konfirmasi untuk aksi berisiko (unduh, mengisi data sensitif, beli, publikasi),
   situs finansial/dewasa/bajakan diblokir atau minta izin, dua klasifier (konten masuk, aksi keluar), mode setujui manual,
   saran memakai profil terpisah tanpa akun sensitif. ([Anthropic](https://support.claude.com/en/articles/12902428-use-claude-in-chrome-safely), [pilot](https://www.anthropic.com/news/claude-for-chrome))
4. **Pelajaran dari insiden browser agent.** Injeksi tidak langsung lewat teks halaman (Perplexity Comet, Atlas) membuat
   same-origin policy tidak berguna begitu agent patuh pada teks halaman; pola "lethal trifecta" = data privat + konten
   tidak tepercaya + jalur eksfiltrasi. Mitigasi yang konsisten: konfirmasi aksi sensitif, hierarki instruksi (konten
   halaman = data), isolasi profil/mode tanpa login, hak akses minimum. Tidak ada yang menjamin 100% aman.
   ([Brave soal Comet](https://brave.com/blog/comet-prompt-injection/), [CSA](https://labs.cloudsecurityalliance.org/research/csa-research-note-ai-browser-prompt-injection-20260630-csa-s/))

**Kesimpulan desain:** pakai pendekatan (1) sebagai jalur utama (snapshot + ref), screenshot sebagai pelengkap, dan
bangun keamanan berlapis ala (2)+(3), dengan asumsi *halaman selalu bisa mencoba menyuntik agent*.

## 4. Desain

### 4.1 Permukaan alat untuk agent
Tool baru `zandor_browser` di plugin (terpisah dari `zandor_app` agar izin dan deskripsinya jelas), memakai transport
berkas yang sama. Aksi:

| Aksi | Argumen | Hasil |
|---|---|---|
| `tabs` | | daftar tab (id, url, judul, aktif) |
| `open` / `close_tab` / `select_tab` | url / id | status |
| `navigate` | url, tab? | status + URL akhir |
| `back` / `forward` / `reload` | | status |
| `snapshot` | tab?, mode `text`\|`full` | pohon ringkas + ref + URL + judul |
| `click` | ref, tombol?, ganda? | status + ringkasan perubahan |
| `type` | ref, teks, kirim? (Enter) | status |
| `fill_form` | daftar {ref, nilai} | status |
| `select_option`, `press_key`, `hover`, `scroll` | | status |
| `wait` | teks muncul/hilang atau ms (maks 10 dtk) | status |
| `screenshot` | tab?, ref? | PNG disimpan di folder sesi; path dikembalikan |
| `stop` | | menghentikan sesi kontrol |

Format snapshot (contoh): `[e12] button "Masuk"`, `[e13] textbox "Email" value=""`, `[e14] link "Lupa sandi" → /reset`.
Teks halaman dibungkus penanda tegas: `<halaman_tidak_tepercaya origin="…"> … </halaman_tidak_tepercaya>` dan dipotong
(mis. 12 rb karakter; elemen di luar viewport diringkas).

### 4.2 Mesin eksekusi (Rust, di `shell/browser_agent.rs`)
- **Sumber kebenaran halaman:** skrip yang disuntik lewat `with_initialization_script` membangun pohon aksesibilitas
  (role/nama/state, elemen terlihat dan interaktif, iframe same-origin), memberi `ref` `e<n>` yang disimpan di
  `window.__zref` (WeakRef) dan di-reset tiap navigasi. Hasil dibaca lewat `evaluate_script_with_callback`.
- **Aksi tepercaya via CDP:** klik/ketik/tombol/scroll memakai `Input.dispatchMouseEvent`/`dispatchKeyEvent`/`insertText`
  setelah `scrollIntoView` dan hit-test elemen (tolak bila tertutup elemen lain atau tak terlihat). Cadangan: event DOM
  sintetis hanya untuk kasus CDP gagal, ditandai di hasil.
- **Screenshot:** `Page.captureScreenshot`; tidak bergantung pada compositing GPUI (webview adalah jendela anak).
- **Ref basi:** bila ref tak ditemukan/berubah, kembalikan error "ref basi, panggil snapshot ulang", jangan menebak.
- **Antrean dan waktu:** satu aksi per tab, serial; batas waktu per aksi (15 dtk); `wait` maksimal 10 dtk. Polling 700 ms
  di `root.rs` terlalu lambat untuk loop browser: tambahkan jalur cepat (mis. 100 ms **hanya saat sesi browser aktif**).
- **Tab milik agent:** agent bekerja di tab yang ia buka/ditandai (badge "dikendalikan agent"); tab pengguna tidak disentuh
  kecuali `select_tab` yang eksplisit dan lolos izin.
- **Webview harus hidup:** webview dibuat malas saat panel tampil. Aksi pertama harus memastikan Panel 4 terbuka dan
  menunggu webview siap; bila panel tersembunyi/overlay, aksi diantre atau gagal jujur (jangan klik di webview tak terlihat).
- **Headless:** di luar cakupan v1 (butuh webview tersembunyi); v1 selalu terlihat oleh pengguna.

### 4.3 Profil
- Profil browser agent **terpisah** dari profil Panel 4 pengguna: `data_directory` sendiri (`<app_data>/browser-agent`),
  tanpa login pengguna secara bawaan. Pengguna dapat login manual di profil itu bila perlu (tab "Browser agent").
- Opsi `with_incognito` untuk sesi sekali pakai.
- Catatan: wry menetapkan data directory per *WebContext*; perlu uji bahwa dua profil berjalan berdampingan (risiko: ADR-020
  mencatat data directory bersama sebagai default). Spike S-B1 di bawah.

## 5. Model ancaman dan kontrol keamanan

**Aset:** akun pengguna yang login di browser, berkas lokal, isi chat/proyek, API key, kendali aplikasi.
**Pelaku:** halaman web berbahaya/disusupi (injeksi tidak langsung), iklan/iframe, hasil pencarian, dokumen yang dibuka;
agent yang salah paham; bukan pelaku: pengguna sendiri.

| # | Ancaman | Kontrol |
|---|---|---|
| T1 | Injeksi tidak langsung: teks halaman memerintah agent | Konten halaman dibungkus sebagai data tidak tepercaya + instruksi sistem tool; **aksi berisiko selalu butuh konfirmasi pengguna di UI** (bukan di prompt); filter heuristik pola injeksi sederhana (peringatan, bukan jaminan); tidak mengandalkan model sebagai satu-satunya penjaga |
| T2 | Eksfiltrasi data lewat navigasi/form/URL | Allowlist domain per sesi **ditegakkan di kode** lewat `with_navigation_handler` (tolak di luar daftar, termasuk redirect dan `window.open` yang sudah ditolak); `type`/`fill` ke origin berbeda dari halaman tempat ref dibuat ditolak; tidak ada tool "kirim data ke URL" |
| T3 | Agent membaca/menulis akun pengguna (sesi login) | Profil terpisah tanpa login (4.3); peringatan bila domain pernah login di profil itu; halaman finansial/pembayaran/kesehatan/pemerintah diblokir (daftar pola + deteksi form kartu/OTP) |
| T4 | Aksi tak dapat dibatalkan (beli, kirim, hapus, publikasi) | Heuristik kata tombol/URL (bayar, beli, kirim, hapus, publish, konfirmasi…) + `<form method=post>` ke domain asing → **konfirmasi eksplisit**; mode "setujui tiap aksi" tersedia; larangan keras: pembayaran, trading, CAPTCHA, unggah/unduh berkas di v1 |
| T5 | Kredensial bocor ke model | `type` ke `input[type=password]` ditolak untuk teks dari model; v2: placeholder rahasia ala browser-use (model hanya lihat `{{kunci}}`, nilai disuntik app); snapshot **menyamarkan** nilai field password/OTP/kartu; screenshot dimatikan/diburamkan pada halaman dengan field sensitif |
| T6 | Halaman menyerang app (IPC, skema aneh) | Tetap tanpa IPC untuk konten remote; skema hanya http/https (`normalize_browser_url`); `javascript:`/`file:`/`data:` ditolak termasuk di navigation handler; skrip inisialisasi hanya mengekspos objek pasif di dunia halaman, jalur hasil lewat `evaluate_script_with_callback` bukan IPC; **tidak ada alat eval JS bebas untuk agent** |
| T7 | Halaman memalsukan hasil/ref (skrip halaman menimpa `window.__zref`) | Simpan ref di closure skrip inisialisasi (bukan properti global yang bisa ditimpa), validasi tipe hasil di Rust, batasi ukuran |
| T8 | Penyalahgunaan terus-menerus / loop | Batas: 60 aksi per giliran agent, 3 navigasi berturut ke domain baru butuh konfirmasi, batas waktu sesi, tombol **Hentikan** besar di panel (membatalkan antrean dan mengembalikan kendali), toast + garis batas warna saat agent mengendalikan |
| T9 | Izin diubah oleh agent | Saklar baru `browserAgent` (bawaan **mati**) dan daftar domain masuk `PROTECTED_SETTINGS`; `set_setting` tidak bisa mengubahnya |
| T10 | Kebocoran lewat log/riwayat | Snapshot/screenshot disimpan di folder sesi, bukan di log; nilai field sensitif disamarkan sebelum masuk jejak sesi (ADR-073); jangan simpan cookie ke jejak |
| T11 | Unduhan otomatis | `with_download_started_handler` menolak (v1) |
| T12 | Penyadapan transport berkas (proses lain menulis `requests/`) | Sama dengan ADR-077: folder data pengguna; tambahkan nonce sesi di permintaan dan cocokkan dengan sesi chat aktif; tolak permintaan tanpa sesi yang sedang berjalan |

**Model izin (berlapis):**
1. Saklar global `Pengaturan → Umum → Agent boleh mengendalikan browser` (bawaan mati).
2. **Izin per domain**, diminta saat pertama (kartu di chat: "Izinkan agent membuka dan mengoperasikan `contoh.id`? Sekali / Sesi ini / Selalu / Tolak"); disimpan di pengaturan; dapat dicabut. `navigate` ke domain belum diizinkan memicu kartu itu.
3. **Konfirmasi per aksi berisiko** (T4) memakai kartu izin yang sudah ada di chat (pola pertanyaan/izin engine) dengan ringkasan: domain, elemen, teks tombol, nilai yang akan dikirim.
4. Mode kepercayaan: `Tanya tiap aksi` | `Tanya aksi berisiko` (bawaan) — tidak ada mode "tanpa tanya" di v1.

## 6. Tahapan implementasi

**F0 — Spike teknis (wajib sebelum komit desain; hasil dicatat di ADR-079)**
- S-B1: dua profil WebView2 berdampingan (`data_directory` berbeda) dan `with_incognito`.
- S-B2: `controller().CoreWebView2().CallDevToolsProtocolMethod` dari wry child webview: `Input.dispatchMouseEvent`,
  `Input.insertText`, `Page.captureScreenshot`; ukur latensi; pastikan versi `webview2-com` cocok dengan yang dipakai wry.
- S-B3: skrip inisialisasi + `evaluate_script_with_callback` mengembalikan snapshot pada 3 situs nyata (form login lokal,
  SPA, halaman dengan iframe/Shadow DOM) dan ukur ukuran/token.
- Gerbang: bila CDP gagal → turun ke event DOM sintetis dan catat batasannya (situs yang cek `isTrusted`).

**F1 — Baca saja (risiko rendah):** `tabs`, `navigate`, `snapshot`, `screenshot`, `back/forward/reload`, saklar `browserAgent`,
allowlist domain + `navigation_handler`, kartu izin domain, tombol Hentikan, jalur polling cepat. Tes: unit (penormalan,
allowlist, redaksi, pemotongan), uji skrip snapshot terhadap HTML fixture lokal.

**F2 — Aksi:** `click`, `type`, `fill_form`, `press_key`, `select_option`, `scroll`, `wait`, hit-test, ref basi, heuristik aksi
berisiko + konfirmasi, redaksi field sensitif, penolakan password, batas aksi/giliran. Tes: halaman fixture lokal
(`localhost`) dengan form, tombol berisiko, field password, halaman injeksi ("abaikan instruksi…") yang harus **tidak**
mengubah perilaku di luar konfirmasi.

**F3 — Pengerasan dan UX:** profil terpisah + tab "Browser agent", indikator kendali agent, daftar domain di Pengaturan,
jejak aksi di panel Trajectory, deteksi halaman sensitif (kartu/OTP/finansial), dokumentasi pengguna `docs/manual`,
skill `zandor-browser` (cara memakai snapshot→aksi, aturan keamanan untuk model).

**F4 (opsional):** placeholder rahasia (`{{kunci}}`), `ref`-scoped screenshot, multi-tab terkoordinasi, mode hybrid vision.

## 7. Berkas yang disentuh (perkiraan)
- Baru: `apps/desktop-gpui/src/shell/browser_agent.rs` (mesin + CDP), `assets/browser/snapshot.js`, `docs/decisions/ADR-079-…md`.
- Ubah: `shell/browser.rs` (profil, handler navigasi/unduhan, akses ke webview per tab, indikator), `shell/agent_control.rs` (aksi
  `browser_*`, validasi, PROTECTED_SETTINGS), `shell/root.rs` (polling cepat), `core/settings.rs` (saklar + domain), i18n id/en,
  `packages/plugin/src/control.ts` (tool `zandor_browser`), `Cargo.toml` (`webview2-com`, `windows`).
- Dokumen wajib sesuai `docs/agents.md`: `docs/context/current.md`, `docs/features/tools.md`, `docs/manual/alat.md`.

## 8. Kriteria selesai
- Agent menyelesaikan tugas "buka situs X, isi form uji, kirim" pada halaman fixture lokal tanpa campur tangan, dan setiap
  aksi terlihat di panel.
- Halaman fixture berisi injeksi tidak bisa membuat agent menavigasi ke domain di luar allowlist, mengetik password,
  atau menekan tombol berisiko tanpa kartu konfirmasi.
- Saklar mati = semua aksi `zandor_browser` gagal dengan pesan jelas; `set_setting` tidak bisa menyalakannya.
- `cargo test` hijau (unit allowlist/redaksi/heuristik), smoke native di Windows, dan catatan jujur bagian yang belum diuji.

## 9. Risiko dan pertanyaan terbuka
- Injeksi tidak bisa dihilangkan, hanya dikurangi; v1 sengaja konservatif (bawaan mati, tanya aksi berisiko, profil tanpa login).
- Akses webview dari thread UI GPUI vs. callback async WebView2: pastikan semua panggilan di thread utama.
- Airspace: webview tak tampak bila overlay/menu terbuka; aksi harus gagal jujur, bukan klik buta.
- Keputusan untuk pemilik: apakah profil agent boleh login (bawaan tidak)? daftar domain terlarang bawaan? batas aksi per giliran?

## Sumber
[Playwright MCP snapshots](https://playwright.dev/mcp/snapshots) · [browser-use sensitive_data](https://docs.browser-use.com/open-source/examples/templates/sensitive-data) ·
[Claude in Chrome safety](https://support.claude.com/en/articles/12902428-use-claude-in-chrome-safely) · [Anthropic: Claude for Chrome](https://www.anthropic.com/news/claude-for-chrome) ·
[Brave: Comet injection](https://brave.com/blog/comet-prompt-injection/) · [CSA research note](https://labs.cloudsecurityalliance.org/research/csa-research-note-ai-browser-prompt-injection-20260630-csa-s/) ·
[WebView2 CallDevToolsProtocolMethod](https://learn.microsoft.com/dotnet/api/microsoft.web.webview2.core.corewebview2.calldevtoolsprotocolmethodasync)
