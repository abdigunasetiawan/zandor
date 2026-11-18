//! Info pemakaian di composer — port ComposerInfo.tsx: cincin kecil kepenuhan jendela konteks; hover/klik membuka popup
//! rincian (model, konteks, token, biaya bila dilaporkan, pesan, alat, waktu kerja, mode izin, varian). Batas tidak
//! diketahui → cincin kosong bergaris putus.
use super::*;
use crate::core::usage::{duration, format_tokens, session_usage, usage_level};

const RADIUS: f32 = 8.;

fn ring(t: &Theme, pct: Option<f64>, level: &str) -> AnyElement {
    let (track, value) = (t.colors.border, match level {
        "high" => t.colors.danger,
        "warn" => t.colors.warning,
        _ => t.colors.accent,
    });
    let size = t.rem(1.25);
    gpui::canvas(
        |_, _, _| {},
        move |b, _, w, _| {
            let s = f32::from(b.size.width).min(f32::from(b.size.height));
            let scale = s / 20.;
            let c = gpui::point(b.origin.x + gpui::px(s / 2.), b.origin.y + gpui::px(s / 2.));
            let r = RADIUS * scale;
            let arc = |from: f32, to: f32, dashed: bool| {
                let mut pb = gpui::PathBuilder::stroke(gpui::px(2. * scale));
                if dashed {
                    pb = pb.dash_array(&[gpui::px(2. * scale), gpui::px(2. * scale)]);
                }
                let steps = 64;
                for k in 0..=steps {
                    let a = (from + (to - from) * k as f32 / steps as f32 - 0.25) * std::f32::consts::TAU;
                    let p = gpui::point(c.x + gpui::px(r * a.cos()), c.y + gpui::px(r * a.sin()));
                    if k == 0 { pb.move_to(p) } else { pb.line_to(p) }
                }
                pb.build()
            };
            if let Ok(p) = arc(0., 1., pct.is_none()) {
                w.paint_path(p, track);
            }
            if let Some(v) = pct.filter(|v| *v > 0.) {
                if let Ok(p) = arc(0., (v / 100.) as f32, false) {
                    w.paint_path(p, value);
                }
            }
        },
    )
    .size(size)
    .into_any_element()
}

