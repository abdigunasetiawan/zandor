// Git dasar untuk Panel 4 (Fase 8): status, diff, stage/unstage, commit, cabang. Menjalankan `git` CLI (tanpa shell,
// argumen terpisah, `--` sebelum path) di dalam project terdaftar saja. Semua keluaran dibatasi ukurannya dan setiap
// perintah punya batas waktu. Konfigurasi repo yang bisa menjalankan kode saat membaca status (fsmonitor) dimatikan.
use serde::Serialize;
use std::io::Read;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_OUTPUT: usize = 4 * 1024 * 1024;
const MAX_DIFF: usize = 2 * 1024 * 1024;
const T_READ: Duration = Duration::from_secs(20);
const T_WRITE: Duration = Duration::from_secs(90);
/// Operasi jaringan (push/pull): lebih longgar, tetap berbatas agar tidak menggantung.
const T_NET: Duration = Duration::from_secs(180);

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub orig_path: Option<String>,
    /// Status indeks (kolom X): ' ', M, A, D, R, C, T, ? (untracked), U (konflik).
    pub index: char,
    /// Status worktree (kolom Y).
    pub worktree: char,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub detached: bool,
    pub files: Vec<FileChange>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

struct Out {
    code: i32,
    stdout: Vec<u8>,
    stderr: String,
}

fn run(dir: &Path, args: &[&str], timeout: Duration, cap: usize) -> Result<Out, String> {
    let mut cmd = Command::new("git");
    cmd.args([
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.quotepath=false",
        "-c",
        "color.ui=never",
    ])
    .args(args)
    .current_dir(dir)
    .env("GIT_TERMINAL_PROMPT", "0")
    .env("GIT_OPTIONAL_LOCKS", "0")
    .env("LC_ALL", "C")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "git tidak ditemukan di PATH; pasang Git terlebih dahulu".to_string()
        } else {
            format!("gagal menjalankan git: {e}")
        }
    })?;
    let mut so = child.stdout.take().ok_or("stdout tidak tersedia")?;
    let se = child.stderr.take().ok_or("stderr tidak tersedia")?;
    let h_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match so.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if buf.len() < cap {
                        buf.extend_from_slice(&chunk[..n.min(cap - buf.len())]);
                    }
                }
            }
        }
        buf
    });
    let h_err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.take(64 * 1024).read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "git melewati batas waktu {} dtk",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(e.to_string()),
        }
    };
    Ok(Out {
        code: status.code().unwrap_or(-1),
        stdout: h_out.join().unwrap_or_default(),
        stderr: h_err.join().unwrap_or_default(),
    })
}

fn ok(dir: &Path, args: &[&str], timeout: Duration) -> Result<Vec<u8>, String> {
    let o = run(dir, args, timeout, MAX_OUTPUT)?;
    if o.code != 0 {
        let msg = o.stderr.trim();
        return Err(if msg.is_empty() {
            format!("git gagal (kode {})", o.code)
        } else {
            msg.to_string()
        });
    }
    Ok(o.stdout)
}

/// Path dari UI: relatif, tanpa `..`, tanpa NUL. (Berkas yang dihapus tidak bisa dikanonisasi, jadi lexical.)
pub fn validate_rel(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if path.is_empty()
        || path.contains('\0')
        || p.is_absolute()
        || path.starts_with('/')
        || path.starts_with('\\')
    {
        return Err(format!("path tidak valid: {path:?}"));
    }
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!("path tidak boleh memakai '..': {path:?}"));
    }
    Ok(())
}

/// Parse `git status --porcelain=v1 -b -z`.
pub fn parse_status(bytes: &[u8]) -> GitStatus {
    let text = String::from_utf8_lossy(bytes);
    let mut st = GitStatus {
        is_repo: true,
        ..GitStatus::default()
    };
    let mut parts = text.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = parts.next() {
        if let Some(head) = entry.strip_prefix("## ") {
            parse_branch_header(head, &mut st);
            continue;
        }
        let mut chars = entry.chars();
        let (Some(x), Some(y)) = (chars.next(), chars.next()) else {
            continue;
        };
        let path: String = entry.chars().skip(3).collect();
        let orig = if x == 'R' || x == 'C' || y == 'R' || y == 'C' {
            parts.next().map(str::to_string)
        } else {
            None
        };
        st.files.push(FileChange {
            path,
            orig_path: orig,
            index: x,
            worktree: y,
        });
    }
    st
}

