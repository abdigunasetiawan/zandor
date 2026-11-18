//! Pemilih model — port ModelPicker.tsx, ModelRow.tsx, ModelPingTab.tsx, dan model-ping.ts. Popover di atas pemicu dengan
//! tab vertikal: Model (cari, Terakhir digunakan, Disematkan, semua provider — bisa dilipat), Uji (ping semua/per
//! provider/per model; urut & kelompok), lalu satu tab per provider. Model tersembunyi di Pengaturan tidak tampil,
//! kecuali yang sedang dipakai.
use super::*;
use crate::core::engine::{ModelOption, ModelRef};
use crate::core::model_prefs::{ModelPrefsStore, Ref};
use crate::core::pings::{ping_label, PingState, PingStore};
use crate::shell::widgets::model_logo;

pub(super) fn model_key(m: &ModelOption) -> String {
    format!("{}/{}", m.provider_id, m.model_id)
}

fn matches(m: &ModelOption, q: &str) -> bool {
    q.is_empty() || m.name.to_lowercase().contains(q) || m.provider_name.to_lowercase().contains(q) || m.model_id.to_lowercase().contains(q)
}

// ---- model-ping.ts ----
const STATUS_ORDER: [&str; 10] = ["ok", "ratelimit", "auth", "notfound", "server", "network", "invalid", "pending", "untested", "unsupported"];
fn status_rank(k: &str) -> usize {
    STATUS_ORDER.iter().position(|x| *x == k).unwrap_or(STATUS_ORDER.len())
}
pub(super) fn status_key(s: Option<&PingState>) -> String {
    match s {
        None => "untested".into(),
        Some(PingState::Pending) => "pending".into(),
        Some(PingState::Done(r)) => if r.ok { "ok".into() } else { r.kind.clone() },
    }
}
fn latency_of(s: Option<&PingState>) -> u64 {
    match s {
        Some(PingState::Done(r)) => r.latency_ms,
        _ => u64::MAX,
    }
}
pub(super) fn sort_rows(rows: &mut [(ModelOption, Option<PingState>)], by: &str) {
    let name = |a: &ModelOption, b: &ModelOption| a.name.to_lowercase().cmp(&b.name.to_lowercase());
    rows.sort_by(|(a, sa), (b, sb)| match by {
        "provider" => a.provider_name.to_lowercase().cmp(&b.provider_name.to_lowercase()).then(name(a, b)),
        "status" => status_rank(&status_key(sa.as_ref())).cmp(&status_rank(&status_key(sb.as_ref()))).then(name(a, b)),
        "latency" => latency_of(sa.as_ref()).cmp(&latency_of(sb.as_ref())).then(name(a, b)),
        _ => name(a, b),
    });
}
pub(super) fn group_rows(rows: Vec<(ModelOption, Option<PingState>)>, by: &str) -> Vec<(String, Vec<(ModelOption, Option<PingState>)>)> {
    if by == "none" {
        return vec![(String::new(), rows)];
    }
    let mut out: Vec<(String, Vec<(ModelOption, Option<PingState>)>)> = vec![];
    for r in rows {
        let id = if by == "provider" { r.0.provider_id.clone() } else { status_key(r.1.as_ref()) };
        match out.iter_mut().find(|(g, _)| *g == id) {
            Some((_, v)) => v.push(r),
            None => out.push((id, vec![r])),
        }
    }
    if by == "provider" {
        out.sort_by(|a, b| a.1[0].0.provider_name.to_lowercase().cmp(&b.1[0].0.provider_name.to_lowercase()));
    } else {
        out.sort_by_key(|g| status_rank(&g.0));
    }
    out
}

#[derive(Default)]
pub(super) struct PickerState {
    pub open: bool,
    pub tab: String,
    pub expanded: HashSet<String>,
    pub folded: HashSet<String>,
    pub sort: String,
    pub group: String,
}

