//! Aksi blok render kaya — port rich/RichMarkdown.tsx (CodeBlock, RichTable), RichActions.tsx, rich-export.ts:
//! blok kode dengan bahasa, bungkus baris, salin, salin berpagar, simpan berkas, lipat untuk kode panjang; tabel dengan
//! salin TSV/Markdown/CSV/HTML dan simpan CSV/Markdown. Aksi lain di menu ⋯ (digambar Root di posisi mouse).
//! Rumus blok: salin LaTeX, salin dengan $$, simpan .tex. Perbedaan jujur: ekspor sebagai gambar (PNG/SVG) dan salin
//! MathML belum diport.
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
    save_bytes(name, text.into_bytes(), cx)
}

/// Simpan byte lewat dialog Simpan sebagai (port saveBytes).
pub(crate) fn save_bytes(name: String, text: Vec<u8>, cx: &mut App) {
    let rx = cx.prompt_for_new_path(&home(), Some(&name));
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(path))) = rx.await else { return };
        let r = std::fs::write(&path, &text);
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

// ---------- ekspor gambar (port imageActions + html-to-image) ----------

/// Warna tema sebagai CSS.
pub fn css(c: gpui::Hsla) -> String {
    let r = gpui::Rgba::from(c);
    let b = |x: f32| (x.clamp(0., 1.) * 255.).round() as u8;
    format!("rgba({},{},{},{:.3})", b(r.r), b(r.g), b(r.b), r.a)
}

pub fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn frame_style(t: &Theme) -> String {
    format!(
        "background:{};color:{};font-family:'Inter','Segoe UI',system-ui,sans-serif;font-size:14px;line-height:1.5;padding:16px;border-radius:8px;border:1px solid {}",
        css(t.colors.bg),
        css(t.colors.fg),
        css(t.colors.border)
    )
}

/// HTML blok kode berwarna (gaya inline) untuk dijadikan gambar.
pub fn code_html(t: &Theme, lang: &str, raw: &str) -> String {
    let spans = super::highlight::highlight_text(lang, raw, t);
    let mut body = String::new();
    let mut at = 0;
    for (range, st) in spans {
        if range.start < at || range.end > raw.len() || !raw.is_char_boundary(range.start) || !raw.is_char_boundary(range.end) {
            continue;
        }
        body.push_str(&esc_html(&raw[at..range.start]));
        let mut style = String::new();
        if let Some(c) = st.color {
            style.push_str(&format!("color:{};", css(c)));
        }
        if st.font_style == Some(gpui::FontStyle::Italic) {
            style.push_str("font-style:italic;");
        }
        if st.font_weight.is_some_and(|w| w.0 >= 600.) {
            style.push_str("font-weight:600;");
        }
        body.push_str(&format!("<span style=\"{style}\">{}</span>", esc_html(&raw[range.clone()])));
        at = range.end;
    }
    body.push_str(&esc_html(&raw[at..]));
    let head = if lang.is_empty() { String::new() } else { format!("<div style=\"font-size:12px;color:{};margin-bottom:8px\">{}</div>", css(t.colors.fg_muted), esc_html(lang)) };
    format!(
        "<div style=\"{}\">{head}<pre style=\"margin:0;padding:12px;border-radius:6px;background:{};font-family:'Fira Code',Consolas,monospace;font-size:13px;white-space:pre\">{body}</pre></div>",
        frame_style(t),
        css(t.colors.raised)
    )
}

/// HTML tabel (gaya inline) untuk dijadikan gambar.
pub fn table_html(t: &Theme, rows: &[Vec<String>]) -> String {
    let cell = |tag: &str, v: &str| format!("<{tag} style=\"border:1px solid {};padding:6px 10px;text-align:left\">{}</{tag}>", css(t.colors.border), esc_html(v));
    let mut out = format!("<div style=\"{}\"><table style=\"border-collapse:collapse\">", frame_style(t));
    for (n, row) in rows.iter().enumerate() {
        let tag = if n == 0 { "th" } else { "td" };
        out.push_str(&format!("<tr{}>", if n == 0 { format!(" style=\"background:{}\"", css(t.colors.raised)) } else { String::new() }));
        for c in row {
            out.push_str(&cell(tag, c));
        }
        out.push_str("</tr>");
    }
    out.push_str("</table></div>");
    out
}