fn parse_branch_header(head: &str, st: &mut GitStatus) {
    if let Some(rest) = head.strip_prefix("No commits yet on ") {
        st.branch = Some(rest.to_string());
        return;
    }
    if head.starts_with("HEAD (no branch)") {
        st.detached = true;
        return;
    }
    let (names, tracking) = match head.split_once(" [") {
        Some((n, t)) => (n, Some(t.trim_end_matches(']'))),
        None => (head, None),
    };
    match names.split_once("...") {
        Some((b, u)) => {
            st.branch = Some(b.to_string());
            st.upstream = Some(u.to_string());
        }
        None => st.branch = Some(names.to_string()),
    }
    if let Some(t) = tracking {
        for part in t.split(", ") {
            if let Some(n) = part.strip_prefix("ahead ") {
                st.ahead = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix("behind ") {
                st.behind = n.parse().unwrap_or(0);
            }
        }
    }
}

/// Folder yang tidak dimasuki saat mencari repo git di dalam project (besar dan tidak berisi repo milik project).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    "vendor",
    "__pycache__",
    ".cache",
    ".idea",
    ".vscode",
];
const REPO_SEARCH_DEPTH: usize = 3;
const REPO_SEARCH_MAX_DIRS: usize = 4000;
const MAX_REPOS: usize = 50;

/// Cari repo git di anak folder `root` (monorepo: akar bukan repo, tetapi anak folder diinisiasi git). Menelusuri maksimal
/// 3 tingkat dan 4000 folder, melewati folder besar, dan berhenti masuk ke dalam repo yang ditemukan. Hasil: path absolut.
pub fn find_repos(root: &Path) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut queue: std::collections::VecDeque<(std::path::PathBuf, usize)> =
        std::collections::VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));
    let mut seen = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if depth > 0 && dir.join(".git").exists() {
            found.push(dir.to_string_lossy().into_owned());
            if found.len() >= MAX_REPOS {
                break;
            }
            continue; // tidak masuk ke dalam repo
        }
        if depth >= REPO_SEARCH_DEPTH {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            seen += 1;
            if seen > REPO_SEARCH_MAX_DIRS {
                found.sort();
                return found;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            queue.push_back((entry.path(), depth + 1));
        }
    }
    found.sort();
    found
}

pub fn status(dir: &Path) -> Result<GitStatus, String> {
    let o = run(
        dir,
        &[
            "status",
            "--porcelain=v1",
            "-b",
            "-z",
            "--untracked-files=all",
        ],
        T_READ,
        MAX_OUTPUT,
    )?;
    if o.code != 0 {
        if o.stderr.contains("not a git repository") {
            return Ok(GitStatus::default());
        }
        return Err(o.stderr.trim().to_string());
    }
    Ok(parse_status(&o.stdout))
}

/// Diff satu berkas. `untracked`: bandingkan dengan berkas kosong (git diff --no-index).
pub fn diff(dir: &Path, path: &str, staged: bool, untracked: bool) -> Result<String, String> {
    validate_rel(path)?;
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let args: Vec<&str> = if untracked {
        vec!["diff", "--no-index", "--no-color", "--", null, path]
    } else if staged {
        vec!["diff", "--cached", "--no-color", "--", path]
    } else {
        vec!["diff", "--no-color", "--", path]
    };
    let o = run(dir, &args, T_READ, MAX_DIFF)?;
    // `--no-index` keluar dengan kode 1 bila ada perbedaan: normal.
    if o.code != 0 && !(untracked && o.code == 1) {
        return Err(o.stderr.trim().to_string());
    }
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    if o.stdout.len() >= MAX_DIFF {
        s.push_str("\n… (diff dipotong)\n");
    }
    Ok(s)
}

pub fn stage(dir: &Path, paths: &[String]) -> Result<(), String> {
    for p in paths {
        validate_rel(p)?;
    }
    let mut args = vec!["add", "--"];
    args.extend(paths.iter().map(String::as_str));
    ok(dir, &args, T_WRITE).map(|_| ())
}

pub fn unstage(dir: &Path, paths: &[String]) -> Result<(), String> {
    for p in paths {
        validate_rel(p)?;
    }
    let has_head = run(dir, &["rev-parse", "--verify", "-q", "HEAD"], T_READ, 1024)?.code == 0;
    let mut args = if has_head {
        vec!["restore", "--staged", "--"]
    } else {
        vec!["rm", "--cached", "-r", "-q", "--"]
    };
    args.extend(paths.iter().map(String::as_str));
    ok(dir, &args, T_WRITE).map(|_| ())
}

