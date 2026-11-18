//! Elemen kecil bersama: ModelLogo (packages/ui atoms/ModelLogo.tsx) dan FileCard (shell/FileCard.tsx).
use crate::core::model_logo::{logo_image, model_logo_key};
use crate::core::model_prefs::ModelPrefsStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{avatar, button, icon, icon_button, ButtonVariant, ControlSize, IconSize, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, SharedString};

/// ModelLogo: logo kustom pengguna > logo terdeteksi (berwarna/monokrom) > tile inisial. `working` = berkedip.
pub fn model_logo(t: &Theme, cx: &App, provider: Option<&str>, model: Option<&str>, name: &str, working: bool, id: &str) -> AnyElement {
    let size = t.rem(rem::ICON_MD);
    let custom = ModelPrefsStore::global(cx).and_then(|p| p.read(cx).override_for(provider, model)).and_then(|u| crate::core::gradient::image_of(&u));
    let img = custom.or_else(|| model_logo_key(provider, model, Some(name)).and_then(|k| logo_image(&k, t.colors.fg_subtle)));
    let el = match img {
        Some(i) => div().flex_none().size(size).child(gpui::img(i).size_full()).into_any_element(),
        None => avatar(if name.is_empty() { provider.or(model).unwrap_or("?").to_string() } else { name.to_string() }).size(ControlSize::Sm).into_any_element(),
    };
    if !working {
        return el;
    }
    use gpui::{Animation, AnimationExt};
    div()
        .flex_none()
        .child(el)
        .with_animation(SharedString::from(id.to_string()), Animation::new(std::time::Duration::from_millis(1200)).repeat(), |d, k| {
            d.opacity(1. - 0.75 * (1. - ((k * 2.) - 1.).abs()))
        })
        .into_any_element()
}

/// SAFE_IMAGE_URI: gambar hanya dari data: URI bertipe gambar.
pub fn safe_image_uri(url: &str) -> bool {
    regex::Regex::new(r"(?i)^data:image/(png|jpe?g|gif|webp|svg\+xml)[;,]").expect("regex").is_match(url)
}

/// fileUrlToPath: file:///C:/x → C:/x.
pub fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("");
    let p = percent_decode(path);
    let win = regex::Regex::new(r"^/[A-Za-z]:").expect("regex");
    Some(if win.is_match(&p) { p[1..].to_string() } else { p })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn warn(cx: &mut App, key: &str) {
    let title = cx.global::<I18n>().t(key);
    Toasts::show(ToastInput { title: title.into(), description: None, tone: ToastTone::Warning, timeout_ms: None, action: None }, cx);
}

/// FileCard: pratinjau gambar (data URI) atau kartu dokumen [ikon] nama [Buka] [Unduh].
pub fn file_card(t: &Theme, i: &I18n, id: &str, filename: Option<&str>, mime: &str, url: &str, path: Option<&str>) -> AnyElement {
    let target = path.map(str::to_string).or_else(|| file_url_to_path(url));
    let name = filename
        .map(str::to_string)
        .or_else(|| target.clone().or_else(|| (!url.is_empty()).then(|| url.to_string())).and_then(|x| x.rsplit(['\\', '/']).next().map(str::to_string)))
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| i.t("file.unnamed"));
    if mime.starts_with("image/") && safe_image_uri(url) {
        if let Some(img) = crate::core::gradient::image_of(url) {
            return div()
                .max_w(t.rem(28.))
                .my(t.space(1))
                .p(t.space(2))
                .border_1()
                .border_color(t.colors.border)
                .rounded(t.rem(rem::RADIUS_LG))
                .child(gpui::img(img).max_w_full().rounded(t.rem(rem::RADIUS_MD)))
                .child(div().mt(t.space(1)).small(t).text_color(t.colors.fg_muted).child(format!("{name} · {mime}")))
                .into_any_element();
        }
    }
    let (t1, t2) = (target.clone(), target.clone());
    let n2 = name.clone();
    div()
        .flex()
        .items_center()
        .gap(t.space(3))
        .my(t.space(1))
        .px(t.space(4))
        .py(t.space(3))
        .border_1()
        .border_color(t.colors.border)
        .rounded(t.rem(rem::RADIUS_LG))
        .child(crate::shell::file_icons::file_type_icon(t, &name, false, false, rem::ICON_LG))
        .child(div().flex_1().min_w_0().truncate().font_weight(gpui::FontWeight(t.ty(crate::theme::LABEL).weight)).child(name.clone()))
        .child(
            button(SharedString::from(format!("fc-open-{id}")))
                .size(ControlSize::Sm)
                .child(icon("external_link").size(IconSize::Sm))
                .label(i.t("doc.openShort"))
                .tooltip_text(i.tp("doc.open", &[("name", &name)]))
                .on_click(move |_, _, cx| match &t1 {
                    Some(p) if std::path::Path::new(p).is_file() => crate::shell::docs_panel::DocsState::global(cx).update(cx, |d, cx| d.open_file(p.clone(), cx)),
                    Some(_) => warn(cx, "doc.outside"),
                    None => warn(cx, "file.demoNote"),
                }),
        )
        .child(
            icon_button(SharedString::from(format!("fc-dl-{id}")), "download", i.tp("file.download", &[("name", &name)]))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, _, cx| {
                    let Some(src) = t2.clone().filter(|p| std::path::Path::new(p).is_file()) else {
                        return warn(cx, if t2.is_some() { "doc.outside" } else { "file.demoNote" });
                    };
                    let dir = std::path::PathBuf::from(&src).parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    let rx = cx.prompt_for_new_path(&dir, Some(&n2));
                    cx.spawn(async move |cx| {
                        let Ok(Ok(Some(dest))) = rx.await else { return };
                        let r = std::fs::copy(&src, &dest);
                        cx.update(|cx| {
                            let i = cx.global::<I18n>();
                            let input = match r {
                                Ok(_) => ToastInput { title: i.t("doc.saved").into(), description: None, tone: ToastTone::Success, timeout_ms: Some(2000), action: None },
                                Err(e) => ToastInput { title: i.tp("sb.error", &[("error", &e.to_string())]).into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None },
                            };
                            Toasts::show(input, cx);
                        });
                    })
                    .detach();
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        assert_eq!(file_url_to_path("file:///C:/a%20b/x.pdf").as_deref(), Some("C:/a b/x.pdf"));
        assert_eq!(file_url_to_path("file:///home/u/x.md").as_deref(), Some("/home/u/x.md"));
        assert_eq!(file_url_to_path("http://x"), None);
        assert!(safe_image_uri("data:image/png;base64,AA"));
        assert!(!safe_image_uri("https://x/a.png"));
    }
}

/// Pesan keadaan panel alat (port tools/ToolNotice.tsx): ikon besar + teks di tengah panel.
pub fn tool_notice(t: &Theme, ic: &'static str, text: impl Into<SharedString>) -> AnyElement {
    div().flex().flex_col().items_center().gap(t.space(2)).p(t.space(6)).text_color(t.colors.fg_muted).child(icon(ic).size(IconSize::Lg)).child(div().small(t).text_center().child(text.into())).into_any_element()
}

/// Varian `inline` ToolNotice: satu baris ringkas di atas daftar.
pub fn tool_notice_inline(t: &Theme, text: impl Into<SharedString>) -> AnyElement {
    div().px(t.space(3)).py(t.space(2)).small(t).text_color(t.colors.fg_muted).child(text.into()).into_any_element()
}
