//! Dialog "Kustomisasi tampilan project" — port ProjectStyleDialog.tsx: logo (teks/emoji atau gambar), latar (warna,
//! gradien dengan generator + templat, atau gambar unggahan dengan posisi/zoom), overlay, warna teks & ikon, dan
//! border. Pratinjau langsung memakai lapisan yang sama dengan baris sidebar. Disimpan lewat AppearanceStore (tervalidasi).
//! GPUI tidak punya `<input type=color>`: warna diisi sebagai hex dengan contoh warna di sampingnya.
use crate::core::appearance::{is_color, parse_color, AppearanceStore};
use crate::core::gradient::{self, Spec, Stop, MAX_STOPS, MIN_STOPS};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, select, tabs, ButtonVariant, ControlSize, IconSize, InputChanged, Slider, SliderChanged, TabItem, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, Context, Entity, EventEmitter, FontWeight, IntoElement, SharedString, Window};

const DEFAULT_PROJECT_COLOR: &str = "#1e293b";
const DEFAULT_OVERLAY_COLOR: &str = "#000000";
const NEW_STOP_COLOR: &str = "#f97316";

pub struct StyleClosed;
impl EventEmitter<StyleClosed> for ProjectStyleDialog {}

fn default_gradient() -> Spec {
    Spec { radial: false, angle: 135., stops: vec![Stop { color: "#2563eb".into(), pos: 0. }, Stop { color: "#7c3aed".into(), pos: 100. }] }
}

/// Port hslToHex.
fn hsl_to_hex(h: f32, s: f32, l: f32) -> String {
    let a = s * l.min(1. - l);
    let f = |n: f32| {
        let k = (n + h / 30.) % 12.;
        let v = l - a * (-1f32).max((k - 3.).min(9. - k).min(1.));
        format!("{:02x}", (v * 255.).round() as u8)
    };
    format!("#{}{}{}", f(0.), f(8.), f(4.))
}

/// Port randomGradient (sumber acak dari jam sistem).
fn random_gradient() -> Spec {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let r1 = (seed % 360) as f32;
    let r2 = ((seed / 360) % 360) as f32;
    Spec { radial: false, angle: r2, stops: vec![Stop { color: hsl_to_hex(r1, 0.65, 0.5), pos: 0. }, Stop { color: hsl_to_hex((r1 + 50.) % 360., 0.65, 0.4), pos: 100. }] }
}

struct StopRow {
    color: Entity<TextInput>,
    pos: Entity<Slider>,
}

pub struct ProjectStyleDialog {
    project: String,
    name: String,
    mode: SharedString,
    color: Entity<TextInput>,
    spec: Spec,
    stops: Vec<StopRow>,
    angle: Entity<Slider>,
    image: Option<String>,
    image_y: Entity<Slider>,
    image_zoom: Entity<Slider>,
    logo: Entity<TextInput>,
    logo_image: Option<String>,
    border: Entity<TextInput>,
    overlay: SharedString,
    overlay_color: Entity<TextInput>,
    overlay_opacity: Entity<Slider>,
    fg: Entity<TextInput>,
}

fn text(cx: &mut Context<ProjectStyleDialog>, v: &str, ph: &str) -> Entity<TextInput> {
    let (v, ph) = (v.to_string(), ph.to_string());
    let e = cx.new(|cx| {
        let mut t = TextInput::new(cx, ph);
        t.set_text(v, cx);
        t
    });
    cx.subscribe(&e, |_, _, _: &InputChanged, cx| cx.notify()).detach();
    e
}

fn slider(cx: &mut Context<ProjectStyleDialog>, v: f32, min: f32, max: f32, step: f32, label: String) -> Entity<Slider> {
    let s = cx.new(|_| Slider::new(v, min, max, step, label));
    cx.subscribe(&s, |me, _, _: &SliderChanged, cx| {
        me.sync_labels(cx);
        cx.notify();
    })
    .detach();
    s
}

