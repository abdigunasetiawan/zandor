//! Panel 4 alat per sesi — port InfoPanel.tsx (info sesi + memori per sesi), SessionFilesPanel.tsx (berkas yang disentuh
//! agent, dilengkapi snapshot engine) dan SessionActivityPanel.tsx (garis waktu seluruh kejadian sesi).
use crate::core::chat::ChatState;
use crate::core::chat_store::ChatStore;
use crate::core::engine::{ModelOption, SessionFileDiff};
use crate::core::mem::{self, memory_enabled, parse_settings, serialize_settings, MemorySettings};
use crate::core::projects::{session_title, ProjectStore};
use crate::core::services::app_data_dir;
use crate::core::session_timeline::{build_timeline, merge_snapshot_files, relative_to, touched_files, Entry, Kind};
use crate::core::types::{Role, SessionRunStatus, TodoStatus};
use crate::core::usage::session_usage;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, select, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, Context, IntoElement, SharedString, Window};
use std::collections::HashSet;

/// Sesi terpilih: (id, judul, folder kerja, nama project, path project, state chat).
struct Sel {
    id: String,
    title: Option<String>,
    dir: String,
    project_name: Option<String>,
    project_path: Option<String>,
    chat: ChatState,
}

fn selected(cx: &App) -> Option<Sel> {
    let st = ProjectStore::global(cx).read(cx);
    let (_, s, p) = st.selected()?;
    let chat = ChatStore::global(cx).read(cx).sessions.get(&s.id).cloned().unwrap_or_else(|| ChatState::new(s.id.clone()));
    Some(Sel { title: session_title(&s).map(str::to_string), dir: s.directory.clone(), project_name: p.as_ref().map(|p| p.name.clone()), project_path: p.map(|p| p.path), id: s.id, chat })
}

fn notice(t: &Theme, ic: &'static str, text: String) -> AnyElement {
    div().flex().flex_col().items_center().gap(t.space(2)).p(t.space(6)).text_color(t.colors.fg_muted).child(icon(ic).size(IconSize::Lg)).child(div().text_center().child(text)).into_any_element()
}
fn inline_note(t: &Theme, text: String) -> AnyElement {
    div().px(t.space(3)).py(t.space(2)).small(t).text_color(t.colors.fg_muted).child(text).into_any_element()
}
fn busy(c: &ChatState) -> bool {
    c.status != SessionRunStatus::Idle
}

// ---------------------------------------------------------------- Info

fn read_mem(rel: &str) -> String {
    crate::memory::read_memory(&app_data_dir().join("memory"), rel).unwrap_or_default()
}

fn duration(ms: f64) -> String {
    let s = (ms / 1000.).round() as i64;
    if s < 60 {
        return format!("{s} dtk");
    }
    format!("{} mnt {} dtk", s / 60, s % 60)
}

