//! Kartu "kesimpulan + saran aksi" di bawah jawaban terakhir asisten (lihat `core::next_steps`). Tombol aksi mengirim
//! pesan yang sudah disiapkan sebagai pesan pengguna; kartu hilang begitu ada pesan baru atau ditutup.
use super::ChatPanel;
use crate::core::next_steps::NextSteps;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon_button, ButtonVariant, ControlSize};
use gpui::{div, prelude::*, AnyElement, Context, FontWeight, SharedString};

impl ChatPanel {
    pub(super) fn render_next_steps(&mut self, t: &Theme, i: &I18n, sid: &str, ns: &NextSteps, active: bool, cx: &mut Context<Self>) -> AnyElement {
        let (sid_close, msg_close) = (sid.to_string(), ns.for_msg.clone());
        let mut card = div()
            .id("next-steps")
            .flex()
            .flex_col()
            .gap(t.space(2))
            .mt(t.space(3))
            .p(t.space(3))
            .rounded(t.rem(rem::RADIUS_LG))
            .border_1()
            .border_color(t.colors.border_soft)
            .bg(t.colors.raised)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(t.space(2))
                    .child(div().flex_1().min_w_0().text_color(t.colors.fg_muted).child(ns.recap.clone()))
                    .when(active, |d| {
                        d.child(
                            icon_button("ns-close", "close", i.t("nextSteps.dismiss"))
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .on_click(cx.listener(move |me, _, _, cx| {
                                    let (sid, msg) = (sid_close.clone(), msg_close.clone());
                                    me.chat.update(cx, |c, cx| c.dismiss_next_step(&sid, &msg, cx));
                                })),
                        )
                    }),
            );
        if let Some(q) = &ns.ask {
            card = card.child(div().font_weight(FontWeight::SEMIBOLD).child(q.clone()));
        }
        // sudah dijawab: tampilkan pilihan sebagai teks biasa di bawah pertanyaan (catatan, bukan tombol)
        if let Some(a) = ns.chosen.and_then(|n| ns.actions.get(n)) {
            card = card.child(div().text_color(t.colors.fg_muted).child(a.label.clone()));
        }
        if active && !ns.actions.is_empty() {
            let mut row = div().flex().flex_wrap().gap(t.space(2));
            for (n, a) in ns.actions.iter().enumerate() {
                let (sid, for_msg) = (sid.to_string(), ns.for_msg.clone());
                row = row.child(
                    button(SharedString::from(format!("ns-act-{n}")))
                        .label(a.label.clone())
                        .size(ControlSize::Sm)
                        .variant(if n == 0 { ButtonVariant::Solid } else { ButtonVariant::Outline })
                        .on_click(cx.listener(move |me, _, _, cx| {
                            let (sid, for_msg) = (sid.clone(), for_msg.clone());
                            me.chat.update(cx, |c, cx| c.choose_next_step(&sid, &for_msg, n, cx));
                        })),
                );
            }
            card = card.child(row);
        }
        card.into_any_element()
    }
}
