//! Halaman Pengaturan — port SettingsPage.tsx: navigasi kategori, cari, baris dari SETTINGS_SCHEMA (Switch/Select),
//! profil, model (dari engine), tata letak, dan Data (ekspor/impor/atur ulang + JSON saat ini).
use super::state::AppState;
use crate::core::engine::ModelOption;
use crate::core::projects::ProjectStore;
use crate::core::settings::{group_of, keys, properties, SettingsStore, GROUPS};
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H4};
use crate::ui::{
    button, icon, icon_button, select, switch, tabs, ButtonVariant, ControlSize, IconSize, InputChanged, TabItem, TextInput,
    ToastInput, ToastTone, Toasts, TypeExt,
};
use gpui::{div, prelude::*, AnyElement, App, ClipboardItem, Context, Entity, FontWeight, IntoElement, ScrollHandle, SharedString, Window};
use serde_json::Value;

const SECTIONS: [(&str, &str, &str); 13] = [
    ("profile", "settings.profile", "user"),
    ("general", "settings.general", "settings"),
    ("appearance", "settings.appearance", "eye"),
    ("chat", "settings.chat", "chat"),
    ("notifications", "settings.notifications", "bell"),
    ("tools", "settings.tools", "wrench"),
    ("layout", "settings.layout", "layout"),
    ("documents", "docStyle.section", "file"),
    ("models", "settings.models", "brain"),
    ("engines", "settings.engines", "globe"),
    ("memory", "settings.more.memory", "bookmark"),
    ("automation", "settings.more.automation", "clock"),
    ("data", "settings.data", "database"),
];

