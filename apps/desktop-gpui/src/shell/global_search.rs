//! Pencarian global lintas sesi (Ctrl+Shift+F) — port GlobalSearch.tsx + kelas SessionSearch: judul + isi pesan di
//! semua project dan Chat. Pesan dimuat dari engine per sesi (cache per sesi selama `updated` sama); hasil bertambah
//! bertahap; klik hasil membuka sesinya dan menggulir ke pesan yang cocok.
use crate::core::engine::SessionInfo;
use crate::core::projects::{session_title, LoadStatus, ProjectStore, CHAT_KEY};
use crate::core::session_search::{rank_results, search_session, SessionResult};
use crate::core::types::ChatMessage;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{InputChanged, InputSubmit, TextInput, TypeExt};
use gpui::{div, prelude::*, Context, Entity, EventEmitter, FontWeight, IntoElement, SharedString, Window};
use std::collections::HashMap;
use std::sync::Arc;

pub enum SearchEvent {
    Close,
}
impl EventEmitter<SearchEvent> for GlobalSearch {}

pub struct GlobalSearch {
    input: Entity<TextInput>,
    cache: HashMap<String, (f64, Arc<Vec<ChatMessage>>)>,
    results: Vec<SessionResult>,
    progress: Option<(usize, usize, usize)>,
    running: bool,
    seq: u64,
}

impl GlobalSearch {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ph = cx.global::<I18n>().t("gsearch.placeholder");
        let input = cx.new(|cx| TextInput::new(cx, ph));
        cx.subscribe(&input, |s, _, _: &InputChanged, cx| s.schedule(cx)).detach();
        cx.subscribe(&input, |s, _, _: &InputSubmit, cx| {
            if let Some(r) = s.results.first().cloned() {
                let mid = r.hits.first().map(|h| h.message_id.clone());
                s.go(&r, mid, cx);
            }
        })
        .detach();
        let store = ProjectStore::global(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        // muat daftar sesi project yang belum pernah dimuat (sidebar memuatnya hanya bila dibuka)
        store.update(cx, |st, cx| {
            let keys: Vec<String> = st.projects.iter().map(|p| p.id.clone()).chain(st.chat_dir.as_ref().map(|_| CHAT_KEY.to_string())).filter(|k| !st.sessions.contains_key(k)).collect();
            for k in keys {
                st.refresh(&k, cx);
            }
        });
        let fh = gpui::Focusable::focus_handle(input.read(cx), cx);
        fh.focus(window, cx);
        Self { input, cache: HashMap::new(), results: vec![], progress: None, running: false, seq: 0 }
    }

