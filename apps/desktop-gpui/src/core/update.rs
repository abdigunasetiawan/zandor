//! Pembaruan otomatis (ADR-084): cek `latest.json` di GitHub Releases, unduh installer di latar, verifikasi
//! tanda tangan Ed25519 + SHA-256, lalu pasang senyap saat Zandor ditutup (atau saat pengguna memilih
//! "Mulai ulang sekarang"). Hanya aktif pada build rilis resmi (`ZANDOR_RELEASE_VERSION` di-set saat build)
//! yang dipasang lewat installer; build pengembangan tidak pernah memperbarui dirinya.
use gpui::{App, AppContext, Context, Entity, Global};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Manifes saluran beta: aset `latest.json` pada rilis GitHub terbaru.
pub const MANIFEST_URL: &str = "https://github.com/abdigunasetiawan/zandor/releases/latest/download/latest.json";
/// Kunci publik Ed25519 (hex, 32 bita) pasangan kunci privat penanda tangan rilis (di luar repo).
const PUBLIC_KEY_HEX: &str = include_str!("update_key.pub");
/// Nama uninstaller yang ditulis installer; keberadaannya menandai Zandor yang dipasang.
pub const UNINSTALLER: &str = "uninstall.exe";

/// Alamat manifes. `ZANDOR_UPDATE_URL` hanya untuk menguji alur pembaruan terhadap server lokal; manifes tetap
/// harus bertanda tangan kunci yang tertanam, jadi override ini tidak melemahkan verifikasi.
fn manifest_url() -> String {
    std::env::var("ZANDOR_UPDATE_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| MANIFEST_URL.to_string())
}

const FIRST_CHECK: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Versi build rilis ini, atau `None` untuk build pengembangan.
pub fn release_version() -> Option<&'static str> {
    option_env!("ZANDOR_RELEASE_VERSION").filter(|v| !v.is_empty())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pub_date: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    /// Tanda tangan Ed25519 (hex) atas `signed_message`.
    pub signature: String,
}

/// Pesan yang ditandatangani skrip rilis. Versi, isi berkas (SHA-256), dan ukuran terikat bersama sehingga
/// manifes atau aset yang ditukar tidak lolos verifikasi.
pub fn signed_message(version: &str, sha256: &str, size: u64) -> String {
    format!("zandor-update\n{version}\n{}\n{size}", sha256.to_ascii_lowercase())
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn verify_manifest(m: &Manifest, public_key: &[u8]) -> Result<(), String> {
    if hex_decode(&m.sha256).is_none_or(|h| h.len() != 32) {
        return Err("sha256 manifes tidak valid".into());
    }
    let sig = hex_decode(&m.signature).ok_or("tanda tangan manifes tidak valid")?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(signed_message(&m.version, &m.sha256, m.size).as_bytes(), &sig)
        .map_err(|_| "tanda tangan manifes tidak cocok".to_string())
}

/// `candidate` lebih baru dari `current` (semver; pra-rilis lebih lama dari rilis bernomor sama).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let p = |v: &str| semver::Version::parse(v.trim().trim_start_matches('v')).ok();
    matches!((p(candidate), p(current)), (Some(a), Some(b)) if a > b)
}

/// Folder unduhan: `%LOCALAPPDATA%\Zandor\updates` (cache mesin, bukan data pengguna yang dijelajah).
pub fn updates_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Zandor").join("updates")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Pending {
    version: String,
    sha256: String,
    installer: PathBuf,
}

fn pending_file(dir: &Path) -> PathBuf {
    dir.join("pending.json")
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex_encode(&h.finalize()))
}

/// Pembaruan tertunda yang masih sah: versinya lebih baru dan isi installer belum berubah.
fn valid_pending(dir: &Path, current: &str) -> Option<Pending> {
    let p: Pending = serde_json::from_str(&std::fs::read_to_string(pending_file(dir)).ok()?).ok()?;
    let ok = is_newer(&p.version, current) && sha256_file(&p.installer).ok()?.eq_ignore_ascii_case(&p.sha256);
    if !ok {
        let _ = std::fs::remove_file(pending_file(dir));
        let _ = std::fs::remove_file(&p.installer);
    }
    ok.then_some(p)
}

