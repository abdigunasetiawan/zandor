//! ChatPanel — port shell/ChatPanel.tsx + AssistantMessage, ToolStep, QuestionCard, kartu izin, composer.
use super::state::AppState;
use crate::core::chat::{message_text, ChatState};
use crate::core::chat_store::ChatStore;
use crate::core::projects::{session_title, ProjectStore, CHAT_KEY};
use crate::core::services::EngineStatus;
use crate::core::types::*;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H3};
use crate::ui::{
    badge, button, icon, icon_button, markdown, working, BadgeTone, ButtonVariant, ControlSize, IconSize, InputSubmit, TextInput,
    TypeExt,
};
use gpui::{
    div, prelude::*, AnyElement, App, ClipboardItem, Context, Entity, FontWeight, IntoElement, ScrollHandle, SharedString,
    WeakEntity, Window,
};
use std::collections::HashSet;

pub struct ChatPanel {
    app: Entity<AppState>,
    store: Entity<ProjectStore>,
    chat: Entity<ChatStore>,
    engine: Entity<EngineStatus>,
    input: Entity<TextInput>,
    scroll: ScrollHandle,
    expanded: HashSet<String>,
    last_count: usize,
    pending_new: bool,
}

impl ChatPanel {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let store = ProjectStore::global(cx);
        let chat = ChatStore::global(cx);
        let engine = EngineStatus::global(cx);
        let ph = cx.global::<I18n>().t("chat.placeholder");
        let input = cx.new(|cx| TextInput::new(cx, ph));
        cx.observe(&store, |this, _, cx| {
            this.sync(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&chat, |_, _, cx| cx.notify()).detach();
        cx.observe(&engine, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&input, |this, _, e: &InputSubmit, cx| this.submit(e.0.to_string(), cx)).detach();
        let mut me = Self {
            app,
            store,
            chat,
            engine,
            input,
            scroll: ScrollHandle::new(),
            expanded: HashSet::new(),
            last_count: 0,
            pending_new: false,
        };
        me.sync(cx);
        me
    }

    /// Sesi terpilih: (kunci, id, direktori).
    fn current(&self, cx: &App) -> Option<(String, String, String, String)> {
        let (key, s, p) = self.store.read(cx).selected()?;
        let dir = if !s.directory.is_empty() { s.directory.clone() } else { p.map(|p| p.path).unwrap_or_default() };
        let title = session_title(&s).map(str::to_string).unwrap_or_default();
        Some((key, s.id, dir, title))
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        if let Some((_, id, dir, _)) = self.current(cx) {
            if !dir.is_empty() {
                self.chat.update(cx, |c, cx| c.open(&dir, &id, cx));
            }
        }
    }

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.input.update(cx, |i, cx| i.set_text("", cx));
        if let Some((_, id, _, _)) = self.current(cx) {
            self.chat.update(cx, |c, cx| c.send(&id, text, cx));
            return;
        }
        // Tanpa sesi: buat sesi Chat lalu kirim.
        if self.pending_new {
            return;
        }
        self.pending_new = true;
        let task = self.store.update(cx, |s, cx| s.create_session(CHAT_KEY, None, true, cx));
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |me, cx| {
                me.pending_new = false;
                match r {
                    Ok(s) => {
                        me.sync(cx);
                        me.chat.update(cx, |c, cx| c.send(&s.id, text, cx));
                    }
                    Err(e) => {
                        crate::ui::Toasts::show(
                        crate::ui::ToastInput {
                            title: e.into(),
                            description: None,
                            tone: crate::ui::ToastTone::Danger,
                            timeout_ms: None,
                            action: None,
                        },
                        cx,
                    );
                    }
                }
            })
            .ok();
        })
        .detach();
    }
}

fn fmt_num(n: f64) -> String {
    if n >= 1000. { format!("{:.1}k", n / 1000.) } else { format!("{}", n as i64) }
}

fn user_msg(t: &Theme, m: &ChatMessage, id: SharedString, i: &I18n) -> AnyElement {
    let text = message_text(m);
    let copy = text.clone();
    div()
        .id(id.clone())
        .group("umsg")
        .flex()
        .flex_col()
        .items_end()
        .gap(t.space(2))
        .self_end()
        .max_w(gpui::relative(0.85))
        .child(
            div()
                .px(t.space(3))
                .py(t.space(2))
                .rounded(t.rem(rem::RADIUS_LG))
                .bg(t.colors.bubble)
                .child(text),
        )
        .child(
            div().flex().justify_end().h(t.rem(rem::CONTROL_SM)).invisible().group_hover("umsg", |s| s.visible()).child(
                icon_button(SharedString::from(format!("{id}-copy")), "copy", i.t("chat.copy"))
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))),
            ),
        )
        .into_any_element()
}

