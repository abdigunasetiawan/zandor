//! Port ToolStep.tsx (ToolCard, DiffBlock, OutputPre, FilePath, Changes, Status, Body) dan AssistantMessage.tsx
//! (ProcessBlock, Tally, baris model). Logika data di core/tool_view.rs.
use super::*;
use crate::core::tool_view::{self as tv, DiffRow, RowKind, ToolKind, ToolView};
use crate::ui::feedback::{WorkingSize, WorkingVariant};

/// Batas baris yang dirender per blok (MAX_ROWS).
const MAX_ROWS: usize = 300;

fn mix(c: gpui::Hsla, a: f32) -> gpui::Hsla {
    gpui::Hsla { a: c.a * a, ..c }
}

/// .fold__pre: blok monospasi, tinggi maks 16rem dengan gulir.
fn pre(t: &Theme, id: SharedString) -> gpui::Stateful<gpui::Div> {
    div().id(id).max_h(t.rem(16.)).overflow_y_scroll().my(t.space(1)).code(t).text_color(t.colors.fg)
}

/// DiffBlock: baris bernomor (tambah/hapus/konteks/hunk).
pub(crate) fn diff_block(t: &Theme, i: &I18n, id: impl Into<SharedString>, rows: &[DiffRow]) -> AnyElement {
    let mut col = div().flex().flex_col().rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.bg);
    let line = |kind: RowKind, no: Option<u32>, text: String| {
        let (fg, bg) = match kind {
            RowKind::Add => (t.colors.success, Some(mix(t.colors.success, 0.12))),
            RowKind::Del => (t.colors.danger, Some(mix(t.colors.danger, 0.12))),
            RowKind::Hunk => (t.colors.fg_subtle, Some(t.colors.hover)),
            RowKind::Ctx => (t.colors.fg_muted, None),
        };
        div()
            .flex()
            .gap(t.space(2))
            .text_color(fg)
            .when_some(bg, |d, b| d.bg(b))
            .child(div().flex_none().w(t.rem(2.4)).pl(t.space(1)).text_color(t.colors.fg_subtle).text_right().child(no.map(|n| n.to_string()).unwrap_or_default()))
            .child(div().flex_1().min_w_0().child(text))
    };
    for r in rows.iter().take(MAX_ROWS) {
        let no = if r.kind == RowKind::Del { r.old_no } else { r.new_no.or(r.old_no) };
        col = col.child(line(r.kind, no, r.text.clone()));
    }
    if rows.len() > MAX_ROWS {
        col = col.child(line(RowKind::Hunk, None, i.tp("tool.more", &[("n", &(rows.len() - MAX_ROWS).to_string())])));
    }
    pre(t, id.into()).child(col).into_any_element()
}

/// OutputPre: teks apa adanya, dipotong per baris.
fn output_pre(t: &Theme, i: &I18n, id: SharedString, text: &str) -> AnyElement {
    let norm = text.replace("\r\n", "\n");
    let lines: Vec<&str> = norm.split('\n').collect();
    let mut body = lines.iter().take(MAX_ROWS).copied().collect::<Vec<_>>().join("\n");
    if lines.len() > MAX_ROWS {
        body.push('\n');
        body.push_str(&i.tp("tool.more", &[("n", &(lines.len() - MAX_ROWS).to_string())]));
    }
    pre(t, id).p(t.space(2)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.bg).child(body).into_any_element()
}

fn file_path(t: &Theme, p: &tv::SplitPath) -> gpui::Div {
    div()
        .flex()
        .min_w_0()
        .font_family(t.font_code.clone())
        .when(!p.dir.is_empty(), |d| d.child(div().flex_none().text_color(t.colors.fg_subtle).child(p.dir.clone())))
        .child(div().truncate().child(p.name.clone()))
}

fn changes(t: &Theme, adds: Option<u32>, dels: Option<u32>) -> Option<gpui::Div> {
    if adds.is_none() && dels.is_none() {
        return None;
    }
    Some(
        div()
            .flex()
            .gap(t.space(1))
            .ml(t.space(1))
            .font_family(t.font_code.clone())
            .when(adds.unwrap_or(0) > 0, |d| d.child(div().text_color(t.colors.success).child(format!("+{}", adds.unwrap_or(0)))))
            .when(dels.unwrap_or(0) > 0, |d| d.child(div().text_color(t.colors.danger).child(format!("−{}", dels.unwrap_or(0))))),
    )
}

fn chip(t: &Theme, text: String, bad: bool) -> gpui::Div {
    div()
        .px(t.space(2))
        .rounded_full()
        .bg(t.colors.hover)
        .text_color(if bad { t.colors.danger } else { t.colors.fg_muted })
        .whitespace_nowrap()
        .child(text)
}

pub(super) fn fmt_secs(i: &I18n, s: u64) -> String {
    if s >= 60 { i.tp("time.minsec", &[("m", &(s / 60).to_string()), ("s", &(s % 60).to_string())]) } else { i.tp("time.sec", &[("s", &s.to_string())]) }
}

