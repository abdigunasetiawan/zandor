//! Log aktivitas — port ActivityLog (core) untuk panel Log: event agent (tool, izin, galat) dan event aplikasi
//! (galat/peringatan, engine, git, terminal). Ring buffer di memori (maks 500), tidak disimpan ke disk.
use gpui::{App, AppContext, Context, Entity, Global};
use std::collections::VecDeque;

pub const SOURCES: [&str; 7] = ["all", "app", "engine", "chat", "tool", "git", "terminal"];
const MAX: usize = 500;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u64,
    pub at: f64,
    pub source: &'static str,
    /// "info" | "warn" | "error"
    pub level: &'static str,
    pub message: String,
}

#[derive(Default)]
pub struct Activity {
    pub entries: VecDeque<Entry>,
    next: u64,
}

struct Handle(Entity<Activity>);
impl Global for Handle {}

impl Activity {
    pub fn init(cx: &mut App) {
        let e = cx.new(|_| Activity::default());
        cx.set_global(Handle(e));
    }
    pub fn global(cx: &App) -> Option<Entity<Activity>> {
        cx.try_global::<Handle>().map(|h| h.0.clone())
    }
    pub fn push(&mut self, source: &'static str, level: &'static str, message: String, cx: &mut Context<Self>) {
        self.next += 1;
        self.entries.push_back(Entry { id: self.next, at: super::projects::now_ms(), source, level, message: message.chars().take(500).collect() });
        while self.entries.len() > MAX {
            self.entries.pop_front();
        }
        cx.notify();
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        cx.notify();
    }
}

/// Catat satu entri (diabaikan bila log belum diinisialisasi, mis. di tes).
pub fn log(cx: &mut App, source: &'static str, level: &'static str, message: impl Into<String>) {
    if let Some(e) = Activity::global(cx) {
        let m = message.into();
        e.update(cx, |a, cx| a.push(source, level, m, cx));
    }
}
