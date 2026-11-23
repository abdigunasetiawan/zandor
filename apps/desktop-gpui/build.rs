fn main() {
    // Ikon aplikasi (taskbar/Explorer) di Windows.
    #[cfg(windows)]
    {
        let _ = embed_resource::compile("app.rc", embed_resource::NONE);
    }
    println!("cargo:rerun-if-changed=app.rc");
    // Versi tampil: 0.0.<jumlah commit>. Cadangan: versi Cargo bila git tidak tersedia.
    let git = |args: &[&str]| std::process::Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    // Build rilis resmi (scripts/rilis.ts) memberi ZANDOR_RELEASE_VERSION; itu juga yang mengaktifkan updater.
    println!("cargo:rerun-if-env-changed=ZANDOR_RELEASE_VERSION");
    let release = std::env::var("ZANDOR_RELEASE_VERSION").ok().filter(|v| !v.is_empty());
    let version = release.or_else(|| git(&["rev-list", "--count", "HEAD"]).filter(|n| !n.is_empty()).map(|n| format!("0.0.{n}"))).unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
    println!("cargo:rustc-env=ZANDOR_VERSION={version}");
    // Hitung ulang tiap commit baru (reflog HEAD berubah; juga bekerja di git worktree).
    for f in ["HEAD", "logs/HEAD"] {
        if let Some(path) = git(&["rev-parse", "--git-path", f]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=src/core/update_key.pub");
}