/// HTML rumus (MathML, dirender Chromium) untuk dijadikan gambar.
pub fn math_html(t: &Theme, latex: &str) -> String {
    format!("<div style=\"{};font-size:20px;font-family:'Cambria Math','Latin Modern Math',serif\">{}</div>", frame_style(t), super::math::to_mathml(latex, true))
}

/// HTML pesan (Markdown → HTML bergaya inline) untuk "Simpan sebagai PNG/SVG" pesan chat.
pub fn message_html(t: &Theme, src: &str) -> String {
    use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_MATH;
    let mut out = format!("<div style=\"{};max-width:760px\">", frame_style(t));
    let border = css(t.colors.border);
    let raised = css(t.colors.raised);
    let mut code: Option<(String, String)> = None;
    let mut in_head = false;
    for ev in Parser::new_ext(src, opts) {
        if let Some((lang, buf)) = code.as_mut() {
            match ev {
                Event::Text(x) => buf.push_str(&x),
                Event::End(TagEnd::CodeBlock) => {
                    let body = buf.trim_end_matches(['\r', '\n']).to_string();
                    let lang = lang.clone();
                    code = None;
                    let inner = code_html(t, &lang, &body);
                    // buang bingkai luar code_html: cukup ambil <pre>
                    let pre = inner.find("<pre").map(|k| inner[k..inner.len() - "</div>".len()].to_string()).unwrap_or(inner);
                    out.push_str(&format!("<div style=\"margin:8px 0\">{pre}</div>"));
                }
                _ => {}
            }
            continue;
        }
        match ev {
            Event::Start(tag) => out.push_str(&match tag {
                Tag::Paragraph => "<p style=\"margin:0 0 8px\">".into(),
                Tag::Heading { level, .. } => {
                    let size = match level {
                        HeadingLevel::H1 => 22,
                        HeadingLevel::H2 => 19,
                        HeadingLevel::H3 => 17,
                        _ => 15,
                    };
                    format!("<div style=\"font-size:{size}px;font-weight:700;margin:12px 0 6px\">")
                }
                Tag::BlockQuote(_) => format!("<blockquote style=\"margin:8px 0;padding-left:12px;border-left:3px solid {border};color:{}\">", css(t.colors.fg_muted)),
                Tag::CodeBlock(kind) => {
                    code = Some((if let CodeBlockKind::Fenced(l) = kind { l.split_whitespace().next().unwrap_or("").to_string() } else { String::new() }, String::new()));
                    String::new()
                }
                Tag::List(Some(n)) => format!("<ol start=\"{n}\" style=\"margin:0 0 8px;padding-left:24px\">"),
                Tag::List(None) => "<ul style=\"margin:0 0 8px;padding-left:24px\">".into(),
                Tag::Item => "<li>".into(),
                Tag::Emphasis => "<em>".into(),
                Tag::Strong => "<strong>".into(),
                Tag::Strikethrough => "<s>".into(),
                Tag::Link { dest_url, .. } => format!("<a href=\"{}\" style=\"color:{};text-decoration:underline\">", esc_html(&dest_url), css(t.colors.accent)),
                Tag::Image { dest_url, .. } => format!("<span style=\"color:{}\">[{}] ", css(t.colors.fg_muted), esc_html(&dest_url)),
                Tag::Table(_) => "<table style=\"border-collapse:collapse;margin:8px 0\">".into(),
                Tag::TableHead => {
                    in_head = true;
                    format!("<tr style=\"background:{raised}\">")
                }
                Tag::TableRow => "<tr>".into(),
                Tag::TableCell => format!("<{} style=\"border:1px solid {border};padding:4px 8px;text-align:left\">", if in_head { "th" } else { "td" }),
                _ => String::new(),
            }),
            Event::End(tag) => out.push_str(match tag {
                TagEnd::Paragraph => "</p>",
                TagEnd::Heading(_) => "</div>",
                TagEnd::BlockQuote(_) => "</blockquote>",
                TagEnd::List(true) => "</ol>",
                TagEnd::List(false) => "</ul>",
                TagEnd::Item => "</li>",
                TagEnd::Emphasis => "</em>",
                TagEnd::Strong => "</strong>",
                TagEnd::Strikethrough => "</s>",
                TagEnd::Link => "</a>",
                TagEnd::Image => "</span>",
                TagEnd::Table => "</table>",
                TagEnd::TableHead => {
                    in_head = false;
                    "</tr>"
                }
                TagEnd::TableRow => "</tr>",
                TagEnd::TableCell => {
                    if in_head { "</th>" } else { "</td>" }
                }
                _ => "",
            }),
            Event::Text(x) => out.push_str(&esc_html(&x)),
            Event::Code(x) => out.push_str(&format!("<code style=\"font-family:'Fira Code',Consolas,monospace;font-size:12.5px;background:{raised};padding:1px 4px;border-radius:4px\">{}</code>", esc_html(&x))),
            Event::InlineMath(x) => out.push_str(&super::math::to_mathml(&x, false)),
            Event::DisplayMath(x) => out.push_str(&format!("<div style=\"margin:8px 0\">{}</div>", super::math::to_mathml(&x, true))),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str("<br/>"),
            Event::Rule => out.push_str(&format!("<hr style=\"border:none;border-top:1px solid {border};margin:12px 0\"/>")),
            Event::TaskListMarker(done) => out.push_str(if done { "☑ " } else { "☐ " }),
            Event::Html(x) | Event::InlineHtml(x) => out.push_str(&esc_html(&x)),
            _ => {}
        }
    }
    out.push_str("</div>");
    out
}