/// Jalur exe dalam bentuk panjang (tanpa nama pendek 8.3 dan tanpa awalan `\\?\`), supaya cocok dengan jalur
/// proses yang dilaporkan Windows saat penerap mencari proses di folder instalasi.
fn current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let full = std::fs::canonicalize(&exe).unwrap_or(exe);
    let s = full.to_string_lossy();
    Some(PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s)))
}

fn exe_dir() -> Option<PathBuf> {
    current_exe()?.parent().map(Path::to_path_buf)
}

/// Mengapa pembaruan otomatis tidak aktif, atau `None` bila aktif.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disabled {
    /// Build pengembangan (tanpa versi rilis).
    DevBuild,
    /// Build rilis yang tidak dijalankan dari folder instalasi (mis. `target/release`).
    NotInstalled,
}

fn disabled_reason() -> Option<Disabled> {
    if release_version().is_none() {
        return Some(Disabled::DevBuild);
    }
    if !exe_dir().is_some_and(|d| d.join(UNINSTALLER).exists()) {
        return Some(Disabled::NotInstalled);
    }
    None
}

/// Jalankan skrip penerap terlepas: tunggu proses ini keluar, pasang installer senyap ke folder instalasi,
/// lalu (opsional) jalankan ulang Zandor. Berkas `pending.json` dihapus lebih dulu supaya kegagalan tidak
/// membuat percobaan berulang tiap start.
fn launch_apply(p: &Pending, relaunch: bool) -> Result<(), String> {
    let dir = updates_dir();
    let (Some(install), Some(exe)) = (exe_dir(), current_exe()) else { return Err("folder instalasi tidak diketahui".into()) };
    let script = dir.join("apply.ps1");
    std::fs::write(&script, APPLY_PS1).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(pending_file(&dir));
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&script)
        .arg("-AppPid")
        .arg(std::process::id().to_string())
        .arg("-Installer")
        .arg(&p.installer)
        .arg("-InstallDir")
        .arg(&install)
        .arg("-Exe")
        .arg(&exe)
        .arg("-Relaunch")
        .arg(if relaunch { "1" } else { "0" })
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

const APPLY_PS1: &str = r#"param([int]$AppPid, [string]$Installer, [string]$InstallDir, [string]$Exe, [int]$Relaunch)
$log = Join-Path (Split-Path $Installer) 'apply.log'
function Log($m) { "$(Get-Date -Format o) $m" | Out-File -FilePath $log -Append -Encoding utf8 }
Log "menunggu Zandor (pid $AppPid) keluar"
try { Wait-Process -Id $AppPid -Timeout 120 -ErrorAction Stop } catch {}
$engine = Join-Path $InstallDir 'opencode.exe'
Get-Process -Name opencode -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $engine } | Stop-Process -Force -ErrorAction SilentlyContinue
# Berkas exe bisa masih terkunci sesaat setelah proses keluar (pemindai antivirus, pemetaan image); installer senyap
# NSIS akan melewati berkas terkunci, jadi tunggu sampai semua berkas bisa ditulis.
$targets = @((Join-Path $InstallDir 'Zandor.exe'), $engine)
for ($i = 0; $i -lt 120; $i++) {
  $locked = $false
  foreach ($t in $targets) {
    if (Test-Path -LiteralPath $t) {
      try { $f = [System.IO.File]::Open($t, 'Open', 'ReadWrite', 'None'); $f.Close() } catch { $locked = $true }
    }
  }
  if (-not $locked) { break }
  Start-Sleep -Milliseconds 500
}
if ($locked) { Log "berkas masih terkunci setelah 60 dtk; installer tetap dijalankan" }
for ($try = 1; $try -le 5; $try++) {
  $p = Start-Process -FilePath $Installer -ArgumentList @('/S', '/UPDATE', "/D=$InstallDir") -Wait -PassThru
  Log "installer selesai: kode $($p.ExitCode) (percobaan $try)"
  if ($p.ExitCode -eq 0) { break }
  $busy = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($InstallDir, 'OrdinalIgnoreCase') } | ForEach-Object { "$($_.Name)#$($_.Id)" }
  Log "masih berjalan dari folder instalasi: $($busy -join ', ')"
  Start-Sleep -Seconds 3
}
Remove-Item -LiteralPath $Installer -Force -ErrorAction SilentlyContinue
if ($Relaunch -eq 1) { Start-Process -FilePath $Exe }
"#;