/// useFinished: jam selesai + durasi.
fn finished(i: &I18n, st: &ToolState) -> Option<(String, String)> {
    let (start, end) = st.time?;
    let end = end?;
    if start <= 0. {
        return None;
    }
    let ms = (end - start).max(0.);
    let s = (ms / 1000.).round() as u64;
    let dur = if ms < 1000. { i.tp("time.ms", &[("ms", &(ms as u64).to_string())]) } else { fmt_secs(i, s) };
    use chrono::TimeZone;
    let at = chrono::Local.timestamp_millis_opt(end as i64).single().map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default();
    Some((i.tp("tool.finished", &[("at", &at), ("dur", &dur)]), i.tp("tool.finishedTitle", &[("at", &at), ("dur", &dur)])))
}

fn status_el(t: &Theme, i: &I18n, pid: &str, s: ToolStatus) -> AnyElement {
    let label = i.t(match s {
        ToolStatus::Pending => "chat.tool.pending",
        ToolStatus::Running => "chat.tool.running",
        ToolStatus::Completed => "chat.tool.completed",
        ToolStatus::Error => "chat.tool.error",
    });
    match s {
        ToolStatus::Error => badge(label).tone(BadgeTone::Danger).into_any_element(),
        ToolStatus::Completed => div().flex_none().text_color(t.colors.fg_subtle).child(icon("check").size(IconSize::Sm)).into_any_element(),
        _ => working(SharedString::from(format!("tw-{pid}"))).variant(WorkingVariant::Grid).size(WorkingSize::Sm).into_any_element(),
    }
}

fn label(t: &Theme, s: String) -> gpui::Div {
    div().small(t).text_color(t.colors.fg_subtle).child(s)
}

