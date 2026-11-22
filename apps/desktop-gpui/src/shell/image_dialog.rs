//! Dialog "Generate gambar" (ADR-077) — port ImageGenerateDialog.tsx + perintah Rust `image_generate`: prompt + ukuran
//! → model gambar bawaan → gambar di `<project>/images/`, lalu dibuka di Panel 3.
use crate::core::model_prefs::ModelPrefsStore;
use crate::core::projects::ProjectStore;
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::{button, select, ButtonVariant, InputChanged, TextInput, TypeExt};
use gpui::{div, prelude::*, Context, Entity, EventEmitter, IntoElement, Window};
use std::path::PathBuf;

pub enum ImageEvent {
    Close,
}
impl EventEmitter<ImageEvent> for ImageDialog {}

const SIZES: [(&str, &str); 3] = [("1024x1024", "img.sizeSquare"), ("1536x1024", "img.sizeLandscape"), ("1024x1536", "img.sizePortrait")];

pub struct ImageDialog {
    prompt: Entity<TextInput>,
    size: &'static str,
    busy: bool,
    error: Option<String>,
}

/// Port imageFileName: `images/<slug>-<YYYYMMDD-HHMMSS>.png`.
pub fn image_file_name(prompt: &str, now: chrono::DateTime<chrono::Local>) -> String {
    let mut slug = String::new();
    for c in prompt.to_lowercase().chars() {
        let c = deunicode_char(c);
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(40).collect();
    let slug = slug.trim_end_matches('-');
    let slug = if slug.is_empty() { "gambar" } else { slug };
    format!("images/{slug}-{}.png", now.format("%Y%m%d-%H%M%S"))
}

/// NFKD sederhana: buang diakritik huruf Latin umum.
fn deunicode_char(c: char) -> char {
    match c {
        'à'..='å' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ñ' => 'n',
        'ò'..='ö' => 'o',
        'ù'..='ü' => 'u',
        'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// Port perintah `image_generate` (blocking: jalankan di thread latar). Mengembalikan path relatif yang ditulis.
pub fn generate(provider_id: &str, model: &str, prompt: &str, size: &str, root: &str, rel: &str) -> Result<String, String> {
    if prompt.trim().is_empty() || prompt.len() > 4000 {
        return Err("prompt gambar kosong atau terlalu panjang (maks 4000 karakter)".into());
    }
    if !SIZES.iter().any(|(s, _)| *s == size) {
        return Err(format!("ukuran gambar tidak didukung: {size}"));
    }
    let root = PathBuf::from(root);
    if !root.is_dir() {
        return Err(format!("folder tujuan tidak ada: {}", root.display()));
    }
    let store = crate::providers::ProviderStore::new(&crate::core::services::app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)));
    let provider = store.load().0.into_iter().find(|p| p.id == provider_id).ok_or_else(|| format!("provider \"{provider_id}\" tidak terdaftar di Zandor (tambahkan kuncinya di Pengaturan → Provider dan model)"))?;
    let key = store.api_key(provider_id)?;
    if key.is_none() && provider.base_url.is_none() {
        return Err(format!("kunci API {} belum tersimpan", provider.name));
    }
    let bytes = crate::images::generate(&provider, key.as_deref(), model, prompt, size)?;
    let ext = crate::images::sniff_ext(&bytes).ok_or("respons provider bukan gambar PNG/JPEG/WebP")?;
    let (full, written) = crate::images::target_path(&root, rel, ext)?;
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&full, bytes).map_err(|e| e.to_string())?;
    Ok(written)
}

/// Folder tujuan: sesi terpilih, lalu project pertama, lalu folder Chat.
fn target_root(cx: &gpui::App) -> Option<String> {
    let st = ProjectStore::global(cx).read(cx);
    st.selected().map(|(_, s, _)| s.directory).or_else(|| st.projects.first().map(|p| p.path.clone())).or_else(|| st.chat_dir.clone())
}

