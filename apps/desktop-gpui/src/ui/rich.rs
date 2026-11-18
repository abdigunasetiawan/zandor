//! Aksi blok render kaya — port rich/RichMarkdown.tsx (CodeBlock, RichTable), RichActions.tsx, rich-export.ts:
//! blok kode dengan bahasa, bungkus baris, salin, salin berpagar, simpan berkas, lipat untuk kode panjang; tabel dengan
//! salin TSV/Markdown/CSV/HTML dan simpan CSV/Markdown. Aksi lain di menu ⋯ (digambar Root di posisi mouse).
//! Perbedaan jujur: ekspor sebagai gambar (PNG/SVG) belum diport.
use super::menu::{entry, MenuEntry};
use super::{button::icon_button, ButtonVariant, ControlSize, TypeExt};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use gpui::{div, prelude::*, AnyElement, App, ElementId, IntoElement, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;

pub const COLLAPSE_LINES: usize = 30;

/// Menu ⋯ blok yang terbuka: entri + posisi mouse.
#[derive(Clone, Default)]
pub struct RichMenu(pub Option<(Vec<MenuEntry>, gpui::Point<gpui::Pixels>)>);
impl gpui::Global for RichMenu {}

thread_local! {
    /// Status UI per blok (kunci = hash isi): (bungkus baris, dibuka penuh).
    static STATE: RefCell<HashMap<u64, (bool, bool)>> = RefCell::new(HashMap::new());
}

fn state(k: u64) -> (bool, bool) {
    STATE.with(|s| s.borrow().get(&k).copied().unwrap_or((false, false)))
}
fn set_state(k: u64, f: impl FnOnce(&mut (bool, bool))) {
    STATE.with(|s| f(s.borrow_mut().entry(k).or_insert((false, false))));
}

pub fn block_key(parts: &[&str]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

pub fn to_tsv(rows: &[Vec<String>]) -> String {
    rows.iter().map(|r| r.iter().map(|c| c.replace('\t', " ")).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n")
}

pub fn to_csv(rows: &[Vec<String>]) -> String {
    let cell = |c: &String| if c.contains(['"', ',', '\n', '\r']) { format!("\"{}\"", c.replace('"', "\"\"")) } else { c.clone() };
    rows.iter().map(|r| r.iter().map(cell).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join("\r\n")
}

pub fn to_markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let line = |r: &Vec<String>| format!("| {} |", (0..width).map(|i| r.get(i).cloned().unwrap_or_default().replace('|', "\\|")).collect::<Vec<_>>().join(" | "));
    let mut out = vec![line(&rows[0]), format!("| {} |", vec!["---"; width].join(" | "))];
    out.extend(rows[1..].iter().map(line));
    out.join("\n")
}

pub fn to_html_table(rows: &[Vec<String>]) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut out = String::from("<table>");
    for (n, r) in rows.iter().enumerate() {
        let tag = if n == 0 { "th" } else { "td" };
        out.push_str("<tr>");
        for c in r {
            out.push_str(&format!("<{tag}>{}</{tag}>", esc(c)));
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

/// Ekstensi berkas untuk bahasa blok kode; tak dikenal → txt (port extForLang).
pub fn ext_for_lang(lang: &str) -> &'static str {
    match lang.to_lowercase().as_str() {
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "python" | "py" => "py",
        "rust" | "rs" => "rs",
        "go" => "go",
        "java" => "java",
        "kotlin" => "kt",
        "c" => "c",
        "cpp" | "c++" => "cpp",
        "csharp" | "cs" => "cs",
        "php" => "php",
        "ruby" | "rb" => "rb",
        "swift" => "swift",
        "bash" | "sh" | "shell" | "zsh" => "sh",
        "powershell" | "ps1" => "ps1",
        "html" => "html",
        "xml" => "xml",
        "css" => "css",
        "scss" => "scss",
        "json" => "json",
        "yaml" => "yaml",
        "yml" => "yml",
        "toml" => "toml",
        "sql" => "sql",
        "markdown" | "md" => "md",
        "dockerfile" => "Dockerfile",
        "latex" | "tex" => "tex",
        "mermaid" => "mmd",
        _ => "txt",
    }
}

pub fn fenced(lang: &str, body: &str) -> String {
    format!("```{lang}\n{body}\n```")
}

/// `dasar-YYYYMMDD-HHMMSS.ext` (Dockerfile tanpa ekstensi).
pub fn stamped_name(base: &str, ext: &str) -> String {
    if ext == "Dockerfile" {
        return "Dockerfile".into();
    }
    let now = chrono::Local::now();
    format!("{base}-{}.{ext}", now.format("%Y%m%d-%H%M%S"))
}

fn copy_text(text: String, cx: &mut App) {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    let title = cx.global::<I18n>().t("chat.copied");
    super::Toasts::show(super::ToastInput { tone: super::ToastTone::Success, timeout_ms: Some(1500), ..super::ToastInput::new(title) }, cx);
}

fn home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default()
}

/// Simpan teks lewat dialog Simpan sebagai (port saveText).
fn save_text(name: String, text: String, cx: &mut App) {
    let rx = cx.prompt_for_new_path(&home(), Some(&name));
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(path))) = rx.await else { return };
        let r = std::fs::write(&path, text.as_bytes());
        cx.update(|cx| {
            let i = cx.global::<I18n>().clone();
            let input = match r {
                Ok(()) => super::ToastInput { tone: super::ToastTone::Success, timeout_ms: Some(2000), ..super::ToastInput::new(i.t("msg.saved")) },
                Err(e) => super::ToastInput { tone: super::ToastTone::Danger, ..super::ToastInput::new(i.tp("sb.error", &[("error", &e.to_string())])) },
            };
            super::Toasts::show(input, cx);
        });
    })
    .detach();
}

