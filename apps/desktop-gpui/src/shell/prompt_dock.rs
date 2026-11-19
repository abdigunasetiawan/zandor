//! Dok di atas composer — port PromptDock (ChatPanel.tsx), QuestionCard + TodoList (QuestionCard.tsx), PermissionCard:
//! pertanyaan agent (bertahap, pilihan tunggal/ganda + jawaban sendiri), izin (alasan risiko, pola, diff), rencana agent,
//! lalu antrean pesan. Berada di luar isi respons agar selalu terlihat sampai dijawab.
use super::ChatPanel;
use crate::core::chat::ChatState;
use crate::core::types::*;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::feedback::{WorkingSize, WorkingVariant};
use crate::ui::{button, icon, working, ButtonVariant, ControlSize, IconSize, InputChanged, TextInput, TypeExt};
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, FontWeight, SharedString};
use std::collections::HashMap;

#[derive(Default)]
struct QState {
    picks: HashMap<usize, Vec<String>>,
    custom: HashMap<usize, Entity<TextInput>>,
    step: usize,
}

#[derive(Default)]
pub(super) struct DockState {
    q: HashMap<String, QState>,
    todos_closed: bool,
}

/// Jawaban per pertanyaan (port `answers` QuestionCard).
fn answers(req: &QuestionRequest, qs: &QState, text_of: impl Fn(usize) -> String) -> Vec<Vec<String>> {
    req.questions
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let typed = if q.custom || q.options.is_empty() { text_of(i).trim().to_string() } else { String::new() };
            let chosen = qs.picks.get(&i).cloned().unwrap_or_default();
            if q.multiple {
                let mut v = chosen;
                if !typed.is_empty() {
                    v.push(typed);
                }
                v
            } else if !typed.is_empty() {
                vec![typed]
            } else {
                chosen
            }
        })
        .collect()
}

fn dock_card(t: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(t.space(3))
        .p(t.space(3))
        .border_1()
        .border_color(t.colors.border_soft)
        .rounded(t.rem(rem::RADIUS_LG * 1.5))
        .bg(t.colors.composer)
        .shadow(t.shadow_1())
}