/// Tiga aksi gambar bersama: salin PNG, simpan PNG, simpan SVG.
/// HTML dibuat malas (saat diklik) dengan tema aktif — tidak dihitung ulang setiap render.
pub(crate) type HtmlFn = std::rc::Rc<dyn Fn(&Theme) -> String>;

pub(crate) fn snap_with(html: &HtmlFn, action: super::mermaid::SnapAction, cx: &mut App) {
    let t = cx.global::<Theme>().clone();
    super::mermaid::snapshot(html(&t), action);
    cx.refresh_windows();
}

pub(crate) fn image_entries(i: &I18n, html: HtmlFn, base: &'static str) -> Vec<MenuEntry> {
    use super::mermaid::SnapAction;
    let (h1, h2, h3) = (html.clone(), html.clone(), html);
    vec![
        entry(i.t("rich.copyImage"), move |_, cx| snap_with(&h1, SnapAction::CopyPng, cx)).icon("image_down"),
        entry(i.t("rich.saveImage"), move |_, cx| snap_with(&h2, SnapAction::SavePng(stamped_name(base, "png")), cx)).icon("image"),
        entry(i.t("rich.saveSvg"), move |_, cx| snap_with(&h3, SnapAction::SaveSvg(stamped_name(base, "svg")), cx)).icon("code"),
    ]
}

fn open_more(entries: Vec<MenuEntry>, window: &gpui::Window, cx: &mut App) {
    cx.set_global(RichMenu(Some((entries, window.mouse_position()))));
    cx.refresh_windows();
}

fn more_button(id: String, i: &I18n, entries: Vec<MenuEntry>) -> impl IntoElement {
    more_button_on(id, i, entries, None)
}