fn obj_str(o: &Obj, k: &str) -> String {
    o.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn body(t: &Theme, i: &I18n, pid: &str, tool: &str, st: &ToolState, v: &ToolView) -> AnyElement {
    let out = if st.status == ToolStatus::Error { String::new() } else { st.output.clone().unwrap_or_default() };
    let input = &st.input;
    let err = st.error.clone().unwrap_or_default();
    let id = |k: &str| SharedString::from(format!("tb-{pid}-{k}"));
    let json = || serde_json::to_string_pretty(&serde_json::Value::Object(input.clone())).unwrap_or_default();
    let mut col = div().flex().flex_col().gap(t.space(1)).p(t.space(2)).border_t_1().border_color(t.colors.border).text_color(t.colors.fg_muted);
    if v.kind == ToolKind::Write && st.metadata.as_ref().and_then(|m| m.get("exists")).and_then(|x| x.as_bool()) == Some(false) {
        col = col.child(label(t, i.t("tool.newFile")));
    }
    match v.kind {
        ToolKind::Edit => {
            col = col.child(match &v.diff {
                Some(d) => diff_block(t, i, id("d"), &tv::parse_diff(d)),
                None => output_pre(t, i, id("o"), &json()),
            })
        }
        ToolKind::Write => {
            let c = obj_str(input, "content");
            if !c.is_empty() {
                col = col.child(diff_block(t, i, id("d"), &tv::content_rows(&c)));
            }
        }
        ToolKind::Read => {
            let (lines, note) = tv::read_lines(&out);
            if !lines.is_empty() {
                col = col.child(diff_block(t, i, id("d"), &tv::as_rows(&lines)));
                if !note.is_empty() {
                    col = col.child(label(t, note));
                }
            } else if !out.is_empty() {
                col = col.child(output_pre(t, i, id("o"), &out));
            }
        }
        ToolKind::Shell => {
            let meta_out = st.metadata.as_ref().map(|m| obj_str(m, "output")).unwrap_or_default();
            let output = if meta_out.is_empty() { out.clone() } else { meta_out };
            col = col.child(label(t, i.t("tool.command"))).child(
                pre(t, id("c"))
                    .p(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .bg(t.colors.bg)
                    .child(div().flex().child(div().flex_none().text_color(t.colors.fg_subtle).child("$ ")).child(div().flex_1().min_w_0().child(v.command.clone().unwrap_or_else(|| v.target.clone())))),
            );
            col = if output.trim().is_empty() {
                col.child(label(t, i.t("tool.noOutput")))
            } else {
                col.child(output_pre(t, i, id("o"), output.strip_suffix('\n').unwrap_or(&output)))
            };
        }
        ToolKind::Search => {
            let groups = tv::grep_groups(&out);
            if tool == "grep" && !groups.is_empty() {
                for (gi, g) in groups.iter().enumerate() {
                    col = col.child(div().flex().flex_col().gap(t.space(1)).child(file_path(t, &tv::split_path(&g.file))).child(diff_block(t, i, id(&format!("g{gi}")), &tv::as_rows(&g.lines))));
                }
            } else if !out.is_empty() {
                for l in out.split('\n').filter(|l| !l.trim().is_empty()).take(MAX_ROWS) {
                    col = col.child(file_path(t, &tv::split_path(l)));
                }
            }
        }
        ToolKind::Task => {
            let result = tv::task_result(&out);
            if let Some(a) = &v.agent {
                col = col.child(label(t, i.tp("tool.agent", &[("name", a)])));
            }
            let prompt = obj_str(input, "prompt");
            if !prompt.is_empty() {
                col = col.child(label(t, i.t("tool.task"))).child(output_pre(t, i, id("p"), &prompt));
            }
            if !result.is_empty() {
                col = col.child(label(t, i.t("tool.result"))).child(output_pre(t, i, id("r"), &result));
            }
            if let Some(c) = &v.child_session {
                col = col.child(label(t, i.tp("tool.child", &[("id", c)])));
            }
        }
        ToolKind::Plan if v.todos.is_some() => {
            for x in v.todos.iter().flatten() {
                let done = x.status == "completed" || x.status == "cancelled";
                col = col.child(div().when(done, |d| d.text_color(t.colors.fg_muted).line_through()).child(x.content.clone()));
            }
        }
        ToolKind::Web => {
            col = col.child(label(t, v.target.clone()));
            if !out.is_empty() {
                col = col.child(output_pre(t, i, id("o"), &out));
            }
        }
        _ => {
            col = col.child(label(t, i.t("chat.input"))).child(output_pre(t, i, id("i"), &json()));
            if !out.is_empty() {
                col = col.child(label(t, i.t("chat.output"))).child(output_pre(t, i, id("o"), &out));
            }
        }
    }
    if !err.is_empty() {
        col = col.child(div().small(t).text_color(t.colors.danger).child(err));
    }
    col.into_any_element()
}

/// ToolCard.
pub(super) fn tool_card(t: &Theme, i: &I18n, me: &WeakEntity<ChatPanel>, p: &MessagePart, tool: &str, st: &ToolState, open: bool) -> AnyElement {
    let v = tv::describe_tool(tool, st);
    let web = tool == "websearch" || tool == "codesearch";
    let file = if matches!(v.kind, ToolKind::Read | ToolKind::Edit | ToolKind::Write) { v.path.as_ref().map(|x| x.full.clone()).unwrap_or_default() } else { String::new() };
    let range = v.range.map(|(f, to, total)| match total {
        Some(tt) => i.tp("tool.linesOf", &[("from", &f.to_string()), ("to", &to.to_string()), ("total", &tt.to_string())]),
        None => i.tp("tool.lines", &[("from", &f.to_string()), ("to", &to.to_string())]),
    });
    let failed_exit = v.exit.is_some_and(|e| e != 0);
    let fin = finished(i, st);
    let pid = p.id.clone();
    let me2 = me.clone();
    let target: AnyElement = match &v.path {
        Some(sp) => file_path(t, sp).into_any_element(),
        None => div().truncate().child(v.target.clone()).into_any_element(),
    };
    let mut meta = div().flex().flex_none().items_center().gap(t.space(2)).font_family(t.font_code.clone());
    if let Some(r) = range {
        meta = meta.child(chip(t, r, false));
    }
    if let Some(c) = changes(t, v.adds, v.dels) {
        meta = meta.child(c);
    }
    if let Some(c) = v.count {
        if v.kind == ToolKind::Search {
            meta = meta.child(chip(t, i.tp("tool.matches", &[("n", &c.to_string())]), false));
        } else if v.kind == ToolKind::Plan {
            meta = meta.child(chip(t, i.tp("tool.items", &[("n", &c.to_string())]), false));
        }
    }
    if let Some(a) = &v.agent {
        meta = meta.child(chip(t, a.clone(), false));
    }
    if failed_exit {
        meta = meta.child(chip(t, i.tp("tool.exit", &[("code", &v.exit.unwrap_or(0).to_string())]), true));
    }
    if let Some((txt, title)) = fin {
        meta = meta.child(
            div()
                .id(SharedString::from(format!("tf-{}", p.id)))
                .text_color(t.colors.fg_subtle)
                .tooltip(crate::ui::tooltip::text_tooltip(title.into()))
                .child(txt),
        );
    }
    let head = div()
        .id(SharedString::from(format!("ts-{}", p.id)))
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
        .on_click(move |_, _, cx| {
            let pid = pid.clone();
            me2.update(cx, |s, cx| {
                if !s.expanded.remove(&pid) {
                    s.expanded.insert(pid);
                }
                cx.notify();
            })
            .ok();
        })
        .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
        .child(div().flex_none().text_color(t.colors.fg_muted).child(icon(tv::kind_icon(v.kind)).size(IconSize::Sm)))
        .child(div().flex_none().font_family(t.font_code.clone()).text_color(t.colors.fg_muted).child(if web { i.t("chat.webSearch") } else { tool.to_string() }))
        .child(div().flex_1().min_w_0().small(t).text_color(t.colors.fg).overflow_hidden().child(target))
        .child(meta.small(t))
        .child(status_el(t, i, &p.id, st.status));
    let mut row = div().flex().items_center().gap(t.space(1)).min_w_0().child(head);
    if !file.is_empty() {
        let name = file.rsplit(['\\', '/']).next().unwrap_or(&file).to_string();
        let f = file.clone();
        row = row.child(
            icon_button(SharedString::from(format!("to-{}", p.id)), "external_link", i.tp("doc.open", &[("name", &name)]))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, _, cx| {
                    if std::path::Path::new(&f).is_file() {
                        crate::shell::docs_panel::DocsState::global(cx).update(cx, |d, cx| d.open_file(f.clone(), cx));
                    } else {
                        let title = cx.global::<I18n>().t("doc.outside");
                        crate::ui::Toasts::show(crate::ui::ToastInput { title: title.into(), description: None, tone: crate::ui::ToastTone::Warning, timeout_ms: None, action: None }, cx);
                    }
                }),
        );
    }
    if !file.is_empty() && v.kind == ToolKind::Edit && st.status == ToolStatus::Completed {
        if let Some(d) = v.diff.clone().filter(|d| !d.is_empty()) {
            let f = file.clone();
            row = row.child(
                icon_button(SharedString::from(format!("tdiff-{}", p.id)), "diff", i.t("diff.open")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(move |_, _, cx| {
                    if std::path::Path::new(&f).is_file() {
                        crate::shell::docs_panel::DocsState::global(cx).update(cx, |s, cx| s.open_diff(f.clone(), d.clone(), cx));
                    } else {
                        let title = cx.global::<I18n>().t("doc.outside");
                        crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Warning, ..crate::ui::ToastInput::new(title) }, cx);
                    }
                }),
            );
        }
    }
    div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(t.rem(rem::RADIUS_MD))
        .border_1()
        .border_color(if open { t.colors.border_soft } else { gpui::transparent_black() })
        .child(row)
        .when(open, |d| d.child(body(t, i, &p.id, tool, st, &v)))
        .into_any_element()
}

