//! Operasi berkas untuk tab Berkas: salin dan pindah berkas/folder ke sebuah folder tujuan (tempel, seret-lepas, unggah).
//!
//! Aturan keselamatan: tidak pernah menimpa (nama bentrok diberi akhiran " (2)", " (3)", …), folder tidak bisa
//! dimasukkan ke dalam dirinya sendiri, dan tautan simbolik tidak diikuti saat menyalin folder (dilewati).
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// Hasil satu operasi: jalur baru, atau `None` bila tidak ada yang berubah (mis. memindahkan ke folder asalnya).
pub type Outcome = Result<Option<PathBuf>, String>;

/// Nama yang belum dipakai di `dir`: "a.txt" → "a (2).txt", folder "x" → "x (2)".
pub fn unique_dest(dir: &Path, name: &OsStr) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let (stem, ext) = match (p.file_stem(), p.extension()) {
        (Some(s), Some(e)) if !s.is_empty() => (s.to_string_lossy().into_owned(), Some(e.to_string_lossy().into_owned())),
        _ => (name.to_string_lossy().into_owned(), None),
    };
    for n in 2..10_000 {
        let candidate = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let path = dir.join(candidate);
        if !path.exists() {
            return path;
        }
    }
    first
}

/// `child` berada di dalam (atau sama dengan) `parent`. Dibandingkan setelah dinormalkan lewat `canonicalize`.
pub fn is_inside(child: &Path, parent: &Path) -> bool {
    let norm = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    norm(child).starts_with(norm(parent))
}

/// Jalur relatif aman: tanpa akar, drive, atau "..".
pub fn safe_rel(rel: &str) -> bool {
    Path::new(rel).components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn copy_tree(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    for item in std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let item = item.map_err(|e| e.to_string())?;
        let ty = item.file_type().map_err(|e| e.to_string())?;
        let to = dest.join(item.file_name());
        if ty.is_symlink() {
            continue; // tautan tidak diikuti: mencegah keluar dari folder atau perulangan
        }
        if ty.is_dir() {
            copy_tree(&item.path(), &to)?;
        } else {
            std::fs::copy(item.path(), &to).map_err(|e| format!("{}: {e}", item.path().display()))?;
        }
    }
    Ok(())
}

/// Salin berkas/folder `src` ke dalam `dest_dir`. Mengembalikan jalur salinan.
pub fn copy_entry(src: &Path, dest_dir: &Path) -> Outcome {
    let meta = std::fs::symlink_metadata(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let name = src.file_name().ok_or("jalur sumber tidak valid")?;
    if !dest_dir.is_dir() {
        return Err(format!("{}: bukan folder", dest_dir.display()));
    }
    if meta.is_dir() && is_inside(dest_dir, src) {
        return Err(format!("{}: folder tidak bisa disalin ke dalam dirinya sendiri", src.display()));
    }
    let to = unique_dest(dest_dir, name);
    if meta.is_dir() {
        copy_tree(src, &to)?;
    } else {
        std::fs::copy(src, &to).map_err(|e| format!("{}: {e}", src.display()))?;
    }
    Ok(Some(to))
}

/// Pindahkan `src` ke dalam `dest_dir` (rename; lintas drive: salin lalu hapus sumber). `Ok(None)` bila sumber sudah di sana.
pub fn move_entry(src: &Path, dest_dir: &Path) -> Outcome {
    let meta = std::fs::symlink_metadata(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let name = src.file_name().ok_or("jalur sumber tidak valid")?;
    if !dest_dir.is_dir() {
        return Err(format!("{}: bukan folder", dest_dir.display()));
    }
    if src.parent().is_some_and(|p| std::fs::canonicalize(p).ok() == std::fs::canonicalize(dest_dir).ok()) {
        return Ok(None);
    }
    if meta.is_dir() && is_inside(dest_dir, src) {
        return Err(format!("{}: folder tidak bisa dipindah ke dalam dirinya sendiri", src.display()));
    }
    let to = unique_dest(dest_dir, name);
    if std::fs::rename(src, &to).is_ok() {
        return Ok(Some(to));
    }
    // rename gagal (mis. beda drive): salin utuh dulu, hapus sumber hanya bila salinan berhasil
    if meta.is_dir() {
        copy_tree(src, &to)?;
        std::fs::remove_dir_all(src).map_err(|e| format!("{}: {e}", src.display()))?;
    } else {
        std::fs::copy(src, &to).map_err(|e| format!("{}: {e}", src.display()))?;
        std::fs::remove_file(src).map_err(|e| format!("{}: {e}", src.display()))?;
    }
    Ok(Some(to))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-fsops-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn nama_bentrok_diberi_akhiran() {
        let d = scratch("uniq");
        std::fs::write(d.join("a.txt"), "1").unwrap();
        std::fs::write(d.join("a (2).txt"), "2").unwrap();
        std::fs::create_dir(d.join("x")).unwrap();
        assert_eq!(unique_dest(&d, OsStr::new("a.txt")), d.join("a (3).txt"));
        assert_eq!(unique_dest(&d, OsStr::new("x")), d.join("x (2)"));
        assert_eq!(unique_dest(&d, OsStr::new("baru.md")), d.join("baru.md"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn salin_folder_utuh_tanpa_menimpa() {
        let d = scratch("copy");
        let src = d.join("proj");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub").join("f.txt"), "isi").unwrap();
        std::fs::create_dir(d.join("tujuan")).unwrap();
        let a = copy_entry(&src, &d.join("tujuan")).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(a.join("sub").join("f.txt")).unwrap(), "isi");
        let b = copy_entry(&src, &d.join("tujuan")).unwrap().unwrap();
        assert_eq!(b.file_name().unwrap(), "proj (2)"); // salinan kedua tidak menimpa
        assert!(src.join("sub").join("f.txt").exists()); // sumber utuh
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn folder_tidak_masuk_ke_dirinya_sendiri() {
        let d = scratch("self");
        let src = d.join("a");
        std::fs::create_dir_all(src.join("b")).unwrap();
        assert!(copy_entry(&src, &src.join("b")).is_err());
        assert!(move_entry(&src, &src.join("b")).is_err());
        assert!(src.join("b").is_dir());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn pindah_berkas_dan_noop_di_folder_asal() {
        let d = scratch("move");
        std::fs::write(d.join("f.txt"), "x").unwrap();
        std::fs::create_dir(d.join("t")).unwrap();
        assert_eq!(move_entry(&d.join("f.txt"), &d).unwrap(), None);
        let to = move_entry(&d.join("f.txt"), &d.join("t")).unwrap().unwrap();
        assert_eq!(to, d.join("t").join("f.txt"));
        assert!(!d.join("f.txt").exists());
        assert!(move_entry(&d.join("tidak-ada"), &d.join("t")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn jalur_relatif_aman() {
        assert!(safe_rel("a/b/c.txt"));
        assert!(safe_rel(""));
        assert!(!safe_rel("../x"));
        assert!(!safe_rel("a/../../x"));
        #[cfg(windows)]
        assert!(!safe_rel("C:\\Windows"));
    }
}
