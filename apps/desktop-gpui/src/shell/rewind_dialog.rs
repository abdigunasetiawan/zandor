//! Dialog Rewind ("Rewind ke pesan ini?"): kutipan pesan, pemilih Pulihkan (3 mode), kartu berkas yang terpengaruh,
//! catatan, Batal/Rewind. Logika (daftar berkas, pemulihan) di `core::rewind` + `ChatStore::rewind`.
use super::ChatPanel;
use super::chat_parts::fail;
use crate::shell::file_icons::file_type_icon;
use crate::core::rewind::{totals, FileChange, RewindMode};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, select, ButtonVariant, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, FontWeight, SharedString};

pub(super) struct RewindDialog {
    pub sid: String,
    pub mid: String,
    pub text: String,
    pub mode: RewindMode,
    /// None = memuat; Some(Err) = gagal memuat daftar berkas.
    pub changes: Option<Result<Vec<FileChange>, String>>,
    pub running: bool,
}

fn name_of(file: &str) -> &str {
    file.rsplit(['/', '\\']).next().unwrap_or(file)
}

fn counts(t: &Theme, a: u32, d: u32) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(t.space(1))
        .small(t)
        .font_weight(FontWeight::MEDIUM)
        .child(div().text_color(t.colors.success).child(format!("+{a}")))
        .child(div().text_color(t.colors.danger).child(format!("\u{2212}{d}")))
        .into_any_element()
}