pub struct SettingsPage {
    app: Entity<AppState>,
    settings: Entity<SettingsStore>,
    search: Entity<TextInput>,
    active: &'static str,
    ap_tab: &'static str,
    scroll: ScrollHandle,
    models: Option<Result<Vec<ModelOption>, String>>,
    providers: Entity<super::providers_section::ProvidersSection>,
    agents: Entity<super::acp_section::AcpSection>,
    models_sec: Entity<super::models_section::ModelsSection>,
    notif: Entity<super::notification_section::NotificationSection>,
    profile: Entity<super::profile_section::ProfileSection>,
    voice: Entity<super::voice_section::VoiceSection>,
    themes: Entity<super::themes_section::ThemesSection>,
    typography: Entity<super::type_section::TypeSection>,
    chat_style: Entity<super::chat_style_section::ChatStyleSection>,
    keys: Entity<super::keys_section::KeysSection>,
    pstyles: Entity<super::projects_style_section::ProjectsStyleSection>,
    editor: Entity<super::editor_section::EditorSection>,
    remote: Entity<super::remote_section::RemoteSection>,
    docs_style: Entity<super::doc_style_section::DocStyleControls>,
    memory: Entity<super::memory_page::MemoryPage>,
    automation: Entity<super::automation_page::AutomationPage>,
    pub on_reset_layout: Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl SettingsPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let settings = SettingsStore::global(cx);
        let ph = cx.global::<I18n>().t("settings.search");
        let search = cx.new(|cx| TextInput::new(cx, ph));
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&search, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        cx.observe(&app, |s, a, cx| {
            let route = a.read(cx).current().to_string();
            // /memory dan /automation adalah alias bagian Pengaturan (seperti Tauri)
            let sec = match route.as_str() {
                "/appearance" => Some("appearance"),
                "/memory" => Some("memory"),
                "/automation" => Some("automation"),
                _ => route.strip_prefix("/settings/"),
            };
            if let Some(sec) = sec {
                if let Some((id, ..)) = SECTIONS.iter().find(|(id, ..)| *id == sec) {
                    s.active = id;
                }
            }
            cx.notify();
        })
        .detach();
        let providers = cx.new(super::providers_section::ProvidersSection::new);
        let agents = cx.new(super::acp_section::AcpSection::new);
        // Daftar model berubah (engine dimulai ulang atau agent CLI diaktifkan): muat ulang.
        cx.observe(&crate::core::services::EngineStatus::global(cx), |s, _, cx| s.load_models(cx)).detach();
        let profile = cx.new(super::profile_section::ProfileSection::new);
        let voice = cx.new(super::voice_section::VoiceSection::new);
        let themes = cx.new(super::themes_section::ThemesSection::new);
        let typography = cx.new(super::type_section::TypeSection::new);
        let chat_style = cx.new(super::chat_style_section::ChatStyleSection::new);
        let keys = cx.new(super::keys_section::KeysSection::new);
        let pstyles = cx.new(super::projects_style_section::ProjectsStyleSection::new);
        let editor = cx.new(super::editor_section::EditorSection::new);
        let remote = cx.new(super::remote_section::RemoteSection::new);
        let docs_style = cx.new(|cx| super::doc_style_section::DocStyleControls::new(super::doc_style_section::GROUPS.to_vec(), cx));
        let models_sec = cx.new(super::models_section::ModelsSection::new);
        let notif = cx.new(super::notification_section::NotificationSection::new);
        let memory = cx.new({
            let app = app.clone();
            move |cx| {
                let mut m = super::memory_page::MemoryPage::new(app, cx);
                m.embedded = true;
                m
            }
        });
        let automation = cx.new({
            let app = app.clone();
            move |cx| {
                let mut a = super::automation_page::AutomationPage::new(app, cx);
                a.embedded = true;
                a
            }
        });
        let mut me = Self { memory, automation, app, settings, search, active: "general", ap_tab: "themes", scroll: ScrollHandle::new(), models: None, on_reset_layout: None, providers, agents, models_sec, notif, profile, voice, themes, typography, chat_style, keys, pstyles, editor, remote, docs_style };
        me.load_models(cx);
        me
    }

    fn load_models(&mut self, cx: &mut Context<Self>) {
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let task = cx.background_spawn(async move { api.list_models().map_err(|e| e.0) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.models = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set(&self, key: &str, v: Value, cx: &mut App) {
        if let Err(e) = self.settings.update(cx, |s, cx| s.set(key, v, cx)) {
            Toasts::show(ToastInput { title: e.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
        }
    }
}

fn row(t: &Theme, label: String, desc: Option<String>, control: impl IntoElement, extra: Option<AnyElement>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(t.space(4))
        .py(t.space(3))
        .child(
            div()
                .flex()
                .flex_1()
                .flex_col()
                .gap(t.space(1))
                .min_w_0()
                .child(div().font_weight(FontWeight::MEDIUM).child(label))
                .children(desc.map(|d| div().small(t).text_color(t.colors.fg_muted).child(d))),
        )
        .child(div().flex().flex_none().justify_end().min_w(t.rem(rem::SPACE[7] * 4.)).child(control))
        .children(extra)
}

fn group(t: &Theme, title: String, rows: Vec<AnyElement>) -> AnyElement {
    let mut g = div()
        .flex()
        .flex_col()
        .mb(t.space(3))
        .py(t.space(3))
        .child(div().typo(t, H4).mb(t.space(2)).child(title));
    for (n, r) in rows.into_iter().enumerate() {
        g = g.child(div().when(n > 0, |d| d.border_t_1().border_color(t.colors.border_soft)).child(r));
    }
    g.into_any_element()
}

fn subs_id(v: &str) -> &'static str {
    ["themes", "type", "chat", "projects", "editor", "keys"].into_iter().find(|x| *x == v).unwrap_or("themes")
}

fn matches_q(q: &str, parts: &[&str]) -> bool {
    q.is_empty() || parts.iter().any(|p| p.to_lowercase().contains(q))
}

impl Render for SettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        let q = self.search.read(cx).text().trim().to_lowercase();
        let st = self.settings.read(cx);
        let show = |sec: &str| !q.is_empty() || self.active == sec;

        // ---- sisi ----
        let mut nav = div().flex().flex_col().gap(t.rem(0.125)).px(t.space(2)).pb(t.space(3));
        for (id, key, ic) in SECTIONS {
            let cur = q.is_empty() && self.active == id;
            nav = nav.child(
                div()
                    .id(SharedString::from(format!("sn-{id}")))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .px(t.space(3))
                    .py(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .text_color(if cur { t.colors.fg } else { t.colors.fg_muted })
                    .when(cur, |d| d.bg(t.colors.active))
                    .hover(|s| s.bg(t.colors.hover).text_color(t.colors.fg))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.active = id;
                        s.search.update(cx, |i, cx| i.set_text("", cx));
                        s.scroll.set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
                        cx.notify();
                    }))
                    .child(icon(ic).size(IconSize::Sm))
                    .child(i.t(key)),
            );
        }
        let side = div()
            .flex()
            .flex_none()
            .flex_col()
            .gap(t.space(3))
            .w(t.rem(rem::SPACE[7] * 6.))
            .bg(t.colors.bg)
            .border_r_1()
            .border_color(t.colors.border_soft)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .mx(t.space(3))
                    .mt(t.space(3))
                    .child(
                        icon_button("set-back", "arrow_left", i.t("mem.back"))
                            .on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/", cx)))),
                    )
                    .child(div().typo(&t, H4).child(i.t("settings.title"))),
            )
            .child(div().mx(t.space(3)).child(self.search.clone()))
            .child(div().id("set-nav").flex_1().overflow_y_scroll().child(nav));

        // ---- isi ----
        let mut content = div()
            .flex()
            .flex_col()
            .w_full()
            .px(t.space(4))
            .py(t.space(3));

        if show("profile") && matches_q(&q, &[&i.t("settings.profile"), &i.t("profile.name")]) {
            content = content.child(group(&t, i.t("settings.profile"), vec![div().py(t.space(2)).child(self.profile.clone()).into_any_element()]));
        }

        for g in GROUPS {
            if !show(g) {
                continue;
            }
            let mut rows: Vec<AnyElement> = vec![];
            for k in keys().into_iter().filter(|k| group_of(k) == g) {
                let p = &properties()[k];
                let meta = &p["x-zandor"];
                let title = i.t(meta["titleKey"].as_str().unwrap_or(k));
                let desc = meta["descriptionKey"].as_str().map(|d| i.t(d));
                if !matches_q(&q, &[&title, desc.as_deref().unwrap_or(""), k]) {
                    continue;
                }
                let value = st.get(k);
                let key: &'static str = k;
                let control: AnyElement = if p["type"] == "boolean" {
                    let me = cx.entity().downgrade();
                    switch(SharedString::from(format!("sw-{k}")), "", value.as_bool().unwrap_or(false))
                        .on_change(move |v, _, cx| {
                            me.update(cx, |s, cx| s.set(key, Value::Bool(v), cx)).ok();
                        })
                        .into_any_element()
                } else {
                    let num = p["type"] == "number";
                    let items: Vec<(SharedString, SharedString)> = p["enum"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|v| {
                                    let vs = match v {
                                        Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                    let label = meta["enumLabelKeys"][&vs]
                                        .as_str()
                                        .map(|lk| i.t(lk))
                                        .unwrap_or_else(|| {
                                            if num && (k == "uiScale" || k == "notifyVolume") {
                                                format!("{}%", (v.as_f64().unwrap_or(0.) * 100.).round())
                                            } else {
                                                vs.clone()
                                            }
                                        });
                                    (SharedString::from(vs), SharedString::from(label))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let cur = match &value {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let me = cx.entity().downgrade();
                    select(SharedString::from(format!("sel-{k}")), items, cur)
                        .on_change(move |v, _, cx| {
                            let val = if num { v.parse::<f64>().map(Value::from).unwrap_or(Value::Null) } else { Value::String(v.to_string()) };
                            me.update(cx, |s, cx| s.set(key, val, cx)).ok();
                        })
                        .into_any_element()
                };
                let explicit = st.is_explicit(k);
                let reset = button(SharedString::from(format!("rs-{k}")))
                    .label(i.t("settings.reset"))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .disabled(!explicit)
                    .on_click(cx.listener(move |s, _, _, cx| s.settings.update(cx, |st, cx| st.reset(key, cx))));
                rows.push(row(&t, title, desc, control, Some(reset.into_any_element())).into_any_element());
            }
            if g == "chat" && matches_q(&q, &[&i.t("voice.vocabulary"), &i.t("voice.corrections"), &i.t("settings.voiceLanguage")]) {
                rows.push(self.voice.clone().into_any_element());
            }
            if g == "layout" && matches_q(&q, &[&i.t("settings.layout.reset"), &i.t("settings.layout.resetBody")]) {
                rows.push(
                    row(
                        &t,
                        i.t("settings.layout.reset"),
                        Some(i.t("settings.layout.resetBody")),
                        button("layout-reset").label(i.t("settings.layout.resetButton")).size(ControlSize::Sm).on_click(cx.listener(
                            |s, _, w, cx| {
                                if let Some(f) = s.on_reset_layout.clone() {
                                    f(w, cx);
                                }
                            },
                        )),
                        None,
                    )
                    .into_any_element(),
                );
            }
            if g == "appearance" {
                let subs: [(&'static str, &str, &str, &str, &'static str); 6] = [
                    ("themes", "ap.tab.themes", "ap.title", "eye", "themes"),
                    ("type", "ap.tab.type", "ap.type.fonts", "file", "type"),
                    ("chat", "ap.tab.chat", "chatStyle.title", "chat", "chat"),
                    ("projects", "ap.tab.projects", "ap.projects.logo", "layout", "projects"),
                    ("editor", "ap.tab.editor", "ap.editor2.select", "wrench", "editor"),
                    ("keys", "ap.tab.keys", "ap.keys.change", "settings", "keys"),
                ];
                let searching = !q.is_empty();
                if !searching {
                    let me = cx.entity().downgrade();
                    let items = subs
                        .iter()
                        .map(|(id, k, _, ic, _)| TabItem { value: SharedString::from(*id), label: SharedString::from(i.t(k)), icon: Some(*ic), disabled: false })
                        .collect();
                    rows.push(
                        div()
                            .py(t.space(2))
                            .child(tabs("ap-tabs", items, self.ap_tab).on_change(move |v, _, cx| {
                                me.update(cx, |s, cx| {
                                    s.ap_tab = subs_id(&v);
                                    cx.notify();
                                })
                                .ok();
                            }))
                            .into_any_element(),
                    );
                }
                for (id, k, k2, _, _) in subs {
                    if !(searching || self.ap_tab == id) || !matches_q(&q, &[&i.t(k), &i.t(k2)]) {
                        continue;
                    }
                    let body = match id {
                        "themes" => self.themes.clone().into_any_element(),
                        "type" => self.typography.clone().into_any_element(),
                        "chat" => self.chat_style.clone().into_any_element(),
                        "projects" => self.pstyles.clone().into_any_element(),
                        "editor" => self.editor.clone().into_any_element(),
                        _ => self.keys.clone().into_any_element(),
                    };
                    let mut d = div().py(t.space(2));
                    if searching {
                        d = d.child(div().typo(&t, crate::theme::H4).mb(t.space(2)).child(i.t(k)));
                    }
                    rows.push(d.child(body).into_any_element());
                }
            }
            if g == "notifications" && matches_q(&q, &[&i.t("notif.permission.title"), &i.t("notif.testAll.title"), &i.t("notif.custom.title")]) {
                rows.push(div().child(self.notif.clone()).into_any_element());
            }
            if !rows.is_empty() {
                content = content.child(group(&t, i.t(&format!("settings.{g}")), rows));
            }
        }

        if show("documents") && matches_q(&q, &[&i.t("docStyle.section"), &i.t("docStyle.body"), &i.t("docStyle.title")]) {
            content = content.child(group(
                &t,
                i.t("docStyle.section"),
                vec![div().flex().flex_col().gap(t.space(3)).py(t.space(2)).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("docStyle.body"))).child(self.docs_style.clone()).into_any_element()],
            ));
        }
        if show("models") && matches_q(&q, &[&i.t("settings.models"), &i.t("ms.defaults"), &i.t("ms.providers"), &i.t("acp.title"), &i.t("acp.keywords")]) {
            let rows = vec![
                div().py(t.space(2)).child(self.models_sec.clone()).into_any_element(),
                div().py(t.space(2)).child(self.agents.clone()).into_any_element(),
                div().py(t.space(2)).child(self.providers.clone()).into_any_element(),
            ];
            content = content.child(group(&t, i.t("settings.models"), rows));
        }

        if show("engines") && matches_q(&q, &[&i.t("settings.engines"), &i.t("remote.intro"), &i.t("remote.add")]) {
            content = content.child(group(&t, i.t("settings.engines"), vec![div().py(t.space(2)).child(self.remote.clone()).into_any_element()]));
        }

        for (sec, key, body, el) in [
            ("memory", "settings.more.memory", "settings.more.memoryBody", self.memory.clone().into_any_element()),
            ("automation", "settings.more.automation", "settings.more.automationBody", self.automation.clone().into_any_element()),
        ] {
            if show(sec) && matches_q(&q, &[&i.t(key), &i.t(body), &i.t(if sec == "memory" { "mem.title" } else { "auto.title" })]) {
                content = content.child(group(&t, i.t(key), vec![div().py(t.space(2)).child(el).into_any_element()]));
            }
        }

        if show("data") && matches_q(&q, &[&i.t("settings.data"), &i.t("settings.export"), &i.t("settings.import"), &i.t("settings.resetAll")]) {
            let json = serde_json::to_string_pretty(&Value::Object(st.all())).unwrap_or_default();
            let explicit = st.explicit_json();
            let rows = vec![
                row(
                    &t,
                    i.t("settings.export"),
                    Some(i.t("settings.exportBody")),
                    button("set-export").label(i.t("settings.exportButton")).size(ControlSize::Sm).on_click(cx.listener(move |_, _, _, cx| {
                        let explicit = explicit.clone();
                        let rx = cx.prompt_for_new_path(&dirs_home(), Some("zandor-settings.json"));
                        cx.spawn(async move |_, _| {
                            if let Ok(Ok(Some(p))) = rx.await {
                                let _ = std::fs::write(p, format!("{explicit}\n"));
                            }
                        })
                        .detach();
                    })),
                    None,
                )
                .into_any_element(),
                row(
                    &t,
                    i.t("settings.import"),
                    Some(i.t("settings.importBody")),
                    button("set-import").label(i.t("settings.importButton")).size(ControlSize::Sm).on_click(cx.listener(|_, _, _, cx| {
                        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
                        cx.spawn(async move |this, cx| {
                            if let Ok(Ok(Some(paths))) = rx.await {
                                let text = paths.first().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
                                this.update(cx, |s, cx| {
                                    let errs = s.settings.update(cx, |st, cx| st.import(&text, cx));
                                    if !errs.is_empty() {
                                        let msg = cx.global::<I18n>().tp("settings.importError", &[("error", &errs.join("; "))]);
                                        Toasts::show(ToastInput { title: msg.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
                                    }
                                })
                                .ok();
                            }
                        })
                        .detach();
                    })),
                    None,
                )
                .into_any_element(),
                row(
                    &t,
                    i.t("settings.resetAll"),
                    Some(i.t("settings.resetAllHelp")),
                    button("set-resetall")
                        .label(i.t("settings.resetAll"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(|s, _, _, cx| s.settings.update(cx, |st, cx| st.reset_all(cx)))),
                    None,
                )
                .into_any_element(),
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .py(t.space(3))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(div().flex_1().font_weight(FontWeight::MEDIUM).child(i.t("settings.current")))
                            .child(icon_button("set-copyjson", "copy", i.t("chat.copy")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click({
                                let j = json.clone();
                                move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(j.clone()))
                            })),
                    )
                    .child(div().p(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised).code(&t).child(json))
                    .into_any_element(),
            ];
            content = content.child(group(&t, i.t("settings.data"), rows));
        }

        div()
            .flex()
            .size_full()
            .child(side)
            .child(
                div().flex_1().min_w_0().h_full().bg(t.colors.frame).p(t.space(4)).child(
                    div()
                        .id("set-scroll")
                        .size_full()
                        .rounded(t.rem(rem::RADIUS_LG))
                        .bg(t.colors.surface)
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(content),
                ),
            )
    }
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(Into::into).unwrap_or_else(std::env::temp_dir)
}