/// Tally: "12 dibaca · 3 diedit +45 −12 · 8 perintah".
fn tally(t: &Theme, i: &I18n, process: &[&MessagePart]) -> Option<gpui::Div> {
    let k = tv::tally_process(process);
    let mut bits = vec![];
    for (n, key) in [(k.read, "read"), (k.edit, "edit"), (k.write, "write"), (k.shell, "shell"), (k.search, "search"), (k.web, "web"), (k.task, "task")] {
        if n > 0 {
            bits.push(i.tp(&format!("work.tally.{key}"), &[("n", &n.to_string())]));
        }
    }
    if bits.is_empty() && k.adds == 0 && k.dels == 0 {
        return None;
    }
    Some(
        div()
            .flex()
            .min_w_0()
            .overflow_hidden()
            .small(t)
            .text_color(t.colors.fg_subtle)
            .child(div().truncate().child(bits.join(" · ")))
            .children(if k.adds > 0 || k.dels > 0 { changes(t, (k.adds > 0).then_some(k.adds), (k.dels > 0).then_some(k.dels)) } else { None }),
    )
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.)
}

/// ProcessBlock.
fn process_block(t: &Theme, i: &I18n, me: &WeakEntity<ChatPanel>, process: &[&MessagePart], working_now: bool, info: &MessageInfo, turn_id: &str, expanded: &HashSet<String>) -> AnyElement {
    let key = format!("proc:{turn_id}");
    let open = expanded.contains(&key);
    let end = if working_now { now_ms() } else { info.completed.unwrap_or_else(now_ms) };
    let secs = tv::elapsed_seconds(end - info.created);
    let steps = tv::count_steps(process);
    let last = process.iter().rev().find_map(|p| tv::step_of(p));
    let toggle = {
        let me = me.clone();
        let key = key.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            let key = key.clone();
            me.update(cx, |s, cx| {
                if !s.expanded.remove(&key) {
                    s.expanded.insert(key);
                }
                cx.notify();
            })
            .ok();
        }
    };
    let chevron = icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm);
    let list = open.then(|| {
        let mut l = div().flex().flex_col().gap(t.space(1)).mt(t.space(1)).mb(t.space(2));
        for p in process {
            if let Some(el) = part_el(t, i, me, p, expanded, false) {
                l = l.child(el);
            }
        }
        l
    });
    let row = |id: String| {
        div()
            .id(SharedString::from(id))
            .flex()
            .items_center()
            .gap(t.space(2))
            .w_full()
            .min_w_0()
            .py(t.space(1))
            .text_color(t.colors.fg_muted)
            .cursor_pointer()
            .hover(|s| s.text_color(t.colors.fg))
    };
    let mut col = div().flex().flex_col().gap(t.space(1)).mb(t.space(2));
    if !working_now {
        col = col
            .child(row(format!("wsum-{turn_id}")).on_click(toggle).child(chevron).child(div().flex_none().child(i.tp("work.done", &[("n", &steps.to_string()), ("time", &fmt_secs(i, secs))]))).children(tally(t, i, process)))
            .children(list);
        return col.into_any_element();
    }
    let label = match &last {
        Some(s) => i.tp(s.key, &[("tool", s.tool.as_deref().unwrap_or(""))]),
        None => i.t("work.starting"),
    };
    col = col.child(shimmer(t, &format!("wsh-{turn_id}"), i.tp("work.for", &[("time", &fmt_secs(i, secs))])));
    if !process.is_empty() {
        col = col.child(
            row(format!("wstep-{turn_id}"))
                .on_click(toggle)
                .child(chevron)
                .child(div().flex_none().text_color(t.colors.fg).child(label))
                .when_some(last.as_ref().filter(|s| !s.detail.is_empty()), |d, s| d.child(div().min_w_0().truncate().small(t).text_color(t.colors.fg_subtle).child(s.detail.clone()))),
        );
    }
    col.children(list).into_any_element()
}