impl ProjectStyleDialog {
    pub fn new(project: String, name: String, cx: &mut Context<Self>) -> Self {
        let i = cx.global::<I18n>().clone();
        let ap = AppearanceStore::global(cx).read(cx);
        let g = |k: &str| ap.project_style(&project, k);
        let saved: Vec<Option<String>> = ["background", "gradient", "image", "logo", "logoImage", "border", "overlay", "overlayColor", "overlayOpacity", "fg", "imageY", "imageZoom"].iter().map(|k| g(k)).collect();
        let [background, grad, image, logo, logo_image, border, overlay, overlay_color, overlay_opacity, fg, image_y, image_zoom]: [Option<String>; 12] = saved.try_into().unwrap_or_default();
        let mode = if image.is_some() {
            "image"
        } else if grad.is_some() {
            "gradient"
        } else if background.is_some() {
            "color"
        } else {
            "none"
        };
        let spec = grad.as_deref().and_then(gradient::parse_gradient).unwrap_or_else(default_gradient);
        let hex = i.t("pstyle.hexHint");
        let num = |v: &Option<String>, d: f32| v.as_deref().and_then(|x| x.parse::<f32>().ok()).unwrap_or(d);
        let mut me = Self {
            project,
            name,
            mode: mode.into(),
            color: text(cx, background.as_deref().unwrap_or(DEFAULT_PROJECT_COLOR), &hex),
            spec: spec.clone(),
            stops: vec![],
            angle: slider(cx, spec.angle, 0., 360., 1., String::new()),
            image,
            image_y: slider(cx, num(&image_y, 50.), 0., 100., 1., String::new()),
            image_zoom: slider(cx, num(&image_zoom, 100.), 100., 300., 5., String::new()),
            logo: text(cx, logo.as_deref().unwrap_or(""), "🚀"),
            logo_image,
            border: text(cx, border.as_deref().unwrap_or(""), &hex),
            overlay: overlay.unwrap_or_else(|| "none".into()).into(),
            overlay_color: text(cx, overlay_color.as_deref().unwrap_or(DEFAULT_OVERLAY_COLOR), &hex),
            overlay_opacity: slider(cx, num(&overlay_opacity, 50.), 0., 100., 1., String::new()),
            fg: text(cx, fg.as_deref().unwrap_or(""), &hex),
        };
        me.set_spec(spec, cx);
        me
    }

    fn set_spec(&mut self, spec: Spec, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        self.stops = spec
            .stops
            .iter()
            .enumerate()
            .map(|(n, s)| StopRow { color: text(cx, &s.color, &i.t("pstyle.hexHint")), pos: slider(cx, s.pos, 0., 100., 1., i.tp("pstyle.stopPos", &[("n", &(n + 1).to_string())])) })
            .collect();
        self.angle.update(cx, |a, cx| {
            a.value = spec.angle;
            cx.notify();
        });
        self.spec = spec;
        self.sync_labels(cx);
        cx.notify();
    }

    /// Spesifikasi gradien terkini dari kolom.
    fn current_spec(&self, cx: &App) -> Spec {
        Spec {
            radial: self.spec.radial,
            angle: self.angle.read(cx).value,
            stops: self.stops.iter().map(|s| Stop { color: s.color.read(cx).text().trim().to_string(), pos: s.pos.read(cx).value }).collect(),
        }
    }

    fn sync_labels(&mut self, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let v = |s: &Entity<Slider>, cx: &Context<Self>| (s.read(cx).value.round() as i64).to_string();
        let (a, y, z, o) = (v(&self.angle, cx), v(&self.image_y, cx), v(&self.image_zoom, cx), v(&self.overlay_opacity, cx));
        self.angle.update(cx, |s, _| s.label = i.tp("pstyle.angle", &[("deg", &a)]).into());
        self.image_y.update(cx, |s, _| s.label = i.tp("pstyle.bg.posY", &[("pct", &y)]).into());
        self.image_zoom.update(cx, |s, _| s.label = i.tp("pstyle.bg.zoom", &[("pct", &z)]).into());
        self.overlay_opacity.update(cx, |s, _| s.label = i.tp("pstyle.overlay.opacity", &[("pct", &o)]).into());
    }

