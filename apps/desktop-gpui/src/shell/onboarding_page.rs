//! Onboarding (rute /welcome) — port OnboardingPage.tsx + onboarding-steps.tsx: Sambutan (bahasa + tema) → Profil →
//! Provider (daftar model per provider + formulir koneksi) → Pemeriksaan sistem → Memori & skill (mode) → Impor memori
//! dari AI lain (konflik wajib dipilih, daftar bisa dicentang/ditambah) → Terima kasih. Status dari data nyata.
use super::state::AppState;
use crate::core::mem;
use crate::core::mem_import::{build_memory, conflicts_of, lists_of, merge_memory, unresolved, value_id, ImportSource, KeyedFact, ListPick, Resolution};
use crate::core::services::{app_data_dir, EngineStatus};
use crate::core::settings::SettingsStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H1, H2, H4};
use crate::ui::{badge, button, icon, icon_button, select, BadgeTone, ButtonVariant, ControlSize, IconSize, InputChanged, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, ClipboardItem, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use std::collections::HashMap;

const STEPS: [&str; 7] = ["welcome", "profile", "provider", "system", "memory", "import", "thanks"];

/// Cara memasang alat yang belum ada: perintah winget + situs resmi.
fn install(id: &str) -> Option<(Option<&'static str>, &'static str)> {
    Some(match id {
        "python" => (Some("winget install -e --id Python.Python.3.12"), "https://www.python.org/downloads/"),
        "pythonOffice" => (Some("python -m pip install python-docx openpyxl python-pptx pypdf"), "https://pypi.org/"),
        "git" => (Some("winget install -e --id Git.Git"), "https://git-scm.com/download/win"),
        "node" | "npm" => (Some("winget install -e --id OpenJS.NodeJS.LTS"), "https://nodejs.org/"),
        "pip" => (Some("python -m ensurepip --upgrade"), "https://pip.pypa.io/en/stable/installation/"),
        "uv" => (Some("winget install -e --id astral-sh.uv"), "https://docs.astral.sh/uv/"),
        "gh" => (Some("winget install -e --id GitHub.cli"), "https://cli.github.com/"),
        "bun" => (Some("winget install -e --id Oven-sh.Bun"), "https://bun.sh/"),
        "opencode" => (Some("npm install -g opencode-ai"), "https://opencode.ai/"),
        "libreoffice" => (Some("winget install -e --id TheDocumentFoundation.LibreOffice"), "https://www.libreoffice.org/"),
        "winget" => (None, "https://aka.ms/getwinget"),
        _ => return None,
    })
}

/// (grup, id, opsional) — urutan sama dengan useSystemRows.
const CHECK_ROWS: [(&str, &str, bool); 12] = [
    ("core", "opencode", true),
    ("python", "python", false),
    ("python", "pip", false),
    ("python", "pythonOffice", true),
    ("python", "uv", true),
    ("dev", "git", false),
    ("dev", "gh", true),
    ("dev", "node", true),
    ("dev", "npm", true),
    ("dev", "bun", true),
    ("docs", "libreoffice", true),
    ("install", "winget", true),
];

const AI_SOURCES: [&str; 7] = ["chatgpt", "gemini", "claude", "deepseek", "grok", "copilot", "other"];
fn ai_label(s: &str) -> &'static str {
    match s {
        "chatgpt" => "ChatGPT",
        "gemini" => "Gemini",
        "claude" => "Claude",
        "deepseek" => "DeepSeek",
        "grok" => "Grok",
        "copilot" => "Copilot",
        _ => "",
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Missing,
    Optional,
    Checking,
    Unavailable,
}

struct Row {
    group: &'static str,
    id: &'static str,
    status: Status,
    version: Option<String>,
    note: Option<String>,
    install: Option<(Option<String>, &'static str)>,
}

pub struct OnboardingPage {
    app: Entity<AppState>,
    step: usize,
    /// None = sedang memeriksa.
    checks: Option<Vec<crate::syscheck::Check>>,
    models: Option<Vec<crate::core::engine::ModelOption>>,
    profile: Entity<super::profile_section::ProfileSection>,
    providers: Entity<super::providers_section::ProvidersSection>,
    adding: bool,
    mem_mode: Option<&'static str>,
    skill_mode: Option<&'static str>,
    // ---- impor ----
    ai: String,
    paste: Entity<TextInput>,
    sources: Vec<ImportSource>,
    existing: String,
    res: HashMap<String, Resolution>,
    drafts: HashMap<String, Entity<TextInput>>,
    picks: HashMap<String, ListPick>,
    extra_inputs: HashMap<String, Vec<Entity<TextInput>>>,
    saved: bool,
}

impl OnboardingPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&EngineStatus::global(cx), |s, _, cx| {
            s.load_models(cx);
            cx.notify();
        })
        .detach();
        let profile = cx.new(|cx| {
            let mut p = super::profile_section::ProfileSection::new(cx);
            p.heading = false;
            p
        });
        let providers = cx.new(|cx| {
            let mut p = super::providers_section::ProvidersSection::new(cx);
            p.form_only = true;
            p
        });
        cx.observe(&providers, |s, p, cx| {
            if s.adding && !p.read(cx).form_open() {
                s.adding = false;
                s.load_models(cx);
            }
            cx.notify();
        })
        .detach();
        let paste = cx.new(|cx| {
            let mut t = TextInput::new(cx, "");
            t.multiline = true;
            t.enter_newline = true;
            t
        });
        cx.subscribe(&paste, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        let existing = crate::memory::read_memory(&app_data_dir().join("memory"), mem::FILE_GLOBAL).unwrap_or_default();
        let mut me = Self {
            app,
            step: 0,
            checks: None,
            models: None,
            profile,
            providers,
            adding: false,
            mem_mode: None,
            skill_mode: None,
            ai: "chatgpt".into(),
            paste,
            sources: vec![],
            existing,
            res: HashMap::new(),
            drafts: HashMap::new(),
            picks: HashMap::new(),
            extra_inputs: HashMap::new(),
            saved: false,
        };
        me.load_models(cx);
        me
    }

    fn load_models(&mut self, cx: &mut Context<Self>) {
        let Some(api) = crate::core::projects::ProjectStore::global(cx).read(cx).api() else { return };
        let t = cx.background_spawn(async move { api.list_models().unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            let m = t.await;
            this.update(cx, |s, cx| {
                s.models = Some(m);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn run_checks(&mut self, cx: &mut Context<Self>) {
        self.checks = None;
        cx.notify();
        let task = cx.background_spawn(async { crate::syscheck::check_all() });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.checks = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn merged_sources(&self) -> Vec<ImportSource> {
        let mut v = vec![];
        if !self.existing.trim().is_empty() {
            v.push(ImportSource { id: "zandor".into(), label: "Zandor".into(), text: self.existing.clone() });
        }
        v.extend(self.sources.iter().cloned());
        v
    }

    fn sync_picks(&mut self, cx: &App) {
        for (k, inputs) in &self.extra_inputs {
            let extra = inputs.iter().map(|e| e.read(cx).text().to_string()).collect();
            self.picks.entry(k.clone()).or_default().extra = extra;
        }
        for (k, e) in &self.drafts {
            if matches!(self.res.get(k), Some(Resolution::Custom(_))) {
                self.res.insert(k.clone(), Resolution::Custom(e.read(cx).text().to_string()));
            }
        }
    }

    fn save_import(&mut self, cx: &mut Context<Self>) -> bool {
        self.sync_picks(cx);
        let preview = build_memory(&merge_memory(&self.merged_sources()), &self.res, &self.picks);
        let i = cx.global::<I18n>().clone();
        let _ = std::fs::create_dir_all(app_data_dir().join("memory"));
        match crate::memory::write_memory(&app_data_dir().join("memory"), mem::FILE_GLOBAL, &preview) {
            Ok(()) => {
                self.existing = preview;
                self.sources.clear();
                self.res.clear();
                self.picks.clear();
                self.drafts.clear();
                self.extra_inputs.clear();
                self.saved = true;
                Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(i.t("onb.import.saved")) }, cx);
                cx.notify();
                true
            }
            Err(e) => {
                Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(i.tp("profile.error", &[("error", &e)])) }, cx);
                false
            }
        }
    }

    /// Penjaga impor: impor belum disimpan disimpan otomatis; konflik terbuka menahan di langkah ini.
    fn leave_ok(&mut self, cx: &mut Context<Self>) -> bool {
        if STEPS[self.step] != "import" || self.sources.is_empty() {
            return true;
        }
        self.sync_picks(cx);
        let m = merge_memory(&self.merged_sources());
        if build_memory(&m, &self.res, &self.picks).is_empty() {
            return true;
        }
        let open = unresolved(&m, &self.res).len();
        if open > 0 {
            let title = cx.global::<I18n>().tp("onb.import.unresolved", &[("n", &open.to_string())]);
            Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
            return false;
        }
        self.save_import(cx)
    }

    fn go(&mut self, n: usize, cx: &mut Context<Self>) {
        if !self.leave_ok(cx) {
            return;
        }
        self.step = n.min(STEPS.len() - 1);
        match STEPS[self.step] {
            "system" => {
                self.run_checks(cx);
                self.load_models(cx);
            }
            "provider" => self.load_models(cx),
            _ => {}
        }
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let st = SettingsStore::global(cx);
        let _ = st.update(cx, |s, cx| s.set("onboarding", serde_json::Value::Bool(false), cx));
        self.app.update(cx, |a, cx| a.push("/", cx));
    }

    fn set_mode(&mut self, kind: &str, m: &'static str, cx: &mut Context<Self>) {
        let dir = app_data_dir().join("memory");
        let mut s = mem::parse_settings(&crate::memory::read_memory(&dir, mem::FILE_SETTINGS).unwrap_or_default());
        if kind == "mem" {
            self.mem_mode = Some(m);
            s.default_enabled = m == "auto";
        } else {
            self.skill_mode = Some(m);
            s.skill_creator = m == "auto";
        }
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = crate::memory::write_memory(&dir, mem::FILE_SETTINGS, &mem::serialize_settings(&s)) {
            let title = cx.global::<I18n>().tp("profile.error", &[("error", &e)]);
            Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
        }
        cx.notify();
    }

    fn rows(&self, cx: &App) -> Vec<Row> {
        let engine = EngineStatus::global(cx).read(cx).state.clone();
        let opencode = self.models.as_ref().map(|m| m.iter().filter(|x| x.provider_id == "opencode").count());
        let views = crate::providers::ProviderStore::new(&app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE))).views().unwrap_or_default();
        let routers: Vec<String> = views.iter().filter(|p| p.id == "openrouter" || !matches!(p.kind, crate::providers::Kind::Builtin)).map(|p| p.name.clone()).collect();
        let mut rows = vec![
            Row { group: "core", id: "engine", status: if engine.status == "ready" { Status::Ok } else if engine.status == "error" { Status::Missing } else { Status::Checking }, version: engine.version.clone(), note: engine.error.clone(), install: None },
            Row { group: "core", id: "opencodeModels", status: match opencode { None => Status::Checking, Some(0) => Status::Optional, _ => Status::Ok }, version: opencode.filter(|n| *n > 0).map(|n| n.to_string()), note: None, install: None },
            Row { group: "core", id: "router", status: if routers.is_empty() { Status::Optional } else { Status::Ok }, version: None, note: (!routers.is_empty()).then(|| routers.join(", ")), install: None },
        ];
        for (group, id, optional) in CHECK_ROWS {
            let mut r = Row { group, id, status: Status::Checking, version: None, note: None, install: None };
            if let Some(checks) = &self.checks {
                match checks.iter().find(|c| c.id == id) {
                    None => r.status = Status::Unavailable,
                    Some(c) => {
                        r.status = if c.found { Status::Ok } else if optional { Status::Optional } else { Status::Missing };
                        let inst = install(id);
                        if id == "pythonOffice" {
                            let py = checks.iter().find(|x| x.id == "python").and_then(|x| x.detail.clone()).unwrap_or_else(|| "python".into());
                            r.note = (!c.found).then(|| c.detail.clone()).flatten();
                            r.install = inst.map(|(cmd, url)| (cmd.map(|c| c.replacen("python", &py, 1)), url));
                        } else {
                            r.version = c.version.clone();
                            r.note = (id == "python" && c.found).then(|| c.detail.clone()).flatten();
                            if !c.found {
                                r.install = inst.map(|(cmd, url)| (cmd.map(str::to_string), url));
                            }
                        }
                    }
                }
            }
            rows.push(r);
        }
        rows
    }
}

