//! Halaman Memori & Skill — port MemoryPage.tsx: tab Pengaturan (memori/skill creator bawaan), Memori (berkas memori),
//! Antrean skill, Skill terpasang (global). Memakai memory.rs bersama dari backend Tauri.
use super::state::AppState;
use crate::core::services::app_data_dir;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H3};
use crate::ui::{button, icon_button, markdown, switch, tabs, ButtonVariant, ControlSize, TabItem, TypeExt};
use gpui::{div, prelude::*, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use serde_json::{json, Value};

pub struct MemoryPage {
    app: Entity<AppState>,
    tab: SharedString,
    open_file: Option<(String, String)>,
}

fn mem_dir() -> std::path::PathBuf {
    app_data_dir().join("memory")
}
fn skills_dir() -> std::path::PathBuf {
    crate::memory::skills_root("global", None, &app_data_dir().join("engine")).unwrap_or_default()
}

fn read_settings() -> Value {
    crate::memory::read_memory(&mem_dir(), "settings.json")
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}))
}

impl MemoryPage {
    pub fn new(app: Entity<AppState>, _cx: &mut Context<Self>) -> Self {
        Self { app, tab: "settings".into(), open_file: None }
    }

    fn set_flag(&mut self, key: &str, v: bool, cx: &mut Context<Self>) {
        let mut s = read_settings();
        if !s.is_object() {
            s = json!({});
        }
        s[key] = json!(v);
        let _ = std::fs::create_dir_all(mem_dir());
        if let Err(e) = crate::memory::write_memory(&mem_dir(), "settings.json", &s.to_string()) {
            eprintln!("[zandor] {e}");
        }
        cx.notify();
    }
}