    /// Draf gaya (kunci → nilai) sesuai aturan ProjectStyleDialog: nilai tidak valid/bawaan tidak disimpan.
    fn draft(&self, cx: &App) -> Vec<(&'static str, Option<String>)> {
        let txt = |e: &Entity<TextInput>| e.read(cx).text().trim().to_string();
        let col = |e: &Entity<TextInput>| Some(txt(e)).filter(|v| !v.is_empty() && is_color(v));
        let num = |s: &Entity<Slider>| s.read(cx).value.round() as i64;
        let mode = self.mode.as_ref();
        let has_bg = mode != "none";
        let overlay_on = has_bg && self.overlay.as_ref() != "none";
        let image_on = mode == "image" && self.image.is_some();
        vec![
            ("border", col(&self.border)),
            ("logo", Some(txt(&self.logo)).filter(|v| !v.is_empty())),
            ("logoImage", self.logo_image.clone()),
            ("background", (mode == "color").then(|| col(&self.color)).flatten()),
            ("gradient", (mode == "gradient").then(|| gradient::gradient_css(&self.current_spec(cx)))),
            ("image", self.image.clone().filter(|_| image_on)),
            ("imageY", (image_on && num(&self.image_y) != 50).then(|| num(&self.image_y).to_string())),
            ("imageZoom", (image_on && num(&self.image_zoom) != 100).then(|| num(&self.image_zoom).to_string())),
            ("overlay", overlay_on.then(|| self.overlay.to_string())),
            ("overlayColor", overlay_on.then(|| col(&self.overlay_color)).flatten().filter(|c| c != DEFAULT_OVERLAY_COLOR)),
            ("overlayOpacity", (overlay_on && num(&self.overlay_opacity) != 50).then(|| num(&self.overlay_opacity).to_string())),
            ("fg", col(&self.fg)),
        ]
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft(cx);
        let store = AppearanceStore::global(cx);
        for (k, v) in draft {
            let pid = self.project.clone();
            if let Some(err) = store.update(cx, |s, cx| s.set_project_style(&pid, k, v.as_deref(), cx)) {
                let title = cx.global::<I18n>().tp("pstyle.error", &[("error", err)]);
                Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
                return;
            }
        }
        cx.emit(StyleClosed);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.mode = "none".into();
        self.logo.update(cx, |t, cx| t.set_text("", cx));
        self.logo_image = None;
        self.border.update(cx, |t, cx| t.set_text("", cx));
        self.image = None;
        self.image_y.update(cx, |s, _| s.value = 50.);
        self.image_zoom.update(cx, |s, _| s.value = 100.);
        self.overlay = "none".into();
        self.overlay_color.update(cx, |t, cx| t.set_text(DEFAULT_OVERLAY_COLOR, cx));
        self.overlay_opacity.update(cx, |s, _| s.value = 50.);
        self.fg.update(cx, |t, cx| t.set_text("", cx));
        self.sync_labels(cx);
        cx.notify();
    }