impl ChatPanel {
    pub(super) fn composer_info(&mut self, t: &Theme, i: &I18n, sid: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = self.chat.read(cx);
        let st = c.sessions.get(sid)?;
        let last = st.messages.iter().rev().find(|m| m.info.role == Role::Assistant && m.info.model_id.is_some());
        let model = c.model.clone();
        let used = self
            .models
            .iter()
            .find(|m| last.is_some_and(|l| l.info.provider_id.as_deref() == Some(m.provider_id.as_str()) && l.info.model_id.as_deref() == Some(m.model_id.as_str())))
            .or_else(|| self.models.iter().find(|m| model.as_ref().is_some_and(|r| r.provider_id == m.provider_id && r.model_id == m.model_id)))
            .cloned();
        let u = session_usage(&st.messages, used.as_ref().and_then(|m| m.context_limit).map(|v| v as f64), used.as_ref().and_then(|m| m.output_limit).map(|v| v as f64));
        let (variant, mode) = (c.variant.clone(), c.permission_mode.clone());
        let level = usage_level(u.context_percent);
        let id_loc = i.locale == crate::i18n::Locale::Id;
        let fmt = |n: f64| format_tokens(n, id_loc);
        let model_label = used.as_ref().map(|m| m.name.clone()).or_else(|| model.as_ref().map(|m| m.model_id.clone())).unwrap_or_else(|| i.t("usage.noModel"));
        let context_text = match (u.context_tokens, u.context_limit) {
            (None, _) => i.t("usage.contextNone"),
            (Some(ct), Some(l)) => format!("{} / {} ({:.0}%)", fmt(ct), fmt(l), u.context_percent.unwrap_or(0.)),
            (Some(ct), None) => format!("{} ({})", fmt(ct), i.t("info.contextUnknown")),
        };
        let pct = u.context_percent.map(|p| p.clamp(0., 100.));
        let label = format!("{}: {}", i.t("usage.toggle"), pct.map(|p| format!("{p:.0}%")).unwrap_or_else(|| i.t("usage.contextNone")));
        let trigger = div()
            .id("cinfo")
            .flex()
            .items_center()
            .justify_center()
            .size(t.rem(rem::CONTROL_SM))
            .rounded(t.rem(rem::RADIUS_MD))
            .cursor_pointer()
            .hover(|s| s.bg(t.colors.hover))
            .tooltip(crate::ui::tooltip::text_tooltip(label.into()))
            .on_click(cx.listener(|s, _, _, cx| {
                s.info_open = !s.info_open;
                cx.notify();
            }))
            .child(ring(t, pct, level));
        let mut root = div().relative().child(trigger);
        if self.info_open {
            let dl = |k: String, v: String| div().flex().gap(t.space(2)).child(div().w(t.rem(7.)).flex_none().text_color(t.colors.fg_muted).child(k)).child(div().flex_1().min_w_0().child(v));
            let mut body = div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .small(t)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(model_label))
                .child(div().flex().gap(t.space(1)).child(i.t("usage.context")).child(div().font_weight(FontWeight::MEDIUM).child(context_text)));
            if let Some(p) = pct {
                let bar = match level {
                    "high" => t.colors.danger,
                    "warn" => t.colors.warning,
                    _ => t.colors.accent,
                };
                body = body.child(div().h(t.space(1)).w_full().rounded_full().bg(t.colors.segment).child(div().h_full().w(gpui::relative((p / 100.) as f32)).rounded_full().bg(bar)));
            }
            if level != "ok" {
                body = body.child(div().text_color(if level == "high" { t.colors.danger } else { t.colors.warning }).child(i.t(if level == "high" { "usage.warnHigh" } else { "usage.warnSoon" })));
            }
            if u.total > 0. {
                let mut line = format!("↑{} ↓{}", fmt(u.input), fmt(u.output));
                if let Some(c) = u.cost {
                    line.push_str(&format!(" · ${c:.4}"));
                }
                body = body.child(div().child(line));
            }
            body = body
                .child(dl(i.t("usage.provider"), used.as_ref().map(|m| format!("{} ({})", m.provider_name, m.provider_id)).unwrap_or_else(|| "—".into())))
                .child(dl(i.t("usage.modelId"), used.as_ref().map(|m| m.model_id.clone()).or_else(|| model.as_ref().map(|m| m.model_id.clone())).unwrap_or_else(|| "—".into())))
                .child(dl(i.t("usage.remaining"), u.context_remaining.map(fmt).unwrap_or_else(|| "—".into())))
                .child(dl(i.t("usage.outputLimit"), u.output_limit.map(fmt).unwrap_or_else(|| "—".into())))
                .child(dl(i.t("usage.tokens"), i.tp("info.tokensValue", &[("input", &fmt(u.input)), ("output", &fmt(u.output)), ("reasoning", &fmt(u.reasoning))])))
                .child(dl(i.t("usage.cost"), u.cost.map(|c| format!("${c:.4}")).unwrap_or_else(|| i.t("info.costNone"))))
                .child(dl(i.t("usage.messages"), i.tp("info.messagesValue", &[("user", &u.user_messages.to_string()), ("assistant", &u.assistant_messages.to_string())])))
                .child(dl(i.t("usage.tools"), u.tool_total.to_string()))
                .child(dl(i.t("usage.work"), duration(u.work_ms)))
                .child(dl(i.t("usage.permission"), i.t(&format!("chat.mode.{mode}"))))
                .child(dl(i.t("usage.variant"), variant.unwrap_or_else(|| i.t("chat.variant.default"))))
                .child(div().text_color(t.colors.fg_subtle).child(i.t("usage.quotaNote")));
            let me = cx.entity().downgrade();
            root = root.child(crate::ui::overlay_deferred(
                gpui::anchored().anchor(gpui::Anchor::BottomLeft).snap_to_window_with_margin(gpui::px(8.)).child(
                    crate::ui::popup(t)
                        .mb(t.space(1))
                        .w(t.rem(rem::SPACE[7] * 8.))
                        .p(t.space(3))
                        .occlude()
                        .on_mouse_down_out(move |_, _, cx| {
                            me.update(cx, |s, cx| {
                                s.info_open = false;
                                cx.notify();
                            })
                            .ok();
                        })
                        .child(body),
                ),
            ));
        }
        Some(root.into_any_element())
    }
}
