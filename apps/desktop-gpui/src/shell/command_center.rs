//! CommandCenter — port shell/CommandCenter.tsx: palet perintah (Ctrl+K) + lompat ke sesi, dengan navigasi keyboard.
use super::commands::{Group, COMMANDS};
use crate::core::projects::{session_title, ProjectStore, CHAT_KEY};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, kbd, popup, IconSize, InputChanged, InputSubmit, TextInput, TypeExt};
use gpui::{
    actions, div, prelude::*, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    HighlightStyle, IntoElement, KeyBinding, StyledText, UnderlineStyle, Window,
};

actions!(palette, [SelectPrev, SelectNext, Dismiss]);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Palette");
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, ctx),
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);
}

#[derive(Clone)]
pub enum PaletteAction {
    Command(&'static str),
    Session(String, String),
    /// Tautan pesan: (key, sessionId, messageId).
    MessageLink(String, String, String),
}

pub enum PaletteEvent {
    Run(PaletteAction),
    Close,
}
impl EventEmitter<PaletteEvent> for CommandCenter {}

struct Row {
    /// Kelompok (redup, di kanan baris).
    group: String,
    title: String,
    /// Kombinasi tombol (perintah) — tampil sebagai `kbd`. Untuk sesi: nama project (redup).
    hint: Option<String>,
    /// Perintah yang tidak berlaku saat ini (mis. Go back tanpa riwayat): redup dan tidak bisa dipilih.
    disabled: bool,
    /// Indeks karakter `title` yang cocok dengan kueri (disorot).
    matched: Vec<usize>,
    action: PaletteAction,
}

pub struct CommandCenter {
    input: Entity<TextInput>,
    selected: usize,
    focus: FocusHandle,
}

/// Pencocokan longgar: semua kata kueri ada di teks (tanpa beda huruf besar).
fn matches(q: &str, hay: &str) -> bool {
    let h = hay.to_lowercase();
    q.split_whitespace().all(|w| h.contains(&w.to_lowercase()))
}

fn is_boundary(t: &[char], at: usize) -> bool {
    at == 0 || matches!(t[at - 1], ' ' | '-' | '_' | '/' | '.' | ':')
}

/// Pencocokan fuzzy berurutan (port `fuzzyMatch` di packages/core/src/commands.ts): skor + indeks karakter cocok.
fn fuzzy_match(query: &str, text: &str) -> Option<(f32, Vec<usize>)> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some((0., vec![]));
    }
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut idx, mut score, mut from, mut prev): (Vec<usize>, f32, usize, i64) = (vec![], 0., 0, -2);
    for ch in q.chars().filter(|c| *c != ' ') {
        let at = from + t.get(from..)?.iter().position(|c| *c == ch)?;
        idx.push(at);
        score += 1.;
        if at as i64 == prev + 1 {
            score += 3.;
        }
        if is_boundary(&t, at) {
            score += 2.;
        }
        if at == 0 {
            score += 2.;
        }
        prev = at as i64;
        from = at + 1;
    }
    let span = idx.last().copied().unwrap_or(0) - idx.first().copied().unwrap_or(0) + 1;
    score -= (span - idx.len()) as f32 * 0.5;
    score -= text.chars().count() as f32 * 0.02;
    Some((score, idx))
}