    fn upload(&mut self, logo: bool, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let r = if logo { gradient::image_to_data_url(&path, 64, gradient::OutFmt::Png, gradient::MAX_LOGO_IMAGE_CHARS) } else { gradient::image_to_data_url(&path, 1280, gradient::OutFmt::Jpeg(80), gradient::MAX_STYLE_IMAGE_CHARS) };
            this.update(cx, |s, cx| {
                match r {
                    Ok(url) if logo => s.logo_image = Some(url),
                    Ok(url) => {
                        s.image = Some(url);
                        s.mode = "image".into();
                    }
                    Err(e) => {
                        let title = cx.global::<I18n>().tp("pstyle.uploadError", &[("error", &e)]);
                        Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn color_field(t: &Theme, input: &Entity<TextInput>, cx: &App) -> AnyElement {
    let v = input.read(cx).text().trim().to_string();
    let swatch = parse_color(&v);
    div()
        .flex()
        .items_center()
        .gap(t.space(2))
        .child(div().flex_none().size(t.rem(rem::CONTROL_SM)).rounded(t.rem(rem::RADIUS_SM)).border_1().border_color(t.colors.border).when_some(swatch, |d, c| d.bg(c)))
        .child(div().flex_1().child(input.clone()))
        .into_any_element()
}

fn labeled(t: &Theme, label: String, hint: Option<String>, el: impl IntoElement) -> gpui::Div {
    div().flex().flex_col().gap(t.space(1)).child(div().small(t).font_weight(FontWeight::MEDIUM).child(label)).child(el).children(hint.map(|h| div().small(t).text_color(t.colors.fg_muted).child(h)))
}

impl Render for ProjectStyleDialog {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let draft = self.draft(cx);
        let get = |k: &str| draft.iter().find(|(x, _)| *x == k).and_then(|(_, v)| v.clone());
        let layers = super::sidebar::style_layers(&get, t.rem(rem::RADIUS_MD));
        let (pbg, pborder, pfg) = (get("background").and_then(|v| parse_color(&v)), get("border").and_then(|v| parse_color(&v)), get("fg").and_then(|v| parse_color(&v)));
        let logo_img = self.logo_image.as_deref().and_then(gradient::image_of);
        let logo_txt = self.logo.read(cx).text().trim().to_string();
        let preview = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .h(t.rem(rem::CONTROL_MD))
            .px(t.space(2))
            .rounded(t.rem(rem::RADIUS_MD))
            .bg(t.colors.frame)
            .relative()
            .overflow_hidden()
            .children(layers)
            .when_some(pbg, |d, c| d.bg(c))
            .when_some(pborder, |d, c| d.border_1().border_color(c))
            .when_some(pfg, |d, c| d.text_color(c))
            .child(match logo_img {
                Some(img) => gpui::img(img).flex_none().size(t.rem(rem::ICON_MD)).rounded(t.rem(rem::RADIUS_SM)).object_fit(gpui::ObjectFit::Cover).into_any_element(),
                None => div().flex_none().w(t.rem(rem::ICON_MD)).flex().justify_center().child(if logo_txt.is_empty() { "📁".to_string() } else { logo_txt }).into_any_element(),
            })
            .child(div().flex_1().min_w_0().truncate().child(self.name.clone()))
            .child(icon("folder").size(IconSize::Sm))
            .child(icon("plus").size(IconSize::Sm))
            .child(icon("more").size(IconSize::Sm));

        // ---- isi tab latar ----
        let mode = self.mode.to_string();
        let bg_body: AnyElement = match mode.as_str() {
            "color" => color_field(&t, &self.color, cx),
            "gradient" => {
                let mut tpl = div().flex().flex_wrap().gap(t.space(1));
                for (n, (_, name, ..)) in gradient::TEMPLATES.iter().enumerate() {
                    let sp = gradient::template(n).unwrap_or_else(default_gradient);
                    let layer = super::sidebar::style_layers(&|k| (k == "gradient").then(|| gradient::gradient_css(&sp)), t.rem(rem::RADIUS_SM));
                    tpl = tpl.child(
                        div()
                            .id(SharedString::from(format!("pst-tpl-{n}")))
                            .relative()
                            .overflow_hidden()
                            .size(t.rem(2.))
                            .rounded(t.rem(rem::RADIUS_SM))
                            .border_1()
                            .border_color(t.colors.border)
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip::text_tooltip(i.tp("pstyle.template", &[("name", name)]).into()))
                            .children(layer)
                            .on_click(cx.listener(move |s, _, _, cx| s.set_spec(gradient::template(n).unwrap_or_else(default_gradient), cx))),
                    );
                }
                let me = cx.entity().downgrade();
                let mut col = div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .child(tpl)
                    .child(labeled(
                        &t,
                        i.t("pstyle.type"),
                        None,
                        select("pst-type", vec![("linear".into(), i.t("pstyle.type.linear").into()), ("radial".into(), i.t("pstyle.type.radial").into())], if self.spec.radial { "radial" } else { "linear" }).on_change(move |v, _, cx| {
                            let r = v.as_ref() == "radial";
                            me.update(cx, |s, cx| {
                                s.spec.radial = r;
                                cx.notify();
                            })
                            .ok();
                        }),
                    ));
                if !self.spec.radial {
                    col = col.child(self.angle.clone());
                }
                let n_stops = self.stops.len();
                for (n, st) in self.stops.iter().enumerate() {
                    col = col.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(div().flex_1().child(labeled(&t, i.tp("pstyle.stop", &[("n", &(n + 1).to_string())]), None, color_field(&t, &st.color, cx))))
                            .child(div().flex_1().child(st.pos.clone()))
                            .child(icon_button(SharedString::from(format!("pst-rm-{n}")), "trash", i.tp("pstyle.removeStop", &[("n", &(n + 1).to_string())])).size(ControlSize::Sm).disabled(n_stops <= MIN_STOPS).on_click(cx.listener(move |s, _, _, cx| {
                                let mut sp = s.current_spec(cx);
                                if n < sp.stops.len() {
                                    sp.stops.remove(n);
                                }
                                s.set_spec(sp, cx);
                            }))),
                    );
                }
                col.child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(button("pst-add").size(ControlSize::Sm).disabled(n_stops >= MAX_STOPS).label(i.t("pstyle.addStop")).on_click(cx.listener(|s, _, _, cx| {
                            let mut sp = s.current_spec(cx);
                            sp.stops.push(Stop { color: NEW_STOP_COLOR.into(), pos: 100. });
                            s.set_spec(sp, cx);
                        })))
                        .child(button("pst-rand").size(ControlSize::Sm).label(i.t("pstyle.random")).on_click(cx.listener(|s, _, _, cx| s.set_spec(random_gradient(), cx)))),
                )
                .into_any_element()
            }
            "image" => div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(button("pst-up").size(ControlSize::Sm).label(i.t("pstyle.bg.upload")).on_click(cx.listener(|s, _, _, cx| s.upload(false, cx))))
                        .when(self.image.is_some(), |d| {
                            d.child(button("pst-rmimg").size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("pstyle.bg.removeImage")).on_click(cx.listener(|s, _, _, cx| {
                                s.image = None;
                                cx.notify();
                            })))
                        }),
                )
                .when(self.image.is_some(), |d| d.child(self.image_y.clone()).child(self.image_zoom.clone()))
                .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("pstyle.bg.imageHint")))
                .into_any_element(),
            _ => div().small(&t).text_color(t.colors.fg_muted).child(i.t("pstyle.bg.noneHint")).into_any_element(),
        };
        let me = cx.entity().downgrade();
        let bg_tabs = tabs(
            "pst-bg",
            ["none", "color", "gradient", "image"].iter().map(|v| TabItem { value: (*v).into(), label: i.t(&format!("pstyle.bg.{v}")).into(), icon: None, disabled: false }).collect(),
            self.mode.clone(),
        )
        .on_change({
            let me = me.clone();
            move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.mode = v;
                    cx.notify();
                })
                .ok();
            }
        });

        let mut body = div()
            .flex()
            .flex_col()
            .gap(t.space(4))
            .child(preview)
            .child(div().flex().flex_col().gap(t.space(2)).child(div().small(&t).font_weight(FontWeight::SEMIBOLD).child(i.t("pstyle.background"))).child(bg_tabs).child(bg_body));
        if mode != "none" {
            let items: Vec<(SharedString, SharedString)> = [("none", "pstyle.overlay.none"), ("full", "pstyle.overlay.full"), ("gradient", "pstyle.overlay.gradient"), ("gradient-rtl", "pstyle.overlay.gradientRtl"), ("gradient-ttb", "pstyle.overlay.gradientTtb"), ("gradient-btt", "pstyle.overlay.gradientBtt")].iter().map(|(v, k)| ((*v).into(), i.t(k).into())).collect();
            let me2 = me.clone();
            let mut sec = div().flex().flex_col().gap(t.space(2)).child(div().small(&t).font_weight(FontWeight::SEMIBOLD).child(i.t("pstyle.overlay"))).child(labeled(
                &t,
                i.t("pstyle.overlay.type"),
                None,
                select("pst-ov", items, self.overlay.clone()).on_change(move |v, _, cx| {
                    me2.update(cx, |s, cx| {
                        s.overlay = v;
                        cx.notify();
                    })
                    .ok();
                }),
            ));
            if self.overlay.as_ref() != "none" {
                sec = sec.child(labeled(&t, i.t("pstyle.overlay.color"), None, color_field(&t, &self.overlay_color, cx))).child(self.overlay_opacity.clone());
            }
            body = body.child(sec);
        }
        body = body
            .child(labeled(&t, i.t("pstyle.fg"), Some(i.t("pstyle.fgHint")), color_field(&t, &self.fg, cx)))
            .child(labeled(
                &t,
                i.t("pstyle.logo"),
                Some(i.t("pstyle.logoHint")),
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().w(t.rem(6.)).child(self.logo.clone()))
                    .child(button("pst-logo-up").label(i.t("pstyle.logo.upload")).on_click(cx.listener(|s, _, _, cx| s.upload(true, cx))))
                    .when(self.logo_image.is_some(), |d| {
                        d.child(button("pst-logo-rm").variant(ButtonVariant::Ghost).label(i.t("pstyle.logo.remove")).on_click(cx.listener(|s, _, _, cx| {
                            s.logo_image = None;
                            cx.notify();
                        })))
                    }),
            ))
            .child(labeled(&t, i.t("pstyle.border"), None, color_field(&t, &self.border, cx)));

        crate::ui::dialog(i.tp("pstyle.title", &[("name", &self.name)]))
            .width_rem(rem::SPACE[7] * 12.)
            .on_dismiss(cx.listener(|_, _, _, cx| cx.emit(StyleClosed)))
            .child(div().id("pst-body").max_h(t.rem(34.)).overflow_y_scroll().child(body))
            .footer(
                div()
                    .flex()
                    .justify_end()
                    .gap(t.space(2))
                    .child(button("pst-reset").variant(ButtonVariant::Ghost).label(i.t("pstyle.reset")).on_click(cx.listener(|s, _, _, cx| s.reset(cx))))
                    .child(button("pst-cancel").label(i.t("ui.cancel")).on_click(cx.listener(|_, _, _, cx| cx.emit(StyleClosed))))
                    .child(button("pst-save").variant(ButtonVariant::Solid).label(i.t("sb.save")).on_click(cx.listener(|s, _, _, cx| s.save(cx)))),
            )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hsl() {
        assert_eq!(super::hsl_to_hex(0., 1., 0.5), "#ff0000");
        assert_eq!(super::hsl_to_hex(120., 1., 0.5), "#00ff00");
        assert_eq!(super::random_gradient().stops.len(), 2);
    }
}
