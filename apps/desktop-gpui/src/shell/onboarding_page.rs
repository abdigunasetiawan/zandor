//! Onboarding (rute /welcome) — port OnboardingPage.tsx: Sambutan → Profil → Provider → Pemeriksaan sistem →
//! Memori & skill → Impor memori → Terima kasih. Pemeriksaan sistem memakai syscheck.rs bersama.
use super::state::AppState;
use crate::core::services::EngineStatus;
use crate::core::settings::SettingsStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H2, H4};
use crate::ui::{avatar, badge, button, icon, icon_button, BadgeTone, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, ClipboardItem, Context, Entity, FontWeight, IntoElement, SharedString, Window};

const STEPS: [&str; 7] = ["welcome", "profile", "provider", "system", "memory", "import", "thanks"];

const INSTALL: [(&str, &str); 11] = [
    ("python", "winget install -e --id Python.Python.3.12"),
    ("pythonOffice", "python -m pip install python-docx openpyxl python-pptx pypdf"),
    ("git", "winget install -e --id Git.Git"),
    ("node", "winget install -e --id OpenJS.NodeJS.LTS"),
    ("pip", "python -m ensurepip --upgrade"),
    ("uv", "winget install -e --id astral-sh.uv"),
    ("gh", "winget install -e --id GitHub.cli"),
    ("npm", "winget install -e --id OpenJS.NodeJS.LTS"),
    ("bun", "winget install -e --id Oven-sh.Bun"),
    ("opencode", "npm install -g opencode-ai"),
    ("libreoffice", "winget install -e --id TheDocumentFoundation.LibreOffice"),
];
const GROUP_OF: [(&str, &str, bool); 12] = [
    ("python", "python", false),
    ("pythonOffice", "python", true),
    ("pip", "python", true),
    ("uv", "python", true),
    ("git", "dev", false),
    ("node", "dev", true),
    ("npm", "dev", true),
    ("bun", "dev", true),
    ("gh", "dev", true),
    ("opencode", "dev", true),
    ("libreoffice", "docs", true),
    ("winget", "install", true),
];

pub struct OnboardingPage {
    app: Entity<AppState>,
    step: usize,
    checks: Option<Vec<crate::syscheck::Check>>,
    models: Option<usize>,
}