fn open_more(entries: Vec<MenuEntry>, window: &gpui::Window, cx: &mut App) {
    cx.set_global(RichMenu(Some((entries, window.mouse_position()))));
    cx.refresh_windows();
}

fn more_button(id: String, i: &I18n, entries: Vec<MenuEntry>) -> impl IntoElement {
    icon_button(ElementId::Name(id.into()), "more", i.t("rich.more")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(move |_, w, cx| open_more(entries.clone(), w, cx))
}

/// Blok kode (port CodeBlock).
pub fn code_block(t: &Theme, i: &I18n, lang: &str, raw: &str) -> AnyElement {
    let key = block_key(&["code", lang, raw]);
    let (wrap, open) = state(key);
    let lines: Vec<&str> = raw.split('\n').collect();
    let long = lines.len() > COLLAPSE_LINES;
    let shown = if long && !open { lines[..COLLAPSE_LINES].join("\n") } else { raw.to_string() };
    let ext = ext_for_lang(lang);
    let (r1, r2, r3) = (raw.to_string(), raw.to_string(), raw.to_string());
    let l2 = lang.to_string();
    let more = vec![
        entry(i.t("rich.copyFenced"), move |_, cx| copy_text(fenced(&l2, &r2), cx)).icon("file_text"),
        entry(i.tp("rich.saveFile", &[("ext", &if ext == "Dockerfile" { ext.to_string() } else { format!(".{ext}") })]), move |_, cx| save_text(stamped_name("kode", ext), r3.clone(), cx)).icon("download"),
    ];
    let head = div()
        .flex()
        .items_center()
        .gap(t.space(1))
        .pl(t.space(3))
        .pr(t.space(1))
        .py(gpui::px(2.))
        .border_b_1()
        .border_color(t.colors.border_soft)
        .small(t)
        .text_color(t.colors.fg_subtle)
        .child(div().flex_1().child(if lang.is_empty() { i.t("md.code").to_string() } else { lang.to_string() }))
        .child(
            icon_button(ElementId::Name(format!("cb-wrap-{key:x}").into()), "wrap", i.t("rich.wrap")).size(ControlSize::Sm).pressed(wrap).on_click(move |_, _, cx| {
                set_state(key, |s| s.0 = !s.0);
                cx.refresh_windows();
            }),
        )
        .child(icon_button(ElementId::Name(format!("cb-copy-{key:x}").into()), "copy", i.t("md.copyCode")).size(ControlSize::Sm).on_click(move |_, _, cx| copy_text(r1.clone(), cx)))
        .child(more_button(format!("cb-more-{key:x}"), i, more));
    let body = div().px(t.space(3)).py(t.space(2)).font_family(t.font_code.clone()).code(t);
    let hl = super::highlight::highlight_text(lang, &shown, t);
    let styled = gpui::StyledText::new(shown).with_highlights(hl);
    let body: AnyElement = if wrap {
        body.child(styled).into_any_element()
    } else {
        div().id(ElementId::Name(format!("cb-x-{key:x}").into())).overflow_x_scroll().child(body.whitespace_nowrap().child(styled)).into_any_element()
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(t.rem(rem::RADIUS_MD))
        .border_1()
        .border_color(t.colors.border_soft)
        .bg(t.colors.code_bg.unwrap_or(t.colors.raised))
        .when_some(t.colors.code_fg, |d, c| d.text_color(c))
        .overflow_hidden()
        .child(head)
        .child(body)
        .when(long, |d| {
            let label = if open { i.t("md.collapse").to_string() } else { i.tp("md.expand", &[("n", &lines.len().to_string())]) };
            d.child(
                div()
                    .id(ElementId::Name(format!("cb-more-lines-{key:x}").into()))
                    .px(t.space(3))
                    .py(t.space(1))
                    .border_t_1()
                    .border_color(t.colors.border_soft)
                    .small(t)
                    .text_color(t.colors.accent)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.hover))
                    .child(label)
                    .on_click(move |_, _, cx| {
                        set_state(key, |s| s.1 = !s.1);
                        cx.refresh_windows();
                    }),
            )
        })
        .into_any_element()
}