fn muted(t: &Theme, s: String) -> gpui::Div {
    div().small(t).text_color(t.colors.fg_muted).child(s)
}

/// Pilihan bersegmen ringkas (Segmented di onboarding-steps.tsx).
fn segmented(t: &Theme, id: &str, legend: String, options: Vec<(String, String)>, value: &str, on: impl Fn(String, &mut Window, &mut App) + 'static) -> AnyElement {
    let on = std::rc::Rc::new(on);
    let mut row = div().flex().flex_wrap().gap(t.space(1)).p(t.rem(0.125)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised);
    for (v, label) in options {
        let cur = v == value;
        let on = on.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("{id}-{v}")))
                .px(t.space(3))
                .py(t.space(1))
                .rounded(t.rem(rem::RADIUS_SM))
                .cursor_pointer()
                .small(t)
                .when(cur, |d| d.bg(t.colors.surface).font_weight(FontWeight::MEDIUM))
                .text_color(if cur { t.colors.fg } else { t.colors.fg_muted })
                .hover(|d| d.text_color(t.colors.fg))
                .on_click(move |_, w, cx| on(v.clone(), w, cx))
                .child(label),
        );
    }
    div().flex().flex_col().gap(t.space(1)).child(div().small(t).font_weight(FontWeight::MEDIUM).child(legend)).child(row).into_any_element()
}