impl OnboardingPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&EngineStatus::global(cx), |_, _, cx| cx.notify()).detach();
        Self { app, step: 0, checks: None, models: None }
    }

    fn run_checks(&mut self, cx: &mut Context<Self>) {
        self.checks = None;
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
        if let Some(api) = crate::core::projects::ProjectStore::global(cx).read(cx).api() {
            let t = cx.background_spawn(async move { api.list_models().map(|m| m.len()).unwrap_or(0) });
            cx.spawn(async move |this, cx| {
                let n = t.await;
                this.update(cx, |s, cx| {
                    s.models = Some(n);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    fn go(&mut self, n: usize, cx: &mut Context<Self>) {
        self.step = n.min(STEPS.len() - 1);
        if STEPS[self.step] == "system" || STEPS[self.step] == "provider" {
            self.run_checks(cx);
        }
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let st = SettingsStore::global(cx);
        let _ = st.update(cx, |s, cx| s.set("onboarding", serde_json::Value::Bool(false), cx));
        self.app.update(cx, |a, cx| a.push("/", cx));
    }
}

impl Render for OnboardingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        let step = STEPS[self.step];
        let engine = EngineStatus::global(cx).read(cx).state.clone();

        // ---- langkah di kiri ----
        let mut rail = div().flex().flex_col().gap(t.space(1)).w(t.rem(rem::SPACE[7] * 5.)).flex_none();
        for (n, s) in STEPS.iter().enumerate() {
            let cur = n == self.step;
            let done = n < self.step;
            let title = if *s == "welcome" { i.t("onb.title") } else { i.t(&format!("onb.{s}.title")) };
            let short = if *s == "welcome" { i.t("onb.welcome.start") } else { i.t(&format!("onb.{s}.short")) };
            rail = rail.child(
                div()
                    .id(SharedString::from(format!("onb-s-{s}")))
                    .flex()
                    .gap(t.space(2))
                    .p(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .when(cur, |d| d.bg(t.colors.active))
                    .hover(|d| d.bg(t.colors.hover))
                    .on_click(cx.listener(move |s, _, _, cx| s.go(n, cx)))
                    .child(
                        div()
                            .flex_none()
                            .size(t.rem(1.25))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if done || cur { t.colors.accent } else { t.colors.raised })
                            .text_color(if done || cur { t.colors.on_accent } else { t.colors.fg_muted })
                            .small(&t)
                            .child(if done { icon("check").size(IconSize::Sm).color(t.colors.on_accent).into_any_element() } else { div().child((n + 1).to_string()).into_any_element() }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().font_weight(FontWeight::MEDIUM).text_color(if cur { t.colors.fg } else { t.colors.fg_muted }).child(title))
                            .child(div().small(&t).text_color(t.colors.fg_subtle).child(short)),
                    ),
            );
        }

        // ---- isi langkah ----
        let title = if step == "welcome" { i.t("onb.title") } else { i.t(&format!("onb.{step}.title")) };
        let mut body = div().flex().flex_col().gap(t.space(3)).child(div().typo(&t, H2).child(title));
        match step {
            "welcome" => {
                let list = STEPS[1..6].iter().map(|s| i.t(&format!("onb.{s}.short")).to_lowercase()).collect::<Vec<_>>().join(", ");
                body = body
                    .child(div().text_color(t.colors.fg_muted).child(i.t("onb.welcome.body")))
                    .child(div().text_color(t.colors.fg_muted).child(i.tp("onb.welcome.setup", &[("list", &list)])));
            }
            "profile" => {
                let name = std::env::var("USERNAME").unwrap_or_else(|_| "Zandor".into());
                body = body.child(div().text_color(t.colors.fg_muted).child(i.t("onb.profile.body"))).child(
                    div().flex().items_center().gap(t.space(3)).child(avatar(SharedString::from(name.clone())).size(ControlSize::Lg)).child(div().typo(&t, H4).child(name)),
                );
            }
            "provider" => {
                body = body
                    .child(div().text_color(t.colors.fg_muted).child(i.t("onb.provider.body")))
                    .child(div().child(match self.models {
                        None => i.t("tour.model.loading"),
                        Some(0) => i.t("tour.model.none"),
                        Some(n) => i.tp("onb.provider.models", &[("n", &n.to_string())]),
                    }))
                    .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("onb.provider.hint")))
                    .child(button("onb-prov").label(i.t("onb.provider.add")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/settings/models", cx)))));
            }
            "system" => {
                body = body.child(div().text_color(t.colors.fg_muted).child(i.t("onb.system.body")));
                let status_badge = |ok: bool, opt: bool, v: Option<String>| {
                    if ok {
                        badge(v.map(|v| i.tp("onb.sys.okVersion", &[("v", &v)])).unwrap_or_else(|| i.t("onb.sys.ok"))).tone(BadgeTone::Success)
                    } else if opt {
                        badge(i.t("onb.sys.optional")).tone(BadgeTone::Outline)
                    } else {
                        badge(i.t("onb.sys.missing")).tone(BadgeTone::Danger)
                    }
                };
                let row = |id: &str, b: crate::ui::feedback::Badge, cmd: Option<&'static str>| {
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(3))
                        .py(t.space(2))
                        .border_b_1()
                        .border_color(t.colors.border_soft)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(div().font_weight(FontWeight::MEDIUM).child(i.t(&format!("onb.sys.{id}"))))
                                .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t(&format!("onb.sys.{id}.why")))),
                        )
                        .children(cmd.map(|c| {
                            icon_button(SharedString::from(format!("cp-{id}")), "copy", i.t("onb.sys.copy"))
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(c.to_string())))
                        }))
                        .child(b)
                };
                // inti: engine
                body = body.child(div().small(&t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(i.t("onb.sys.group.core")));
                body = body.child(row("engine", status_badge(engine.status == "ready", false, engine.version.clone()), None));
                match &self.checks {
                    None => body = body.child(div().text_color(t.colors.fg_muted).child(i.t("onb.sys.checking"))),
                    Some(checks) => {
                        for g in ["python", "dev", "docs", "install"] {
                            body = body.child(div().mt(t.space(2)).small(&t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(i.t(&format!("onb.sys.group.{g}"))));
                            for (id, gg, opt) in GROUP_OF.iter().filter(|x| x.1 == g) {
                                let _ = gg;
                                let c = checks.iter().find(|c| c.id == *id);
                                let found = c.is_some_and(|c| c.found);
                                let cmd = (!found).then(|| INSTALL.iter().find(|(k, _)| k == id).map(|(_, c)| *c)).flatten();
                                body = body.child(row(id, status_badge(found, *opt, c.and_then(|c| c.version.clone())), cmd));
                            }
                        }
                    }
                }
                body = body.child(div().child(button("onb-recheck").label(i.t("onb.sys.recheck")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.run_checks(cx)))));
            }
            "memory" => {
                body = body
                    .child(div().text_color(t.colors.fg_muted).child(i.t("onb.memory.body")))
                    .child(button("onb-mem").label(i.t("mem.manage")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/memory", cx)))))
                    .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("onb.memory.later")));
            }
            "import" => {
                let prompt = i.t("onb.import.prompt");
                body = body
                    .child(div().text_color(t.colors.fg_muted).child(i.t("onb.import.body")))
                    .child(div().p(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised).small(&t).child(prompt.clone()))
                    .child(div().child(button("onb-copy").label(i.t("onb.import.copy")).size(ControlSize::Sm).on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(prompt.clone())))));
            }
            _ => {
                body = body
                    .child(div().text_color(t.colors.fg_muted).child(i.t("onb.thanks.text")))
                    .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("onb.thanks.tip")));
            }
        }

        let last = self.step + 1 == STEPS.len();
        let nav = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .pt(t.space(4))
            .child(button("onb-skip").label(i.t("onb.skip")).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| s.finish(cx))))
            .child(div().flex_1())
            .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.tp("tour.step", &[("i", &(self.step + 1).to_string()), ("n", &STEPS.len().to_string())])))
            .when(self.step > 0, |d| {
                d.child(button("onb-back").label(i.t("tour.back")).on_click(cx.listener(|s, _, _, cx| {
                    let n = s.step.saturating_sub(1);
                    s.go(n, cx)
                })))
            })
            .child(
                button("onb-next")
                    .label(if last { i.t("onb.finish") } else if self.step == 0 { i.t("onb.welcome.start") } else { i.t("tour.next") })
                    .variant(ButtonVariant::Solid)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if last {
                            s.finish(cx)
                        } else {
                            let n = s.step + 1;
                            s.go(n, cx)
                        }
                    })),
            );

        div().id("onb").size_full().overflow_y_scroll().bg(t.colors.frame).child(
            div()
                .flex()
                .gap(t.space(5))
                .w_full()
                .max_w(t.rem(rem::SPACE[7] * 18.))
                .mx_auto()
                .px(t.space(4))
                .py(t.space(5))
                .child(rail)
                .child(div().flex().flex_col().flex_1().min_w_0().p(t.space(5)).rounded(t.rem(rem::RADIUS_LG)).bg(t.colors.surface).child(body).child(nav)),
        )
    }
}