/// .work__shimmer: teks dengan kilau bergerak (gradien teks CSS) — dianimasikan sebagai pulsa warna fg_subtle↔fg.
fn shimmer(t: &Theme, id: &str, text: String) -> AnyElement {
    use gpui::{Animation, AnimationExt};
    let (a, b) = (t.colors.fg_subtle, t.colors.fg);
    div()
        .child(text)
        .with_animation(SharedString::from(id.to_string()), Animation::new(std::time::Duration::from_millis(1800)).repeat(), move |d, k| {
            let x = (1. - ((k * 2.) - 1.).abs()) as f32;
            d.text_color(gpui::Hsla { h: a.h + (b.h - a.h) * x, s: a.s + (b.s - a.s) * x, l: a.l + (b.l - a.l) * x, a: a.a + (b.a - a.a) * x })
        })
        .into_any_element()
}

/// Part (teks, berpikir, tool); `answer` = bagian jawaban (teks memakai gaya chat).
fn part_el(t: &Theme, i: &I18n, me: &WeakEntity<ChatPanel>, p: &MessagePart, expanded: &HashSet<String>, answer: bool) -> Option<AnyElement> {
    match &p.kind {
        PartKind::Text { text, synthetic: false } if !text.is_empty() => Some(if answer { chat_text(div(), t).child(markdown(text, &msg_theme(t), i)).into_any_element() } else { div().child(markdown(text, &msg_theme(t), i)).into_any_element() }),
        PartKind::Reasoning { text } if !text.is_empty() => {
            let key = format!("think:{}", p.id);
            let open = expanded.contains(&key);
            let me = me.clone();
            let k2 = key.clone();
            Some(
                div()
                    .flex()
                    .flex_col()
                    .rounded(t.rem(rem::RADIUS_MD))
                    .border_1()
                    .border_color(t.colors.border)
                    .child(
                        div()
                            .id(SharedString::from(format!("rs-{}", p.id)))
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .px(t.space(2))
                            .py(t.space(1))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                let k = k2.clone();
                                me.update(cx, |s, cx| {
                                    if !s.expanded.remove(&k) {
                                        s.expanded.insert(k);
                                    }
                                    cx.notify();
                                })
                                .ok();
                            })
                            .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                            .child(i.t("chat.thinking")),
                    )
                    .when(open, |d| d.child(div().p(t.space(2)).border_t_1().border_color(t.colors.border).small(t).text_color(t.colors.fg_muted).child(text.clone())))
                    .into_any_element(),
            )
        }
        PartKind::Tool { tool, state, .. } => Some(tool_card(t, i, me, p, tool, state, expanded.contains(&p.id))),
        _ => None,
    }
}

/// AssistantMessage (tanpa toolbar aksi; lihat message_toolbar).
#[allow(clippy::too_many_arguments)]
pub(super) fn assistant_msg(t: &Theme, i: &I18n, cx: &App, me: &WeakEntity<ChatPanel>, turn: &crate::core::chat::Turn, expanded: &HashSet<String>, streaming: bool, has_project: bool, provider_name: &str, toolbar: Option<AnyElement>) -> AnyElement {
    let m = &turn.message;
    let turn_id = turn.source_ids.first().cloned().unwrap_or_else(|| m.info.id.clone());
    let info = &m.info;
    let parts: Vec<&MessagePart> = m.parts.iter().filter(|p| !matches!(p.kind, PartKind::Other { .. } | PartKind::StepFinish { .. })).collect();
    let (process, answer) = tv::split_turn(&parts);
    let done = info.completed.is_some() && !streaming;
    let mut col = div().flex().flex_col().gap(t.space(2)).w_full().min_w_0();
    if !process.is_empty() || streaming {
        col = col.child(process_block(t, i, me, &process, streaming, info, &turn_id, expanded));
    }
    let mut ans = div().flex().flex_col().gap(t.space(2));
    for p in &answer {
        if let PartKind::File { filename, mime, url } = &p.kind {
            // gambar selalu tampil sebagai pratinjau; berkas lain hanya di sesi tanpa project
            let image = mime.starts_with("image/") && crate::shell::widgets::safe_image_uri(url);
            if image || !has_project {
                ans = ans.child(crate::shell::widgets::file_card(t, i, &p.id, filename.as_deref(), mime, url, None));
            }
        } else if let Some(el) = part_el(t, i, me, p, expanded, true) {
            ans = ans.child(el);
        }
    }
    if !has_project {
        for (n, path) in tv::created_documents(&process).iter().enumerate() {
            ans = ans.child(crate::shell::widgets::file_card(t, i, &format!("{turn_id}-doc{n}"), None, "", "", Some(path)));
        }
    }
    col = col.child(ans);
    if let Some(e) = &info.error {
        col = col.child(div().small(t).text_color(t.colors.danger).child(if e.name == "MessageAbortedError" { i.t("chat.aborted") } else { e.message.clone() }));
    }
    if let Some(model) = &info.model_id {
        let mut meta = div().flex().flex_wrap().items_center().gap(t.space(2)).min_w_0().small(t).text_color(t.colors.fg_subtle);
        meta = meta.child(crate::shell::widgets::model_logo(t, cx, info.provider_id.as_deref(), Some(model), provider_name, streaming, &format!("ml-{}", info.id)));
        if !provider_name.is_empty() {
            meta = meta.child(div().text_color(t.colors.fg_muted).child(provider_name.to_string()));
        }
        meta = meta.child("•").child(div().font_family(t.font_code.clone()).child(model.clone()));
        if done {
            if let Some(c) = info.completed {
                meta = meta.child("•").child(fmt_secs(i, tv::elapsed_seconds(c - info.created)));
                use chrono::TimeZone;
                let when = chrono::Local.timestamp_millis_opt(c as i64).single().map(|d| d.format("%-d %b %H.%M").to_string()).unwrap_or_default();
                meta = meta.child("•").child(when);
            }
        }
        col = col.child(div().flex().items_center().justify_between().gap(t.space(3)).mt(t.space(2)).child(meta).children(if done { toolbar } else { None }));
    }
    col.into_any_element()
}