/// Opsi radio/centang berlabel (onb__option).
fn option_row(t: &Theme, id: SharedString, checked: bool, radio: bool, title: String, hint: String, on: impl Fn(&mut Window, &mut App) + 'static) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_start()
        .gap(t.space(2))
        .p(t.space(2))
        .rounded(t.rem(rem::RADIUS_MD))
        .border_1()
        .border_color(if checked { t.colors.accent } else { t.colors.border_soft })
        .cursor_pointer()
        .hover(|d| d.bg(t.colors.hover))
        .on_click(move |_, w, cx| on(w, cx))
        .child(
            div()
                .flex_none()
                .mt(t.rem(0.15))
                .size(t.rem(1.))
                .when(radio, |d| d.rounded_full())
                .when(!radio, |d| d.rounded(t.rem(rem::RADIUS_SM)))
                .border_1()
                .border_color(if checked { t.colors.accent } else { t.colors.border })
                .when(checked, |d| d.bg(t.colors.accent))
                .flex()
                .items_center()
                .justify_center()
                .when(checked && !radio, |d| d.child(icon("check").size(IconSize::Sm).color(t.colors.on_accent))),
        )
        .child(div().flex().flex_col().min_w_0().child(div().font_weight(FontWeight::MEDIUM).child(title)).child(muted(t, hint)))
        .into_any_element()
}