/// Bilah aksi tabel (port RichTable): salin tabel (TSV) + menu ⋯.
pub fn table_bar(t: &Theme, i: &I18n, rows: Vec<Vec<String>>) -> AnyElement {
    let key = block_key(&[&"table", &to_tsv(&rows).as_str()]);
    let r = std::rc::Rc::new(rows);
    let mk = |f: fn(&[Vec<String>]) -> String| {
        let r = r.clone();
        move |_: &mut gpui::Window, cx: &mut App| copy_text(f(&r), cx)
    };
    let save = |base: &'static str, ext: &'static str, f: fn(&[Vec<String>]) -> String| {
        let r = r.clone();
        move |_: &mut gpui::Window, cx: &mut App| save_text(stamped_name(base, ext), f(&r), cx)
    };
    let more = vec![
        entry(i.t("rich.copyMarkdown"), mk(to_markdown_table)).icon("file_text"),
        entry(i.t("rich.copyTsv"), mk(to_tsv)).icon("table"),
        entry(i.t("rich.copyCsv"), mk(to_csv)).icon("copy"),
        entry(i.t("rich.copyHtml"), mk(to_html_table)).icon("code"),
        entry(i.t("rich.saveCsv"), save("tabel", "csv", to_csv)).icon("download"),
        entry(i.t("rich.saveMarkdown"), save("tabel", "md", to_markdown_table)).icon("download"),
    ];
    let r0 = r.clone();
    div()
        .flex()
        .justify_end()
        .gap(t.space(1))
        .child(icon_button(ElementId::Name(format!("tb-copy-{key:x}").into()), "table", i.t("rich.copyTable")).size(ControlSize::Sm).on_click(move |_, _, cx| copy_text(to_tsv(&r0), cx)))
        .child(more_button(format!("tb-more-{key:x}"), i, more))
        .into_any_element()
}

/// Popup menu ⋯ (digambar Root).
pub fn rich_menu(t: &Theme, cx: &mut App) -> Option<AnyElement> {
    let (entries, pos) = cx.try_global::<RichMenu>()?.0.clone()?;
    Some(
        super::overlay_deferred(gpui::anchored().position(pos).snap_to_window().child(super::menu::menu_popup(SharedString::from("rich-menu"), t, entries, gpui::Anchor::TopLeft, |_, cx| {
            cx.set_global(RichMenu(None));
            cx.refresh_windows();
        })))
        .with_priority(3)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ekspor_tabel_dan_kode() {
        let rows = vec![vec!["a".to_string(), "b|c".to_string()], vec!["1,2".to_string(), "x\"y".to_string()]];
        assert_eq!(to_tsv(&rows), "a\tb|c\n1,2\tx\"y");
        assert_eq!(to_csv(&rows), "a,b|c\r\n\"1,2\",\"x\"\"y\"");
        assert_eq!(to_markdown_table(&rows), "| a | b\\|c |\n| --- | --- |\n| 1,2 | x\"y |");
        assert_eq!(ext_for_lang("Python"), "py");
        assert_eq!(ext_for_lang("zz"), "txt");
        assert_eq!(fenced("rs", "fn x(){}"), "```rs\nfn x(){}\n```");
        assert_eq!(stamped_name("k", "Dockerfile"), "Dockerfile");
        assert!(stamped_name("kode", "py").starts_with("kode-") && stamped_name("kode", "py").ends_with(".py"));
    }
}