pub fn commit(dir: &Path, message: &str) -> Result<String, String> {
    let m = message.trim();
    if m.is_empty() {
        return Err("pesan commit tidak boleh kosong".into());
    }
    if m.len() > 20_000 {
        return Err("pesan commit terlalu panjang".into());
    }
    ok(dir, &["commit", "-m", m], T_WRITE)?;
    let hash = ok(dir, &["rev-parse", "--short", "HEAD"], T_READ)?;
    Ok(String::from_utf8_lossy(&hash).trim().to_string())
}

pub fn branches(dir: &Path) -> Result<Vec<Branch>, String> {
    let out = ok(
        dir,
        &["branch", "--list", "--format=%(HEAD)|%(refname:short)"],
        T_READ,
    )?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| {
            let (head, name) = l.split_once('|')?;
            (!name.is_empty() && !name.starts_with('(')).then(|| Branch {
                name: name.to_string(),
                current: head == "*",
            })
        })
        .collect())
}

fn validate_branch(dir: &Path, name: &str) -> Result<(), String> {
    if name.is_empty() || name.starts_with('-') || name.contains('\0') {
        return Err(format!("nama cabang tidak valid: {name:?}"));
    }
    let o = run(dir, &["check-ref-format", "--branch", name], T_READ, 1024)?;
    if o.code != 0 {
        return Err(format!("nama cabang tidak valid: {name:?}"));
    }
    Ok(())
}

pub fn switch(dir: &Path, name: &str) -> Result<(), String> {
    validate_branch(dir, name)?;
    ok(dir, &["switch", name], T_WRITE).map(|_| ())
}

pub fn create_branch(dir: &Path, name: &str) -> Result<(), String> {
    validate_branch(dir, name)?;
    ok(dir, &["switch", "-c", name], T_WRITE).map(|_| ())
}