// ---------------- MessageToolbar.tsx + PinnedBar ----------------

/// Nama berkas ekspor `respons-YYYYMMDD-HHMMSS.md` (exportFileName).
pub(super) fn export_file_name(ext: &str) -> String {
    format!("respons-{}.{ext}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
}

/// zandor://chat/{key}/{sessionId}/{messageId} (messageLink, encodeURIComponent).
pub(super) fn message_link(key: &str, sid: &str, mid: &str) -> String {
    const KEEP: &[u8] = b"-_.!~*'()";
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || KEEP.contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    format!("zandor://chat/{}/{}/{}", enc(key), enc(sid), enc(mid))
}

/// Urai tautan pesan (parseMessageLink): (key, sessionId, messageId); None bila bukan tautan valid.
pub(crate) fn parse_message_link(text: &str) -> Option<(String, String, String)> {
    let rest = text.trim().strip_prefix("zandor://chat/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() != 3 {
        return None;
    }
    let dec = |s: &str| -> Option<String> {
        let b = s.as_bytes();
        let mut out = vec![];
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' {
                let h = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(h, 16).ok()?);
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).ok()
    };
    let (k, si, m) = (dec(parts[0])?, dec(parts[1])?, dec(parts[2])?);
    (!k.is_empty() && !si.is_empty() && !m.is_empty()).then_some((k, si, m))
}

fn toast(cx: &mut App, title: String, tone: crate::ui::ToastTone, ms: Option<u64>) {
    crate::ui::Toasts::show(crate::ui::ToastInput { title: title.into(), description: None, tone, timeout_ms: ms, action: None }, cx);
}
pub(super) fn fail(cx: &mut App, e: impl std::fmt::Display) {
    let title = cx.global::<I18n>().tp("sb.error", &[("error", &e.to_string())]);
    toast(cx, title, crate::ui::ToastTone::Danger, None);
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default()
}

impl ChatPanel {
    /// Simpan teks pesan ke berkas memori (catatan project/global, atau sematan konteks sesi).
    fn save_note(&self, scope: &'static str, text: String, sid: String, cx: &mut Context<Self>) {
        use crate::core::mem;
        let i = cx.global::<I18n>().clone();
        if text.trim().is_empty() {
            return fail(cx, i.t("msg.noContent"));
        }
        if !mem::scan_secrets(&text).is_empty() {
            return fail(cx, i.t("msg.noteSecret"));
        }
        let rel = match scope {
            "project" => match self.store.read(cx).selected().and_then(|s| s.2) {
                Some(p) => mem::project_file(&mem::project_key(&p.path)),
                None => return fail(cx, i.t("msg.noProject")),
            },
            "session" => mem::pin_file(&sid),
            _ => mem::FILE_GLOBAL.to_string(),
        };
        let path = crate::core::services::app_data_dir().join("memory").join(&rel);
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let stamp = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
        let r = match path.parent() {
            Some(d) => std::fs::create_dir_all(d),
            None => Ok(()),
        }
        .and_then(|_| std::fs::write(&path, mem::append_note(&existing, &stamp, &text)));
        match r {
            Ok(_) => {
                let scope_l = i.t(&format!("msg.scope.{scope}"));
                toast(cx, i.tp("msg.noteSaved", &[("scope", &scope_l)]), crate::ui::ToastTone::Success, None);
            }
            Err(e) => fail(cx, e),
        }
    }