impl OnboardingPage {
    fn welcome(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let st = SettingsStore::global(cx).read(cx);
        let (lang, theme) = (st.str("language"), st.str("theme"));
        let list = STEPS[1..6].iter().map(|s| i.t(&format!("onb.{s}.title")).to_lowercase()).collect::<Vec<_>>().join(", ");
        let set = |key: &'static str| {
            move |v: String, _: &mut Window, cx: &mut App| {
                let st = SettingsStore::global(cx);
                let _ = st.update(cx, |s, cx| s.set(key, serde_json::Value::String(v), cx));
            }
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(t.space(4))
            .child(div().size(t.rem(3.5)).rounded(t.rem(rem::RADIUS_LG)).bg(t.colors.accent).flex().items_center().justify_center().child(icon("app_logo").size(IconSize::Lg).color(t.colors.on_accent)))
            .child(div().typo(t, H1).child(i.t("onb.title")))
            .child(div().text_color(t.colors.fg_muted).child(i.t("onb.welcome.body")))
            .child(div().text_color(t.colors.fg_muted).child(i.tp("onb.welcome.setup", &[("list", &list)])))
            .child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .child(button("onb-start").variant(ButtonVariant::Solid).label(i.t("onb.welcome.start")).child(icon("arrow_right").size(IconSize::Sm)).on_click(cx.listener(|s, _, _, cx| s.go(1, cx))))
                    .child(button("onb-skip").variant(ButtonVariant::Ghost).label(i.t("onb.skip")).on_click(cx.listener(|s, _, _, cx| s.finish(cx)))),
            )
            .child(segmented(t, "onb-lang", i.t("settings.language"), vec![("id".into(), i.t("language.id")), ("en".into(), i.t("language.en"))], &lang, set("language")))
            .child(segmented(t, "onb-theme", i.t("theme.label"), ["system", "light", "dark"].iter().map(|v| (v.to_string(), i.t(&format!("theme.{v}")))).collect(), &theme, set("theme")))
            .into_any_element()
    }

    fn provider_step(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        if self.adding {
            return self.providers.clone().into_any_element();
        }
        let mut col = div().flex().flex_col().gap(t.space(3));
        col = col.child(match &self.models {
            None => div().flex().items_center().gap(t.space(2)).child(crate::ui::spinner(IconSize::Sm)).child(i.t("tour.model.loading")),
            Some(m) if !m.is_empty() => div().flex().items_center().gap(t.space(2)).text_color(t.colors.success).child(icon("check").size(IconSize::Sm)).child(i.tp("tour.model.ready", &[("n", &m.len().to_string())])),
            Some(_) => div().child(i.t("tour.model.none")),
        });
        let mut by: Vec<(String, usize)> = vec![];
        for m in self.models.iter().flatten() {
            match by.iter_mut().find(|(n, _)| *n == m.provider_name) {
                Some(x) => x.1 += 1,
                None => by.push((m.provider_name.clone(), 1)),
            }
        }
        if !by.is_empty() {
            let mut list = div().flex().flex_col();
            for (name, n) in by {
                list = list.child(div().flex().items_center().gap(t.space(2)).py(t.space(2)).border_b_1().border_color(t.colors.border_soft).child(div().flex_1().font_weight(FontWeight::MEDIUM).child(name)).child(badge(i.tp("onb.provider.models", &[("n", &n.to_string())]))));
            }
            col = col.child(list);
        }
        col.child(div().child(button("onb-prov-add").variant(ButtonVariant::Solid).label(i.t("onb.provider.add")).on_click(cx.listener(|s, _, _, cx| {
            s.adding = true;
            s.providers.update(cx, |p, cx| p.open_form(None, cx));
            cx.notify();
        }))))
        .child(muted(t, i.t("onb.provider.hint")))
        .into_any_element()
    }

    fn system_step(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.rows(cx);
        let mut col = div().flex().flex_col().gap(t.space(4));
        for g in ["core", "python", "dev", "docs", "install"] {
            let mut sec = div().flex().flex_col().gap(t.space(2)).child(div().typo(t, H4).child(i.t(&format!("onb.sys.group.{g}"))));
            for r in rows.iter().filter(|r| r.group == g) {
                let status: AnyElement = match r.status {
                    Status::Checking => div().flex().items_center().gap(t.space(1)).small(t).text_color(t.colors.fg_muted).child(crate::ui::spinner(IconSize::Sm)).child(i.t("onb.sys.checking")).into_any_element(),
                    Status::Ok => badge(r.version.as_ref().map(|v| i.tp("onb.sys.okVersion", &[("v", v)])).unwrap_or_else(|| i.t("onb.sys.ok"))).into_any_element(),
                    s => badge(i.t(match s {
                        Status::Missing => "onb.sys.missing",
                        Status::Optional => "onb.sys.optional",
                        _ => "onb.sys.unavailable",
                    }))
                    .tone(BadgeTone::Outline)
                    .into_any_element(),
                };
                let mut item = div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .border_1()
                    .border_color(if r.status == Status::Missing { t.colors.danger } else { t.colors.border_soft })
                    .child(div().flex().items_center().gap(t.space(2)).child(div().flex_1().font_weight(FontWeight::MEDIUM).child(i.t(&format!("onb.sys.{}", r.id)))).child(status))
                    .child(muted(t, i.t(&format!("onb.sys.{}.why", r.id))))
                    .children(r.note.clone().map(|n| div().small(t).text_color(t.colors.fg_subtle).child(n)));
                if let Some((cmd, url)) = r.install.clone().filter(|_| matches!(r.status, Status::Missing | Status::Optional)) {
                    let mut btns = div().flex().gap(t.space(2));
                    if let Some(c) = cmd.clone() {
                        btns = btns.child(button(SharedString::from(format!("onb-cp-{}", r.id))).size(ControlSize::Sm).child(icon("copy").size(IconSize::Sm)).label(i.t("onb.sys.copy")).on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(c.clone()));
                            let title = cx.global::<I18n>().t("onb.sys.copied");
                            Toasts::show(ToastInput { timeout_ms: Some(2500), ..ToastInput::new(title) }, cx);
                        }));
                    }
                    btns = btns.child(button(SharedString::from(format!("onb-site-{}", r.id))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).child(icon("external_link").size(IconSize::Sm)).label(i.t("onb.sys.site")).on_click(move |_, _, cx| cx.open_url(url)));
                    item = item.child(div().flex().flex_col().gap(t.space(1)).children(cmd.map(|c| div().code(t).px(t.space(2)).py(t.space(1)).rounded(t.rem(rem::RADIUS_SM)).bg(t.colors.raised).child(c))).child(btns));
                }
                sec = sec.child(item);
            }
            col = col.child(sec);
        }
        col.child(div().child(button("onb-recheck").child(icon("refresh").size(IconSize::Sm)).label(i.t("onb.sys.recheck")).on_click(cx.listener(|s, _, _, cx| s.run_checks(cx))))).into_any_element()
    }

    fn memory_step(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let s = mem::parse_settings(&crate::memory::read_memory(&app_data_dir().join("memory"), mem::FILE_SETTINGS).unwrap_or_default());
        let mem_v = self.mem_mode.unwrap_or(if s.default_enabled { "auto" } else { "manual" });
        let skill_v = self.skill_mode.unwrap_or(if s.skill_creator { "auto" } else { "manual" });
        let me = cx.entity().downgrade();
        let choice = |kind: &'static str, value: &'static str| {
            let me = me.clone();
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_MD))
                .border_1()
                .border_color(t.colors.border_soft)
                .child(segmented(t, &format!("onb-{kind}"), i.t(&format!("onb.{kind}.legend")), ["auto", "manual", "off"].iter().map(|m| (m.to_string(), i.t(&format!("onb.mode.{m}")))).collect(), value, move |v, _, cx| {
                    let m: &'static str = match v.as_str() {
                        "auto" => "auto",
                        "off" => "off",
                        _ => "manual",
                    };
                    me.update(cx, |s, cx| s.set_mode(kind, m, cx)).ok();
                }))
                .child(muted(t, i.t(&format!("onb.{kind}.what"))))
                .child(div().small(t).text_color(t.colors.success).child(format!("+ {}", i.t(&format!("onb.{kind}.{value}.pro")))))
                .child(div().small(t).text_color(t.colors.danger).child(format!("− {}", i.t(&format!("onb.{kind}.{value}.con")))))
        };
        div().flex().flex_col().gap(t.space(3)).child(choice("mem", mem_v)).child(choice("skill", skill_v)).child(muted(t, i.t("onb.memory.later"))).into_any_element()
    }

    fn import_step(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        self.sync_picks(cx);
        let merged = merge_memory(&self.merged_sources());
        let conflicts: Vec<KeyedFact> = conflicts_of(&merged).into_iter().cloned().collect();
        let lists: Vec<KeyedFact> = lists_of(&merged).into_iter().cloned().collect();
        let open = unresolved(&merged, &self.res).len();
        let preview = build_memory(&merged, &self.res, &self.picks);
        let other = i.t("onb.import.other");
        let ai_name = |s: &str| if ai_label(s).is_empty() { other.clone() } else { ai_label(s).to_string() };
        let src_names: HashMap<String, String> = self.sources.iter().map(|s| (s.id.clone(), s.label.clone())).collect();
        let existing_label = i.t("onb.import.existing");
        let source_name = |id: &str| if id == "zandor" { existing_label.clone() } else { src_names.get(id).cloned().unwrap_or_else(|| id.to_string()) };
        let me = cx.entity().downgrade();
        let prompt = i.t("onb.import.prompt");
        let pre = |s: String| div().p(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised).code(t).child(s);

        let mut col = div()
            .flex()
            .flex_col()
            .gap(t.space(4))
            .child(
                div().flex().flex_col().gap(t.space(2)).child(div().typo(t, H4).child(i.t("onb.import.step1"))).child(pre(prompt.clone())).child(div().child(button("onb-imp-copy").child(icon("copy").size(IconSize::Sm)).label(i.t("onb.import.copy")).on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(prompt.clone()));
                    let title = cx.global::<I18n>().t("onb.import.copied");
                    Toasts::show(ToastInput { timeout_ms: Some(2500), ..ToastInput::new(title) }, cx);
                }))),
            );
        let ai_items: Vec<(SharedString, SharedString)> = AI_SOURCES.iter().map(|s| ((*s).into(), ai_name(s).into())).collect();
        let empty_paste = self.paste.read(cx).text().trim().is_empty();
        let mut step2 = div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .child(div().typo(t, H4).child(i.t("onb.import.step2")))
            .child(select("onb-ai", ai_items, self.ai.clone()).on_change({
                let me = me.clone();
                move |v, _, cx| {
                    let v = v.to_string();
                    me.update(cx, |s, cx| {
                        s.ai = v;
                        cx.notify();
                    })
                    .ok();
                }
            }))
            .child(div().min_h(t.rem(8.)).p(t.space(2)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).child(self.paste.clone()))
            .child(div().child(button("onb-imp-add").disabled(empty_paste).label(i.t("onb.import.add")).on_click(cx.listener(|s, _, _, cx| {
                let text = s.paste.read(cx).text().to_string();
                let other = cx.global::<I18n>().t("onb.import.other");
                let base = if ai_label(&s.ai).is_empty() { other } else { ai_label(&s.ai).to_string() };
                let n = s.sources.iter().filter(|x| x.label == base || x.label.starts_with(&format!("{base} "))).count();
                let label = if n > 0 { format!("{base} {}", n + 1) } else { base };
                s.sources.push(ImportSource { id: format!("{}-{}", s.ai, s.sources.len()), label, text });
                s.paste.update(cx, |p, cx| p.set_text("", cx));
                s.saved = false;
                cx.notify();
            }))));
        if !self.sources.is_empty() {
            let mut chips = div().flex().flex_wrap().gap(t.space(2));
            for src in &self.sources {
                let id = src.id.clone();
                chips = chips.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(1))
                        .child(badge(src.label.clone()))
                        .child(button(SharedString::from(format!("onb-src-x-{}", src.id))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.tp("onb.import.remove", &[("name", &src.label)])).on_click(cx.listener(move |s, _, _, cx| {
                            s.sources.retain(|x| x.id != id);
                            cx.notify();
                        }))),
                );
            }
            step2 = step2.child(chips);
        }
        if merged.secrets > 0 {
            step2 = step2.child(div().small(t).text_color(t.colors.danger).child(i.tp("onb.import.secrets", &[("n", &merged.secrets.to_string())])));
        }
        col = col.child(step2);

        if !conflicts.is_empty() {
            let mut sec = div().flex().flex_col().gap(t.space(2)).child(div().typo(t, H4).child(i.tp("onb.import.conflicts", &[("n", &conflicts.len().to_string())]))).child(muted(t, i.t("onb.import.conflictsHint")));
            for c in conflicts {
                let mut fs = div().flex().flex_col().gap(t.space(1)).child(div().small(t).font_weight(FontWeight::SEMIBOLD).child(c.label.clone()));
                for (k, o) in c.options.iter().enumerate() {
                    let (me, key) = (me.clone(), c.key.clone());
                    let checked = self.res.get(&c.key) == Some(&Resolution::Pick(k));
                    let from = i.tp("onb.import.from", &[("list", &o.sources.iter().map(|s| source_name(s)).collect::<Vec<_>>().join(", "))]);
                    fs = fs.child(option_row(t, SharedString::from(format!("onb-c-{}-{k}", c.key)), checked, true, o.value.clone(), from, move |_, cx| {
                        me.update(cx, |s, cx| {
                            s.res.insert(key.clone(), Resolution::Pick(k));
                            cx.notify();
                        })
                        .ok();
                    }));
                }
                let custom = matches!(self.res.get(&c.key), Some(Resolution::Custom(_)));
                let (me2, key) = (me.clone(), c.key.clone());
                fs = fs.child(option_row(t, SharedString::from(format!("onb-c-{}-custom", c.key)), custom, true, i.t("onb.import.custom"), i.t("onb.import.customHint"), move |w, cx| {
                    me2.update(cx, |s, cx| {
                        let ed = s
                            .drafts
                            .entry(key.clone())
                            .or_insert_with(|| {
                                let e = cx.new(|cx| TextInput::new(cx, cx.global::<I18n>().t("onb.import.customPlaceholder")));
                                cx.subscribe(&e, |_, _, _: &InputChanged, cx| cx.notify()).detach();
                                e
                            })
                            .clone();
                        let text = ed.read(cx).text().to_string();
                        s.res.insert(key.clone(), Resolution::Custom(text));
                        gpui::Focusable::focus_handle(ed.read(cx), cx).focus(w, cx);
                        cx.notify();
                    })
                    .ok();
                }));
                if custom {
                    if let Some(ed) = self.drafts.get(&c.key) {
                        fs = fs.child(div().p(t.space(1)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).child(ed.clone()));
                    }
                }
                sec = sec.child(fs);
            }
            col = col.child(sec);
        }

        if !lists.is_empty() {
            let mut sec = div().flex().flex_col().gap(t.space(2)).child(div().typo(t, H4).child(i.t("onb.import.lists"))).child(muted(t, i.t("onb.import.listsHint")));
            for l in lists {
                let pick = self.picks.get(&l.key).cloned().unwrap_or_default();
                let mut fs = div().flex().flex_col().gap(t.space(1)).child(div().small(t).font_weight(FontWeight::SEMIBOLD).child(l.label.clone()));
                for o in &l.options {
                    let vid = value_id(&o.value);
                    let on = !pick.excluded.contains(&vid);
                    let (me, key) = (me.clone(), l.key.clone());
                    let from = i.tp("onb.import.from", &[("list", &o.sources.iter().map(|s| source_name(s)).collect::<Vec<_>>().join(", "))]);
                    fs = fs.child(option_row(t, SharedString::from(format!("onb-l-{}-{vid}", l.key)), on, false, o.value.clone(), from, move |_, cx| {
                        me.update(cx, |s, cx| {
                            let p = s.picks.entry(key.clone()).or_default();
                            if on {
                                p.excluded.push(vid.clone());
                            } else {
                                p.excluded.retain(|x| *x != vid);
                            }
                            cx.notify();
                        })
                        .ok();
                    }));
                }
                for (n, e) in self.extra_inputs.get(&l.key).cloned().unwrap_or_default().into_iter().enumerate() {
                    let key = l.key.clone();
                    fs = fs.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(1))
                            .child(div().flex_1().p(t.space(1)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).child(e))
                            .child(icon_button(SharedString::from(format!("onb-lx-{}-{n}", l.key)), "close", i.t("onb.import.removeValue")).on_click(cx.listener(move |s, _, _, cx| {
                                if let Some(v) = s.extra_inputs.get_mut(&key) {
                                    if n < v.len() {
                                        v.remove(n);
                                    }
                                }
                                s.sync_picks(cx);
                                cx.notify();
                            }))),
                    );
                }
                let key = l.key.clone();
                fs = fs.child(div().child(button(SharedString::from(format!("onb-la-{}", l.key))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).child(icon("plus").size(IconSize::Sm)).label(i.t("onb.import.addValue")).on_click(cx.listener(move |s, _, w, cx| {
                    let ph = cx.global::<I18n>().t("onb.import.customPlaceholder");
                    let e = cx.new(|cx| TextInput::new(cx, ph));
                    cx.subscribe(&e, |_, _, _: &InputChanged, cx| cx.notify()).detach();
                    gpui::Focusable::focus_handle(e.read(cx), cx).focus(w, cx);
                    s.extra_inputs.entry(key.clone()).or_default().push(e);
                    cx.notify();
                }))));
                sec = sec.child(fs);
            }
            col = col.child(sec);
        }

        if !self.sources.is_empty() {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .child(div().typo(t, H4).child(i.t("onb.import.preview")))
                    .child(pre(if preview.is_empty() { i.t("onb.import.empty") } else { preview.clone() }))
                    .when(open > 0, |d| d.child(muted(t, i.tp("onb.import.unresolved", &[("n", &open.to_string())]))))
                    .child(div().child(button("onb-imp-save").variant(ButtonVariant::Solid).disabled(open > 0 || preview.is_empty()).label(i.t("onb.import.save")).on_click(cx.listener(|s, _, _, cx| {
                        s.save_import(cx);
                    })))),
            );
        }
        if self.saved {
            col = col.child(div().flex().items_center().gap(t.space(1)).text_color(t.colors.success).child(icon("check").size(IconSize::Sm)).child(i.t("onb.import.saved")));
            let s = mem::parse_settings(&crate::memory::read_memory(&app_data_dir().join("memory"), mem::FILE_SETTINGS).unwrap_or_default());
            if !s.default_enabled {
                col = col.child(muted(t, i.t("onb.import.memOff")));
            }
        }
        col.child(muted(t, i.t("onb.import.optional"))).into_any_element()
    }
}