impl ChatPanel {
    /// Sesi demo: jawaban izin/pertanyaan hanya lokal (kartu dilepas + toast), tidak dikirim ke engine (port DemoChat).
    /// `true` bila `sid` adalah sesi demo yang terbuka.
    pub(super) fn demo_reply(&mut self, sid: &str, req: &str, answers: Option<&Vec<Vec<String>>>, perm: Option<PermissionReply>, cx: &mut Context<Self>) -> bool {
        let Some(st) = self.demo_state.as_mut().filter(|s| s.session_id == sid) else { return false };
        let i = cx.global::<I18n>().clone();
        let msg = if let Some(r) = perm {
            st.permissions.retain(|p| p.id != req);
            i.tp("demo.answered.permission", &[("reply", &i.t(&format!("demo.reply.{}", r.as_str())))])
        } else {
            st.questions.retain(|q| q.id != req);
            match answers {
                Some(a) => i.tp("demo.answered.question", &[("answers", &a.iter().map(|x| x.join(", ")).collect::<Vec<_>>().join(" | "))]),
                None => i.t("demo.answered.skipped"),
            }
        };
        crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Success, ..crate::ui::ToastInput::new(msg) }, cx);
        cx.notify();
        true
    }

    pub(super) fn render_dock(&mut self, st: &ChatState, sid: &str, queue: Option<AnyElement>, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pending = st.permissions.len() + st.questions.len();
        if pending == 0 && st.todos.is_empty() && queue.is_none() {
            return None;
        }
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let mut dock = div().id("prompt-dock").flex().flex_col().gap(t.space(2)).max_h(t.rem(28.)).overflow_y_scroll().pb(t.space(1));
        let live: Vec<String> = st.questions.iter().map(|q| q.id.clone()).collect();
        self.dock.q.retain(|k, _| live.contains(k));
        for q in &st.questions {
            dock = dock.child(self.question_card(&t, &i, q, sid, cx));
        }
        for p in &st.permissions {
            dock = dock.child(self.permission_card(&t, &i, p, sid, pending > 0, cx));
        }
        if !st.todos.is_empty() {
            dock = dock.child(self.todo_list(&t, &i, &st.todos, cx));
        }
        Some(dock.children(queue).into_any_element())
    }

    fn question_card(&mut self, t: &Theme, i: &I18n, req: &QuestionRequest, sid: &str, cx: &mut Context<Self>) -> AnyElement {
        let rid = req.id.clone();
        let total = req.questions.len();
        // kolom jawaban sendiri dibuat sekali per pertanyaan yang membolehkannya
        for (qi, q) in req.questions.iter().enumerate() {
            let need = q.custom || q.options.is_empty();
            let has = self.dock.q.get(&rid).is_some_and(|s| s.custom.contains_key(&qi));
            if need && !has {
                let ph = i.t(if q.options.is_empty() { "q.freePlaceholder" } else { "q.customPlaceholder" });
                let ti = cx.new(|cx| {
                    let mut ti = TextInput::new(cx, ph);
                    ti.multiline = true;
                    ti
                });
                let (r2, multiple) = (rid.clone(), q.multiple);
                cx.subscribe(&ti, move |s, _, e: &InputChanged, cx| {
                    // pilihan tunggal: teks bebas menggantikan pilihan
                    if !multiple && !e.0.trim().is_empty() {
                        if let Some(qs) = s.dock.q.get_mut(&r2) {
                            qs.picks.insert(qi, vec![]);
                        }
                    }
                    cx.notify();
                })
                .detach();
                self.dock.q.entry(rid.clone()).or_default().custom.insert(qi, ti);
            }
        }
        let qs = self.dock.q.entry(rid.clone()).or_default();
        let step = qs.step.min(total.saturating_sub(1));
        let ans = answers(req, qs, |n| qs.custom.get(&n).map(|e| e.read(cx).text().to_string()).unwrap_or_default());
        let complete = ans.iter().all(|a| !a.is_empty());
        let last = step + 1 >= total;
        let mut card = dock_card(t).child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap(t.space(2))
                .typo(t, crate::theme::P)
                .font_weight(FontWeight(t.ty(crate::theme::H4).weight))
                .child(i.t("q.title"))
                .when(total > 1, |d| d.child(div().small(t).font_weight(FontWeight::NORMAL).text_color(t.colors.fg_muted).child(i.tp("q.progress", &[("n", &(step + 1).to_string()), ("total", &total.to_string())])))),
        );
        if let Some(q) = req.questions.get(step) {
            let qi = step;
            let mut fs = div().flex().flex_col().gap(t.space(2)).min_w_0().child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .when(!q.header.is_empty(), |d| d.child(tracked(t, &q.header.to_uppercase(), 0.04)))
                    .child(div().typo(t, crate::theme::P).child(if q.question.is_empty() { i.t("q.malformed") } else { q.question.clone() })),
            );
            if !q.options.is_empty() {
                fs = fs.child(div().small(t).text_color(t.colors.fg_muted).child(i.t(if q.multiple { "q.multiHint" } else { "q.singleHint" })));
            }
            let picked = qs.picks.get(&qi).cloned().unwrap_or_default();
            let mut opts = div().flex().flex_col().gap(t.space(2));
            for (oi, o) in q.options.iter().enumerate() {
                let checked = picked.contains(&o.label);
                let (r2, label, multiple) = (rid.clone(), o.label.clone(), q.multiple);
                let mark = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(t.rem(rem::ICON_MD))
                    .mt(t.space(1) / 2.)
                    .border_1()
                    .border_color(if checked { t.colors.accent } else { t.colors.border_strong })
                    .when(checked, |d| d.bg(t.colors.accent))
                    .text_color(t.colors.on_accent)
                    .rounded(if multiple { t.rem(rem::RADIUS_MD / 2.) } else { t.rem(rem::RADIUS_FULL) })
                    .when(multiple && checked, |d| d.child(icon("check").size(IconSize::Sm)))
                    .when(!multiple, |d| d.child(radio_dot(t, &format!("qd-{rid}-{qi}-{oi}"), checked)));
                opts = opts.child(
                    div()
                        .id(SharedString::from(format!("qo-{rid}-{qi}-{oi}")))
                        .flex()
                        .items_start()
                        .gap(t.space(3))
                        .px(t.space(3))
                        .py(t.space(2))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .bg(if checked { t.colors.active } else { t.colors.hover })
                        .hover(|s| s.bg(t.colors.active))
                        .cursor_pointer()
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let qs = s.dock.q.entry(r2.clone()).or_default();
                            let cur = qs.picks.entry(qi).or_default();
                            if multiple {
                                if let Some(p) = cur.iter().position(|x| *x == label) {
                                    cur.remove(p);
                                } else {
                                    cur.push(label.clone());
                                }
                            } else {
                                *cur = vec![label.clone()];
                                if let Some(ti) = qs.custom.get(&qi).cloned() {
                                    ti.update(cx, |ti, cx| ti.set_text("", cx));
                                }
                            }
                            cx.notify();
                        }))
                        .child(mark)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(div().font_weight(FontWeight(t.ty(crate::theme::LABEL).weight)).child(o.label.clone()))
                                .when(!o.description.is_empty(), |d| d.child(div().small(t).text_color(t.colors.fg_muted).child(o.description.clone()))),
                        ),
                );
            }
            fs = fs.child(opts);
            if let Some(ti) = qs.custom.get(&qi) {
                let rows = if q.options.is_empty() { 3. } else { 1. };
                fs = fs.child(
                    div()
                        .min_h(t.rem(rows * 1.5 + 1.))
                        .px(t.space(2))
                        .py(t.space(1))
                        .border_1()
                        .border_color(t.colors.border)
                        .rounded(t.rem(rem::RADIUS_MD))
                        .bg(t.colors.bg)
                        .child(ti.clone()),
                );
            }
            card = card.child(fs);
        }
        let step_ok = ans.get(step).is_some_and(|a| !a.is_empty());
        let mut actions = div().flex().flex_wrap().gap(t.space(2));
        if step > 0 {
            let r2 = rid.clone();
            actions = actions.child(button(SharedString::from(format!("qb-{rid}"))).label(i.t("q.back")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| {
                if let Some(qs) = s.dock.q.get_mut(&r2) {
                    qs.step = qs.step.saturating_sub(1);
                }
                cx.notify();
            })));
        }
        if last {
            let (r2, sid2, req2) = (rid.clone(), sid.to_string(), req.clone());
            actions = actions.child(
                button(SharedString::from(format!("qsend-{rid}")))
                    .label(i.t("q.send"))
                    .variant(ButtonVariant::Solid)
                    .size(ControlSize::Sm)
                    .disabled(!complete)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        let Some(qs) = s.dock.q.get(&r2) else { return };
                        let a = answers(&req2, qs, |n| qs.custom.get(&n).map(|e| e.read(cx).text().to_string()).unwrap_or_default());
                        if a.iter().all(|x| !x.is_empty()) {
                            if !s.demo_reply(&sid2, &r2, Some(&a), None, cx) {
                                s.chat.update(cx, |c, cx| c.reply_question(&sid2, &r2, Some(a), cx));
                            }
                        }
                    })),
            );
        } else {
            let r2 = rid.clone();
            actions = actions.child(
                button(SharedString::from(format!("qn-{rid}")))
                    .label(i.t("q.next"))
                    .variant(ButtonVariant::Solid)
                    .size(ControlSize::Sm)
                    .disabled(!step_ok)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if let Some(qs) = s.dock.q.get_mut(&r2) {
                            qs.step += 1;
                        }
                        cx.notify();
                    })),
            );
        }
        let (r2, sid2) = (rid.clone(), sid.to_string());
        actions = actions.child(
            button(SharedString::from(format!("qs-{rid}")))
                .label(i.t("q.skip"))
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .on_click(cx.listener(move |s, _, _, cx| {
                    if !s.demo_reply(&sid2, &r2, None, None, cx) {
                        s.chat.update(cx, |c, cx| c.reply_question(&sid2, &r2, None, cx))
                    }
                })),
        );
        card.child(actions).into_any_element()
    }

    fn permission_card(&mut self, t: &Theme, i: &I18n, p: &PermissionRequest, sid: &str, pending: bool, cx: &mut Context<Self>) -> AnyElement {
        let mode = self.chat.read(cx).permission_mode.clone();
        let risk = if mode == "auto-safe" { crate::core::composer::permission_risk(p) } else { None };
        let mk = |r: PermissionReply, label: &str, v: ButtonVariant, cx: &mut Context<Self>| {
            let (sid, rid) = (sid.to_string(), p.id.clone());
            button(SharedString::from(format!("pm-{}-{}", p.id, r.as_str())))
                .label(i.t(label))
                .variant(v)
                .size(ControlSize::Sm)
                .on_click(cx.listener(move |s, _, _, cx| {
                    if !s.demo_reply(&sid, &rid, None, Some(r), cx) {
                        s.chat.update(cx, |c, cx| c.reply_permission(&sid, &rid, r, cx))
                    }
                }))
        };
        let title = i.tp("chat.permission.title", &[("permission", &p.permission)]);
        let mut card = dock_card(t)
            .gap(t.space(2))
            .px(t.space(4))
            .when(pending, |d| d.border_color(t.colors.border))
            .child(div().typo(t, crate::theme::LABEL).child(title));
        if let Some(r) = risk {
            card = card.child(div().small(t).text_color(t.colors.fg_muted).child(i.t(&format!("chat.risk.{r}"))));
        }
        if !p.patterns.is_empty() {
            card = card.child(
                div()
                    .id(SharedString::from(format!("pp-{}", p.id)))
                    .max_h(t.rem(16.))
                    .overflow_y_scroll()
                    .px(t.space(3))
                    .py(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .bg(t.colors.hover)
                    .code(t)
                    .child(p.patterns.join("\n")),
            );
        }
        if let Some(d) = p.metadata.get("diff").and_then(|v| v.as_str()) {
            card = card.child(super::chat_parts::diff_block(t, i, format!("pd-{}", p.id), &crate::core::tool_view::parse_diff(d)));
        }
        let mut actions = div().flex().flex_wrap().gap(t.space(2)).child(mk(PermissionReply::Once, "chat.permission.once", ButtonVariant::Solid, cx));
        if !p.always.is_empty() {
            actions = actions.child(mk(PermissionReply::Always, "chat.permission.always", ButtonVariant::Outline, cx));
        }
        actions = actions.child(mk(PermissionReply::Reject, "chat.permission.reject", ButtonVariant::Danger, cx));
        card.child(actions).into_any_element()
    }

    fn todo_list(&mut self, t: &Theme, i: &I18n, todos: &[Todo], cx: &mut Context<Self>) -> AnyElement {
        let open = !self.dock.todos_closed;
        let done = todos.iter().filter(|x| matches!(x.status, TodoStatus::Completed | TodoStatus::Cancelled)).count();
        let active = todos.iter().position(|x| x.status == TodoStatus::InProgress);
        let current = active.map(|a| todos[a].content.clone()).unwrap_or_else(|| if done == todos.len() { i.t("todo.allDone") } else { String::new() });
        let head = div()
            .id("todos-head")
            .flex()
            .items_center()
            .gap(t.space(2))
            .w_full()
            .min_h(t.rem(rem::CONTROL_MD))
            .px(t.space(3))
            .py(t.space(2))
            .cursor_pointer()
            .hover(|s| s.bg(t.colors.hover))
            .on_click(cx.listener(|s, _, _, cx| {
                s.dock.todos_closed = !s.dock.todos_closed;
                cx.notify();
            }))
            .child(div().flex_none().text_color(t.colors.fg_muted).child(icon("list_checks").size(IconSize::Sm)))
            .child(div().flex_none().font_weight(FontWeight(t.ty(crate::theme::H4).weight)).child(i.t("todo.heading")))
            .child(div().flex_none().font_family(t.font_code.clone()).text_size(t.rem(t.ty(crate::theme::SMALL).size)).text_color(t.colors.fg_muted).child(format!("{done}/{}", todos.len())))
            .child(div().flex_1().min_w_0().truncate().small(t).text_color(t.colors.fg_muted).when(open, |d| d.invisible()).child(current))
            .child(div().flex_none().text_color(t.colors.fg_muted).child(icon(if open { "chevron_up" } else { "chevron_down" }).size(IconSize::Sm)));
        let frac = if todos.is_empty() { 0. } else { done as f32 / todos.len() as f32 };
        let bar = div().h(t.space(1) / 2.).bg(t.colors.hover).child(div().h_full().w(gpui::relative(frac)).bg(t.colors.fg_muted));
        let mut card = div()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_1()
            .border_color(t.colors.border_soft)
            .rounded(t.rem(rem::RADIUS_LG))
            .bg(t.colors.composer)
            .shadow(t.shadow_1())
            .child(head)
            .child(bar);
        if open {
            let mut list = div().id("todos-list").flex().flex_col().max_h(t.rem(14.)).overflow_y_scroll().p(t.space(1));
            for (n, x) in todos.iter().enumerate() {
                let st = x.status;
                let ip = st == TodoStatus::InProgress;
                let mark = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(t.rem(rem::ICON_MD))
                    .mt(px(2.))
                    .rounded_full()
                    .border_1()
                    .border_color(match st {
                        TodoStatus::Completed | TodoStatus::Cancelled => gpui::transparent_black(),
                        TodoStatus::InProgress => t.colors.fg_muted,
                        TodoStatus::Pending => t.colors.border_strong,
                    })
                    .when(st == TodoStatus::Completed, |d| d.bg(t.colors.active))
                    .text_color(if st == TodoStatus::Pending { t.colors.fg_muted } else { t.colors.fg })
                    .child(match st {
                        TodoStatus::Completed => icon("check").size(IconSize::Sm).into_any_element(),
                        TodoStatus::Cancelled => icon("x").size(IconSize::Sm).into_any_element(),
                        TodoStatus::InProgress => working(SharedString::from(format!("todo-w-{n}"))).variant(WorkingVariant::Grid).size(WorkingSize::Dot).into_any_element(),
                        TodoStatus::Pending => div().into_any_element(),
                    });
                let struck = matches!(st, TodoStatus::Completed | TodoStatus::Cancelled);
                list = list.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(t.space(2))
                        .px(t.space(2))
                        .py(t.space(1))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .when(ip, |d| d.bg(t.colors.hover))
                        .text_size(t.rem(t.ty(crate::theme::SMALL).size))
                        .text_color(if ip { t.colors.fg } else { t.colors.fg_muted })
                        .child(mark)
                        .child(div().min_w_0().when(struck, |d| d.text_color(t.colors.fg_subtle).line_through()).child(x.content.clone())),
                );
            }
            card = card.child(list);
        }
        card.into_any_element()
    }
}

