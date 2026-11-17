//! Profil pengguna — port bagian profil AppearanceStore + shell/profile.ts: nama tampilan (maks 40) dan avatar
//! (inisial, gambar yang disalin ke folder data aplikasi, atau ilustrasi DiceBear gaya+seed). Disimpan di prefs.json.
use super::chat_store::{read_pref, write_pref};
use gpui::{App, AppContext, Context, Entity, Global};

pub const MAX_PROFILE_NAME: usize = 40;
const NAME_PREF: &str = "zandor.profileName";
const AVATAR_PREF: &str = "zandor.profileAvatar";
const ILLU_PREF: &str = "zandor.profileIllustration";

pub struct Profile {
    /// Nama tampilan pilihan pengguna (None = nama akun OS).
    pub name: Option<String>,
    /// Jalur gambar avatar (None = inisial).
    pub avatar: Option<String>,
    /// Ilustrasi DiceBear (gaya, seed); dipakai bila tidak ada gambar.
    pub illustration: Option<(String, String)>,
}
struct Handle(Entity<Profile>);
impl Global for Handle {}

pub fn os_name() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "Zandor".into())
}

impl Profile {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let avatar = read_pref(AVATAR_PREF).filter(|p| !p.is_empty() && std::path::Path::new(p).exists());
        let name = read_pref(NAME_PREF).filter(|n| !n.trim().is_empty());
        let illustration = read_pref(ILLU_PREF)
            .and_then(|v| v.split_once(':').map(|(a, b)| (a.to_string(), b.to_string())))
            .filter(|(s, d)| super::dicebear::valid(s, d));
        let e = cx.new(|_| Self { name, avatar, illustration });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<Handle>().0.clone()
    }
    /// Nama yang ditampilkan (profil, atau nama akun).
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(os_name)
    }

    pub fn set_name(&mut self, raw: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let name = raw.trim();
        if name.chars().count() > MAX_PROFILE_NAME {
            return Err(format!("nama maksimal {MAX_PROFILE_NAME} karakter"));
        }
        self.name = (!name.is_empty()).then(|| name.to_string());
        write_pref(NAME_PREF, name);
        cx.notify();
        Ok(())
    }

    /// Salin gambar ke folder data aplikasi lalu jadikan avatar.
    pub fn set_image(&mut self, src: &std::path::Path, cx: &mut Context<Self>) -> Result<(), String> {
        let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
            return Err("format gambar tidak didukung".into());
        }
        let meta = std::fs::metadata(src).map_err(|e| e.to_string())?;
        if meta.len() > 8 * 1024 * 1024 {
            return Err("gambar terlalu besar (maks 8 MB)".into());
        }
        let dir = super::services::app_data_dir();
        // nama unik agar cache gambar GPUI tidak menampilkan avatar lama
        // diperkecil ke 256 px JPEG seperti ProfileSettings.tsx (maxSide 256)
        let bytes = std::fs::read(src).map_err(|e| e.to_string())?;
        let (out, _) = super::gradient::resize_image_bytes(&bytes, 256, super::gradient::OutFmt::Jpeg(85))?;
        let dest = dir.join(format!("avatar-{}.jpg", super::projects::new_id()));
        std::fs::write(&dest, out).map_err(|e| e.to_string())?;
        if let Some(old) = self.avatar.take() {
            let _ = std::fs::remove_file(old);
        }
        let p = dest.to_string_lossy().into_owned();
        write_pref(AVATAR_PREF, &p);
        self.avatar = Some(p);
        self.illustration = None;
        write_pref(ILLU_PREF, "");
        cx.notify();
        Ok(())
    }

    pub fn use_initials(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.avatar.take() {
            let _ = std::fs::remove_file(old);
        }
        write_pref(AVATAR_PREF, "");
        self.illustration = None;
        write_pref(ILLU_PREF, "");
        cx.notify();
    }

    /// Pakai ilustrasi DiceBear (menggantikan gambar).
    pub fn set_illustration(&mut self, style: &str, seed: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if !super::dicebear::valid(style, seed) {
            return Err("avatar tidak valid".into());
        }
        if let Some(old) = self.avatar.take() {
            let _ = std::fs::remove_file(old);
        }
        write_pref(AVATAR_PREF, "");
        write_pref(ILLU_PREF, &format!("{style}:{seed}"));
        self.illustration = Some((style.into(), seed.into()));
        cx.notify();
        Ok(())
    }
}

/// Avatar profil siap pakai.
pub fn profile_avatar(cx: &App) -> crate::ui::feedback::Avatar {
    let p = Profile::global(cx).read(cx);
    let a = crate::ui::avatar(p.display_name());
    match (&p.avatar, &p.illustration) {
        (Some(src), _) => a.src(std::path::PathBuf::from(src)),
        (None, Some((s, d))) => match super::dicebear::image(s, d) {
            Some(img) => a.src(img),
            None => a,
        },
        _ => a,
    }
}