/// Dipanggil sebelum jendela dibuka: bila ada pembaruan terunduh yang belum terpasang (mis. Zandor sempat
/// dihentikan paksa), pasang sekarang lalu jalankan ulang. `true` = proses ini harus langsung keluar.
pub fn apply_pending_at_start() -> bool {
    let (None, Some(current)) = (disabled_reason(), release_version()) else { return false };
    let Some(p) = valid_pending(&updates_dir(), current) else { return false };
    match launch_apply(&p, true) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[zandor] update: gagal memasang pembaruan tertunda: {e}");
            false
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Disabled(Disabled),
    Idle,
    Checking,
    UpToDate,
    Downloading { version: String, done: u64, total: u64 },
    Ready { version: String, notes: String },
    Error(String),
}

pub struct Updater {
    pub phase: Phase,
    /// Waktu cek terakhir yang selesai (lokal).
    pub last_check: Option<chrono::DateTime<chrono::Local>>,
    pending: Option<Pending>,
    applied: bool,
}

struct UpdaterHandle(Entity<Updater>);
impl Global for UpdaterHandle {}

enum Event {
    Progress(String, u64, u64),
    Done(Result<Option<(Pending, String)>, String>),
}

impl Updater {
    pub fn init(cx: &mut App) {
        let reason = disabled_reason();
        let pending = match (reason, release_version()) {
            (None, Some(v)) => valid_pending(&updates_dir(), v),
            _ => None,
        };
        let phase = match (&reason, &pending) {
            (Some(r), _) => Phase::Disabled(*r),
            (None, Some(p)) => Phase::Ready { version: p.version.clone(), notes: String::new() },
            _ => Phase::Idle,
        };
        let e = cx.new(|_| Updater { phase, last_check: None, pending, applied: false });
        cx.set_global(UpdaterHandle(e.clone()));
        if reason.is_some() {
            return;
        }
        // Beri tahu sekali saat versi baru siap; pemasangan tetap otomatis saat Zandor ditutup.
        cx.subscribe(&e, |e, _: &UpdateReady, cx| {
            let Phase::Ready { version, .. } = e.read(cx).phase.clone() else { return };
            let i = cx.global::<crate::i18n::I18n>().clone();
            let e2 = e.downgrade();
            crate::ui::Toasts::show(
                crate::ui::ToastInput {
                    description: Some(i.t("update.toast.readyBody").into()),
                    tone: crate::ui::ToastTone::Info,
                    timeout_ms: Some(15_000),
                    action: Some(crate::ui::ToastAction {
                        label: i.t("update.restartNow").into(),
                        on_click: std::rc::Rc::new(move |_, cx| {
                            e2.update(cx, |u, cx| u.restart_now(cx)).ok();
                        }),
                    }),
                    ..crate::ui::ToastInput::new(i.tp("update.toast.ready", &[("version", &version)]))
                },
                cx,
            );
        })
        .detach();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(FIRST_CHECK).await;
            loop {
                cx.update(|cx| e.update(cx, |u, cx| u.check(cx)));
                cx.background_executor().timer(CHECK_EVERY).await;
            }
        })
        .detach();
    }

    pub fn global(cx: &App) -> Option<Entity<Updater>> {
        cx.try_global::<UpdaterHandle>().map(|h| h.0.clone())
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Checking | Phase::Downloading { .. })
    }

    /// Cek manifes; bila ada versi lebih baru, unduh dan verifikasi di latar.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let Some(current) = release_version() else { return };
        if self.busy() || matches!(self.phase, Phase::Disabled(_)) {
            return;
        }
        self.phase = Phase::Checking;
        cx.notify();
        let have = self.pending.clone();
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Event>();
        std::thread::spawn(move || {
            let res = run_check(current, have.as_ref(), &|v, d, t| {
                let _ = tx.unbounded_send(Event::Progress(v.to_string(), d, t));
            });
            let _ = tx.unbounded_send(Event::Done(res));
        });
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(ev) = rx.next().await {
                let done = matches!(ev, Event::Done(_));
                let ok = this.update(cx, |u, cx| {
                    u.on_event(ev, cx);
                });
                if done || ok.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn on_event(&mut self, ev: Event, cx: &mut Context<Self>) {
        match ev {
            Event::Progress(version, done, total) => self.phase = Phase::Downloading { version, done, total },
            Event::Done(res) => {
                self.last_check = Some(chrono::Local::now());
                match res {
                    Ok(None) => {
                        self.phase = match &self.pending {
                            Some(p) => Phase::Ready { version: p.version.clone(), notes: String::new() },
                            None => Phase::UpToDate,
                        }
                    }
                    Ok(Some((p, notes))) => {
                        let fresh = self.pending.as_ref().is_none_or(|old| old.version != p.version);
                        self.phase = Phase::Ready { version: p.version.clone(), notes };
                        self.pending = Some(p);
                        if fresh {
                            cx.emit(UpdateReady);
                        }
                    }
                    Err(e) => {
                        crate::core::activity::log(cx, "update", "error", format!("pembaruan: {e}"));
                        self.phase = Phase::Error(e);
                    }
                }
            }
        }
        cx.notify();
    }

    /// Pasang sekarang: jalankan penerap lalu keluar; Zandor terbuka lagi setelah terpasang.
    pub fn restart_now(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.pending.clone() else { return };
        match launch_apply(&p, true) {
            Ok(()) => {
                self.applied = true;
                cx.quit();
            }
            Err(e) => {
                self.phase = Phase::Error(e);
                cx.notify();
            }
        }
    }

    /// Dipanggil saat aplikasi keluar: pembaruan yang siap dipasang senyap tanpa membuka ulang Zandor.
    pub fn apply_on_quit(cx: &mut App) {
        let Some(e) = Self::global(cx) else { return };
        let (pending, applied) = {
            let u = e.read(cx);
            (u.pending.clone(), u.applied)
        };
        if let (Some(p), false) = (pending, applied) {
            if let Err(err) = launch_apply(&p, false) {
                eprintln!("[zandor] update: gagal menjadwalkan pemasangan: {err}");
            }
        }
    }
}