impl ImageDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prompt = cx.new(|cx| {
            let mut t = TextInput::new(cx, "");
            t.multiline = true;
            t.wrap = true;
            t
        });
        cx.subscribe(&prompt, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        gpui::Focusable::focus_handle(prompt.read(cx), cx).focus(window, cx);
        Self { prompt, size: "1024x1024", busy: false, error: None }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let prompt = self.prompt.read(cx).text().trim().to_string();
        let model = ModelPrefsStore::global(cx).and_then(|p| p.read(cx).prefs.default_image.clone());
        let (Some(model), Some(root)) = (model, target_root(cx)) else { return };
        if prompt.is_empty() || self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let size = self.size;
        let rel = image_file_name(&prompt, chrono::Local::now());
        let r2 = root.clone();
        let task = cx.background_spawn(async move { generate(&model.provider_id, &model.model_id, &prompt, size, &r2, &rel) });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            this.update(cx, |s, cx| {
                s.busy = false;
                match res {
                    Ok(rel) => {
                        let full = PathBuf::from(&root).join(&rel).to_string_lossy().to_string();
                        super::docs_panel::DocsState::global(cx).update(cx, |d, cx| d.open_file(full, cx));
                        let title = cx.global::<I18n>().tp("img.saved", &[("path", &rel)]);
                        crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Success, timeout_ms: Some(5000), ..crate::ui::ToastInput::new(title) }, cx);
                        cx.emit(ImageEvent::Close);
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ImageDialog {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let model = ModelPrefsStore::global(cx).and_then(|p| p.read(cx).prefs.default_image.clone());
        let root = target_root(cx);
        let blocker = if model.is_none() { Some(i.t("img.noModel")) } else if root.is_none() { Some(i.t("img.noProject")) } else { None };
        let empty = self.prompt.read(cx).text().trim().is_empty();
        let busy = self.busy;
        let items: Vec<(gpui::SharedString, gpui::SharedString)> = SIZES.iter().map(|(v, k)| ((*v).into(), i.t(k).into())).collect();
        let me = cx.entity().downgrade();
        let mut footer = div().flex().justify_end().gap(t.space(2));
        if model.is_none() {
            footer = footer.child(button("img-settings").label(i.t("img.openSettings")).on_click(cx.listener(|_, _, _, cx| {
                super::state::AppState::global(cx).update(cx, |a, cx| a.push("/settings/models", cx));
                cx.emit(ImageEvent::Close);
            })));
        }
        footer = footer
            .child(button("img-cancel").label(i.t("ui.cancel")).disabled(busy).on_click(cx.listener(|_, _, _, cx| cx.emit(ImageEvent::Close))))
            .child(button("img-go").variant(ButtonVariant::Solid).disabled(busy || blocker.is_some() || empty).label(i.t(if busy { "img.busy" } else { "img.generate" })).on_click(cx.listener(|s, _, _, cx| s.submit(cx))));
        crate::ui::dialog(i.t("img.title"))
            .description(i.t("img.body"))
            .on_dismiss(cx.listener(|s, _, _, cx| {
                if !s.busy {
                    cx.emit(ImageEvent::Close);
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .children(model.map(|m| div().small(&t).text_color(t.colors.fg_muted).child(i.tp("img.model", &[("model", &format!("{}/{}", m.provider_id, m.model_id))]))))
                    .child(div().small(&t).font_weight(gpui::FontWeight::MEDIUM).child(i.t("img.prompt")))
                    .child(div().min_h(t.rem(5.)).child(self.prompt.clone()))
                    .child(select("img-size", items, self.size.to_string()).disabled(busy).on_change(move |v, _, cx| {
                        let v = SIZES.iter().find(|(s, _)| *s == v.as_ref()).map(|(s, _)| *s).unwrap_or("1024x1024");
                        me.update(cx, |s, cx| {
                            s.size = v;
                            cx.notify();
                        })
                        .ok();
                    }))
                    .children(blocker.map(|b| div().small(&t).text_color(t.colors.fg_muted).child(b)))
                    .children(self.error.clone().map(|e| div().small(&t).text_color(t.colors.danger).child(e))),
            )
            .footer(footer)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn nama_berkas() {
        use chrono::TimeZone;
        let now = chrono::Local.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        assert_eq!(super::image_file_name("Kucing Café!!", now), "images/kucing-cafe-20260102-030405.png");
        assert_eq!(super::image_file_name("???", now), "images/gambar-20260102-030405.png");
    }
}