impl ChatPanel {
    fn visible_models(&self, cx: &App) -> Vec<ModelOption> {
        let cur = self.chat.read(cx).model.clone();
        let prefs = ModelPrefsStore::global(cx);
        self.models
            .iter()
            .filter(|m| {
                let is_cur = cur.as_ref().is_some_and(|c| c.provider_id == m.provider_id && c.model_id == m.model_id);
                is_cur || !prefs.as_ref().is_some_and(|p| p.read(cx).is_hidden(&Ref { provider_id: m.provider_id.clone(), model_id: m.model_id.clone() }))
            })
            .cloned()
            .collect()
    }

    fn close_picker(&mut self, cx: &mut Context<Self>) {
        self.picker.open = false;
        self.picker.tab = "models".into();
        self.mp_search.update(cx, |s, cx| s.set_text("", cx));
        cx.notify();
    }

    fn choose_model(&mut self, m: &ModelOption, cx: &mut Context<Self>) {
        let r = ModelRef { provider_id: m.provider_id.clone(), model_id: m.model_id.clone() };
        self.chat.update(cx, |c, cx| c.select_model(r, cx));
        self.close_picker(cx);
    }

    /// Baris model (ModelRow): pilih, status ping, uji, sematkan.
    fn model_row(&self, t: &Theme, i: &I18n, m: &ModelOption, show_provider: bool, scope: &str, cx: &mut Context<Self>) -> AnyElement {
        let key = model_key(m);
        let c = self.chat.read(cx);
        let selected = c.model.as_ref().is_some_and(|r| r.provider_id == m.provider_id && r.model_id == m.model_id);
        let pinned = c.pinned_models.contains(&key);
        let ping = PingStore::global(cx).read(cx).get(&m.provider_id, &m.model_id).cloned();
        let id = format!("mrow-{scope}-{key}");
        let pick = {
            let mc = m.clone();
            div()
                .id(SharedString::from(format!("{id}-pick")))
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(t.space(2))
                .px(t.space(2))
                .py(t.space(1))
                .rounded(t.rem(rem::RADIUS_MD))
                .cursor_pointer()
                .hover(|s| s.bg(t.colors.hover))
                .when(selected, |d| d.bg(t.colors.active))
                .on_click(cx.listener(move |s, _, _, cx| s.choose_model(&mc, cx)))
                .child(model_logo(t, cx, Some(&m.provider_id), Some(&m.model_id), &m.provider_name, false, &id))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(div().truncate().child(m.name.clone()))
                        .when(show_provider, |d| d.child(div().small(t).text_color(t.colors.fg_subtle).truncate().child(m.provider_name.clone()))),
                )
        };
        let status = match &ping {
            Some(PingState::Pending) => Some(crate::ui::spinner(IconSize::Sm).into_any_element()),
            Some(PingState::Done(r)) => Some(
                div()
                    .id(SharedString::from(format!("{id}-st")))
                    .small(t)
                    .whitespace_nowrap()
                    .text_color(if r.ok { t.colors.success } else { t.colors.danger })
                    .tooltip(crate::ui::tooltip::text_tooltip(ping_label(i, r).into()))
                    .child(if r.ok { i.tp("pv.ping.ok", &[("ms", &r.latency_ms.to_string())]) } else { i.t(&format!("pv.ping.{}", r.kind)) })
                    .into_any_element(),
            ),
            None => None,
        };
        let (pp, pm) = (m.provider_id.clone(), m.model_id.clone());
        let r = ModelRef { provider_id: m.provider_id.clone(), model_id: m.model_id.clone() };
        div()
            .flex()
            .items_center()
            .gap(t.space(1))
            .child(pick)
            .children(status)
            .child(
                icon_button(SharedString::from(format!("{id}-ping")), "ping", i.tp("pv.ping", &[("name", &m.name)]))
                    .size(ControlSize::Sm)
                    .disabled(ping == Some(PingState::Pending))
                    .on_click(cx.listener(move |_, _, _, cx| PingStore::global(cx).update(cx, |p, cx| p.ping(&pp, &pm, cx)))),
            )
            .child(
                icon_button(SharedString::from(format!("{id}-pin")), "pin", i.tp(if pinned { "mp.unpin" } else { "mp.pin" }, &[("name", &m.name)]))
                    .size(ControlSize::Sm)
                    .pressed(pinned)
                    .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.toggle_model_pin(&r, cx)))),
            )
            .into_any_element()
    }

    fn picker_section(&self, t: &Theme, id: &str, title: String, count: usize, body: Vec<AnyElement>, cx: &mut Context<Self>) -> AnyElement {
        let open = !self.picker.folded.contains(id);
        let key = id.to_string();
        div()
            .flex()
            .flex_col()
            .gap(t.space(1))
            .child(
                div()
                    .id(SharedString::from(format!("msec-{id}")))
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .cursor_pointer()
                    .small(t)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.colors.fg_muted)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if !s.picker.folded.remove(&key) {
                            s.picker.folded.insert(key.clone());
                        }
                        cx.notify();
                    }))
                    .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                    .child(title)
                    .child(div().text_color(t.colors.fg_subtle).child(format!("({count})"))),
            )
            .when(open, |d| d.children(body))
            .into_any_element()
    }

    fn picker_models_tab(&self, t: &Theme, i: &I18n, visible: &[ModelOption], groups: &[(String, String, Vec<ModelOption>)], cx: &mut Context<Self>) -> AnyElement {
        let q = self.mp_search.read(cx).text().trim().to_lowercase();
        let find = |k: &String| visible.iter().find(|m| model_key(m) == *k).filter(|m| matches(m, &q)).cloned();
        let c = self.chat.read(cx);
        let recent: Vec<_> = c.recent_models.iter().filter_map(find).collect();
        let pinned: Vec<_> = c.pinned_models.iter().filter_map(find).collect();
        let shown: Vec<_> = groups
            .iter()
            .map(|(id, name, ms)| (id.clone(), name.clone(), ms.iter().filter(|m| matches(m, &q)).cloned().collect::<Vec<_>>()))
            .filter(|g| !g.2.is_empty())
            .collect();
        let mut page = div().flex().flex_col().gap(t.space(2));
        if recent.is_empty() && pinned.is_empty() && shown.is_empty() {
            page = page.child(div().small(t).text_color(t.colors.fg_muted).child(i.t("mp.none")));
        }
        if !recent.is_empty() {
            let rows = recent.iter().map(|m| self.model_row(t, i, m, true, "recent", cx)).collect();
            page = page.child(self.picker_section(t, "recent", i.t("mp.recent"), recent.len(), rows, cx));
        }
        if !pinned.is_empty() {
            let rows = pinned.iter().map(|m| self.model_row(t, i, m, true, "pin", cx)).collect();
            page = page.child(self.picker_section(t, "pinned", i.t("mp.pinned"), pinned.len(), rows, cx));
        }
        if !shown.is_empty() {
            let mut body = vec![];
            for (id, name, ms) in &shown {
                let is_open = !q.is_empty() || self.picker.expanded.contains(id);
                let gid = id.clone();
                body.push(
                    div()
                        .id(SharedString::from(format!("mgrp-{id}")))
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .px(t.space(1))
                        .py(t.space(1))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.colors.hover))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if !s.picker.expanded.remove(&gid) {
                                s.picker.expanded.insert(gid.clone());
                            }
                            cx.notify();
                        }))
                        .child(icon(if is_open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                        .child(model_logo(t, cx, Some(id), None, name, false, &format!("mgl-{id}")))
                        .child(div().flex_1().truncate().font_weight(FontWeight::MEDIUM).child(name.clone()))
                        .child(div().small(t).text_color(t.colors.fg_subtle).child(format!("({})", ms.len())))
                        .into_any_element(),
                );
                if is_open {
                    for m in ms {
                        body.push(div().pl(t.space(4)).child(self.model_row(t, i, m, false, "all", cx)).into_any_element());
                    }
                }
            }
            page = page.child(self.picker_section(t, "all", i.t("mp.all"), shown.len(), body, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .size_full()
            .child(self.mp_search.clone())
            .child(div().id("mp-scroll").flex_1().min_h_0().overflow_y_scroll().child(page))
            .into_any_element()
    }

    fn picker_ping_tab(&self, t: &Theme, i: &I18n, visible: &[ModelOption], cx: &mut Context<Self>) -> AnyElement {
        let pings = PingStore::global(cx).read(cx).pings.clone();
        let mut rows: Vec<(ModelOption, Option<PingState>)> =
            visible.iter().map(|m| (m.clone(), pings.get(&crate::core::pings::ping_key(&m.provider_id, &m.model_id)).cloned())).collect();
        let (mut ok, mut bad, mut pending, mut todo) = (0, 0, 0, 0);
        for (_, s) in &rows {
            match status_key(s.as_ref()).as_str() {
                "ok" => ok += 1,
                "pending" => pending += 1,
                "untested" => todo += 1,
                _ => bad += 1,
            }
        }
        sort_rows(&mut rows, &self.picker.sort);
        let all: Vec<(String, String)> = rows.iter().map(|(m, _)| (m.provider_id.clone(), m.model_id.clone())).collect();
        let run = |list: Vec<(String, String)>| {
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                let ps = PingStore::global(cx);
                ps.update(cx, |p, cx| {
                    for (a, b) in &list {
                        p.ping(a, b, cx);
                    }
                })
            }
        };
        let sorts = ["status", "name", "provider", "latency"]
            .iter()
            .map(|v| (SharedString::from(*v), SharedString::from(format!("{}: {}", i.t("mp.sort.short"), i.t(&format!("mp.sort.{v}"))))))
            .collect();
        let groups_sel = ["provider", "status", "none"]
            .iter()
            .map(|v| (SharedString::from(*v), SharedString::from(format!("{}: {}", i.t("mp.group.short"), i.t(&format!("mp.group.{v}"))))))
            .collect();
        let me = cx.entity().downgrade();
        let me2 = me.clone();
        let bar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(t.space(2))
            .child(
                crate::ui::button("mping-all")
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Solid)
                    .disabled(pending > 0 || all.is_empty())
                    .label(i.tp("mp.ping.all", &[("n", &all.len().to_string())]))
                    .on_click(run(all.clone())),
            )
            .child(crate::ui::select("mping-sort", sorts, self.picker.sort.clone()).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.picker.sort = v.to_string();
                    cx.notify();
                })
                .ok();
            }))
            .child(crate::ui::select("mping-group", groups_sel, self.picker.group.clone()).on_change(move |v, _, cx| {
                me2.update(cx, |s, cx| {
                    s.picker.group = v.to_string();
                    cx.notify();
                })
                .ok();
            }));
        let summary = i.tp("mp.ping.summary", &[("ok", &ok.to_string()), ("bad", &bad.to_string()), ("todo", &(todo + pending).to_string()), ("na", "0")]);
        let status_label = |k: &str| match k {
            "untested" => i.t("mp.st.untested"),
            "unsupported" => i.t("mp.st.unsupported"),
            "pending" => i.t("pv.pinging"),
            "ok" => i.t("mp.st.ok"),
            k => i.t(&format!("pv.ping.{k}")),
        };
        let group_by = self.picker.group.clone();
        let mut list = div().flex().flex_col().gap(t.space(2));
        for (gid, rs) in group_rows(rows, &group_by) {
            let title = match group_by.as_str() {
                "provider" => rs[0].0.provider_name.clone(),
                "status" => status_label(&gid),
                _ => String::new(),
            };
            let mut sec = div().flex().flex_col().gap(t.space(1));
            if group_by != "none" {
                let ids: Vec<(String, String)> = rs.iter().map(|(m, _)| (m.provider_id.clone(), m.model_id.clone())).collect();
                let label = if group_by == "provider" { i.tp("mp.ping.provider", &[("name", &title)]) } else { i.tp("mp.ping.group.run", &[("name", &title)]) };
                sec = sec.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .when(group_by == "provider", |d| d.child(model_logo(t, cx, Some(&gid), None, &title, false, &format!("mpg-{gid}"))))
                        .child(div().flex_1().truncate().font_weight(FontWeight::MEDIUM).child(title.clone()))
                        .child(div().small(t).text_color(t.colors.fg_subtle).child(format!("({})", rs.len())))
                        .child(
                            icon_button(SharedString::from(format!("mpg-run-{gid}")), "ping", label)
                                .size(ControlSize::Sm)
                                .disabled(rs.iter().any(|(_, s)| *s == Some(PingState::Pending)))
                                .on_click(run(ids)),
                        ),
                );
            }
            for (m, st) in &rs {
                let k = status_key(st.as_ref());
                let tone = match k.as_str() {
                    "ok" => t.colors.success,
                    "untested" | "pending" | "unsupported" => t.colors.fg_muted,
                    _ => t.colors.danger,
                };
                let meta = match st {
                    Some(PingState::Done(r)) => Some(
                        [
                            r.status.map(|s| format!("HTTP {s}")),
                            Some(i.tp("mp.ping.models", &[("n", &r.models.len().to_string())])),
                            r.error.clone(),
                        ]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · "),
                    ),
                    _ => None,
                };
                let rid = format!("mping-{}", model_key(m));
                let (pp, pm) = (m.provider_id.clone(), m.model_id.clone());
                let sub = if group_by == "provider" { m.model_id.clone() } else { format!("{} · {}", m.provider_name, m.model_id) };
                sec = sec.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .px(t.space(1))
                        .child(model_logo(t, cx, Some(&m.provider_id), Some(&m.model_id), &m.provider_name, false, &rid))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().child(m.name.clone()))
                                .child(div().small(t).text_color(t.colors.fg_subtle).truncate().child(sub))
                                .children(meta.map(|mt| {
                                    div()
                                        .id(SharedString::from(format!("{rid}-meta")))
                                        .small(t)
                                        .text_color(t.colors.fg_muted)
                                        .truncate()
                                        .tooltip(crate::ui::tooltip::text_tooltip(mt.clone().into()))
                                        .child(mt)
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(t.space(1))
                                .small(t)
                                .whitespace_nowrap()
                                .text_color(tone)
                                .child(if k == "pending" { crate::ui::spinner(IconSize::Sm).into_any_element() } else { status_label(&k).into_any_element() })
                                .children(match st {
                                    Some(PingState::Done(r)) => Some(div().text_color(t.colors.fg_subtle).child(i.tp("mp.ping.latency", &[("ms", &r.latency_ms.to_string())]))),
                                    _ => None,
                                }),
                        )
                        .child(
                            icon_button(SharedString::from(format!("{rid}-run")), "ping", i.tp("pv.ping", &[("name", &m.name)]))
                                .size(ControlSize::Sm)
                                .disabled(k == "pending")
                                .on_click(cx.listener(move |_, _, _, cx| PingStore::global(cx).update(cx, |p, cx| p.ping(&pp, &pm, cx)))),
                        ),
                );
            }
            list = list.child(sec);
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .size_full()
            .child(bar)
            .child(div().small(t).text_color(t.colors.fg_muted).child(summary))
            .child(div().small(t).text_color(t.colors.fg_subtle).child(i.t("mp.ping.cost")))
            .child(div().id("mping-scroll").flex_1().min_h_0().overflow_y_scroll().child(list))
            .into_any_element()
    }

    fn picker_provider_tab(&self, t: &Theme, i: &I18n, id: &str, name: &str, models: &[ModelOption], cx: &mut Context<Self>) -> AnyElement {
        let owned = crate::core::pings::owned_provider(id);
        let kind = i.t(if owned { "pv.kind.custom" } else { "pv.kind.builtin" });
        let sub = format!("{kind} · {}", i.tp("mp.provider.models", &[("n", &models.len().to_string())]));
        let ids: Vec<(String, String)> = models.iter().map(|m| (m.provider_id.clone(), m.model_id.clone())).collect();
        let mut list = div().flex().flex_col().gap(t.space(1));
        for m in models {
            list = list.child(self.model_row(t, i, m, false, "prov", cx));
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .size_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(model_logo(t, cx, Some(id), None, name, false, &format!("mph-{id}")))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(div().font_weight(FontWeight::SEMIBOLD).truncate().child(name.to_string()))
                            .child(div().small(t).text_color(t.colors.fg_subtle).truncate().child(sub)),
                    )
                    .child(
                        crate::ui::button(SharedString::from(format!("mph-ping-{id}")))
                            .size(ControlSize::Sm)
                            .child(icon("ping").size(IconSize::Sm))
                            .label(i.t("mp.provider.pingAll"))
                            .on_click(move |_, _, cx| {
                                PingStore::global(cx).update(cx, |p, cx| {
                                    for (a, b) in &ids {
                                        p.ping(a, b, cx);
                                    }
                                })
                            }),
                    ),
            )
            .child(div().id(SharedString::from(format!("mph-scroll-{id}"))).flex_1().min_h_0().overflow_y_scroll().child(list))
            .into_any_element()
    }

    /// Pemicu + popover pemilih model.
    pub(super) fn model_picker(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.models.is_empty() {
            return None;
        }
        let cur_ref = self.chat.read(cx).model.clone();
        let current = cur_ref.as_ref().and_then(|r| self.models.iter().find(|m| m.provider_id == r.provider_id && m.model_id == r.model_id)).cloned();
        let cur_name = current.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| i.t("chat.model"));
        let trigger = crate::ui::button("chat-model")
            .size(ControlSize::Sm)
            .variant(ButtonVariant::Ghost)
            .tooltip_text(i.tp("mp.open", &[("name", &cur_name)]))
            .child(model_logo(t, cx, current.as_ref().map(|m| m.provider_id.as_str()), current.as_ref().map(|m| m.model_id.as_str()), current.as_ref().map(|m| m.provider_name.as_str()).unwrap_or(""), false, "mp-trigger"))
            .child(div().max_w(t.rem(rem::SPACE[7] * 3.)).truncate().child(cur_name))
            .on_click(cx.listener(|s, _, w, cx| {
                if s.picker.open {
                    s.close_picker(cx);
                } else {
                    s.picker.open = true;
                    if let Some(r) = s.chat.read(cx).model.clone() {
                        s.picker.expanded.insert(r.provider_id);
                    }
                    let fh = gpui::Focusable::focus_handle(s.mp_search.read(cx), cx);
                    fh.focus(w, cx);
                    cx.notify();
                }
            }));
        let mut root = div().relative().child(trigger);
        if self.picker.open {
            let visible = self.visible_models(cx);
            let mut groups: Vec<(String, String, Vec<ModelOption>)> = vec![];
            for m in &visible {
                match groups.iter_mut().find(|g| g.0 == m.provider_id) {
                    Some(g) => g.2.push(m.clone()),
                    None => groups.push((m.provider_id.clone(), m.provider_name.clone(), vec![m.clone()])),
                }
            }
            groups.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
            let mut items = vec![
                crate::ui::TabItem { value: "models".into(), label: i.t("mp.tab.models").into(), icon: None, disabled: false },
                crate::ui::TabItem { value: "ping".into(), label: i.t("mp.tab.ping").into(), icon: Some("ping"), disabled: false },
            ];
            for (id, name, _) in &groups {
                items.push(crate::ui::TabItem { value: format!("p:{id}").into(), label: name.clone().into(), icon: None, disabled: false });
            }
            let tab = self.picker.tab.clone();
            let panel = if tab == "ping" {
                self.picker_ping_tab(t, i, &visible, cx)
            } else if let Some((id, name, ms)) = tab.strip_prefix("p:").and_then(|p| groups.iter().find(|g| g.0 == p)) {
                self.picker_provider_tab(t, i, id, name, ms, cx)
            } else {
                self.picker_models_tab(t, i, &visible, &groups, cx)
            };
            let me = cx.entity().downgrade();
            let me2 = me.clone();
            let me3 = me.clone();
            let tabs = crate::ui::tabs("mp-tabs", items, if tab.is_empty() { "models".to_string() } else { tab }).vertical(true).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.picker.tab = v.to_string();
                    cx.notify();
                })
                .ok();
            });
            let pop = crate::ui::popup(t)
                .w(t.rem(rem::SPACE[7] * 14.))
                .h(t.rem(rem::SPACE[7] * 11.))
                .p(t.space(2))
                .gap(t.space(2))
                .occlude()
                .on_mouse_down_out(move |_, _, cx| {
                    me2.update(cx, |s, cx| s.close_picker(cx)).ok();
                })
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .gap(t.space(2))
                        .child(div().id("mp-tablist").flex_none().w(t.rem(rem::SPACE[7] * 4.)).overflow_y_scroll().child(tabs))
                        .child(div().flex_1().min_w_0().min_h_0().child(panel)),
                )
                .child(
                    div().flex().justify_end().child(
                        crate::ui::button("mp-manage").size(ControlSize::Sm).label(i.t("mp.manage")).on_click(move |_, _, cx| {
                            me3.update(cx, |s, cx| {
                                s.close_picker(cx);
                                s.app.update(cx, |a, cx| a.push("/settings/models", cx));
                            })
                            .ok();
                        }),
                    ),
                );
            root = root.child(crate::ui::overlay_deferred(gpui::anchored().anchor(gpui::Anchor::BottomRight).snap_to_window_with_margin(gpui::px(8.)).child(div().mb(t.space(1)).child(pop))).with_priority(65));
        }
        Some(root.into_any_element())
    }
}

