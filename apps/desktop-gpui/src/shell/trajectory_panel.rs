//! Panel alat Trajectory — port TrajectoryPanel.tsx (ADR-073): bilah (lipat semua giliran, tampil/sembunyi konteks/
//! berpikir/tool/langkah, mode durasi/urutan, salin JSON, cari), statistik sesi, ikhtisar tiga jalur (Masukan/Model/
//! Tool), daftar rekaman per giliran, dan inspektur (Ringkasan/Input/Output/Raw). Data murni dari pesan sesi.
use crate::core::chat_store::ChatStore;
use crate::core::projects::ProjectStore;
use crate::core::trajectory::{self as tj, Kind, Lane, Record, Turn};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, tabs, ButtonVariant, ControlSize, IconSize, InputChanged, TabItem, TextInput, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, px, relative, AnyElement, ClipboardItem, Context, Entity, FontWeight, Hsla, IntoElement, SharedString, Window};
use std::collections::HashSet;

const HIDEABLE: [&str; 4] = ["context", "thinking", "tool", "step"];

pub struct TrajectoryPanel {
    query: Entity<TextInput>,
    hidden: HashSet<&'static str>,
    folded: HashSet<usize>,
    time_mode: bool,
    selected: Option<String>,
    tab: SharedString,
    session: Option<String>,
}

fn clock(ms: Option<f64>) -> String {
    let Some(ms) = ms else { return "—".into() };
    let secs = (ms / 1000.).floor() as i64;
    let local = chrono::DateTime::from_timestamp(secs, ((ms % 1000.) * 1e6) as u32).map(|d| d.with_timezone(&chrono::Local));
    local.map(|d| d.format("%H:%M:%S%.3f").to_string()).unwrap_or_else(|| "—".into())
}

fn kind_color(t: &Theme, k: Kind) -> Hsla {
    match k {
        Kind::User | Kind::Context => t.colors.accent,
        Kind::Tool => t.colors.warning,
        Kind::Error => t.colors.danger,
        Kind::Thinking | Kind::Step | Kind::Other => t.colors.fg_subtle,
        Kind::Model | Kind::Assistant => t.colors.success,
    }
}