fn tool_step(t: &Theme, i: &I18n, me: &WeakEntity<ChatPanel>, p: &MessagePart, tool: &str, st: &ToolState, open: bool) -> AnyElement {
    let (tone, key, ic) = match st.status {
        ToolStatus::Pending => (BadgeTone::Neutral, "chat.tool.pending", None),
        ToolStatus::Running => (BadgeTone::Warning, "chat.tool.running", None),
        ToolStatus::Completed => (BadgeTone::Success, "chat.tool.completed", Some("check")),
        ToolStatus::Error => (BadgeTone::Danger, "chat.tool.error", Some("triangle_alert")),
    };
    let title = st.title.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        ["filePath", "path", "command", "pattern", "url", "description"]
            .iter()
            .find_map(|k| st.input.get(*k).and_then(|v| v.as_str()).map(str::to_string))
            .unwrap_or_default()
    });
    let pid = p.id.clone();
    let me = me.clone();
    let tool_icon = match tool {
        "bash" => "terminal",
        "read" | "write" | "edit" => "file_text",
        "grep" | "glob" => "search",
        "webfetch" | "websearch" => "globe",
        "todowrite" | "todoread" => "list_checks",
        "task" => "bot",
        _ => "wrench",
    };
    let mut el = div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(t.rem(rem::RADIUS_MD))
        .border_1()
        .border_color(t.colors.border_soft)
        .child(
            div()
                .id(SharedString::from(format!("ts-{}", p.id)))
                .flex()
                .items_center()
                .gap(t.space(2))
                .px(t.space(2))
                .h(t.rem(rem::CONTROL_MD))
                .cursor_pointer()
                .hover(|s| s.bg(t.colors.hover))
                .on_click(move |_, _, cx| {
                    let pid = pid.clone();
                    me.update(cx, |s, cx| {
                        if !s.expanded.remove(&pid) {
                            s.expanded.insert(pid);
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm).color(t.colors.fg_subtle))
                .child(icon(tool_icon).size(IconSize::Sm).color(t.colors.fg_muted))
                .child(div().flex_none().font_weight(FontWeight::MEDIUM).child(tool.to_string()))
                .child(div().flex_1().min_w_0().truncate().text_color(t.colors.fg_muted).code(t).child(title))
                .child(match ic {
                    None if st.status == ToolStatus::Running => working(SharedString::from(format!("tw-{}", p.id))).into_any_element(),
                    _ => badge(i.t(key)).tone(tone).into_any_element(),
                }),
        );
    if open {
        let input = serde_json::to_string_pretty(&serde_json::Value::Object(st.input.clone())).unwrap_or_default();
        let out = st.error.clone().or_else(|| st.output.clone()).unwrap_or_else(|| i.t("tool.noOutput"));
        let block = |label: String, body: String| {
            div()
                .flex()
                .flex_col()
                .gap(t.space(1))
                .child(div().small(t).text_color(t.colors.fg_subtle).child(label))
                .child(
                    div()
                        .px(t.space(2))
                        .py(t.space(1))
                        .rounded(t.rem(rem::RADIUS_SM))
                        .bg(t.colors.raised)
                        .code(t)
                        .child(body.lines().take(200).collect::<Vec<_>>().join("\n")),
                )
        };
        el = el.child(
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(2))
                .border_t_1()
                .border_color(t.colors.border_soft)
                .child(block(i.t("chat.input"), input))
                .child(block(i.t("chat.output"), out)),
        );
    }
    el.into_any_element()
}

fn assistant_msg(t: &Theme, i: &I18n, me: &WeakEntity<ChatPanel>, m: &ChatMessage, expanded: &HashSet<String>) -> AnyElement {
    let mut col = div().flex().flex_col().gap(t.space(2)).w_full().min_w_0();
    for p in &m.parts {
        match &p.kind {
            PartKind::Text { text, synthetic: false } if !text.trim().is_empty() => col = col.child(markdown(text, t)),
            PartKind::Reasoning { text } if !text.trim().is_empty() => {
                let open = expanded.contains(&p.id);
                let pid = p.id.clone();
                let me = me.clone();
                col = col.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(1))
                        .text_color(t.colors.fg_muted)
                        .child(
                            div()
                                .id(SharedString::from(format!("rs-{}", p.id)))
                                .flex()
                                .items_center()
                                .gap(t.space(1))
                                .cursor_pointer()
                                .small(t)
                                .on_click(move |_, _, cx| {
                                    let pid = pid.clone();
                                    me.update(cx, |s, cx| {
                                        if !s.expanded.remove(&pid) {
                                            s.expanded.insert(pid);
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                })
                                .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                                .child(i.t("chat.thinking")),
                        )
                        .when(open, |d| d.child(div().pl(t.space(4)).border_l_1().border_color(t.colors.border).child(markdown(text, t)))),
                );
            }
            PartKind::Tool { tool, state, .. } => col = col.child(tool_step(t, i, me, p, tool, state, expanded.contains(&p.id))),
            PartKind::File { filename, mime, .. } => {
                col = col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .px(t.space(2))
                        .py(t.space(1))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .border_1()
                        .border_color(t.colors.border_soft)
                        .child(icon("file").size(IconSize::Sm))
                        .child(filename.clone().unwrap_or_else(|| mime.clone())),
                )
            }
            _ => {}
        }
    }
    if let Some(e) = &m.info.error {
        col = col.child(div().text_color(t.colors.danger).child(if e.name == "MessageAbortedError" { i.t("chat.aborted") } else { e.message.clone() }));
    }
    if m.info.completed.is_some() {
        let mut meta: Vec<String> = vec![];
        if let Some(model) = &m.info.model_id {
            meta.push(model.clone());
        }
        if let Some(tk) = &m.info.tokens {
            if tk.total > 0. {
                meta.push(i.tp("chat.meta.tokens", &[("input", &fmt_num(tk.input)), ("output", &fmt_num(tk.output))]));
            }
        }
        if !meta.is_empty() {
            col = col.child(div().small(t).text_color(t.colors.fg_subtle).child(meta.join(" · ")));
        }
    }
    col.into_any_element()
}

fn card(t: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(t.space(2))
        .p(t.space(3))
        .rounded(t.rem(rem::RADIUS_LG))
        .border_1()
        .border_color(t.colors.border)
        .bg(t.colors.raised)
}

impl Render for ChatPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        let me = cx.entity().downgrade();
        let cur = self.current(cx);
        let engine_ready = self.engine.read(cx).state.info.is_some();
        let project_name = {
            let s = self.store.read(cx);
            s.selected().and_then(|(_, _, p)| p.map(|p| p.name))
        };
        let state: Option<ChatState> = cur.as_ref().and_then(|(_, id, _, _)| self.chat.read(cx).sessions.get(id).cloned());

        // ---- kepala ----
        let head = cur.as_ref().map(|(_, _, _, title)| {
            let title = if title.is_empty() { i.t("sb.untitled") } else { title.clone() };
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(t.space(2))
                .px(t.space(3))
                .py(t.space(2))
                .border_b_1()
                .border_color(t.colors.border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(div().truncate().small(&t).text_color(t.colors.fg_muted).child(match &project_name {
                            Some(n) => i.tp("chat.inProject", &[("name", n)]),
                            None => i.t("chat.noProject"),
                        })),
                )
        });

        let busy = state.as_ref().is_some_and(|s| s.busy());
        let send_btn = if busy {
            let id = cur.as_ref().map(|c| c.1.clone()).unwrap_or_default();
            icon_button("chat-stop", "stop", i.t("chat.stop"))
                .variant(ButtonVariant::Solid)
                .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.abort(&id, cx))))
        } else {
            icon_button("chat-send", "arrow_up", i.t("chat.send"))
                .variant(ButtonVariant::Solid)
                .disabled(!engine_ready)
                .on_click(cx.listener(|s, _, _, cx| {
                    let text = s.input.read(cx).text().to_string();
                    s.submit(text, cx);
                }))
        };
        let composer = div().w_full().px(t.space(3)).pb(t.space(3)).child(
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .border_1()
                .border_color(t.colors.border_soft)
                .rounded(t.rem(rem::RADIUS_LG * 1.5))
                .bg(t.colors.composer)
                .shadow(t.shadow_1())
                .child(self.input.clone())
                .child(div().flex().items_center().gap(t.space(2)).child(div().flex_1()).child(send_btn)),
        );

        let Some(st) = state else {
            // ---- sambutan ----
            let hour = {
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                ((secs / 3600 + 7) % 24) as u32
            };
            let greet = match hour {
                4..=10 => "chat.greet.morning",
                11..=14 => "chat.greet.afternoon",
                15..=17 => "chat.greet.evening",
                _ => "chat.greet.night",
            };
            return div()
                .flex()
                .flex_col()
                .size_full()
                .children(head)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap(t.space(4))
                        .p(t.space(4))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(div().typo(&t, H3).child(i.tp(greet, &[("name", "")])))
                                .child(div().text_color(t.colors.fg_muted).child(i.t(if engine_ready { "chat.greet.sub" } else { "chat.engineDown" }))),
                        )
                        .child(div().w_full().max_w(t.rem(rem::CHAT_WIDTH)).child(composer)),
                )
                .into_any_element();
        };
        let sid = st.session_id.clone();

        // ---- isi ----
        let mut body = div()
            .id("chat-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(t.space(3));
        let mut inner = div().flex().flex_col().gap(t.space(4)).w_full().max_w(t.rem(rem::CHAT_WIDTH)).mx_auto();
        if !st.loaded {
            inner = inner.child(div().text_color(t.colors.fg_muted).child(i.t("chat.loading")));
        } else if st.messages.is_empty() {
            inner = inner.child(div().text_color(t.colors.fg_muted).child(i.t("chat.empty")));
        }
        let mut count = 0;
        for m in st.visible() {
            count += m.parts.len() + 1;
            inner = inner.child(match m.info.role {
                Role::User => user_msg(&t, m, SharedString::from(format!("m-{}", m.info.id)), i),
                Role::Assistant => assistant_msg(&t, i, &me, m, &self.expanded),
            });
        }
        if let SessionRunStatus::Retry { attempt, message } = &st.status {
            inner = inner.child(div().text_color(t.colors.warning).child(i.tp("chat.retry", &[("attempt", &attempt.to_string()), ("message", message)])));
        } else if busy {
            inner = inner.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .text_color(t.colors.fg_muted)
                    .child(working("chat-working"))
                    .child(i.t("chat.working")),
            );
        }
        if let Some(e) = &st.error {
            inner = inner.child(div().text_color(t.colors.danger).child(e.clone()));
        }
        // ---- izin ----
        for p in &st.permissions {
            let mk = |r: PermissionReply, label: &str, v: ButtonVariant| {
                let (sid, rid) = (sid.clone(), p.id.clone());
                button(SharedString::from(format!("pm-{}-{}", p.id, r.as_str())))
                    .label(i.t(label))
                    .variant(v)
                    .size(ControlSize::Sm)
                    .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.reply_permission(&sid, &rid, r, cx))))
            };
            inner = inner.child(
                card(&t)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(icon("shield").size(IconSize::Md).color(t.colors.warning))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(i.tp("chat.permission.title", &[("permission", &p.permission)]))),
                    )
                    .children((!p.patterns.is_empty()).then(|| div().code(&t).text_color(t.colors.fg_muted).child(p.patterns.join("\n"))))
                    .child(
                        div()
                            .flex()
                            .gap(t.space(2))
                            .child(mk(PermissionReply::Once, "chat.permission.once", ButtonVariant::Solid))
                            .child(mk(PermissionReply::Always, "chat.permission.always", ButtonVariant::Outline))
                            .child(mk(PermissionReply::Reject, "chat.permission.reject", ButtonVariant::Ghost)),
                    ),
            );
        }
        // ---- pertanyaan ----
        for q in &st.questions {
            let mut c = card(&t).child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(icon("info").size(IconSize::Md).color(t.colors.accent))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("q.title"))),
            );
            let total = q.questions.len();
            for (qi, item) in q.questions.iter().enumerate() {
                c = c
                    .when(!item.header.is_empty(), |d| d.child(div().small(&t).text_color(t.colors.fg_subtle).child(item.header.clone())))
                    .child(div().child(item.question.clone()))
                    .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t(if item.multiple { "q.multiHint" } else { "q.singleHint" })));
                let mut opts = div().flex().flex_col().gap(t.space(1));
                for (oi, o) in item.options.iter().enumerate() {
                    let (sid, rid, label) = (sid.clone(), q.id.clone(), o.label.clone());
                    opts = opts.child(
                        div()
                            .id(SharedString::from(format!("qo-{}-{qi}-{oi}", q.id)))
                            .flex()
                            .flex_col()
                            .px(t.space(2))
                            .py(t.space(1))
                            .rounded(t.rem(rem::RADIUS_MD))
                            .border_1()
                            .border_color(t.colors.border_soft)
                            .cursor_pointer()
                            .hover(|s| s.bg(t.colors.hover))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let mut ans = vec![vec![]; total];
                                ans[qi] = vec![label.clone()];
                                s.chat.update(cx, |c, cx| c.reply_question(&sid, &rid, Some(ans), cx));
                            }))
                            .child(div().font_weight(FontWeight::MEDIUM).child(o.label.clone()))
                            .when(!o.description.is_empty(), |d| d.child(div().small(&t).text_color(t.colors.fg_muted).child(o.description.clone()))),
                    );
                }
                c = c.child(opts);
            }
            let (sid2, rid2) = (sid.clone(), q.id.clone());
            c = c.child(
                div().flex().justify_end().child(
                    button(SharedString::from(format!("qs-{}", q.id)))
                        .label(i.t("q.skip"))
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.reply_question(&sid2, &rid2, None, cx)))),
                ),
            );
            inner = inner.child(c);
        }
        body = body.child(inner);
        if count != self.last_count {
            self.last_count = count;
            self.scroll.scroll_to_bottom();
        }
        let _ = &self.app;

        div()
            .flex()
            .flex_col()
            .size_full()
            .children(head)
            .child(body)
            .child(div().w_full().max_w(t.rem(rem::CHAT_WIDTH + 1.5)).mx_auto().child(composer))
            .into_any_element()
    }
}
