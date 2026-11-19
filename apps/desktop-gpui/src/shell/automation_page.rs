//! Halaman Automasi — port AutomationPage.tsx: daftar tugas (aktif, berikutnya/terakhir, jalankan, ubah, hapus)
//! dan formulir tugas (nama, project, prompt, jadwal interval/harian/sekali).
use super::state::AppState;
use crate::core::automation::{fmt_time, next_run, parse_local, run_task, AutomationStore, Schedule, Task};
use crate::core::projects::{now_ms, ProjectStore};
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H3};
use crate::ui::{button, icon_button, select, switch, ButtonVariant, ControlSize, TextInput, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};

const DAYS: [&str; 7] = ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"];

struct Form {
    id: Option<String>,
    name: Entity<TextInput>,
    prompt: Entity<TextInput>,
    minutes: Entity<TextInput>,
    time: Entity<TextInput>,
    at: Entity<TextInput>,
    kind: SharedString,
    project: SharedString,
    days: Vec<u32>,
    error: Option<String>,
}

pub struct AutomationPage {
    /// Tertanam di halaman Pengaturan: tanpa bingkai halaman dan tombol kembali.
    pub embedded: bool,
    app: Entity<AppState>,
    store: Entity<AutomationStore>,
    form: Option<Form>,
}