impl TrajectoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ph = cx.global::<I18n>().t("traj.search");
        let query = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.size = ControlSize::Sm;
            ti
        });
        cx.subscribe(&query, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        cx.observe(&ChatStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&ProjectStore::global(cx), |_, _, cx| cx.notify()).detach();
        Self { query, hidden: HashSet::new(), folded: HashSet::new(), time_mode: true, selected: None, tab: "summary".into(), session: None }
    }

    fn select(&mut self, r: &Record, cx: &mut Context<Self>) {
        self.selected = if self.selected.as_deref() == Some(r.id.as_str()) { None } else { Some(r.id.clone()) };
        self.folded.remove(&r.turn);
        cx.notify();
    }

    fn copy_all(&self, sid: &str, turns: &[Turn], cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let v = serde_json::json!({
            "session": sid,
            "turns": turns.iter().map(|t| serde_json::json!({
                "turn": t.turn, "start": t.start, "end": t.end, "cost": t.cost, "calls": t.calls, "modelCalls": t.model_calls, "errors": t.errors,
                "tokens": { "input": t.tokens.input, "output": t.tokens.output, "reasoning": t.tokens.reasoning, "total": t.tokens.total },
                "records": t.records.iter().map(|r| serde_json::json!({
                    "id": r.id, "index": r.index, "turn": r.turn, "kind": r.kind.key(), "lane": r.kind.lane().key(), "name": r.name,
                    "summary": r.summary, "preview": r.preview, "start": r.start, "end": r.end, "status": r.status, "messageId": r.message_id,
                    "cost": r.cost, "input": r.input, "output": r.output, "raw": r.raw,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        match serde_json::to_string_pretty(&v) {
            Ok(s) => {
                cx.write_to_clipboard(ClipboardItem::new_string(s));
                Toasts::show(ToastInput { title: i.t("traj.copied").into(), description: None, tone: ToastTone::Success, timeout_ms: None, action: None }, cx);
            }
            Err(_) => {
                Toasts::show(ToastInput { title: i.t("traj.copyFail").into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
            }
        }
    }

    fn inspector(&self, r: &Record, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let d = tj::duration_of(r);
        let mut facts: Vec<(String, String)> = vec![(i.t("traj.f.kind"), i.t(&format!("traj.kind.{}", r.kind.key())))];
        if let Some(n) = &r.name {
            facts.push((i.t("traj.f.name"), n.clone()));
        }
        if let Some(s) = r.status {
            facts.push((i.t("traj.f.status"), s.into()));
        }
        facts.push((i.t("traj.f.turn"), r.turn.to_string()));
        facts.push((i.t("traj.f.start"), clock(r.start)));
        facts.push((i.t("traj.f.end"), clock(r.end)));
        facts.push((i.t("traj.f.duration"), d.map(tj::format_ms).unwrap_or_else(|| "—".into())));
        if let Some(tk) = &r.tokens {
            let f = |x: f64| (x as i64).to_string();
            facts.push((i.t("traj.f.tokens"), i.tp("traj.tokens", &[("input", &f(tk.input)), ("output", &f(tk.output)), ("reasoning", &f(tk.reasoning)), ("total", &f(tk.total))])));
        }
        if let Some(c) = r.cost {
            facts.push((i.t("traj.f.cost"), format!("${c:.5}")));
        }
        facts.push((i.t("traj.f.id"), r.id.clone()));
        let pre = |s: Option<String>| -> AnyElement {
            match s.filter(|s| !s.is_empty()) {
                Some(s) => div().p(t.space(2)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.surface).font_family(t.font_code.clone()).code(t).child(s).into_any_element(),
                None => div().small(t).text_color(t.colors.fg_muted).child(i.t("traj.none")).into_any_element(),
            }
        };
        let panel: AnyElement = match self.tab.as_ref() {
            "input" => pre(r.input.clone()),
            "output" => pre(r.output.clone()),
            "raw" => pre(serde_json::to_string_pretty(&r.raw).ok()),
            _ => div()
                .flex()
                .flex_col()
                .gap(t.space(1))
                .children(facts.into_iter().map(|(k, v)| div().flex().gap(t.space(2)).child(div().w(t.rem(6.)).flex_none().small(t).text_color(t.colors.fg_muted).child(k)).child(div().flex_1().min_w_0().child(v))))
                .into_any_element(),
        };
        let me = cx.entity();
        let items = ["summary", "input", "output", "raw"].iter().map(|v| TabItem { value: (*v).into(), label: i.t(&format!("traj.tab.{v}")).into(), icon: None, disabled: false }).collect();
        div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .p(t.space(3))
            .border_t_1()
            .border_color(t.colors.border)
            .bg(t.colors.raised)
            .max_h(t.rem(24.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t(&format!("traj.kind.{}", r.kind.key()))))
                    .children(r.name.clone().map(|n| div().font_family(t.font_code.clone()).code(t).child(n)))
                    .child(div().flex_1())
                    .child({
                        let mid = r.message_id.clone();
                        button("traj-jump").label(i.t("sact.jump")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(move |_, w, cx| {
                            cx.set_global(super::chat_panel::MessageFocus(Some(mid.clone())));
                            w.refresh();
                        })
                    })
                    .child(icon_button("traj-close", "close", i.t("traj.close")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| {
                        s.selected = None;
                        cx.notify();
                    }))),
            )
            .child(
                div().id("traj-insp").flex_1().min_h_0().overflow_y_scroll().child(tabs("traj-tabs", items, self.tab.clone()).panel(panel).on_change(move |v, _, cx| {
                    me.update(cx, |s, cx| {
                        s.tab = v;
                        cx.notify();
                    })
                })),
            )
            .into_any_element()
    }
}

impl Render for TrajectoryPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let note = |s: String| super::widgets::tool_notice_inline(&t, s);
        let Some((_, sess, _)) = ProjectStore::global(cx).read(cx).selected() else { return super::widgets::tool_notice(&t, "trajectory", i.t("info.empty")) };
        if self.session.as_deref() != Some(sess.id.as_str()) {
            self.session = Some(sess.id.clone());
            self.selected = None;
            self.folded.clear();
        }
        let msgs = ChatStore::global(cx).read(cx).sessions.get(&sess.id).map(|c| c.messages.clone()).unwrap_or_default();
        let turns = tj::build(&msgs);
        let sum = tj::totals(&turns);
        let now = crate::core::projects::now_ms();
        let tl = tj::timeline(&turns, self.time_mode, now);
        let all: Vec<Record> = turns.iter().flat_map(|x| x.records.clone()).collect();
        let q = self.query.read(cx).text().trim().to_string();
        let current = self.selected.as_ref().and_then(|id| all.iter().find(|r| &r.id == id).cloned());
        let all_folded = !turns.is_empty() && turns.iter().all(|x| self.folded.contains(&x.turn));

        // ---- bilah ----
        let mut bar = div().flex().flex_wrap().items_center().gap(t.space(1)).p(t.space(2));
        let turn_ids: Vec<usize> = turns.iter().map(|x| x.turn).collect();
        bar = bar.child(
            button("traj-fold")
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .child(icon(if all_folded { "chevron_right" } else { "chevron_down" }).size(IconSize::Sm))
                .label(i.t("traj.turns"))
                .on_click(cx.listener(move |s, _, _, cx| {
                    if all_folded {
                        s.folded.clear();
                    } else {
                        s.folded = turn_ids.iter().copied().collect();
                    }
                    cx.notify();
                })),
        );
        for h in HIDEABLE {
            let on = !self.hidden.contains(h);
            bar = bar.child(
                button(SharedString::from(format!("traj-h-{h}")))
                    .size(ControlSize::Sm)
                    .variant(if on { ButtonVariant::Outline } else { ButtonVariant::Ghost })
                    .label(i.t(&format!("traj.show.{h}")))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if !s.hidden.remove(h) {
                            s.hidden.insert(h);
                        }
                        cx.notify();
                    })),
            );
        }
        let sid = sess.id.clone();
        let turns_copy = turns.clone();
        bar = bar
            .child(
                button("traj-mode")
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .label(if self.time_mode { i.t("traj.mode.time") } else { i.t("traj.mode.sequence") })
                    .tooltip_text(i.t("traj.modeHint"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.time_mode = !s.time_mode;
                        cx.notify();
                    })),
            )
            .child(icon_button("traj-copy", "copy", i.t("traj.copyAll")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.copy_all(&sid, &turns_copy, cx))))
            .child(div().flex_1().min_w(t.rem(8.)).child(self.query.clone()));

        // ---- statistik ----
        let mut stats = i.tp(
            "traj.stats",
            &[
                ("turns", &turns.len().to_string()),
                ("model", &sum.model_calls.to_string()),
                ("calls", &sum.calls.to_string()),
                ("input", &tj::format_tokens(sum.tokens.input)),
                ("output", &tj::format_tokens(sum.tokens.output)),
            ],
        );
        if sum.cost > 0. {
            stats.push_str(&format!(" · ${:.4}", sum.cost));
        }
        let stats_el = div()
            .px(t.space(2))
            .small(&t)
            .text_color(t.colors.fg_muted)
            .child(stats)
            .when(sum.errors > 0, |d| d.child(div().text_color(t.colors.danger).child(format!(" · {}", i.tp("traj.errors", &[("count", &sum.errors.to_string())])))));

        // ---- ikhtisar ----
        let overview = tl.map(|tl| {
            let mut sec = div().flex().flex_col().gap(px(2.)).px(t.space(2));
            for lane in [Lane::Input, Lane::Model, Lane::Tools] {
                let mut track = div().relative().flex_1().h(t.rem(0.75)).rounded(t.rem(rem::RADIUS_SM)).bg(t.colors.surface).overflow_hidden();
                for (_, left) in &tl.boundaries {
                    track = track.child(div().absolute().top_0().bottom_0().left(relative(*left / 100.)).w(px(1.)).bg(t.colors.border));
                }
                for s in tl.spans.iter().filter(|s| s.lane == lane) {
                    let Some(r) = all.get(s.index).cloned() else { continue };
                    let active = self.selected.as_deref() == Some(r.id.as_str());
                    let dim = !q.is_empty() && !tj::matches_query(&r, &q);
                    let color = if s.error { t.colors.danger } else { kind_color(&t, s.kind) };
                    let d = tj::duration_of(&r);
                    let tip = format!("{}{} · {}{}", i.t(&format!("traj.kind.{}", r.kind.key())), r.name.as_ref().map(|n| format!(" {n}")).unwrap_or_default(), clock(r.start), d.map(|d| format!(" · {}", tj::format_ms(d))).unwrap_or_default());
                    track = track.child(
                        div()
                            .id(SharedString::from(format!("trs-{}", r.id)))
                            .absolute()
                            .top(px(1.))
                            .bottom(px(1.))
                            .left(relative(s.left / 100.))
                            .w(relative(s.width / 100.))
                            .min_w(px(3.))
                            .rounded(px(2.))
                            .bg(color)
                            .opacity(if dim { 0.25 } else { 1. })
                            .when(active, |d| d.border_1().border_color(t.colors.focus))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip::text_tooltip(tip.into()))
                            .on_click(cx.listener(move |s, _, _, cx| s.select(&r, cx))),
                    );
                }
                sec = sec.child(div().flex().items_center().gap(t.space(2)).child(div().w(t.rem(4.)).flex_none().small(&t).text_color(t.colors.fg_muted).child(i.t(&format!("traj.lane.{}", lane.key())))).child(track));
            }
            sec
        });

        // ---- daftar ----
        let mut list = div().flex().flex_col().gap(t.space(1)).p(t.space(2));
        if turns.is_empty() {
            list = list.child(note(i.t("traj.empty")));
        }
        for turn in &turns {
            let folded = self.folded.contains(&turn.turn);
            let rows: Vec<&Record> = turn.records.iter().filter(|r| !r.kind.hide_key().is_some_and(|h| self.hidden.contains(h)) && tj::matches_query(r, &q)).collect();
            if !q.is_empty() && rows.is_empty() {
                continue;
            }
            let mut meta = i.tp(
                "traj.turnMeta",
                &[("records", &turn.records.len().to_string()), ("model", &turn.model_calls.to_string()), ("calls", &turn.calls.to_string()), ("tokens", &tj::format_tokens(turn.tokens.total))],
            );
            if let (Some(a), Some(b)) = (turn.start, turn.end) {
                meta.push_str(&format!(" · {}", tj::format_ms(b - a)));
            }
            let tn = turn.turn;
            list = list.child(
                div()
                    .id(SharedString::from(format!("trt-{tn}")))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .py(t.space(1))
                    .cursor_pointer()
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if !s.folded.remove(&tn) {
                            s.folded.insert(tn);
                        }
                        cx.notify();
                    }))
                    .child(icon(if folded { "chevron_right" } else { "chevron_down" }).size(IconSize::Sm))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(i.tp("traj.turn", &[("n", &tn.to_string())])))
                    .child(div().small(&t).text_color(t.colors.fg_muted).child(meta))
                    .when(turn.errors > 0, |d| d.child(div().small(&t).text_color(t.colors.danger).child(i.tp("traj.errors", &[("count", &turn.errors.to_string())])))),
            );
            if folded {
                continue;
            }
            for r in rows {
                let active = self.selected.as_deref() == Some(r.id.as_str());
                let d = tj::duration_of(r);
                let dur = match r.status {
                    Some(s @ ("running" | "pending")) => i.t(&format!("sact.status.{s}")),
                    _ => d.map(tj::format_ms).unwrap_or_default(),
                };
                let rr = r.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("trr-{}", r.id)))
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .pl(t.space(4))
                        .pr(t.space(2))
                        .py(px(3.))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .cursor_pointer()
                        .when(active, |d| d.bg(t.colors.active))
                        .when(!active, |d| d.hover(|s| s.bg(t.colors.hover)))
                        .on_click(cx.listener(move |s, _, _, cx| s.select(&rr, cx)))
                        .child(div().flex_none().w(t.rem(5.)).small(&t).text_color(if r.status == Some("error") { t.colors.danger } else { kind_color(&t, r.kind) }).child(i.t(&format!("traj.kind.{}", r.kind.key()))))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .gap(t.space(1))
                                .children(r.name.clone().map(|n| div().flex_none().font_family(t.font_code.clone()).code(&t).child(n)))
                                .child(div().min_w_0().truncate().child(if r.summary.is_empty() { "—".to_string() } else { r.summary.clone() })),
                        )
                        .children(r.preview.clone().map(|p| div().max_w(t.rem(12.)).truncate().small(&t).text_color(t.colors.fg_muted).child(format!("→ {p}"))))
                        .child(div().flex_none().small(&t).text_color(t.colors.fg_subtle).child(dur)),
                );
            }
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(bar)
            .child(stats_el)
            .children(overview.map(|o| div().py(t.space(2)).child(o)))
            .child(div().id("traj-list").flex_1().min_h_0().overflow_y_scroll().child(list))
            .children(current.map(|r| self.inspector(&r, &t, &i, cx)))
            .into_any_element()
    }
}
