//! Bagian dikte di Pengaturan > Chat: status + unduh model, kosakata (initial prompt Whisper) dan kamus koreksi.
//! Bahasa dan ukuran model ada di skema (dropdown otomatis); teks bebas di sini disimpan di prefs.json.
//! Unduhan dipicu pengguna (tombol), tampil progres, dan bisa dibatalkan (`host/voice_models.rs`).
use crate::core::chat_store::{read_pref, write_pref};
use crate::core::settings::SettingsStore;
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::{button, ButtonVariant, ControlSize, InputChanged, TextInput, ToastInput, ToastTone, Toasts, TypeExt};
use crate::voice_models::{self, DlEvent};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct Download {
    model: String,
    done: u64,
    total: Option<u64>,
    cancel: Arc<AtomicBool>,
}

pub struct VoiceSection {
    vocab: Entity<TextInput>,
    corrections: Entity<TextInput>,
    dl: Option<Download>,
    dl_error: Option<String>,
}

fn multiline_input(cx: &mut Context<VoiceSection>, placeholder: String, pref: &'static str) -> Entity<TextInput> {
    let input = cx.new(|cx| {
        let mut ti = TextInput::new(cx, placeholder);
        ti.multiline = true;
        ti.wrap = true;
        ti.set_text(read_pref(pref).unwrap_or_default(), cx);
        ti
    });
    cx.subscribe(&input, move |_, _, e: &InputChanged, _| write_pref(pref, &e.0)).detach();
    input
}

fn fmt_bytes(b: u64) -> String {
    if b >= 1_000_000_000 {
        format!("{:.1} GB", b as f64 / 1e9)
    } else {
        format!("{} MB", b / 1_000_000)
    }
}

impl VoiceSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (ph_vocab, ph_corr) = {
            let i = cx.global::<I18n>();
            (i.t("voice.vocabulary"), i.t("voice.correctionsPlaceholder"))
        };
        let vocab = multiline_input(cx, ph_vocab, crate::voice::VOCAB_KEY);
        let corrections = multiline_input(cx, ph_corr, crate::voice::CORRECTIONS_KEY);
        // Status model mengikuti pilihan ukuran model di pengaturan.
        let settings = SettingsStore::global(cx);
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        Self { vocab, corrections, dl: None, dl_error: None }
    }

    /// Mulai mengunduh model yang dipilih di pengaturan (hanya bila belum ada unduhan berjalan).
    fn start_download(&mut self, cx: &mut Context<Self>) {
        if self.dl.is_some() {
            return;
        }
        let model = SettingsStore::global(cx).read(cx).str("voiceModel");
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<DlEvent>();
        let (m, c) = (model.clone(), cancel.clone());
        let spawned = std::thread::Builder::new().name("voice-model-dl".into()).spawn(move || {
            voice_models::download(&m, &c, |e| {
                let _ = tx.unbounded_send(e);
            })
        });
        if let Err(e) = spawned {
            self.dl_error = Some(e.to_string());
            cx.notify();
            return;
        }
        self.dl_error = None;
        self.dl = Some(Download { model, done: 0, total: None, cancel });
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(ev) = rx.next().await {
                let end = !matches!(ev, DlEvent::Progress { .. });
                if this.update(cx, |s, cx| s.on_download(ev, cx)).is_err() || end {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn on_download(&mut self, ev: DlEvent, cx: &mut Context<Self>) {
        match ev {
            DlEvent::Progress { done, total } => {
                if let Some(d) = &mut self.dl {
                    d.done = done;
                    d.total = total;
                }
            }
            DlEvent::Done => {
                let model = self.dl.take().map(|d| d.model).unwrap_or_default();
                let title = cx.global::<I18n>().tp("voice.modelReady", &[("model", &model)]);
                Toasts::show(ToastInput { title: title.into(), description: None, tone: ToastTone::Success, timeout_ms: Some(4000), action: None }, cx);
            }
            DlEvent::Cancelled => self.dl = None,
            DlEvent::Failed(e) => {
                self.dl = None;
                self.dl_error = Some(e);
            }
        }
        cx.notify();
    }
}

impl Drop for VoiceSection {
    fn drop(&mut self) {
        if let Some(d) = &self.dl {
            d.cancel.store(true, Ordering::SeqCst);
        }
    }
}

impl Render for VoiceSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let model = SettingsStore::global(cx).read(cx).str("voiceModel");
        let ready = crate::voice_stt::model_ready(&model);
        let mut status = i.tp(if ready { "voice.modelReady" } else { "voice.modelMissing" }, &[("model", &model)]);
        if !ready {
            // Sebutkan path yang dicek supaya salah folder data langsung terlihat.
            if let Some(p) = crate::voice_stt::model_path(&model) {
                status = format!("{status} ({})", p.display());
            }
        }

        // Baris unduhan: progres + Batal (berjalan), atau tombol Unduh (model belum ada), atau tidak ada (siap).
        let action: Option<AnyElement> = if let Some(d) = &self.dl {
            let text = match d.total {
                Some(tot) if tot > 0 => i.tp("voice.downloading", &[("model", &d.model), ("pct", &(d.done * 100 / tot).min(100).to_string()), ("done", &fmt_bytes(d.done)), ("total", &fmt_bytes(tot))]),
                _ => i.tp("voice.downloadingNoTotal", &[("model", &d.model), ("done", &fmt_bytes(d.done))]),
            };
            let cancel = d.cancel.clone();
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(3))
                    .child(div().flex_1().min_w_0().small(&t).text_color(t.colors.fg_muted).child(text))
                    .child(
                        button("voice-dl-cancel")
                            .label(i.t("voice.cancel"))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |_, _, _, _| cancel.store(true, Ordering::SeqCst))),
                    )
                    .into_any_element(),
            )
        } else if !ready {
            let size = voice_models::info(&model).map(|m| fmt_bytes(m.approx_mb * 1_000_000)).unwrap_or_default();
            Some(
                button("voice-dl")
                    .label(i.tp("voice.download", &[("size", &size)]))
                    .size(ControlSize::Sm)
                    .on_click(cx.listener(|s, _, _, cx| s.start_download(cx)))
                    .into_any_element(),
            )
        } else {
            None
        };

        let field = |title: String, help: String, input: Entity<TextInput>| {
            div()
                .flex()
                .flex_col()
                .gap(t.space(1))
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(div().small(&t).text_color(t.colors.fg_muted).child(help))
                .child(div().mt(t.space(1)).child(input))
        };
        div()
            .flex()
            .flex_col()
            .gap(t.space(4))
            .py(t.space(3))
            .child(div().small(&t).text_color(if ready { t.colors.fg_muted } else { t.colors.warning }).child(SharedString::from(status)))
            .children(action)
            .children(self.dl_error.as_ref().map(|e| div().small(&t).text_color(t.colors.danger).child(i.tp("voice.downloadFailed", &[("error", e)]))))
            .child(field(i.t("voice.vocabulary"), i.t("voice.vocabularyHelp"), self.vocab.clone()))
            .child(field(i.t("voice.corrections"), i.t("voice.correctionsHelp"), self.corrections.clone()))
    }
}