    fn sessions(cx: &gpui::App) -> Vec<(String, SessionInfo)> {
        let st = ProjectStore::global(cx).read(cx);
        st.sessions.iter().filter(|(_, g)| matches!(g.status, LoadStatus::Ready)).flat_map(|(k, g)| g.items.iter().map(move |s| (k.clone(), s.clone()))).collect()
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.seq += 1;
        let my = self.seq;
        let q = self.input.read(cx).text().trim().to_string();
        if q.is_empty() {
            self.results.clear();
            self.progress = None;
            self.running = false;
            cx.notify();
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
            this.update(cx, |s, cx| {
                if s.seq == my {
                    s.run(q, my, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn run(&mut self, q: String, my: u64, cx: &mut Context<Self>) {
        let sessions = Self::sessions(cx);
        let api = ProjectStore::global(cx).read(cx).api();
        self.running = true;
        self.results.clear();
        self.progress = Some((0, sessions.len(), 0));
        cx.notify();
        if sessions.is_empty() {
            self.running = false;
            return;
        }
        // 4 pekerja paralel, masing-masing memproses sebagian sesi
        let chunks: Vec<Vec<(String, SessionInfo)>> = (0..4).map(|w| sessions.iter().skip(w).step_by(4).cloned().collect()).collect();
        for chunk in chunks {
            for (key, info) in chunk {
                let cached = self.cache.get(&info.id).filter(|(u, _)| *u == info.updated).map(|(_, m)| m.clone());
                let api = api.clone();
                let q = q.clone();
                let task = cx.background_spawn(async move {
                    match cached {
                        Some(m) => (Ok(m), false),
                        None => match api.map(|a| a.list_messages(&info.directory, &info.id)) {
                            Some(Ok(m)) => (Ok(Arc::new(m)), true),
                            _ => (Err(()), false),
                        },
                    }
                    .pipe(|(m, fresh)| {
                        let r = m.as_ref().ok().and_then(|m| search_session(&key, &info, m, &q));
                        (key, info, m, fresh, r)
                    })
                });
                cx.spawn(async move |this, cx| {
                    let (_, info, m, fresh, r) = task.await;
                    this.update(cx, |s, cx| {
                        if s.seq != my {
                            return;
                        }
                        match &m {
                            Ok(msgs) if fresh => {
                                s.cache.insert(info.id.clone(), (info.updated, msgs.clone()));
                            }
                            Err(_) => {
                                if let Some(p) = s.progress.as_mut() {
                                    p.2 += 1;
                                }
                            }
                            _ => {}
                        }
                        if let Some(r) = r {
                            let mut v = std::mem::take(&mut s.results);
                            v.push(r);
                            s.results = rank_results(v);
                        }
                        if let Some(p) = s.progress.as_mut() {
                            p.0 += 1;
                            if p.0 >= p.1 {
                                s.running = false;
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn go(&mut self, r: &SessionResult, mid: Option<String>, cx: &mut Context<Self>) {
        let (k, id) = (r.key.clone(), r.session.id.clone());
        ProjectStore::global(cx).update(cx, |st, cx| st.select(Some(k), Some(id), cx));
        super::state::AppState::global(cx).update(cx, |a, cx| a.push("/", cx));
        if let Some(m) = mid {
            cx.set_global(super::chat_panel::MessageFocus(Some(m)));
        }
        self.seq += 1;
        cx.emit(SearchEvent::Close);
    }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}

impl Render for GlobalSearch {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let q = self.input.read(cx).text().trim().to_string();
        let store = ProjectStore::global(cx).read(cx);
        let names: HashMap<String, String> = store.projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
        let n_sessions = Self::sessions(cx).len();
        let mut status = String::new();
        if !q.is_empty() {
            match self.progress {
                Some((d, tot, _)) if self.running => status = i.tp("gsearch.progress", &[("done", &d.to_string()), ("total", &tot.to_string())]),
                Some(_) if self.results.is_empty() => status = i.t("ui.noResults"),
                _ => status = i.tp("gsearch.count", &[("n", &self.results.len().to_string())]),
            }
            if let Some((_, _, f)) = self.progress.filter(|p| p.2 > 0) {
                status.push_str(&format!(" · {}", i.tp("gsearch.failed", &[("n", &f.to_string())])));
            }
        }
        let mut list = div().flex().flex_col().gap(t.space(1));
        for (ri, r) in self.results.clone().into_iter().enumerate() {
            let title = session_title(&r.session).map(str::to_string).unwrap_or_else(|| i.t("sb.untitled"));
            let proj = if r.key == CHAT_KEY { i.t("chat.noProject") } else { names.get(&r.key).cloned().unwrap_or_else(|| r.key.clone()) };
            let date = chrono::DateTime::from_timestamp_millis(r.session.updated as i64).map(|d| d.with_timezone(&chrono::Local).format("%-d/%-m/%Y").to_string()).unwrap_or_default();
            let mut meta = format!("{proj} · {date}");
            if r.total > 0 {
                meta.push_str(&format!(" · {}", i.tp("gsearch.matches", &[("n", &r.total.to_string())])));
            }
            let r1 = r.clone();
            let first = r.hits.first().map(|h| h.message_id.clone());
            let mut item = div().flex().flex_col().child(
                div()
                    .id(SharedString::from(format!("gs-{ri}")))
                    .flex()
                    .flex_col()
                    .px(t.space(2))
                    .py(t.space(1))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.hover))
                    .on_click(cx.listener(move |s, _, _, cx| s.go(&r1, first.clone(), cx)))
                    .child(div().font_weight(FontWeight::MEDIUM).truncate().child(title))
                    .child(div().small(&t).text_color(t.colors.fg_muted).child(meta)),
            );
            for (hi, h) in r.hits.iter().enumerate() {
                let chars: Vec<char> = h.snippet.chars().collect();
                let (a, b) = (h.mat.0.min(chars.len()), h.mat.1.min(chars.len()));
                let (pre, hit, post): (String, String, String) = (chars[..a].iter().collect(), chars[a..b].iter().collect(), chars[b..].iter().collect());
                let r2 = r.clone();
                let mid = h.message_id.clone();
                item = item.child(
                    div()
                        .id(SharedString::from(format!("gs-{ri}-{hi}")))
                        .flex()
                        .gap(t.space(2))
                        .pl(t.space(4))
                        .pr(t.space(2))
                        .py(t.space(1))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.colors.hover))
                        .small(&t)
                        .on_click(cx.listener(move |s, _, _, cx| s.go(&r2, Some(mid.clone()), cx)))
                        .child(div().flex_none().text_color(t.colors.fg_subtle).child(i.t(if h.user { "gsearch.you" } else { "gsearch.assistant" })))
                        .child(div().flex().flex_wrap().min_w_0().child(pre).child(div().bg(t.colors.warning.opacity(0.35)).rounded(t.rem(rem::RADIUS_SM)).child(hit)).child(post)),
                );
            }
            list = list.child(item);
        }
        crate::ui::dialog(i.t("gsearch.title"))
            .width_rem(rem::SPACE[7] * 12.)
            .on_dismiss(cx.listener(|s, _, _, cx| {
                s.seq += 1;
                cx.emit(SearchEvent::Close);
            }))
            .child(self.input.clone())
            .child(div().flex().items_center().gap(t.space(1)).small(&t).text_color(t.colors.fg_muted).when(self.running, |d| d.child(crate::ui::spinner(crate::ui::IconSize::Sm))).child(status))
            .child(div().when(q.is_empty(), |d| d.small(&t).text_color(t.colors.fg_subtle).child(i.tp("gsearch.hint", &[("n", &n_sessions.to_string())]))))
            .child(list)
    }
}