impl Render for OnboardingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let step = STEPS[self.step];
        let frame = |inner: AnyElement| div().id("onb").size_full().overflow_y_scroll().bg(t.colors.frame).child(div().flex().flex_col().w_full().max_w(t.rem(rem::SPACE[7] * 12.)).mx_auto().px(t.space(4)).py(t.space(6)).child(inner));
        if step == "welcome" {
            let w = self.welcome(&t, &i, cx);
            return frame(w);
        }
        let body = match step {
            "profile" => self.profile.clone().into_any_element(),
            "provider" => self.provider_step(&t, &i, cx),
            "system" => self.system_step(&t, &i, cx),
            "memory" => self.memory_step(&t, &i, cx),
            "import" => self.import_step(&t, &i, cx),
            _ => div()
                .flex()
                .flex_col()
                .items_start()
                .gap(t.space(3))
                .child(div().size(t.rem(3.5)).rounded(t.rem(rem::RADIUS_LG)).bg(t.colors.accent).flex().items_center().justify_center().child(icon("app_logo").size(IconSize::Lg).color(t.colors.on_accent)))
                .child(div().child(i.t("onb.thanks.text")))
                .child(muted(&t, i.t("onb.thanks.tip")))
                .into_any_element(),
        };
        // bilah progres hanya untuk langkah setup (tanpa sambutan)
        let at = self.step - 1;
        let n_setup = STEPS.len() - 1;
        let step_label = i.tp("tour.step", &[("i", &(at + 1).to_string()), ("n", &n_setup.to_string())]);
        let mut dots = div().flex().gap(t.space(1)).w_full();
        for (n, s) in STEPS[1..].iter().enumerate() {
            let (cur, done) = (n == at, n < at);
            dots = dots.child(
                div()
                    .id(SharedString::from(format!("onb-dot-{s}")))
                    .flex_1()
                    .h(t.rem(0.375))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if cur || done { t.colors.accent } else { t.colors.raised })
                    .when(done, |d| d.opacity(0.6))
                    .tooltip(crate::ui::tooltip::text_tooltip(i.t(&format!("onb.{s}.title")).into()))
                    .on_click(cx.listener(move |me, _, _, cx| me.go(n + 1, cx))),
            );
        }
        let last = self.step + 1 == STEPS.len();
        let foot = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .pt(t.space(4))
            .mt(t.space(4))
            .border_t_1()
            .border_color(t.colors.border_soft)
            .child(button("onb-back").child(icon("arrow_left").size(IconSize::Sm)).label(i.t("tour.back")).on_click(cx.listener(|s, _, _, cx| {
                let n = s.step - 1;
                s.go(n, cx)
            })))
            .child(div().flex_1())
            .child(if last {
                button("onb-finish").variant(ButtonVariant::Solid).label(i.t("onb.finish")).child(icon("arrow_right").size(IconSize::Sm)).on_click(cx.listener(|s, _, _, cx| {
                    if s.leave_ok(cx) {
                        s.finish(cx)
                    }
                }))
            } else {
                button("onb-next").variant(ButtonVariant::Solid).label(i.t("tour.next")).child(icon("arrow_right").size(IconSize::Sm)).on_click(cx.listener(|s, _, _, cx| {
                    let n = s.step + 1;
                    s.go(n, cx)
                }))
            });
        frame(
            div()
                .flex()
                .flex_col()
                .gap(t.space(4))
                .child(dots)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(2))
                        .child(muted(&t, step_label))
                        .child(div().typo(&t, H2).child(i.t(&format!("onb.{step}.title"))))
                        .child(div().text_color(t.colors.fg_muted).child(i.t(&format!("onb.{step}.body"))))
                        .child(div().pt(t.space(2)).child(body)),
                )
                .child(foot)
                .into_any_element(),
        )
    }
}