fn nf(n: f64) -> String {
    let s = (n.round() as i64).to_string();
    let mut out = String::new();
    for (k, c) in s.chars().enumerate() {
        if k > 0 && (s.len() - k) % 3 == 0 && c != '-' {
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub struct InfoPanel {
    models: Vec<ModelOption>,
    settings: MemorySettings,
}

impl InfoPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&ProjectStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&ChatStore::global(cx), |_, _, cx| cx.notify()).detach();
        if let Some(api) = ProjectStore::global(cx).read(cx).api() {
            let task = cx.background_spawn(async move { api.list_models().unwrap_or_default() });
            cx.spawn(async move |this, cx| {
                let m = task.await;
                this.update(cx, |s, cx| {
                    s.models = m;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        Self { models: vec![], settings: parse_settings(&read_mem(mem::FILE_SETTINGS)) }
    }

    fn set_session_memory(&mut self, sid: String, v: Option<bool>) {
        let mut cur = parse_settings(&read_mem(mem::FILE_SETTINGS));
        match v {
            Some(b) => cur.sessions.insert(sid, b),
            None => cur.sessions.remove(&sid),
        };
        let dir = app_data_dir().join("memory");
        let _ = std::fs::create_dir_all(&dir);
        if crate::memory::write_memory(&dir, mem::FILE_SETTINGS, &serialize_settings(&cur)).is_ok() {
            self.settings = cur;
        }
    }
}

impl Render for InfoPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let Some(sel) = selected(cx) else { return notice(&t, "info", i.t("info.empty")) };
        let chat = ChatStore::global(cx).read(cx);
        let (cur_model, mode) = (chat.model.clone(), chat.permission_mode.clone());
        let s = &sel.chat;
        let model = self.models.iter().find(|m| cur_model.as_ref().is_some_and(|c| c.provider_id == m.provider_id && c.model_id == m.model_id));
        let last = s.messages.iter().rev().find(|m| m.info.role == Role::Assistant && m.info.model_id.is_some());
        let used = self.models.iter().find(|m| last.is_some_and(|l| l.info.provider_id.as_deref() == Some(m.provider_id.as_str()) && l.info.model_id.as_deref() == Some(m.model_id.as_str()))).or(model);
        let u = session_usage(&s.messages, used.and_then(|m| m.context_limit).map(|v| v as f64), None);
        let sending = false;

        let mut rows: Vec<(String, AnyElement)> = vec![];
        let txt = |s: String| div().child(s).into_any_element();
        let waiting = s.permissions.len() + s.questions.len();
        let status = i.t(if sending || s.status == SessionRunStatus::Busy {
            "info.status.busy"
        } else if matches!(s.status, SessionRunStatus::Retry { .. }) {
            "info.status.retry"
        } else {
            "info.status.idle"
        });
        rows.push((i.t("info.status"), txt(if waiting > 0 { format!("{status} · {}", i.tp("info.waiting", &[("n", &waiting.to_string())])) } else { status })));
        rows.push((i.t("info.project"), txt(sel.project_name.clone().unwrap_or_else(|| i.t("chat.noProject")))));
        rows.push((i.t("info.dir"), div().code(&t).child(sel.dir.clone()).into_any_element()));
        let model_txt = match last {
            Some(l) => format!("{} ({})", l.info.model_id.clone().unwrap_or_default(), l.info.provider_id.clone().unwrap_or_default()),
            None => used.map(|m| m.name.clone()).unwrap_or_else(|| "—".into()),
        };
        rows.push((i.t("info.model"), txt(model_txt)));
        rows.push((i.t("info.messages"), txt(i.tp("info.messagesValue", &[("user", &u.user_messages.to_string()), ("assistant", &u.assistant_messages.to_string())]))));
        let ctx = match (u.context_tokens, u.context_limit) {
            (None, _) => "—".to_string(),
            (Some(c), Some(l)) => format!("{} / {} ({:.1}%)", nf(c), nf(l), u.context_percent.unwrap_or(0.)),
            (Some(c), None) => format!("{} ({})", nf(c), i.t("info.contextUnknown")),
        };
        let mut ctx_el = div().flex().flex_col().gap(t.space(1)).child(ctx);
        if let Some(p) = u.context_percent {
            ctx_el = ctx_el.child(div().h(t.rem(0.375)).w_full().rounded_full().bg(t.colors.raised).child(div().h_full().rounded_full().bg(if p >= 90. { t.colors.danger } else if p >= 70. { t.colors.warning } else { t.colors.accent }).w(gpui::relative((p / 100.) as f32))));
        }
        rows.push((i.t("info.context"), ctx_el.into_any_element()));
        rows.push((i.t("info.tokens"), txt(i.tp("info.tokensValue", &[("input", &nf(u.input)), ("output", &nf(u.output)), ("reasoning", &nf(u.reasoning))]))));
        rows.push((i.t("info.cost"), txt(u.cost.map(|c| format!("${c:.4}")).unwrap_or_else(|| i.t("info.costNone")))));
        rows.push((
            i.t("info.tools"),
            txt(if u.tool_total == 0 {
                "—".into()
            } else {
                format!("{} ({})", u.tool_total, i.tp("info.toolsValue", &[("ok", &u.tool_completed.to_string()), ("failed", &u.tool_failed.to_string()), ("running", &u.tool_running.to_string())]))
            }),
        ));
        let mut by: Vec<(String, usize)> = vec![];
        for m in &s.messages {
            for p in &m.parts {
                if let crate::core::types::PartKind::Tool { tool, .. } = &p.kind {
                    match by.iter_mut().find(|(k, _)| k == tool) {
                        Some(e) => e.1 += 1,
                        None => by.push((tool.clone(), 1)),
                    }
                }
            }
        }
        if !by.is_empty() {
            by.sort_by(|a, b| b.1.cmp(&a.1));
            rows.push((i.t("info.byTool"), txt(by.iter().map(|(k, n)| format!("{k} ×{n}")).collect::<Vec<_>>().join(", "))));
        }
        rows.push((i.t("info.work"), txt(duration(u.work_ms))));
        rows.push((i.t("info.files"), txt(touched_files(&s.messages).len().to_string())));
        if !s.todos.is_empty() {
            let done = s.todos.iter().filter(|x| x.status == TodoStatus::Completed).count();
            rows.push((i.t("info.plan"), txt(i.tp("todo.title", &[("done", &done.to_string()), ("total", &s.todos.len().to_string())]))));
        }
        rows.push((i.t("info.permissionMode"), txt(i.t(&format!("chat.mode.{mode}")))));
        if let Some(first) = s.messages.first().map(|m| m.info.created).filter(|c| *c > 0.) {
            let dt = chrono::DateTime::from_timestamp_millis(first as i64).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y %H.%M.%S").to_string()).unwrap_or_default();
            rows.push((i.t("info.first"), txt(dt)));
        }
        rows.push((i.t("info.session"), div().code(&t).child(sel.id.clone()).into_any_element()));

        let mut grid = div().flex().flex_col().gap(t.space(2));
        for (k, v) in rows {
            grid = grid.child(div().flex().flex_col().child(div().small(&t).text_color(t.colors.fg_subtle).child(k)).child(v));
        }

        // memori sesi
        let key = sel.project_path.as_deref().map(mem::project_key);
        let own = self.settings.sessions.get(&sel.id).copied();
        let effective = memory_enabled(&self.settings, key.as_deref(), Some(&sel.id));
        let state = i.t(if effective { "mem.session.stateOn" } else { "mem.session.stateOff" });
        let mut memory = div().flex().flex_col().gap(t.space(2)).mt(t.space(3)).child(div().small(&t).font_weight(gpui::FontWeight::MEDIUM).child(i.t("mem.session.title")));
        if key.is_some() {
            memory = memory.child(div().small(&t).child(i.tp("mem.session.projectShared", &[("state", &state)])));
        } else {
            let me = cx.entity().downgrade();
            let sid = sel.id.clone();
            let value = match own {
                Some(true) => "on",
                Some(false) => "off",
                None => "follow",
            };
            memory = memory
                .child(select("info-mem", vec![("follow".into(), i.t("mem.session.follow").into()), ("on".into(), i.t("mem.session.on").into()), ("off".into(), i.t("mem.session.off").into())], value).on_change(move |v, _, cx| {
                    let b = match v.as_ref() {
                        "on" => Some(true),
                        "off" => Some(false),
                        _ => None,
                    };
                    let sid = sid.clone();
                    me.update(cx, |s, cx| {
                        s.set_session_memory(sid, b);
                        cx.notify();
                    })
                    .ok();
                }))
                .child(div().small(&t).child(i.tp("mem.session.effective", &[("state", &state)])));
        }

        div()
            .id("infop")
            .size_full()
            .overflow_y_scroll()
            .child(div().flex().flex_col().gap(t.space(3)).p(t.space(3)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(sel.title.clone().unwrap_or_else(|| i.t("sb.untitled")))).child(grid).child(memory))
            .into_any_element()
    }
}

// ---------------------------------------------------------------- Berkas sesi

pub struct SessionFiles {
    /// Snapshot engine: (session id, diff).
    snap: Option<(String, Vec<SessionFileDiff>)>,
    /// Pemicu muat ulang terakhir (sesi, jumlah pesan, sibuk).
    trigger: Option<(String, usize, bool)>,
}

impl SessionFiles {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&ProjectStore::global(cx), |s, _, cx| s.sync(cx)).detach();
        cx.observe(&ChatStore::global(cx), |s, _, cx| s.sync(cx)).detach();
        let mut me = Self { snap: None, trigger: None };
        me.sync(cx);
        me
    }

    /// Snapshot engine menangkap berkas yang dibuat lewat bash; dimuat ulang tiap giliran selesai.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = selected(cx) else { return };
        let b = busy(&sel.chat) || false;
        let trig = (sel.id.clone(), sel.chat.messages.len(), b);
        if self.trigger.as_ref() == Some(&trig) {
            return;
        }
        self.trigger = Some(trig);
        cx.notify();
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        if b {
            return;
        }
        let (dir, id) = (sel.dir.clone(), sel.id.clone());
        let task = cx.background_spawn(async move { api.session_diff(&dir, &id).unwrap_or_default() });
        let id = sel.id.clone();
        cx.spawn(async move |this, cx| {
            let d = task.await;
            this.update(cx, |s, cx| {
                s.snap = Some((id, d));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for SessionFiles {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let Some(sel) = selected(cx) else { return notice(&t, "files", i.t("info.empty")) };
        let touched = touched_files(&sel.chat.messages);
        let files = match &self.snap {
            Some((id, diffs)) if *id == sel.id => {
                let last_at = sel.chat.messages.last().map(|m| m.info.created).unwrap_or(0.);
                merge_snapshot_files(touched, diffs, &sel.dir, last_at)
            }
            _ => touched,
        };
        let mut col = div().flex().flex_col().p(t.space(2));
        if files.is_empty() {
            col = col.child(inline_note(&t, i.t("sfil.empty")));
        }
        for f in files {
            let name = relative_to(&sel.dir, &f.path);
            let base = name.rsplit('/').next().unwrap_or(&name).to_string();
            let parent = (name != base).then(|| name[..name.len() - base.len() - 1].to_string());
            let p = f.path.clone();
            let mut ops = div().flex().flex_none().items_center().gap(t.space(1));
            for op in &f.ops {
                let color = match *op {
                    "read" => t.colors.fg_muted,
                    "write" => t.colors.code_add,
                    _ => t.colors.warning,
                };
                ops = ops.child(div().small(&t).font_weight(gpui::FontWeight::SEMIBOLD).text_color(color).child(i.t(&format!("sfil.op.{op}"))));
            }
            if f.count > 1 {
                ops = ops.child(div().small(&t).text_color(t.colors.fg_subtle).child(format!("×{}", f.count)));
            }
            if f.failed {
                ops = ops.child(div().small(&t).text_color(t.colors.danger).child(i.t("sfil.failed")));
            }
            col = col.child(
                div()
                    .id(SharedString::from(format!("sfil-{}", f.path)))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .px(t.space(1))
                    .py(t.space(1))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.hover))
                    .tooltip(crate::ui::tooltip::text_tooltip(f.path.clone().into()))
                    .on_click(move |_, _, cx| super::docs_panel::DocsState::global(cx).update(cx, |d, cx| d.open_file(p.clone(), cx)))
                    .child(super::file_icons::file_type_icon(&t, &base, false, false, rem::ICON_SM))
                    .child(div().flex().flex_col().flex_1().min_w_0().child(div().truncate().child(base.clone())).children(parent.map(|d| div().small(&t).text_color(t.colors.fg_subtle).truncate().child(d))))
                    .child(ops),
            );
        }
        div().id("sfil").size_full().overflow_y_scroll().child(col).into_any_element()
    }
}

// ---------------------------------------------------------------- Aktivitas sesi

const FILTERS: [&str; 5] = ["all", "user", "agent", "tool", "error"];

fn matches(e: &Entry, f: &str) -> bool {
    match f {
        "user" => e.kind == Kind::User,
        "agent" => matches!(e.kind, Kind::Reply | Kind::Thinking),
        "tool" => e.kind == Kind::Tool,
        "error" => e.kind == Kind::Error || matches!(e.status, Some("error") | Some("pending")),
        _ => true,
    }
}

fn fmt_duration(ms: f64) -> String {
    if ms < 1000. { format!("{} ms", ms.round() as i64) } else { format!("{:.1} dtk", ms / 1000.) }
}

pub struct SessionActivity {
    filter: SharedString,
    open: HashSet<String>,
}

impl SessionActivity {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&ProjectStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&ChatStore::global(cx), |_, _, cx| cx.notify()).detach();
        Self { filter: "all".into(), open: HashSet::new() }
    }
}

impl Render for SessionActivity {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let Some(sel) = selected(cx) else { return notice(&t, "activity", i.t("info.empty")) };
        let timeline = build_timeline(&sel.chat.messages, &sel.chat.permissions, &sel.chat.questions);
        let shown: Vec<Entry> = timeline.into_iter().filter(|e| matches(e, &self.filter)).collect();
        let me = cx.entity().downgrade();
        let bar = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .px(t.space(3))
            .py(t.space(2))
            .child(select("sact-filter", FILTERS.iter().map(|f| (SharedString::from(*f), SharedString::from(i.t(&format!("sact.f.{f}"))))).collect(), self.filter.clone()).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.filter = v;
                    cx.notify();
                })
                .ok();
            }))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.tp("act.count", &[("count", &shown.len().to_string())])));
        let mut list = div().flex().flex_col().px(t.space(2)).pb(t.space(2));
        if shown.is_empty() {
            list = list.child(inline_note(&t, i.t("sact.empty")));
        }
        for e in shown {
            let expanded = self.open.contains(&e.id);
            let label = e.step.as_ref().map(|s| i.t(s.key)).unwrap_or_else(|| i.t(&format!("sact.kind.{}", e.kind.key())));
            let time = if e.at > 0. { chrono::DateTime::from_timestamp_millis(e.at as i64).map(|d| d.with_timezone(&chrono::Local).format("%H.%M.%S").to_string()).unwrap_or_default() } else { "—".into() };
            let status_color = match e.status {
                Some("error") => t.colors.danger,
                Some("pending") => t.colors.warning,
                _ => t.colors.fg_muted,
            };
            let mut actions = div().flex().gap(t.space(1));
            if e.detail.is_some() {
                let id = e.id.clone();
                actions = actions.child(button(SharedString::from(format!("sact-d-{}", e.id))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t(if expanded { "sact.hide" } else { "sact.detail" })).on_click(cx.listener(move |s, _, _, cx| {
                    if !s.open.remove(&id) {
                        s.open.insert(id.clone());
                    }
                    cx.notify();
                })));
            }
            if let Some(mid) = e.message_id.clone() {
                actions = actions.child(button(SharedString::from(format!("sact-j-{}", e.id))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("sact.jump")).on_click(move |_, _, cx| cx.set_global(super::chat_panel::MessageFocus(Some(mid.clone())))));
            }
            list = list.child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .py(t.space(2))
                    .border_b_1()
                    .border_color(t.colors.border_soft)
                    .child(div().flex_none().w(t.rem(4.5)).small(&t).text_color(t.colors.fg_subtle).child(time))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(t.space(1))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(t.space(2))
                                    .child(div().font_weight(gpui::FontWeight::MEDIUM).child(label))
                                    .when_some(e.status.filter(|s| *s != "completed"), |d, s| d.child(div().small(&t).text_color(status_color).child(i.t(&format!("sact.status.{s}")))))
                                    .when_some(e.duration_ms, |d, ms| d.child(div().small(&t).text_color(t.colors.fg_subtle).child(fmt_duration(ms)))),
                            )
                            .child(div().text_color(t.colors.fg_muted).child(e.title.clone()))
                            .when(expanded, |d| d.when_some(e.detail.clone(), |d, x| d.child(div().p(t.space(2)).rounded(t.rem(rem::RADIUS_SM)).bg(t.colors.raised).code(&t).child(x))))
                            .child(actions),
                    ),
            );
        }
        div().id("sact").size_full().overflow_y_scroll().child(div().flex().flex_col().child(bar).child(list)).into_any_element()
    }
}