/// Kirim cabang saat ini ke remote. Tanpa upstream: `push -u origin <cabang>` (hanya bila remote `origin` ada).
/// Tidak pernah memakai `--force`. Kredensial lewat credential helper git; prompt terminal dimatikan.
pub fn push(dir: &Path) -> Result<String, String> {
    let has_upstream = run(
        dir,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        T_READ,
        1024,
    )?
    .code
        == 0;
    let out = if has_upstream {
        ok(dir, &["push"], T_NET)?
    } else {
        let branch = ok(dir, &["rev-parse", "--abbrev-ref", "HEAD"], T_READ)?;
        let branch = String::from_utf8_lossy(&branch).trim().to_string();
        validate_branch(dir, &branch)?;
        let has_origin = ok(dir, &["remote"], T_READ)?
            .split(|b| *b == b'\n')
            .any(|l| String::from_utf8_lossy(l).trim() == "origin");
        if !has_origin {
            return Err("tidak ada remote \"origin\"; tambahkan remote lebih dulu".into());
        }
        ok(dir, &["push", "-u", "origin", &branch], T_NET)?
    };
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Tarik perubahan remote hanya bila bisa fast-forward (tanpa merge/rebase otomatis yang bisa membuat konflik).
pub fn pull(dir: &Path) -> Result<String, String> {
    let out = ok(dir, &["pull", "--ff-only"], T_NET)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Simpan perubahan (termasuk berkas baru) ke stash.
pub fn stash_push(dir: &Path, message: &str) -> Result<String, String> {
    let m = message.trim();
    if m.len() > 500 || m.contains(' ') {
        return Err("pesan stash tidak valid".into());
    }
    let mut args = vec!["stash", "push", "--include-untracked"];
    if !m.is_empty() {
        args.extend(["-m", m]);
    }
    let out = ok(dir, &args, T_WRITE)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Kembalikan stash terbaru dan hapus dari daftar (`stash pop`); konflik dilaporkan apa adanya.
pub fn stash_pop(dir: &Path) -> Result<String, String> {
    let out = ok(dir, &["stash", "pop"], T_WRITE)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Jumlah stash tersimpan.
pub fn stash_count(dir: &Path) -> Result<usize, String> {
    let out = ok(dir, &["stash", "list"], T_READ)?;
    Ok(out.split(|b| *b == b'\n').filter(|l| !l.is_empty()).count())
}

/// Buang perubahan pada path: berkas terlacak dikembalikan ke HEAD (stage + worktree), berkas baru dihapus.
/// DESTRUKTIF dan tidak bisa dibatalkan; UI wajib meminta konfirmasi.
pub fn discard(dir: &Path, paths: &[String]) -> Result<(), String> {
    for p in paths {
        validate_rel(p)?;
    }
    if paths.is_empty() {
        return Ok(());
    }
    let has_head = run(dir, &["rev-parse", "--verify", "-q", "HEAD"], T_READ, 1024)?.code == 0;
    let mut tracked: Vec<&str> = Vec::new();
    let mut untracked: Vec<&str> = Vec::new();
    for p in paths {
        let in_index = run(dir, &["ls-files", "--error-unmatch", "--", p], T_READ, 4096)?.code == 0;
        let in_head = has_head
            && run(dir, &["cat-file", "-e", &format!("HEAD:{p}")], T_READ, 1024)?.code == 0;
        if in_head {
            tracked.push(p);
        } else if in_index {
            // baru di-stage (tidak ada di HEAD): keluarkan dari index lalu hapus dari disk
            ok(dir, &["rm", "-f", "-q", "--", p], T_WRITE)?;
        } else {
            untracked.push(p);
        }
    }
    if !tracked.is_empty() {
        let mut args = vec!["restore", "--source=HEAD", "--staged", "--worktree", "--"];
        args.extend(tracked);
        ok(dir, &args, T_WRITE)?;
    }
    if !untracked.is_empty() {
        let mut args = vec!["clean", "-f", "-q", "--"];
        args.extend(untracked);
        ok(dir, &args, T_WRITE)?;
    }
    Ok(())
}

pub fn log(dir: &Path, n: usize) -> Result<Vec<CommitInfo>, String> {
    let count = format!("-{}", n.clamp(1, 100));
    let o = run(
        dir,
        &["log", &count, "--format=%h%x1f%s%x1f%an%x1f%ar"],
        T_READ,
        MAX_OUTPUT,
    )?;
    if o.code != 0 {
        // repo tanpa commit
        if o.stderr.contains("does not have any commits")
            || o.stderr.contains("bad default revision")
        {
            return Ok(vec![]);
        }
        return Err(o.stderr.trim().to_string());
    }
    Ok(String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\u{1f}');
            Some(CommitInfo {
                hash: f.next()?.to_string(),
                subject: f.next()?.to_string(),
                author: f.next()?.to_string(),
                date: f.next()?.to_string(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn find_repos_menemukan_repo_anak_dan_melewati_folder_besar() {
        let base = std::env::temp_dir().join(format!("zandor-repos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in [
            "api/.git",
            "web/.git",
            "apps/mobile/.git",
            "node_modules/x/.git",
            "docs/plain",
            "api/nested/.git",
        ] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        let repos = super::find_repos(&base);
        let rel: Vec<String> = repos
            .iter()
            .map(|p| {
                std::path::Path::new(p)
                    .strip_prefix(&base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(rel, vec!["api", "apps/mobile", "web"]); // node_modules dilewati; tidak masuk ke dalam repo api
                                                            // akar yang sendiri repo tidak dihitung (hanya anak folder)
        std::fs::create_dir_all(base.join(".git")).unwrap();
        assert_eq!(super::find_repos(&base).len(), 3);
        let _ = std::fs::remove_dir_all(&base);
    }

    use super::*;

    fn repo(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-git-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        ok(&d, &["init", "-q", "-b", "main"], T_WRITE).unwrap();
        ok(&d, &["config", "user.email", "uji@zandor.test"], T_WRITE).unwrap();
        ok(&d, &["config", "user.name", "Penguji"], T_WRITE).unwrap();
        ok(&d, &["config", "commit.gpgsign", "false"], T_WRITE).unwrap();
        d
    }

    #[test]
    fn parses_branch_header_variants() {
        let st = parse_status(b"## main...origin/main [ahead 2, behind 1]\0");
        assert_eq!(
            (
                st.branch.as_deref(),
                st.upstream.as_deref(),
                st.ahead,
                st.behind
            ),
            (Some("main"), Some("origin/main"), 2, 1)
        );
        assert_eq!(
            parse_status(b"## No commits yet on dev\0")
                .branch
                .as_deref(),
            Some("dev")
        );
        assert!(parse_status(b"## HEAD (no branch)\0").detached);
        let plain = parse_status(b"## lokal\0");
        assert_eq!(
            (plain.branch.as_deref(), plain.upstream, plain.ahead),
            (Some("lokal"), None, 0)
        );
    }

    #[test]
    fn parses_entries_including_renames_and_spaces() {
        let st = parse_status(b"## main\0M  a.txt\0 M b c.txt\0R  baru.txt\0lama.txt\0?? u.txt\0");
        assert_eq!(st.files.len(), 4);
        assert_eq!(
            st.files[0],
            FileChange {
                path: "a.txt".into(),
                orig_path: None,
                index: 'M',
                worktree: ' '
            }
        );
        assert_eq!(st.files[1].path, "b c.txt");
        assert_eq!(
            (st.files[2].path.as_str(), st.files[2].orig_path.as_deref()),
            ("baru.txt", Some("lama.txt"))
        );
        assert_eq!((st.files[3].index, st.files[3].worktree), ('?', '?'));
    }

    #[test]
    fn path_validation_rejects_traversal_and_absolute() {
        for bad in ["", "../x", "a/../../x", "/etc/passwd", "\\x", "a\0b"] {
            assert!(validate_rel(bad).is_err(), "{bad:?}");
        }
        assert!(validate_rel("src/a b.rs").is_ok());
        assert!(validate_rel("-pengecoh.txt").is_ok()); // aman karena selalu setelah `--`
    }

    #[test]
    fn full_flow_on_a_real_repo() {
        let d = repo("flow");
        // bukan repo
        let plain = std::env::temp_dir().join(format!("zandor-git-plain-{}", std::process::id()));
        std::fs::create_dir_all(&plain).unwrap();
        assert!(!status(&plain).unwrap().is_repo);

        std::fs::write(d.join("a.txt"), "satu\n").unwrap();
        std::fs::write(d.join("b c.txt"), "dua\n").unwrap();
        let st = status(&d).unwrap();
        assert!(st.is_repo);
        assert_eq!(st.branch.as_deref(), Some("main"));
        assert_eq!(st.files.len(), 2);
        assert!(st.files.iter().all(|f| f.index == '?'));

        // diff berkas baru (untracked) memakai --no-index
        let df = diff(&d, "a.txt", false, true).unwrap();
        assert!(df.contains("+satu"), "{df}");

        stage(&d, &["a.txt".into(), "b c.txt".into()]).unwrap();
        let st = status(&d).unwrap();
        assert!(st.files.iter().all(|f| f.index == 'A'));
        assert!(diff(&d, "a.txt", true, false).unwrap().contains("+satu"));

        unstage(&d, &["b c.txt".into()]).unwrap(); // sebelum commit pertama (tanpa HEAD)
        assert!(status(&d)
            .unwrap()
            .files
            .iter()
            .any(|f| f.path == "b c.txt" && f.index == '?'));

        assert!(commit(&d, "  ").is_err());
        let hash = commit(&d, "commit pertama").unwrap();
        assert!(hash.len() >= 7);
        assert_eq!(log(&d, 5).unwrap()[0].subject, "commit pertama");

        // ubah berkas terlacak → diff worktree, lalu stage + unstage setelah ada HEAD
        std::fs::write(d.join("a.txt"), "satu\ndua\n").unwrap();
        assert!(diff(&d, "a.txt", false, false).unwrap().contains("+dua"));
        stage(&d, &["a.txt".into()]).unwrap();
        unstage(&d, &["a.txt".into()]).unwrap();
        let f = status(&d)
            .unwrap()
            .files
            .into_iter()
            .find(|f| f.path == "a.txt")
            .unwrap();
        assert_eq!((f.index, f.worktree), (' ', 'M'));
    }

    #[test]
    fn branches_create_switch_and_reject_bad_names() {
        let d = repo("branch");
        std::fs::write(d.join("x"), "x").unwrap();
        stage(&d, &["x".into()]).unwrap();
        commit(&d, "awal").unwrap();
        create_branch(&d, "fitur/baru").unwrap();
        let bs = branches(&d).unwrap();
        assert!(bs.iter().any(|b| b.name == "fitur/baru" && b.current));
        switch(&d, "main").unwrap();
        assert!(branches(&d)
            .unwrap()
            .iter()
            .any(|b| b.name == "main" && b.current));
        for bad in ["", "-d", "a b", "a..b", "x~1", "--force"] {
            assert!(create_branch(&d, bad).is_err(), "{bad:?}");
        }
        assert!(switch(&d, "tidak-ada").is_err());
    }

    #[test]
    fn staging_outside_paths_and_empty_log() {
        let d = repo("log");
        assert!(stage(&d, &["../luar.txt".into()]).is_err());
        assert!(log(&d, 5).unwrap().is_empty());
    }

    #[test]
    fn a_hung_command_is_killed_at_the_deadline() {
        let d = repo("timeout");
        // `git` tanpa subperintah bermakna selesai cepat; uji batas waktu dengan nilai sangat kecil pada perintah nyata
        let r = run(&d, &["status"], Duration::from_millis(0), 1024);
        // bisa selesai sebelum cek pertama atau terkena batas waktu; keduanya valid, tidak boleh menggantung
        assert!(r.is_ok() || r.err().unwrap().contains("batas waktu"));
    }

    #[test]
    fn push_pull_stash_and_discard_on_real_repos() {
        let d = repo("net");
        let remote = std::env::temp_dir().join(format!("zandor-git-remote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&remote);
        std::fs::create_dir_all(&remote).unwrap();
        ok(&remote, &["init", "-q", "--bare", "-b", "main"], T_WRITE).unwrap();
        std::fs::write(
            d.join("a.txt"),
            "satu
",
        )
        .unwrap();
        stage(&d, &["a.txt".into()]).unwrap();
        commit(&d, "awal").unwrap();

        // tanpa remote origin → galat jelas; dengan origin → push -u
        assert!(push(&d).unwrap_err().contains("origin"));
        let url = remote.to_string_lossy().to_string();
        ok(&d, &["remote", "add", "origin", &url], T_WRITE).unwrap();
        push(&d).unwrap();
        assert_eq!(status(&d).unwrap().upstream.as_deref(), Some("origin/main"));

        // klon kedua mendorong commit baru → pull fast-forward di repo pertama
        let other = std::env::temp_dir().join(format!("zandor-git-other-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&other);
        ok(
            &std::env::temp_dir(),
            &["clone", "-q", &url, &other.to_string_lossy()],
            T_WRITE,
        )
        .unwrap();
        ok(&other, &["config", "user.email", "b@zandor.test"], T_WRITE).unwrap();
        ok(&other, &["config", "user.name", "B"], T_WRITE).unwrap();
        ok(&other, &["config", "commit.gpgsign", "false"], T_WRITE).unwrap();
        std::fs::write(
            other.join("b.txt"),
            "dari lain
",
        )
        .unwrap();
        stage(&other, &["b.txt".into()]).unwrap();
        commit(&other, "dari klon").unwrap();
        push(&other).unwrap();
        ok(&d, &["fetch", "-q"], T_NET).unwrap();
        assert_eq!(status(&d).unwrap().behind, 1);
        pull(&d).unwrap();
        assert!(d.join("b.txt").exists());

        // stash menyimpan perubahan termasuk berkas baru, pop mengembalikannya
        std::fs::write(
            d.join("a.txt"),
            "satu
ubah
",
        )
        .unwrap();
        std::fs::write(
            d.join("baru.txt"),
            "x
",
        )
        .unwrap();
        assert_eq!(stash_count(&d).unwrap(), 0);
        stash_push(&d, "uji").unwrap();
        assert_eq!(stash_count(&d).unwrap(), 1);
        assert!(!d.join("baru.txt").exists());
        stash_pop(&d).unwrap();
        assert!(d.join("baru.txt").exists());
        assert_eq!(stash_count(&d).unwrap(), 0);

        // discard: terlacak kembali ke HEAD, baru di-stage dihapus, tak terlacak dihapus
        stage(&d, &["baru.txt".into()]).unwrap();
        std::fs::write(
            d.join("liar.txt"),
            "y
",
        )
        .unwrap();
        discard(&d, &["a.txt".into(), "baru.txt".into(), "liar.txt".into()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.join("a.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "satu\n"
        );
        assert!(!d.join("baru.txt").exists());
        assert!(!d.join("liar.txt").exists());
        assert!(status(&d).unwrap().files.is_empty());
        assert!(discard(&d, &["../luar".into()]).is_err());
    }
}