impl ChatPanel {
    /// Buka dialog untuk pesan `mid` dan muat daftar berkas yang terpengaruh dari engine.
    pub(super) fn open_rewind(&mut self, sid: String, mid: String, cx: &mut Context<Self>) {
        let text = self.chat.read(cx).sessions.get(&sid).and_then(|s| s.messages.iter().find(|m| m.info.id == mid).map(crate::core::chat::message_text)).unwrap_or_default();
        let task = self.chat.update(cx, |c, cx| c.rewind_preview(&sid, &mid, cx));
        self.rewind = Some(RewindDialog { sid: sid.clone(), mid: mid.clone(), text, mode: RewindMode::Both, changes: None, running: false });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if let Some(d) = s.rewind.as_mut().filter(|d| d.sid == sid && d.mid == mid) {
                    d.changes = Some(r);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn run_rewind(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.rewind.as_mut().filter(|d| !d.running) else { return };
        let changes = match (&d.changes, d.mode.touches_code()) {
            (Some(Ok(c)), true) => c.clone(),
            (_, true) => return,
            _ => vec![],
        };
        if d.mode == RewindMode::CodeOnly && changes.is_empty() {
            return;
        }
        d.running = true;
        let (sid, mid, mode) = (d.sid.clone(), d.mid.clone(), d.mode);
        // Tanpa berkas terdeteksi, mode gabungan hanya memotong percakapan: toast tidak boleh mengaku berkas dipulihkan.
        let done_key = if mode == RewindMode::Both && changes.is_empty() { RewindMode::ConversationOnly.key() } else { mode.key() };
        let task = self.chat.update(cx, |c, cx| c.rewind(&sid, &mid, mode, changes, cx));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.rewind = None;
                match r {
                    Ok(text) => {
                        if let Some(text) = text {
                            let n = text.len();
                            s.assist.files.clear();
                            s.input.update(cx, |inp, cx| inp.set_text_at(text, n, cx));
                        }
                        let title = cx.global::<I18n>().t(&format!("chat.rewind.done.{done_key}"));
                        Toasts::show(ToastInput { title: title.into(), description: None, tone: ToastTone::Success, timeout_ms: None, action: None }, cx);
                    }
                    Err(e) => fail(cx, e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn rewind_dialog(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let d = self.rewind.as_ref()?;
        let me = cx.entity();
        let mode = d.mode;
        let running = d.running;
        let items: Vec<(SharedString, SharedString)> = [RewindMode::Both, RewindMode::ConversationOnly, RewindMode::CodeOnly].iter().map(|m| (SharedString::from(m.key()), SharedString::from(i.t(&format!("chat.rewind.mode.{}", m.key()))))).collect();
        let picker = select("rewind-mode", items, mode.key()).disabled(running).on_change(move |v, _, cx| {
            me.update(cx, |s, cx| {
                if let Some(d) = s.rewind.as_mut() {
                    d.mode = RewindMode::from_key(&v);
                    cx.notify();
                }
            })
        });

        // Kutipan: kartu berbingkai (border-soft); isi panjang digulir di dalamnya, bukan dipotong di tengah baris.
        let quote = div()
            .w_full()
            .rounded(t.rem(rem::RADIUS_MD))
            .border_1()
            .border_color(t.colors.border_soft)
            .bg(t.colors.raised)
            .text_color(t.colors.fg)
            .py(t.space(2))
            .child(div().id("rewind-quote").max_h(t.rem(rem::SPACE[7] * 2.)).overflow_y_scroll().px(t.space(4)).py(t.space(1)).child(d.text.clone()));

        let msg = |text: String, color| div().p(t.space(3)).small(t).text_color(color).child(text).into_any_element();
        let files_el: AnyElement = if !mode.touches_code() {
            msg(i.t("chat.rewind.noCode"), t.colors.fg_muted)
        } else {
            match &d.changes {
                None => msg(i.t("chat.rewind.loading"), t.colors.fg_muted),
                Some(Err(e)) => msg(i.tp("chat.rewind.loadError", &[("error", e)]), t.colors.danger),
                Some(Ok(c)) if c.is_empty() => msg(i.t("chat.rewind.noFiles"), t.colors.fg_muted),
                Some(Ok(c)) => {
                    let (a, del) = totals(c);
                    let head = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(t.space(3))
                        .py(t.space(2))
                        .border_b_1()
                        .border_color(t.colors.border_soft)
                        .child(div().text_color(t.colors.fg).child(i.tp(if c.len() == 1 { "chat.rewind.file" } else { "chat.rewind.files" }, &[("n", &c.len().to_string())])))
                        .child(counts(t, a, del));
                    let rows = c.iter().enumerate().map(|(k, f)| {
                        div()
                            .id(("rewind-file", k))
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .px(t.space(3))
                            .py(t.space(1))
                            .child(file_type_icon(t, name_of(&f.file), false, false, rem::ICON_SM))
                            .child(div().flex_1().min_w_0().truncate().text_color(t.colors.fg).child(name_of(&f.file).to_string()))
                            .child(counts(t, f.additions, f.deletions))
                    });
                    div().flex().flex_col().child(head).child(div().id("rewind-files").flex().flex_col().max_h(t.rem(rem::SPACE[7] * 3.)).overflow_y_scroll().py(t.space(1)).children(rows)).into_any_element()
                }
            }
        };
        let card = div().w_full().flex().flex_col().rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border_soft).child(files_el);

        let note = match mode {
            RewindMode::Both if matches!(&d.changes, Some(Ok(c)) if c.is_empty()) => i.t("chat.rewind.noteNoFiles"),
            RewindMode::Both => i.t("chat.rewind.note"),
            RewindMode::ConversationOnly => i.t("chat.rewind.noteConversation"),
            RewindMode::CodeOnly => i.t("chat.rewind.noteCode"),
        };
        let can_run = !running
            && match mode {
                RewindMode::ConversationOnly => true,
                RewindMode::Both => !matches!(d.changes, Some(Err(_)) | None),
                RewindMode::CodeOnly => matches!(&d.changes, Some(Ok(c)) if !c.is_empty()),
            };
        let body = div()
            .flex()
            .flex_col()
            .gap(t.space(4))
            .child(quote)
            .child(div().flex().flex_col().gap(t.space(2)).child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("chat.rewind.restore"))).child(picker))
            .child(card)
            .child(div().small(t).text_color(t.colors.fg_muted).child(note));

        Some(
            crate::ui::dialog(i.t("chat.rewind.title"))
                .on_dismiss(cx.listener(|s, _, _, cx| {
                    if s.rewind.as_ref().is_some_and(|d| !d.running) {
                        s.rewind = None;
                        cx.notify();
                    }
                }))
                .child(body)
                .footer(button("rewind-cancel").label(i.t("ui.cancel")).disabled(running).on_click(cx.listener(|s, _, _, cx| {
                    s.rewind = None;
                    cx.notify();
                })))
                .footer(button("rewind-go").variant(ButtonVariant::Solid).label(i.t("chat.rewind.confirm")).disabled(!can_run).on_click(cx.listener(|s, _, _, cx| s.run_rewind(cx)))),
        )
    }
}