fn more_button_on(id: String, i: &I18n, entries: Vec<MenuEntry>, surface_fg: Option<gpui::Hsla>) -> impl IntoElement {
    icon_button(ElementId::Name(id.into()), "more", i.t("rich.more")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_surface(surface_fg).on_click(move |_, w, cx| open_more(entries.clone(), w, cx))
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
    let mut more = image_entries(i, { let (l, r) = (lang.to_string(), raw.to_string()); std::rc::Rc::new(move |t: &Theme| code_html(t, &l, &r)) }, "kode");
    more.extend([
        entry(i.t("rich.copyFenced"), move |_, cx| copy_text(fenced(&l2, &r2), cx)).icon("file_text"),
        entry(i.tp("rich.saveFile", &[("ext", &if ext == "Dockerfile" { ext.to_string() } else { format!(".{ext}") })]), move |_, cx| save_text(stamped_name("kode", ext), r3.clone(), cx)).icon("download"),
    ]);
    // Latar blok kode dari tema editor bisa gelap di tema aplikasi terang (atau sebaliknya): semua warna kepala
    // diturunkan dari teks permukaan itu, bukan token tema aplikasi.
    let surf = t.colors.code_bg.and(t.colors.code_fg);
    let (head_fg, head_line) = match surf {
        Some(f) => (f.opacity(0.7), f.opacity(0.14)),
        None => (t.colors.fg_subtle, t.colors.border_soft),
    };
    let head = div()
        .flex()
        .items_center()
        .gap(t.space(1))
        .pl(t.space(3))
        .pr(t.space(1))
        .py(gpui::px(2.))
        .border_b_1()
        .border_color(head_line)
        .small(t)
        .text_color(head_fg)
        .child(div().flex_1().child(if lang.is_empty() { i.t("md.code").to_string() } else { lang.to_string() }))
        .child(
            icon_button(ElementId::Name(format!("cb-wrap-{key:x}").into()), "wrap", i.t("rich.wrap")).size(ControlSize::Sm).pressed(wrap).on_surface(surf).on_click(move |_, _, cx| {
                set_state(key, |s| s.0 = !s.0);
                cx.refresh_windows();
            }),
        )
        .child(icon_button(ElementId::Name(format!("cb-copy-{key:x}").into()), "copy", i.t("md.copyCode")).size(ControlSize::Sm).on_surface(surf).on_click(move |_, _, cx| copy_text(r1.clone(), cx)))
        .child(more_button_on(format!("cb-more-{key:x}"), i, more, surf));
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
        .border_color(surf.map_or(t.colors.border_soft, |f| f.opacity(0.14)))
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
                    .border_color(head_line)
                    .small(t)
                    .text_color(surf.unwrap_or(t.colors.accent))
                    .cursor_pointer()
                    .hover(|s| s.bg(surf.map_or(t.colors.hover, |f| f.opacity(0.1))))
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
    let mut more = vec![
        entry(i.t("rich.copyMarkdown"), mk(to_markdown_table)).icon("file_text"),
        entry(i.t("rich.copyTsv"), mk(to_tsv)).icon("table"),
        entry(i.t("rich.copyCsv"), mk(to_csv)).icon("copy"),
        entry(i.t("rich.copyHtml"), mk(to_html_table)).icon("code"),
        entry(i.t("rich.saveCsv"), save("tabel", "csv", to_csv)).icon("download"),
        entry(i.t("rich.saveMarkdown"), save("tabel", "md", to_markdown_table)).icon("download"),
    ];
    more.splice(4..4, image_entries(i, { let r = r.clone(); std::rc::Rc::new(move |t: &Theme| table_html(t, &r)) }, "tabel"));
    let r0 = r.clone();
    div()
        .flex()
        .justify_end()
        .gap(t.space(1))
        .child(icon_button(ElementId::Name(format!("tb-copy-{key:x}").into()), "table", i.t("rich.copyTable")).size(ControlSize::Sm).on_click(move |_, _, cx| copy_text(to_tsv(&r0), cx)))
        .child(more_button(format!("tb-more-{key:x}"), i, more))
        .into_any_element()
}

/// Bilah aksi rumus blok (port MathBlock): salin LaTeX + menu ⋯ (salin dengan $$, simpan .tex).
pub fn math_bar(t: &Theme, i: &I18n, latex: &str) -> AnyElement {
    let key = block_key(&["math", latex]);
    let (l1, l2, l3) = (latex.to_string(), latex.to_string(), latex.to_string());
    let l4 = latex.to_string();
    let mut more = vec![
        entry(i.t("rich.copyLatexDisplay"), move |_, cx| copy_text(format!("$$
{l2}
$$"), cx)).icon("copy"),
        entry(i.t("rich.copyMathml"), move |_, cx| copy_text(super::math::to_mathml(&l4, true), cx)).icon("code"),
    ];
    more.extend(image_entries(i, { let l = latex.to_string(); std::rc::Rc::new(move |t: &Theme| math_html(t, &l)) }, "rumus"));
    more.push(entry(i.tp("rich.saveFile", &[("ext", ".tex")]), move |_, cx| save_text(stamped_name("rumus", "tex"), l3.clone(), cx)).icon("download"));
    div()
        .flex()
        .justify_end()
        .gap(t.space(1))
        .child(icon_button(ElementId::Name(format!("mb-copy-{key:x}").into()), "sigma", i.t("rich.copyLatex")).size(ControlSize::Sm).on_click(move |_, _, cx| copy_text(l1.clone(), cx)))
        .child(more_button(format!("mb-more-{key:x}"), i, more))
        .into_any_element()
}

/// Bilah aksi diagram Mermaid (port Mermaid.tsx): salin sumber, simpan PNG + menu ⋯.
pub fn mermaid_bar(t: &Theme, i: &I18n, code: &str, svg: &str, png: std::sync::Arc<gpui::Image>) -> AnyElement {
    let key = block_key(&["mmd", code]);
    let (c1, c2, c3) = (code.to_string(), code.to_string(), code.to_string());
    let (s1, s2) = (svg.to_string(), svg.to_string());
    let (p1, p2) = (png.clone(), png);
    let more = vec![
        entry(i.t("rich.copyImage"), move |_, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_image(&p2));
            let title = cx.global::<I18n>().t("rich.imageCopied");
            super::Toasts::show(super::ToastInput { tone: super::ToastTone::Success, timeout_ms: Some(1500), ..super::ToastInput::new(title) }, cx);
        })
        .icon("image_down"),
        entry(i.t("rich.copySvg"), move |_, cx| copy_text(s1.clone(), cx)).icon("code"),
        entry(i.t("rich.saveSvg"), move |_, cx| save_text(stamped_name("diagram", "svg"), s2.clone(), cx)).icon("code"),
        entry(i.tp("rich.saveFile", &[("ext", ".mmd")]), move |_, cx| save_text(stamped_name("diagram", "mmd"), c2.clone(), cx)).icon("download"),
        entry(i.t("rich.copyFenced"), move |_, cx| copy_text(fenced("mermaid", &c3), cx)).icon("file_text"),
    ];
    div()
        .flex()
        .justify_end()
        .gap(t.space(1))
        .child(icon_button(ElementId::Name(format!("mm-src-{key:x}").into()), "copy", i.t("rich.copySource")).size(ControlSize::Sm).on_click(move |_, _, cx| copy_text(c1.clone(), cx)))
        .child(icon_button(ElementId::Name(format!("mm-png-{key:x}").into()), "image", i.t("rich.saveImage")).size(ControlSize::Sm).on_click(move |_, _, cx| save_bytes(stamped_name("diagram", "png"), p1.bytes().to_vec(), cx)))
        .child(more_button(format!("mm-more-{key:x}"), i, more))
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
