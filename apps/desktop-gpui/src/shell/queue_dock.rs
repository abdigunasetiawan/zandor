//! Dok antrean pesan — port QueueDock.tsx: pesan yang ditulis selagi agent bekerja menumpuk di atas composer dan
//! dikirim berurutan. Urutan bisa diubah (seret-lepas pada gagang, atau Alt+↑/↓ saat gagang difokus), pesan bisa
//! diedit, dihapus, atau dikirim langsung; antrean yang dijeda bisa dilanjutkan.
use super::ChatPanel;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, ButtonVariant, ControlSize, IconSize, InputSubmit, TextInput, TypeExt};
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, Focusable, FontWeight, SharedString, Window};

/// Nilai yang diseret: id item antrean.
#[derive(Clone)]
pub(super) struct DragQueued(pub String, pub String);

struct DragPreview(String);
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        div()
            .max_w(t.rem(20.))
            .truncate()
            .px(t.space(2))
            .py(t.space(1))
            .rounded(t.rem(rem::RADIUS_MD))
            .bg(t.colors.raised)
            .border_1()
            .border_color(t.colors.border)
            .shadow(t.shadow_2())
            .child(self.0.clone())
    }
}

#[derive(Default)]
pub(super) struct QueueState {
    editing: Option<(String, Entity<TextInput>)>,
}

impl ChatPanel {
    fn save_edit(&mut self, sid: &str, cx: &mut Context<Self>) {
        if let Some((qid, ed)) = self.queue.editing.take() {
            let text = ed.read(cx).text().to_string();
            self.chat.update(cx, |c, cx| c.edit_queued(sid, &qid, &text, cx));
        }
        cx.notify();
    }

    pub(super) fn render_queue(&mut self, sid: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let (items, paused) = {
            let c = self.chat.read(cx);
            (c.queue_of(sid).to_vec(), c.queue_paused.contains(sid))
        };
        if items.is_empty() {
            self.queue.editing = None;
            return None;
        }
        let n = items.len().to_string();
        let sid_s = sid.to_string();
        let head = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(i.tp("queue.title", &[("n", &n)])))
            .when(paused, |d| {
                let s = sid_s.clone();
                d.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("queue.paused"))).child(
                    button("q-resume")
                        .label(i.t("queue.resume"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Solid)
                        .on_click(cx.listener(move |me, _, _, cx| me.chat.update(cx, |c, cx| c.resume_queue(&s, cx)))),
                )
            })
            .when(!paused, |d| d.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("queue.hint"))));

        let mut list = div().flex().flex_col().gap(t.space(1));
        for (idx, q) in items.iter().enumerate() {
            let qid = q.id.clone();
            let editing = self.queue.editing.as_ref().filter(|(e, _)| *e == qid).map(|(_, ed)| ed.clone());
            let s1 = sid_s.clone();
            let grip = div()
                .id(SharedString::from(format!("qgrip-{qid}")))
                .flex_none()
                .cursor_grab()
                .text_color(t.colors.fg_subtle)
                .focusable()
                .tooltip(crate::ui::tooltip::text_tooltip(i.t("queue.drag").into()))
                .on_drag(DragQueued(sid_s.clone(), qid.clone()), {
                    let text = q.text.clone();
                    move |_, _, _, cx| cx.new(|_| DragPreview(text.clone()))
                })
                .on_key_down(cx.listener({
                    let (s, id) = (s1.clone(), qid.clone());
                    move |me, e: &gpui::KeyDownEvent, _, cx| {
                        if !e.keystroke.modifiers.alt {
                            return;
                        }
                        let d = match e.keystroke.key.as_str() {
                            "up" => -1,
                            "down" => 1,
                            _ => return,
                        };
                        me.chat.update(cx, |c, cx| c.move_queued(&s, &id, idx as isize + d, cx));
                        cx.stop_propagation();
                    }
                }))
                .child(icon("grip").size(IconSize::Sm));
            let body: AnyElement = match editing {
                Some(ed) => {
                    let (s2, s3) = (sid_s.clone(), sid_s.clone());
                    div()
                        .flex()
                        .flex_1()
                        .items_start()
                        .gap(t.space(1))
                        .child(div().flex_1().child(ed))
                        .child(icon_button(SharedString::from(format!("qsave-{qid}")), "check", i.t("queue.save")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |me, _, _, cx| me.save_edit(&s2, cx))))
                        .child(icon_button(SharedString::from(format!("qcancel-{qid}")), "close", i.t("queue.cancel")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |me, _, _, cx| {
                            let _ = &s3;
                            me.queue.editing = None;
                            cx.notify();
                        })))
                        .into_any_element()
                }
                None => {
                    let files = (!q.files.is_empty()).then(|| format!(" · {}", i.tp("queue.files", &[("n", &q.files.len().to_string())])));
                    let (e_id, e_text, s4, s5, d_id, n_id) = (qid.clone(), q.text.clone(), sid_s.clone(), sid_s.clone(), qid.clone(), qid.clone());
                    div()
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap(t.space(1))
                        .min_w_0()
                        .child(div().flex_1().min_w_0().truncate().child(q.text.clone()).children(files.map(|f| div().small(&t).child(f))))
                        .child(
                            icon_button(SharedString::from(format!("qedit-{qid}")), "edit", i.t("queue.edit")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |me, _, w, cx| {
                                let s = s4.clone();
                                let ed = cx.new(|cx| {
                                    let mut ti = TextInput::new(cx, "");
                                    ti.multiline = true;
                                    ti.size = ControlSize::Sm;
                                    ti.set_text(e_text.clone(), cx);
                                    ti
                                });
                                cx.subscribe(&ed, move |me, _, _: &InputSubmit, cx| me.save_edit(&s, cx)).detach();
                                let fh = ed.read(cx).focus_handle(cx);
                                fh.focus(w, cx);
                                me.queue.editing = Some((e_id.clone(), ed));
                                cx.notify();
                            })),
                        )
                        .child(icon_button(SharedString::from(format!("qdel-{qid}")), "trash", i.t("queue.delete")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |me, _, _, cx| me.chat.update(cx, |c, cx| c.remove_queued(&s5, &d_id, cx)))))
                        .child({
                            let s6 = sid_s.clone();
                            icon_button(SharedString::from(format!("qnow-{qid}")), "arrow_up", i.t("queue.sendNow")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |me, _, _, cx| me.chat.update(cx, |c, cx| c.send_queued_now(&s6, &n_id, cx))))
                        })
                        .into_any_element()
                }
            };
            let s7 = sid_s.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("qitem-{qid}")))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .px(t.space(2))
                    .py(t.space(1))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .bg(t.colors.surface)
                    .drag_over::<DragQueued>(move |s, _, _, cx| s.border_t_2().border_color(cx.global::<Theme>().colors.focus))
                    .on_drop(cx.listener(move |me, d: &DragQueued, _, cx| {
                        if d.0 == s7 {
                            me.chat.update(cx, |c, cx| c.move_queued(&d.0, &d.1, idx as isize, cx));
                        }
                    }))
                    .child(grip)
                    .child(div().flex_none().w(px(16.)).small(&t).text_color(t.colors.fg_subtle).child((idx + 1).to_string()))
                    .child(body),
            );
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.border_soft)
                .bg(t.colors.raised)
                .child(head)
                .child(list)
                .into_any_element(),
        )
    }
}