/// Sinyal satu kali saat sebuah versi baru selesai diunduh dan terverifikasi.
pub struct UpdateReady;
impl gpui::EventEmitter<UpdateReady> for Updater {}

fn http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .http_status_as_error(true)
        .user_agent(format!("Zandor/{}", release_version().unwrap_or("dev")))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

fn public_key() -> Result<Vec<u8>, String> {
    hex_decode(PUBLIC_KEY_HEX).filter(|k| k.len() == 32).ok_or_else(|| "kunci publik pembaruan tidak valid".into())
}

/// Satu putaran cek: `Ok(None)` = tidak ada yang lebih baru dari yang sudah ada.
fn run_check(current: &str, have: Option<&Pending>, progress: &dyn Fn(&str, u64, u64)) -> Result<Option<(Pending, String)>, String> {
    let agent = http();
    let m: Manifest = agent
        .get(&manifest_url())
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(404) => "belum ada rilis yang diterbitkan".to_string(),
            e => format!("gagal mengambil manifes: {e}"),
        })?
        .body_mut()
        .read_json()
        .map_err(|e| format!("manifes tidak terbaca: {e}"))?;
    verify_manifest(&m, &public_key()?)?;
    let baseline = have.map(|p| p.version.as_str()).unwrap_or(current);
    if !is_newer(&m.version, baseline) {
        return Ok(None);
    }
    let dir = updates_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("Zandor-Setup-{}.exe", m.version));
    let pending = Pending { version: m.version.clone(), sha256: m.sha256.to_ascii_lowercase(), installer: file.clone() };
    if !sha256_file(&file).is_ok_and(|h| h == pending.sha256) {
        download(&agent, &m, &file, progress)?;
    }
    // Versi lama yang sempat siap tidak diperlukan lagi.
    if let Some(old) = have.filter(|o| o.installer != file) {
        let _ = std::fs::remove_file(&old.installer);
    }
    std::fs::write(pending_file(&dir), serde_json::to_vec(&pending).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(Some((pending, m.notes)))
}

