//! CommandCenter — port shell/CommandCenter.tsx: palet perintah (Ctrl+K) + lompat ke sesi, dengan navigasi keyboard.
use super::commands::{Group, COMMANDS};
use crate::core::projects::{session_title, ProjectStore, CHAT_KEY};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, kbd, popup, IconSize, InputChanged, InputSubmit, TextInput, TypeExt};
use gpui::{
    actions, div, prelude::*, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    IntoElement, KeyBinding, Window,
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
    group: String,
    title: String,
    hint: Option<String>,
    icon: &'static str,
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

impl CommandCenter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ph = cx.global::<I18n>().t("palette.placeholder");
        let input = cx.new(|cx| TextInput::new(cx, ph));
        cx.subscribe(&input, |s, _, _: &InputChanged, cx| {
            s.selected = 0;
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
            out.push(Row { group: "App".into(), title: i.t("msg.openLink"), hint: None, icon: "link", action: PaletteAction::MessageLink(k, si, m) });
            return out;
        }
        for c in COMMANDS {
            if c.id == "app.palette" {
                continue;
            }
            let title = i.t(c.title_key);
            if !q.is_empty() && !matches(&q, &format!("{title} {} {}", c.id, c.keywords.join(" "))) {
                continue;
            }
            let group = match c.group {
                Group::App => "App".to_string(),
                Group::View => i.t("menu.view"),
                Group::Help => i.t("menu.help"),
                Group::Dev => "Dev".to_string(),
            };
            out.push(Row {
                group,
                title,
                hint: super::commands::shortcut_of(c.id, cx).map(|s| s.replace("ArrowLeft", "←").replace("ArrowRight", "→")),
                icon: "sparkles",
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
                    icon: "chat",
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
        if let Some(r) = rows.get(self.selected) {
            cx.emit(PaletteEvent::Run(r.action.clone()));
        }
    }
}

impl Focusable for CommandCenter {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CommandCenter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let rows = self.rows(cx);
        if self.selected >= rows.len() {
            self.selected = rows.len().saturating_sub(1);
        }
        let empty = cx.global::<I18n>().t("sb.noMatch");
        let mut list = div().id("palette-list").flex().flex_col().max_h(t.rem(rem::SPACE[7] * 7.)).overflow_y_scroll().p(t.space(1));
        let mut last_group = String::new();
        for (n, r) in rows.iter().enumerate() {
            if r.group != last_group {
                last_group = r.group.clone();
                list = list.child(
                    div().px(t.space(2)).pt(t.space(2)).pb(t.space(1)).small(&t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(r.group.clone()),
                );
            }
            let sel = n == self.selected;
            list = list.child(
                div()
                    .id(("pal", n))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .h(t.rem(rem::CONTROL_MD))
                    .px(t.space(2))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .when(sel, |d| d.bg(t.colors.active))
                    .hover(|s| s.bg(t.colors.hover))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.selected = n;
                        s.confirm(cx);
                    }))
                    .child(icon(r.icon).size(IconSize::Sm).color(t.colors.fg_muted))
                    .child(div().flex_1().min_w_0().truncate().child(r.title.clone()))
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
            list = list.child(div().p(t.space(3)).text_color(t.colors.fg_muted).child(empty));
        }
        let count = rows.len();
        crate::ui::overlay_deferred(
            div()
                .absolute()
                .inset_0()
                .flex()
                .justify_center()
                .pt(t.rem(4.))
                .bg(t.colors.backdrop)
                .occlude()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| cx.emit(PaletteEvent::Close)))
                .child(
                    popup(&t)
                        .key_context("Palette")
                        .on_action(cx.listener(|s, _: &SelectPrev, _, cx| {
                            s.selected = s.selected.saturating_sub(1);
                            cx.notify();
                        }))
                        .on_action(cx.listener(move |s, _: &SelectNext, _, cx| {
                            if s.selected + 1 < count {
                                s.selected += 1;
                            }
                            cx.notify();
                        }))
                        .on_action(cx.listener(|_, _: &Dismiss, _, cx| cx.emit(PaletteEvent::Close)))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .w(t.rem(rem::SPACE[7] * 12.))
                        .max_w(px(9999.))
                        .h_auto()
                        .p_0()
                        .child(div().p(t.space(2)).border_b_1().border_color(t.colors.border_soft).child(self.input.clone()))
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
    fn cocok() {
        assert!(super::matches("set buka", "Buka Pengaturan settings"));
        assert!(!super::matches("xyz", "Buka"));
    }
}