impl CommandCenter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ph = cx.global::<I18n>().t("palette.placeholder");
        let input = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.bare = true; // kolom cari menyatu dengan bilah (tanpa border/latar), seperti .z-palette__input
            ti
        });
        cx.subscribe(&input, |s, _, _: &InputChanged, cx| {
            // sorot butir pertama yang tidak dinonaktifkan
            s.selected = s.rows(cx).iter().position(|r| !r.disabled).unwrap_or(0);
            cx.notify();
        })
        .detach();
        cx.subscribe(&input, |s, _, _: &InputSubmit, cx| s.confirm(cx)).detach();
        let fh = input.read(cx).focus_handle(cx);
        window.focus(&fh, cx);
        Self { input, selected: 0, focus: cx.focus_handle() }
    }

    fn rows(&self, cx: &App) -> Vec<Row> {
        let i = cx.global::<I18n>();
        let q = self.input.read(cx).text().to_string();
        let mut out = vec![];
        if let Some((k, si, m)) = super::chat_panel::chat_parts::parse_message_link(&q) {
            out.push(Row { group: i.t("group.app"), title: i.t("msg.openLink"), hint: None, disabled: false, matched: vec![], action: PaletteAction::MessageLink(k, si, m) });
            return out;
        }
        let app = super::state::AppState::global(cx).read(cx);
        let (can_back, can_forward) = (app.can_back(), app.can_forward());
        let mut hits: Vec<(f32, &'static super::commands::CommandDef, String, Vec<usize>)> = vec![];
        for c in COMMANDS {
            if c.id == "app.palette" {
                continue;
            }
            let title = i.t(c.title_key);
            let mut m = fuzzy_match(&q, &title);
            if m.is_none() {
                // kata kunci: kecocokan tanpa sorotan, skor dikurangi 2 (seperti versi Tauri)
                m = c.keywords.iter().filter_map(|k| fuzzy_match(&q, k)).map(|(s, _)| (s - 2., vec![])).max_by(|a, b| a.0.total_cmp(&b.0));
            }
            let Some((score, matched)) = m else { continue };
            hits.push((score, c, title, matched));
        }
        let group_key = |g: Group| match g {
            Group::App => "app",
            Group::View => "view",
            Group::Help => "help",
            Group::Dev => "dev",
        };
        if q.trim().is_empty() {
            hits.sort_by(|a, b| group_key(a.1.group).cmp(group_key(b.1.group)).then_with(|| a.2.cmp(&b.2)));
        } else {
            hits.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.2.cmp(&b.2)));
        }
        for (_, c, title, matched) in hits {
            out.push(Row {
                group: i.t(&format!("group.{}", group_key(c.group))),
                title,
                hint: super::commands::shortcut_of(c.id, cx).map(|s| s.replace("ArrowLeft", "←").replace("ArrowRight", "→")),
                disabled: (c.id == "nav.back" && !can_back) || (c.id == "nav.forward" && !can_forward),
                matched,
                action: PaletteAction::Command(c.id),
            });
        }
        let store = ProjectStore::global(cx).read(cx);
        let untitled = i.t("sb.untitled");
        let mut sessions = vec![];
        for (key, st) in &store.sessions {
            let pname = if key == CHAT_KEY {
                "Chat".to_string()
            } else {
                store.projects.iter().find(|p| &p.id == key).map(|p| p.name.clone()).unwrap_or_default()
            };
            for s in &st.items {
                let title = session_title(s).map(str::to_string).unwrap_or_else(|| untitled.clone());
                if !q.is_empty() && !matches(&q, &format!("{title} {pname}")) {
                    continue;
                }
                sessions.push((s.updated, Row {
                    group: i.t("sb.tab.sessions"),
                    title,
                    hint: Some(pname.clone()),
                    disabled: false,
                    matched: vec![],
                    action: PaletteAction::Session(key.clone(), s.id.clone()),
                }));
            }
        }
        sessions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        out.extend(sessions.into_iter().take(if q.is_empty() { 8 } else { 50 }).map(|x| x.1));
        out
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if let Some(r) = rows.get(self.selected).filter(|r| !r.disabled) {
            cx.emit(PaletteEvent::Run(r.action.clone()));
        }
    }

    /// Indeks baris aktif berikutnya yang tidak dinonaktifkan (arah +1/-1); tetap bila tidak ada.
    fn step(rows: &[Row], from: usize, dir: i32) -> usize {
        let mut n = from as i32 + dir;
        while n >= 0 && (n as usize) < rows.len() {
            if !rows[n as usize].disabled {
                return n as usize;
            }
            n += dir;
        }
        from
    }
}