#[cfg(test)]
mod picker_tests {
    use super::*;
    use crate::netprobe::ProbeResult;
    fn m(p: &str, id: &str, name: &str) -> ModelOption {
        ModelOption { provider_id: p.into(), provider_name: p.to_uppercase(), model_id: id.into(), name: name.into(), variants: vec![], context_limit: None, output_limit: None, reasoning: None, tool_call: None, attachment: None, inputs: vec![] }
    }
    fn done(ok: bool, kind: &str, ms: u64) -> Option<PingState> {
        Some(PingState::Done(ProbeResult { ok, status: Some(200), latency_ms: ms, models: vec![], info: Default::default(), kind: kind.into(), error: None }))
    }
    #[test]
    fn urut_dan_kelompok() {
        let mut rows = vec![(m("b", "1", "Zeta"), None), (m("a", "2", "Alpha"), done(false, "auth", 5)), (m("a", "3", "Beta"), done(true, "ok", 9))];
        sort_rows(&mut rows, "status");
        assert_eq!(rows.iter().map(|r| r.0.name.as_str()).collect::<Vec<_>>(), ["Beta", "Alpha", "Zeta"]);
        sort_rows(&mut rows, "latency");
        assert_eq!(rows[0].0.name, "Alpha");
        let g = group_rows(rows.clone(), "provider");
        assert_eq!(g.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        let g = group_rows(rows, "status");
        assert_eq!(g.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["ok", "auth", "untested"]);
        assert!(matches(&m("x", "gpt-4o", "GPT"), "4o"));
    }
}