impl Render for MemoryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        let body: gpui::AnyElement = match self.tab.as_ref() {
            "settings" => {
                let s = read_settings();
                let def = s["defaultEnabled"].as_bool().unwrap_or(false);
                let sc = s["skillCreator"].as_bool().unwrap_or(false);
                let row = |id: &'static str, label: String, hint: String, on: bool, key: &'static str| {
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(4))
                        .py(t.space(3))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .gap(t.space(1))
                                .child(div().font_weight(FontWeight::MEDIUM).child(label))
                                .child(div().small(&t).text_color(t.colors.fg_muted).child(hint)),
                        )
                        .child(switch(id, "", on).on_change({
                            let me = cx.entity().downgrade();
                            move |v, _, cx| {
                                me.update(cx, |s, cx| s.set_flag(key, v, cx)).ok();
                            }
                        }))
                };
                div()
                    .flex()
                    .flex_col()
                    .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("mem.settings.intro")))
                    .child(row("mem-def", i.t("mem.settings.default"), i.t("mem.settings.defaultHint"), def, "defaultEnabled"))
                    .child(row("mem-sc", i.t("mem.settings.skillCreator"), i.t("mem.settings.skillCreatorHint"), sc, "skillCreator"))
                    .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("mem.settings.sessionHint")))
                    .into_any_element()
            }
            "notes" => {
                let files: Vec<_> = crate::memory::list_memory(&mem_dir()).into_iter().filter(|f| f.rel.ends_with(".md")).collect();
                let mut col = div().flex().gap(t.space(3)).min_h(t.rem(20.));
                let mut list = div().flex().flex_col().gap(t.rem(0.125)).w(t.rem(rem::SPACE[7] * 5.)).flex_none();
                for f in &files {
                    let label = if f.rel == "global.md" {
                        i.t("mem.notes.global")
                    } else if let Some(k) = f.rel.strip_prefix("projects/").and_then(|r| r.strip_suffix(".md")) {
                        i.tp("mem.notes.projectUnknown", &[("key", k)])
                    } else if let Some(k) = f.rel.strip_prefix("pins/").and_then(|r| r.strip_suffix(".md")) {
                        i.tp("mem.notes.pin", &[("id", k)])
                    } else {
                        f.rel.clone()
                    };
                    let rel = f.rel.clone();
                    let cur = self.open_file.as_ref().is_some_and(|(r, _)| *r == f.rel);
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("mf-{}", f.rel)))
                            .px(t.space(2))
                            .py(t.space(1))
                            .rounded(t.rem(rem::RADIUS_MD))
                            .cursor_pointer()
                            .when(cur, |d| d.bg(t.colors.active))
                            .hover(|s| s.bg(t.colors.hover))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let text = crate::memory::read_memory(&mem_dir(), &rel).unwrap_or_default();
                                s.open_file = Some((rel.clone(), text));
                                cx.notify();
                            }))
                            .child(label),
                    );
                }
                if files.is_empty() {
                    list = list.child(div().text_color(t.colors.fg_muted).child(i.t("mem.loading")));
                }
                col = col.child(list);
                if let Some((rel, text)) = self.open_file.clone() {
                    col = col.child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(t.space(2))
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .child(div().flex_1().code(&t).text_color(t.colors.fg_muted).child(rel.clone()))
                                    .child(
                                        button("mem-del")
                                            .label(i.t("mem.notes.delete"))
                                            .size(ControlSize::Sm)
                                            .variant(ButtonVariant::Danger)
                                            .on_click(cx.listener(move |s, _, _, cx| {
                                                let _ = crate::memory::delete_memory(&mem_dir(), &rel);
                                                s.open_file = None;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("mem.notes.hint")))
                            .child(div().p(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised).child(markdown(&text, &t))),
                    );
                }
                col.into_any_element()
            }
            "queue" => {
                let text = crate::memory::read_memory(&mem_dir(), "skill-queue.jsonl").unwrap_or_default();
                let items: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
                let mut col = div().flex().flex_col().gap(t.space(2));
                if items.is_empty() {
                    col = col.child(div().text_color(t.colors.fg_muted).child(i.t("mem.queue.empty")));
                }
                for it in items.iter().rev() {
                    let trig = it["trigger"].as_str().unwrap_or("manual");
                    let status = it["status"].as_str().unwrap_or("candidate");
                    col = col.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.space(1))
                            .p(t.space(3))
                            .rounded(t.rem(rem::RADIUS_MD))
                            .border_1()
                            .border_color(t.colors.border_soft)
                            .child(
                                div()
                                    .flex()
                                    .gap(t.space(2))
                                    .child(div().font_weight(FontWeight::MEDIUM).child(i.t(&format!("mem.queue.trigger.{}", trig.replace("tool-count", "toolCount")))))
                                    .child(div().text_color(t.colors.fg_muted).child(i.t(&format!("mem.queue.status.{status}")))),
                            )
                            .children(it["excerpt"].as_str().map(|e| div().small(&t).text_color(t.colors.fg_muted).child(e.to_string()))),
                    );
                }
                col.into_any_element()
            }
            _ => {
                let skills = crate::memory::list_skills(&skills_dir(), "global");
                let mut col = div().flex().flex_col();
                if skills.is_empty() {
                    col = col.child(div().text_color(t.colors.fg_muted).child(i.t("mem.skills.empty")));
                }
                for s in skills {
                    let name = s.name.clone();
                    col = col.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .py(t.space(2))
                            .border_b_1()
                            .border_color(t.colors.border_soft)
                            .child(div().flex_1().child(s.name.clone()))
                            .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("mem.skills.global")))
                            .child(
                                icon_button(SharedString::from(format!("sk-del-{}", s.name)), "trash", i.t("mem.skills.delete"))
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        let _ = crate::memory::delete_skill(&skills_dir(), &name);
                                        cx.notify();
                                    })),
                            ),
                    );
                }
                col.into_any_element()
            }
        };
        div()
            .id("mem-page")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(t.colors.frame)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(4))
                    .w_full()
                    .max_w(t.rem(rem::SPACE[7] * 16.))
                    .mx_auto()
                    .px(t.space(4))
                    .py(t.space(5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(icon_button("mem-back", "arrow_left", i.t("mem.back")).on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/", cx)))))
                            .child(div().typo(&t, H3).child(i.t("mem.title"))),
                    )
                    .child(
                        tabs(
                            "mem-tabs",
                            ["settings", "notes", "queue", "skills"]
                                .iter()
                                .map(|v| TabItem { value: (*v).into(), label: i.t(&format!("mem.tab.{v}")).into(), icon: None, disabled: false })
                                .collect(),
                            self.tab.clone(),
                        )
                        .on_change({
                            let me = cx.entity().downgrade();
                            move |v, _, cx| {
                                me.update(cx, |s, cx| {
                                    s.tab = v;
                                    cx.notify();
                                })
                                .ok();
                            }
                        }),
                    )
                    .child(div().p(t.space(4)).rounded(t.rem(rem::RADIUS_LG)).bg(t.colors.surface).child(body)),
            )
    }
}