impl Focusable for CommandCenter {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CommandCenter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let vp_h = window.viewport_size().height;
        let rows = self.rows(cx);
        if self.selected >= rows.len() {
            self.selected = rows.len().saturating_sub(1);
        }
        // mengisi sisa tinggi popup lalu menggulir (popup dibatasi tinggi jendela, lihat `max_h` di bawah)
        let mut list = div().id("palette-list").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().p(t.space(1));
        for (n, r) in rows.iter().enumerate() {
            let sel = n == self.selected;
            let disabled = r.disabled;
            let label: gpui::AnyElement = if r.matched.is_empty() {
                r.title.clone().into_any_element()
            } else {
                let hl = HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    underline: Some(UnderlineStyle { thickness: px(1.), color: Some(t.colors.fg), wavy: false }),
                    ..Default::default()
                };
                let bytes: Vec<(usize, char)> = r.title.char_indices().collect();
                let ranges = r.matched.iter().filter_map(|i| bytes.get(*i)).map(|(b, c)| (*b..*b + c.len_utf8(), hl)).collect::<Vec<_>>();
                StyledText::new(r.title.clone()).with_highlights(ranges).into_any_element()
            };
            list = list.child(
                div()
                    .id(("pal", n))
                    .flex()
                    .items_center()
                    .gap(t.space(3))
                    .min_h(t.rem(rem::CONTROL_MD))
                    .px(t.space(3))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .text_size(t.rem(t.ty(crate::theme::BUTTON).size))
                    .when(sel && !disabled, |d| d.bg(t.colors.hover))
                    .when(disabled, |d| d.opacity(0.5).cursor_not_allowed())
                    .when(!disabled, |d| {
                        d.cursor_pointer()
                            .on_mouse_move(cx.listener(move |s, _, _, cx| {
                                if s.selected != n {
                                    s.selected = n;
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.selected = n;
                                s.confirm(cx);
                            }))
                    })
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .child(div().flex_none().small(&t).text_color(t.colors.fg_subtle).child(r.group.clone()))
                    .children(r.hint.clone().map(|h| {
                        if matches!(r.action, PaletteAction::Command(_)) {
                            kbd(h).into_any_element()
                        } else {
                            div().small(&t).text_color(t.colors.fg_subtle).child(h).into_any_element()
                        }
                    })),
            );
        }
        if rows.is_empty() {
            list = list.child(div().p(t.space(4)).small(&t).text_color(t.colors.fg_muted).text_center().child(cx.global::<I18n>().t("ui.noResults")));
        }
        crate::ui::overlay_deferred(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_start() // tanpa ini popup ditarik setinggi jendela (perataan silang default = stretch)
                .justify_center()
                .pt(t.rem(4.))
                .bg(t.colors.backdrop)
                .occlude()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| cx.emit(PaletteEvent::Close)))
                .child(
                    popup(&t)
                        .key_context("Palette")
                        .on_action(cx.listener(|s, _: &SelectPrev, _, cx| {
                            let rows = s.rows(cx);
                            s.selected = Self::step(&rows, s.selected, -1);
                            cx.notify();
                        }))
                        .on_action(cx.listener(|s, _: &SelectNext, _, cx| {
                            let rows = s.rows(cx);
                            s.selected = Self::step(&rows, s.selected, 1);
                            cx.notify();
                        }))
                        .on_action(cx.listener(|_, _: &Dismiss, _, cx| cx.emit(PaletteEvent::Close)))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .w(t.rem(rem::SPACE[7] * 12.))
                        .max_w(px(9999.))
                        .h_auto()
                        .max_h(vp_h - t.rem(4.) - t.rem(2.)) // jarak atas (pt) + margin bawah
                        .p_0()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(t.space(2))
                                .px(t.space(3))
                                .h(t.rem(rem::CONTROL_LG))
                                .border_b_1()
                                .border_color(t.colors.border)
                                .child(icon("search").size(IconSize::Sm).color(t.colors.fg_subtle))
                                .child(div().flex_1().min_w_0().child(self.input.clone())),
                        )
                        .child(list),
                ),
        )
        .with_priority(80)
    }
}

/// Bentuk kombinasi tombol sama dengan string pintasan registry ("Ctrl+Shift+F", "Alt+ArrowLeft").
pub fn combo(k: &gpui::Keystroke) -> String {
    let m = &k.modifiers;
    let mut s = String::new();
    if m.control || m.platform {
        s.push_str("Ctrl+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    if m.shift {
        s.push_str("Shift+");
    }
    let key = match k.key.as_str() {
        "left" => "ArrowLeft".to_string(),
        "right" => "ArrowRight".to_string(),
        "up" => "ArrowUp".to_string(),
        "down" => "ArrowDown".to_string(),
        "+" => "=".to_string(),
        other if other.len() == 1 => other.to_uppercase(),
        other => other.to_string(),
    };
    s.push_str(&key);
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn fuzzy_urut_dan_indeks() {
        let (s, idx) = super::fuzzy_match("gh", "Go home").unwrap();
        assert_eq!(idx, vec![0, 3]);
        assert!(s > 0.);
        // kueri kosong cocok dengan semua; karakter yang tidak ada → tidak cocok
        assert_eq!(super::fuzzy_match("", "apa saja").unwrap().1.len(), 0);
        assert!(super::fuzzy_match("zq", "Go home").is_none());
        // kecocokan berurutan/awal kata menang atas yang tersebar
        let rapat = super::fuzzy_match("set", "Settings").unwrap().0;
        let tersebar = super::fuzzy_match("set", "Show extra things").unwrap().0;
        assert!(rapat > tersebar);
    }

    #[test]
    fn cocok() {
        assert!(super::matches("set buka", "Buka Pengaturan settings"));
        assert!(!super::matches("xyz", "Buka"));
    }
}