    fn toggle_msg_menu(&mut self, mid: &str, which: u8, cx: &mut Context<Self>) {
        self.msg_menu = if self.msg_menu.as_ref().is_some_and(|(a, w)| a == mid && *w == which) { None } else { Some((mid.to_string(), which)) };
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn message_toolbar(&self, t: &Theme, i: &I18n, turn: &crate::core::chat::Turn, sid: &str, key: &str, is_last_assistant: bool, cx: &mut Context<Self>) -> AnyElement {
        let m = &turn.message;
        let mid = m.info.id.clone();
        let text = message_text(m);
        let assistant = m.info.role == Role::Assistant;
        let pinned = self.chat.read(cx).pinned_messages.get(sid).is_some_and(|ids| turn.source_ids.iter().any(|x| ids.contains(x)));
        let ib = |id: &str, ic: &'static str, label: String| icon_button(SharedString::from(format!("{id}-{mid}")), ic, label).size(ControlSize::Sm).variant(ButtonVariant::Ghost);
        let mut row = div().flex().flex_none().gap(t.space(1)).text_color(t.colors.fg_muted);
        if assistant {
            let link = message_link(if key.is_empty() { "chat" } else { key }, sid, &mid);
            row = row.child(ib("mlink", "link", i.t("msg.copyLink")).on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(link.clone()));
                let ti = cx.global::<I18n>().t("msg.linkCopied");
                toast(cx, ti, crate::ui::ToastTone::Success, Some(1500));
            }));
        }
        if !text.is_empty() {
            let tx = text.clone();
            row = row.child(ib("mcopy", "copy", i.t("chat.copy")).on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(tx.clone()));
                let ti = cx.global::<I18n>().t("chat.copied");
                toast(cx, ti, crate::ui::ToastTone::Success, Some(1500));
            }));
        }
        let menu_open = |which: u8| self.msg_menu.as_ref().is_some_and(|(id, w)| *id == mid && *w == which);
        let me = cx.entity().downgrade();
        if assistant && !text.is_empty() {
            // Simpan sebagai: Teks (Markdown). PNG/SVG di aplikasi TS merender DOM lewat html-to-image; GPUI tidak punya
            // perender elemen ke gambar di luar layar, jadi pilihan itu belum tersedia di port ini.
            let tx = text.clone();
            let entries = vec![crate::ui::entry(i.t("msg.saveText"), move |_, cx| {
                let body = if tx.ends_with('\n') { tx.clone() } else { format!("{tx}\n") };
                let rx = cx.prompt_for_new_path(&dirs_home(), Some(&export_file_name("md")));
                cx.spawn(async move |cx| {
                    let Ok(Ok(Some(dest))) = rx.await else { return };
                    let r = std::fs::write(&dest, body);
                    cx.update(|cx| match r {
                        Ok(_) => {
                            let ti = cx.global::<I18n>().t("msg.saved");
                            toast(cx, ti, crate::ui::ToastTone::Success, Some(2000));
                        }
                        Err(e) => fail(cx, e),
                    });
                })
                .detach();
            })
            .icon("file_text")];
            let (mid2, me2) = (mid.clone(), me.clone());
            row = row.child(
                div()
                    .relative()
                    .child(ib("msave", "image_down", i.t("msg.saveAs")).on_click(cx.listener(move |s, _, _, cx| s.toggle_msg_menu(&mid2, 1, cx))))
                    .when(menu_open(1), |d| {
                        d.child(crate::ui::menu_popup(SharedString::from(format!("msave-{mid}")), t, entries, gpui::Anchor::BottomRight, move |_, cx| {
                            me2.update(cx, |s, cx| {
                                s.msg_menu = None;
                                cx.notify();
                            })
                            .ok();
                        }))
                    }),
            );
            let mut notes = vec![];
            let has_project = self.store.read(cx).selected().is_some_and(|s| s.2.is_some());
            for scope in ["project", "global", "session"] {
                if scope == "project" && !has_project {
                    continue;
                }
                let (me, tx, sid) = (me.clone(), text.clone(), sid.to_string());
                notes.push(crate::ui::entry(i.t(&format!("msg.scope.{scope}")), move |_, cx| {
                    me.update(cx, |s, cx| s.save_note(scope, tx.clone(), sid.clone(), cx)).ok();
                }));
            }
            let (mid3, me3) = (mid.clone(), me.clone());
            row = row.child(
                div()
                    .relative()
                    .child(ib("mnote", "bookmark", i.t("msg.saveNote")).on_click(cx.listener(move |s, _, _, cx| s.toggle_msg_menu(&mid3, 2, cx))))
                    .when(menu_open(2), |d| {
                        d.child(crate::ui::menu_popup(SharedString::from(format!("mnote-{mid}")), t, notes, gpui::Anchor::BottomRight, move |_, cx| {
                            me3.update(cx, |s, cx| {
                                s.msg_menu = None;
                                cx.notify();
                            })
                            .ok();
                        }))
                    }),
            );
        }
        if assistant {
            let (sid2, mid2) = (sid.to_string(), mid.clone());
            let mut pin = ib("mpin", "pin", i.t(if pinned { "msg.unpin" } else { "msg.pin" })).on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.toggle_message_pin(&sid2, &mid2, cx))));
            if pinned {
                pin = pin.variant(ButtonVariant::Outline);
            }
            row = row.child(pin);
            let (sid2, mid2, key2) = (sid.to_string(), mid.clone(), key.to_string());
            row = row.child(ib("mfork", "git", i.t("chat.fork")).on_click(cx.listener(move |s, _, _, cx| {
                let task = s.chat.update(cx, |c, cx| c.fork(&sid2, &mid2, cx));
                let key = key2.clone();
                cx.spawn(async move |this, cx| {
                    let r = task.await;
                    this.update(cx, |s, cx| match r {
                        Ok(info) if !key.is_empty() => s.store.update(cx, |p, cx| p.adopt_session(&key, info, cx)),
                        Ok(_) => {}
                        Err(e) => fail(cx, e),
                    })
                    .ok();
                })
                .detach();
            })));
        }
        if m.info.role == Role::User {
            let (sid2, mid2) = (sid.to_string(), mid.clone());
            row = row.child(ib("mrev", "undo", i.t("chat.revert")).on_click(cx.listener(move |s, _, _, cx| {
                if s.chat.read(cx).sessions.get(&sid2).is_some_and(|x| x.busy()) {
                    return fail(cx, "hentikan sesi sebelum revert");
                }
                s.chat.update(cx, |c, cx| c.revert_to(&sid2, &mid2, cx));
            })));
        }
        if is_last_assistant {
            let sid2 = sid.to_string();
            row = row.child(ib("mregen", "refresh", i.t("chat.regenerate")).on_click(cx.listener(move |s, _, _, cx| {
                if let Err(e) = s.chat.update(cx, |c, cx| c.regenerate(&sid2, cx)) {
                    fail(cx, e);
                }
            })));
        }
        row.into_any_element()
    }

    /// PinnedBar: daftar pesan yang disematkan (dapat dilipat); klik → gulir ke pesan.
    pub(super) fn pinned_bar(&self, t: &Theme, i: &I18n, sid: &str, turns: &[crate::core::chat::Turn], cx: &mut Context<Self>) -> Option<AnyElement> {
        let ids = self.chat.read(cx).pinned_messages.get(sid).cloned().unwrap_or_default();
        let pinned: Vec<&crate::core::chat::Turn> = ids.iter().filter_map(|id| turns.iter().find(|x| x.source_ids.contains(id))).collect();
        if pinned.is_empty() {
            return None;
        }
        let open = self.pins_open;
        let mut col = div()
            .flex()
            .flex_col()
            .mb(t.space(2))
            .px(t.space(2))
            .py(t.space(1))
            .border_1()
            .border_color(t.colors.border)
            .rounded(t.rem(rem::RADIUS_LG))
            .bg(t.colors.surface)
            .child(
                div()
                    .id("pins-sum")
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .cursor_pointer()
                    .typo(t, crate::theme::LABEL)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.pins_open = !s.pins_open;
                        cx.notify();
                    }))
                    .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                    .child(icon("pin").size(IconSize::Sm))
                    .child(i.tp("msg.pinned", &[("n", &pinned.len().to_string())])),
            );
        if open {
            for tr in pinned {
                let m = &tr.message;
                let ex: String = message_text(m).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(90).collect();
                let ex = if ex.is_empty() { i.t("sb.untitled") } else { ex };
                let (go_id, sid2) = (m.info.id.clone(), sid.to_string());
                let unpin: Vec<String> = tr.source_ids.iter().filter(|x| ids.contains(x)).cloned().collect();
                col = col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(1))
                        .mt(t.space(1))
                        .child(
                            div()
                                .id(SharedString::from(format!("pin-go-{}", m.info.id)))
                                .flex()
                                .flex_1()
                                .items_baseline()
                                .gap(t.space(2))
                                .min_w_0()
                                .px(t.space(2))
                                .py(t.space(1))
                                .rounded(t.rem(rem::RADIUS_MD))
                                .cursor_pointer()
                                .hover(|s| s.bg(t.colors.hover))
                                .on_click(move |_, _, cx| cx.set_global(MessageFocus(Some(go_id.clone()))))
                                .child(div().flex_none().small(t).text_color(t.colors.fg_subtle).child(i.t(if m.info.role == Role::User { "chat.you" } else { "chat.assistant" })))
                                .child(div().min_w_0().truncate().child(ex)),
                        )
                        .child(
                            icon_button(SharedString::from(format!("pin-un-{}", m.info.id)), "pin", i.t("msg.unpin"))
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Outline)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    for id in &unpin {
                                        s.chat.update(cx, |c, cx| c.toggle_message_pin(&sid2, id, cx));
                                    }
                                })),
                        ),
                );
            }
        }
        Some(col.into_any_element())
    }
}

#[cfg(test)]
mod toolbar_tests {
    use super::*;
    #[test]
    fn tautan() {
        assert_eq!(message_link("chat", "ses 1", "msg/2"), "zandor://chat/chat/ses%201/msg%2F2");
        assert_eq!(parse_message_link(" zandor://chat/chat/ses%201/msg%2F2 "), Some(("chat".into(), "ses 1".into(), "msg/2".into())));
        assert_eq!(parse_message_link("zandor://chat/a/b"), None);
        assert_eq!(parse_message_link("zandor://chat/a/b/%ZZ"), None);
        assert!(export_file_name("md").starts_with("respons-"));
    }
}