impl AutomationPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let store = AutomationStore::global(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        Self { embedded: false, app, store, form: None }
    }

    fn open_form(&mut self, task: Option<Task>, cx: &mut Context<Self>) {
        let mk = |cx: &mut Context<Self>, ph: &str, v: String| {
            cx.new(|cx| {
                let mut ti = TextInput::new(cx, ph.to_string());
                ti.set_text(v, cx);
                ti
            })
        };
        let t = task.as_ref();
        let (kind, minutes, time, at, days) = match t.map(|t| &t.schedule) {
            Some(Schedule::Interval { every_minutes }) => ("interval", every_minutes.to_string(), "09:00".into(), String::new(), vec![]),
            Some(Schedule::Daily { time, days }) => ("daily", "60".into(), time.clone(), String::new(), days.clone()),
            Some(Schedule::Once { at }) => ("once", "60".into(), "09:00".into(), {
                chrono::TimeZone::timestamp_millis_opt(&chrono::Local, *at as i64).single().map(|d| d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()
            }, vec![]),
            None => ("daily", "60".into(), "09:00".into(), String::new(), vec![]),
        };
        self.form = Some(Form {
            id: t.map(|t| t.id.clone()),
            name: mk(cx, "", t.map(|t| t.name.clone()).unwrap_or_default()),
            prompt: {
                let e = mk(cx, "", t.map(|t| t.prompt.clone()).unwrap_or_default());
                e.update(cx, |ti, _| {
                    ti.multiline = true;
                    ti.enter_newline = true;
                });
                e
            },
            minutes: mk(cx, "60", minutes),
            time: mk(cx, "09:00", time),
            at: mk(cx, "2026-12-31 09:00", at),
            kind: kind.into(),
            project: t.and_then(|t| t.project_id.clone()).unwrap_or_default().into(),
            days,
            error: None,
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.form.as_mut() else { return };
        let schedule = match f.kind.as_ref() {
            "interval" => Schedule::Interval { every_minutes: f.minutes.read(cx).text().trim().parse().unwrap_or(0) },
            "once" => match parse_local(&f.at.read(cx).text()) {
                Some(at) => Schedule::Once { at },
                None => {
                    f.error = Some("waktu tidak valid (YYYY-MM-DD HH:MM)".into());
                    cx.notify();
                    return;
                }
            },
            _ => Schedule::Daily { time: f.time.read(cx).text().trim().to_string(), days: f.days.clone() },
        };
        let (id, name, prompt) = (f.id.clone(), f.name.read(cx).text().to_string(), f.prompt.read(cx).text().to_string());
        let project = (!f.project.is_empty()).then(|| f.project.to_string());
        match self.store.update(cx, |s, cx| s.upsert(id, name, project, prompt, schedule, cx)) {
            Ok(()) => self.form = None,
            Err(e) => {
                if let Some(f) = self.form.as_mut() {
                    f.error = Some(e);
                }
            }
        }
        cx.notify();
    }
}

fn describe(i: &I18n, s: &Schedule) -> String {
    match s {
        Schedule::Interval { every_minutes } => i.tp("auto.sch.interval", &[("n", &every_minutes.to_string())]),
        Schedule::Once { at } => i.tp("auto.sch.once", &[("at", &fmt_time(*at))]),
        Schedule::Daily { time, days } => {
            let d = if days.is_empty() { i.t("auto.sch.everyDay") } else { days.iter().map(|d| DAYS[*d as usize % 7]).collect::<Vec<_>>().join(", ") };
            i.tp("auto.sch.daily", &[("time", time), ("days", &d)])
        }
    }
}

impl Render for AutomationPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        let st = self.store.read(cx);
        let projects = ProjectStore::global(cx).read(cx).projects.clone();
        let now = now_ms();

        let mut body = div().flex().flex_col().gap(t.space(3));
        body = body
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("auto.openOnly")))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("auto.noAutoApprove")));

        if let Some(f) = &self.form {
            let field = |label: String, el: AnyElement| div().flex().flex_col().gap(t.space(1)).child(div().font_weight(FontWeight::MEDIUM).child(label)).child(el);
            let mut proj_items: Vec<(SharedString, SharedString)> = vec![("".into(), i.t("chat.noProject").into())];
            proj_items.extend(projects.iter().map(|p| (p.id.clone().into(), p.name.clone().into())));
            let me = cx.entity().downgrade();
            let me2 = me.clone();
            let mut sched = div().flex().flex_col().gap(t.space(2)).child(
                select(
                    "auto-kind",
                    ["daily", "interval", "once"].iter().map(|k| ((*k).into(), i.t(&format!("auto.kind.{k}")).into())).collect(),
                    f.kind.clone(),
                )
                .on_change(move |v, _, cx| {
                    me.update(cx, |s, cx| {
                        if let Some(f) = s.form.as_mut() {
                            f.kind = v;
                        }
                        cx.notify();
                    })
                    .ok();
                }),
            );
            sched = match f.kind.as_ref() {
                "interval" => sched.child(field(i.t("auto.everyMinutes"), f.minutes.clone().into_any_element())),
                "once" => sched.child(field(format!("{} (YYYY-MM-DD HH:MM)", i.t("auto.at")), f.at.clone().into_any_element())),
                _ => {
                    let mut days = div().flex().gap(t.space(1));
                    for (n, d) in DAYS.iter().enumerate() {
                        let on = f.days.contains(&(n as u32));
                        days = days.child(
                            button(SharedString::from(format!("day-{n}")))
                                .label(*d)
                                .size(ControlSize::Sm)
                                .variant(if on { ButtonVariant::Solid } else { ButtonVariant::Outline })
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    if let Some(f) = s.form.as_mut() {
                                        let d = n as u32;
                                        if let Some(p) = f.days.iter().position(|x| *x == d) {
                                            f.days.remove(p);
                                        } else {
                                            f.days.push(d);
                                            f.days.sort();
                                        }
                                    }
                                    cx.notify();
                                })),
                        );
                    }
                    sched
                        .child(field(i.t("auto.time"), f.time.clone().into_any_element()))
                        .child(field(i.t("auto.days"), days.into_any_element()))
                        .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("auto.daysHint")))
                }
            };
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(3))
                    .p(t.space(4))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .border_1()
                    .border_color(t.colors.border)
                    .bg(t.colors.surface)
                    .child(field(i.t("auto.name"), f.name.clone().into_any_element()))
                    .child(field(
                        i.t("auto.project"),
                        select("auto-proj", proj_items, f.project.clone())
                            .on_change(move |v, _, cx| {
                                me2.update(cx, |s, cx| {
                                    if let Some(f) = s.form.as_mut() {
                                        f.project = v;
                                    }
                                    cx.notify();
                                })
                                .ok();
                            })
                            .into_any_element(),
                    ))
                    .child(field(i.t("auto.prompt"), div().flex().flex_col().gap(t.space(1)).child(f.prompt.clone()).child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("auto.promptHint"))).into_any_element()))
                    .child(field(i.t("auto.schedule"), sched.into_any_element()))
                    .children(f.error.clone().map(|e| div().text_color(t.colors.danger).child(e)))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(t.space(2))
                            .child(button("auto-cancel").label(i.t("ui.cancel")).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                                s.form = None;
                                cx.notify();
                            })))
                            .child(button("auto-save").label(i.t("auto.save")).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| s.save(cx)))),
                    ),
            );
        }

        if st.tasks.is_empty() && self.form.is_none() {
            body = body.child(div().py(t.space(4)).text_color(t.colors.fg_muted).child(i.t("auto.empty")));
        }
        for task in &st.tasks {
            let running = st.running.contains(&task.id);
            let proj = match &task.project_id {
                None => i.t("chat.noProject"),
                Some(id) => projects.iter().find(|p| &p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| i.t("auto.projectGone")),
            };
            let next = if !task.enabled {
                i.t("auto.off")
            } else {
                next_run(task, now).map(fmt_time).unwrap_or_else(|| i.t("auto.done"))
            };
            let (id1, id2, id3, id4) = (task.id.clone(), task.id.clone(), task.id.clone(), task.clone());
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .bg(t.colors.surface)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(task.name.clone()))
                            .when(running, |d| d.child(div().small(&t).text_color(t.colors.warning).child(i.t("auto.running"))))
                            .child(switch(SharedString::from(format!("en-{}", task.id)), i.t("auto.enabled"), task.enabled).on_change({
                                let me = cx.entity().downgrade();
                                move |v, _, cx| {
                                    me.update(cx, |s, cx| s.store.update(cx, |st, cx| st.set_enabled(&id1, v, cx))).ok();
                                }
                            })),
                    )
                    .child(div().small(&t).text_color(t.colors.fg_muted).child(format!("{} · {}", describe(i, &task.schedule), proj)))
                    .child(
                        div()
                            .small(&t)
                            .text_color(t.colors.fg_subtle)
                            .child(format!("{}: {} · {}: {}", i.t("auto.next"), next, i.t("auto.last"), task.last_run_at.map(fmt_time).unwrap_or_else(|| "—".into()))),
                    )
                    .children(task.last_error.clone().map(|e| div().small(&t).text_color(t.colors.danger).child(i.tp("auto.lastError", &[("error", &e)]))))
                    .child(
                        div()
                            .flex()
                            .gap(t.space(2))
                            .child(button(SharedString::from(format!("run-{}", task.id))).label(i.t("auto.runNow")).size(ControlSize::Sm).disabled(running).on_click(cx.listener(move |_, _, _, cx| run_task(id2.clone(), cx))))
                            .child(button(SharedString::from(format!("edit-{}", task.id))).label(i.t("auto.edit")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.open_form(Some(id4.clone()), cx))))
                            .child(button(SharedString::from(format!("del-{}", task.id))).label(i.t("auto.delete")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.store.update(cx, |st, cx| st.remove(&id3, cx))))),
                    ),
            );
        }

        let embedded = self.embedded;
        let col = div()
            .flex()
            .flex_col()
            .gap(t.space(4))
            .w_full()
            .when(!embedded, |d| d.max_w(t.rem(rem::SPACE[7] * 16.)).mx_auto().px(t.space(4)).py(t.space(5)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .when(!embedded, |d| d.child(icon_button("auto-back", "arrow_left", i.t("mem.back")).on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/", cx))))))
                    .child(div().flex_1().typo(&t, H3).child(i.t("auto.title")))
                    .child(button("auto-new").label(i.t("auto.new")).variant(ButtonVariant::Solid).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.open_form(None, cx)))),
            )
            .child(body);
        if embedded {
            return div().w_full().child(col).into_any_element();
        }
        div().id("auto-page").flex().flex_col().size_full().overflow_y_scroll().bg(t.colors.frame).child(col).into_any_element()
    }
}