fn download(agent: &ureq::Agent, m: &Manifest, dest: &Path, progress: &dyn Fn(&str, u64, u64)) -> Result<(), String> {
    let part = dest.with_extension("exe.part");
    let mut resp = agent.get(&m.url).call().map_err(|e| format!("gagal mengunduh: {e}"))?;
    let mut reader = resp.body_mut().as_reader();
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    let mut last = 0u64;
    progress(&m.version, 0, m.size);
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("unduhan terputus: {e}"))?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > m.size {
            let _ = std::fs::remove_file(&part);
            return Err("ukuran unduhan melebihi manifes".into());
        }
        h.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        if done - last >= 1 << 20 {
            last = done;
            progress(&m.version, done, m.size);
        }
    }
    drop(out);
    let sum = hex_encode(&h.finalize());
    if done != m.size || !sum.eq_ignore_ascii_case(&m.sha256) {
        let _ = std::fs::remove_file(&part);
        return Err("installer terunduh tidak cocok dengan manifes (SHA-256)".into());
    }
    let _ = std::fs::remove_file(dest);
    std::fs::rename(&part, dest).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::KeyPair;

    fn keypair() -> ring::signature::Ed25519KeyPair {
        ring::signature::Ed25519KeyPair::from_seed_unchecked(&[7u8; 32]).unwrap()
    }

    fn manifest(kp: &ring::signature::Ed25519KeyPair, version: &str, sha: &str, size: u64) -> Manifest {
        let sig = kp.sign(signed_message(version, sha, size).as_bytes());
        Manifest {
            version: version.into(),
            notes: String::new(),
            pub_date: String::new(),
            url: "https://example.invalid/x.exe".into(),
            sha256: sha.into(),
            size,
            signature: hex_encode(sig.as_ref()),
        }
    }

    #[test]
    fn versi_dibandingkan_dengan_semver() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("v0.10.0", "0.9.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-beta.1", "0.1.0"));
        assert!(!is_newer("bukan-versi", "0.1.0"));
    }

    #[test]
    fn manifes_bertanda_tangan_lolos() {
        let kp = keypair();
        let m = manifest(&kp, "0.2.0", &"ab".repeat(32), 123);
        assert_eq!(verify_manifest(&m, kp.public_key().as_ref()), Ok(()));
    }

    #[test]
    fn manifes_diubah_ditolak() {
        let kp = keypair();
        let mut m = manifest(&kp, "0.2.0", &"ab".repeat(32), 123);
        m.sha256 = "cd".repeat(32);
        assert!(verify_manifest(&m, kp.public_key().as_ref()).is_err());
        let mut m = manifest(&kp, "0.2.0", &"ab".repeat(32), 123);
        m.version = "9.9.9".into();
        assert!(verify_manifest(&m, kp.public_key().as_ref()).is_err());
        let other = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let m = manifest(&kp, "0.2.0", &"ab".repeat(32), 123);
        assert!(verify_manifest(&m, other.public_key().as_ref()).is_err());
    }

    #[test]
    fn kunci_publik_tertanam_valid() {
        assert_eq!(public_key().map(|k| k.len()), Ok(32));
    }

    #[test]
    fn tertunda_dibuang_bila_berkas_berubah() {
        let dir = std::env::temp_dir().join(format!("zandor-upd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let inst = dir.join("Zandor-Setup-0.2.0.exe");
        std::fs::write(&inst, b"isi").unwrap();
        let p = Pending { version: "0.2.0".into(), sha256: sha256_file(&inst).unwrap(), installer: inst.clone() };
        std::fs::write(pending_file(&dir), serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(valid_pending(&dir, "0.1.0"), Some(p.clone()));
        assert_eq!(valid_pending(&dir, "0.2.0"), None, "versi sama bukan pembaruan");
        std::fs::write(pending_file(&dir), serde_json::to_vec(&p).unwrap()).unwrap();
        std::fs::write(&inst, b"diubah").unwrap();
        assert_eq!(valid_pending(&dir, "0.1.0"), None);
        assert!(!pending_file(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