/// Titik radio: CSS memakai transform scale(0→1); GPUI tidak punya transform skala pada div, jadi ukurannya
/// dianimasikan dari 0 ke 1/3 ikon dengan durasi --z-duration-fast (hasil visual sama: titik tumbuh dari tengah).
fn radio_dot(t: &Theme, id: &str, on: bool) -> AnyElement {
    use gpui::{Animation, AnimationExt};
    let full = t.rem(rem::ICON_MD / 3.);
    let c = t.colors.on_accent;
    if !on {
        return div().into_any_element();
    }
    div()
        .rounded_full()
        .bg(c)
        .size(full)
        .with_animation(SharedString::from(id.to_string()), Animation::new(crate::theme::dur_ms(120)), move |d, k| d.size(full * k))
        .into_any_element()
}

/// Letter-spacing: GPUI tidak punya properti jarak huruf; untuk label pendek (mis. .qcard__topic 0.04em) setiap huruf
/// diletakkan sebagai elemen sendiri dengan jarak `em` × ukuran font — hasilnya setara letter-spacing CSS.
pub(super) fn tracked(t: &Theme, text: &str, em: f32) -> AnyElement {
    let size = t.ty(crate::theme::SMALL).size;
    let gap = t.rem(size * em);
    let mut row = div().flex().flex_wrap().small(t).text_color(t.colors.fg_muted);
    for w in text.split(' ') {
        let mut word = div().flex().mr(t.rem(size * 0.3));
        for ch in w.chars() {
            word = word.child(div().mr(gap).child(ch.to_string()));
        }
        row = row.child(word);
    }
    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn q(multiple: bool, custom: bool, opts: &[&str]) -> QuestionItem {
        QuestionItem { question: "?".into(), header: String::new(), options: opts.iter().map(|o| QuestionOption { label: o.to_string(), description: String::new() }).collect(), multiple, custom }
    }
    #[test]
    fn jawaban() {
        let req = QuestionRequest { id: "r".into(), session_id: "s".into(), questions: vec![q(false, true, &["a", "b"]), q(true, true, &["x", "y"]), q(false, false, &[])], tool: None };
        let mut qs = QState::default();
        qs.picks.insert(0, vec!["a".into()]);
        qs.picks.insert(1, vec!["x".into(), "y".into()]);
        let a = answers(&req, &qs, |n| if n == 1 { " z ".into() } else if n == 2 { "bebas".into() } else { String::new() });
        assert_eq!(a, vec![vec!["a".to_string()], vec!["x".into(), "y".into(), "z".into()], vec!["bebas".to_string()]]);
        let a = answers(&req, &qs, |n| if n == 0 { "ketik".into() } else { String::new() });
        assert_eq!(a[0], vec!["ketik".to_string()]);
        assert!(a[2].is_empty());
    }
}
