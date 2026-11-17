//! Automasi terjadwal — port packages/core/src/automation (schedule.ts + store.ts). Waktu lokal pengguna.
use chrono::{Datelike, Local, NaiveTime, TimeZone};
use gpui::{App, AppContext, Context, Entity, Global};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MIN_INTERVAL: i64 = 5;
pub const MAX_INTERVAL: i64 = 7 * 24 * 60;
pub const MAX_TASKS: usize = 50;
const NAME_MAX: usize = 60;
const PROMPT_MAX: usize = 4000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Schedule {
    Interval {
        #[serde(rename = "everyMinutes")]
        every_minutes: i64,
    },
    Daily { time: String, days: Vec<u32> },
    Once { at: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub prompt: String,
    pub schedule: Schedule,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub created_at: f64,
    pub last_run_at: Option<f64>,
    pub last_session_id: Option<String>,
    pub last_error: Option<String>,
}
fn yes() -> bool {
    true
}

fn parse_time(t: &str) -> Option<NaiveTime> {
    let (h, m) = t.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    NaiveTime::from_hms_opt(h.parse().ok()?, m.parse().ok()?, 0)
}

pub fn validate_schedule(s: &Schedule, now: f64) -> Option<String> {
    match s {
        Schedule::Interval { every_minutes: n } => {
            (!(MIN_INTERVAL..=MAX_INTERVAL).contains(n)).then(|| format!("interval harus bilangan bulat {MIN_INTERVAL}–{MAX_INTERVAL} menit"))
        }
        Schedule::Daily { time, days } => {
            if parse_time(time).is_none() {
                Some("jam harus berformat HH:MM (24 jam)".into())
            } else if days.iter().any(|d| *d > 6) {
                Some("hari harus berisi angka 0–6".into())
            } else {
                None
            }
        }
        Schedule::Once { at } => {
            if !at.is_finite() {
                Some("waktu tidak valid".into())
            } else if *at < now - 60_000. {
                Some("waktu sudah lewat".into())
            } else {
                None
            }
        }
    }
}

pub fn validate_task(name: &str, prompt: &str, s: &Schedule, now: f64) -> Option<String> {
    if name.trim().is_empty() || name.chars().count() > NAME_MAX {
        return Some(format!("nama wajib (maks {NAME_MAX} karakter)"));
    }
    if prompt.trim().is_empty() || prompt.chars().count() > PROMPT_MAX {
        return Some(format!("prompt wajib (maks {PROMPT_MAX} karakter)"));
    }
    validate_schedule(s, now)
}

fn at_local(base_ms: f64, day_offset: i64, t: NaiveTime) -> Option<(f64, u32)> {
    let d = Local.timestamp_millis_opt(base_ms as i64).single()?.date_naive() + chrono::Duration::days(day_offset);
    let dt = Local.from_local_datetime(&d.and_time(t)).earliest()?;
    Some((dt.timestamp_millis() as f64, dt.weekday().num_days_from_sunday()))
}

fn last_daily(time: &str, days: &[u32], now: f64) -> Option<f64> {
    let t = parse_time(time)?;
    for back in 0..8 {
        let (ms, wd) = at_local(now, -back, t)?;
        if ms > now {
            continue;
        }
        if days.is_empty() || days.contains(&wd) {
            return Some(ms);
        }
    }
    None
}

/// Kebijakan "kejar sekali" — sama dengan isDue di TS.
pub fn is_due(t: &Task, now: f64) -> bool {
    match &t.schedule {
        Schedule::Once { at } => t.last_run_at.is_none() && *at <= now,
        Schedule::Interval { every_minutes } => now >= t.last_run_at.unwrap_or(t.created_at) + *every_minutes as f64 * 60_000.,
        Schedule::Daily { time, days } => match last_daily(time, days, now) {
            None => false,
            Some(occ) => occ >= t.created_at && t.last_run_at.is_none_or(|l| l < occ),
        },
    }
}

pub fn next_run(t: &Task, now: f64) -> Option<f64> {
    match &t.schedule {
        Schedule::Once { at } => t.last_run_at.is_none().then_some(*at),
        Schedule::Interval { every_minutes } => Some(t.last_run_at.unwrap_or(t.created_at) + *every_minutes as f64 * 60_000.),
        Schedule::Daily { time, days } => {
            let tm = parse_time(time)?;
            for ahead in 0..8 {
                let (ms, wd) = at_local(now, ahead, tm)?;
                if ms <= now {
                    continue;
                }
                if days.is_empty() || days.contains(&wd) {
                    return Some(ms);
                }
            }
            None
        }
    }
}

pub fn fmt_time(ms: f64) -> String {
    Local.timestamp_millis_opt(ms as i64).single().map(|d| d.format("%d/%m %H:%M").to_string()).unwrap_or_default()
}

pub fn parse_local(s: &str) -> Option<f64> {
    let dt = chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M").ok()?;
    Local.from_local_datetime(&dt).earliest().map(|d| d.timestamp_millis() as f64)
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    version: u32,
    tasks: Vec<Task>,
}

pub struct AutomationStore {
    path: PathBuf,
    pub tasks: Vec<Task>,
    pub running: Vec<String>,
}

struct Handle(Entity<AutomationStore>);
impl Global for Handle {}

impl AutomationStore {
    pub fn init(path: PathBuf, cx: &mut App) -> Entity<Self> {
        let tasks = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Persisted>(&t).ok())
            .map(|p| p.tasks.into_iter().take(MAX_TASKS).filter(|t| validate_task(&t.name, &t.prompt, &t.schedule, 0.).is_none()).collect())
            .unwrap_or_default();
        let e = cx.new(|_| Self { path, tasks, running: vec![] });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<Handle>().0.clone()
    }
    fn save(&self) {
        let p = Persisted { version: 1, tasks: self.tasks.clone() };
        if let Ok(s) = serde_json::to_string(&p) {
            let _ = std::fs::write(&self.path, s);
        }
    }
    pub fn upsert(&mut self, id: Option<String>, name: String, project_id: Option<String>, prompt: String, schedule: Schedule, cx: &mut Context<Self>) -> Result<(), String> {
        let now = super::projects::now_ms();
        if let Some(e) = validate_task(&name, &prompt, &schedule, now) {
            return Err(e);
        }
        match id.and_then(|id| self.tasks.iter_mut().find(|t| t.id == id)) {
            Some(t) => {
                t.name = name;
                t.project_id = project_id;
                t.prompt = prompt;
                if t.schedule != schedule {
                    t.schedule = schedule;
                    t.created_at = now;
                    t.last_run_at = None;
                }
            }
            None => {
                if self.tasks.len() >= MAX_TASKS {
                    return Err(format!("maksimal {MAX_TASKS} tugas"));
                }
                self.tasks.push(Task {
                    id: format!("at_{}", &super::projects::new_id()[..8.min(super::projects::new_id().len())]),
                    name,
                    project_id,
                    prompt,
                    schedule,
                    enabled: true,
                    created_at: now,
                    last_run_at: None,
                    last_session_id: None,
                    last_error: None,
                })
            }
        }
        self.save();
        cx.notify();
        Ok(())
    }
    pub fn set_enabled(&mut self, id: &str, v: bool, cx: &mut Context<Self>) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.enabled = v;
        }
        self.save();
        cx.notify();
    }
    pub fn remove(&mut self, id: &str, cx: &mut Context<Self>) {
        self.tasks.retain(|t| t.id != id);
        self.save();
        cx.notify();
    }
    pub fn mark(&mut self, id: &str, at: f64, session: Option<String>, err: Option<String>, cx: &mut Context<Self>) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.last_run_at = Some(at);
            if session.is_some() {
                t.last_session_id = session;
            }
            t.last_error = err.map(|e| e.chars().take(300).collect());
        }
        self.running.retain(|r| r != id);
        self.save();
        cx.notify();
    }
}

/// Jalankan satu tugas: buat sesi di project/Chat lalu kirim prompt.
pub fn run_task(id: String, cx: &mut App) {
    let store = AutomationStore::global(cx);
    let Some(task) = store.read(cx).tasks.iter().find(|t| t.id == id).cloned() else { return };
    if store.read(cx).running.contains(&id) {
        return;
    }
    store.update(cx, |s, cx| {
        s.running.push(id.clone());
        cx.notify();
    });
    let projects = super::projects::ProjectStore::global(cx);
    let key = task.project_id.clone().unwrap_or_else(|| super::projects::CHAT_KEY.to_string());
    let create = projects.update(cx, |p, cx| p.create_session(&key, Some(task.name.clone()), false, cx));
    cx.spawn(async move |cx| {
        let r = create.await;
        let now = super::projects::now_ms();
        cx.update(|cx| match r {
            Ok(s) => {
                let dir = s.directory.clone();
                let chat = super::chat_store::ChatStore::global(cx);
                chat.update(cx, |c, cx| {
                    c.open(&dir, &s.id, cx);
                    c.send(&s.id, task.prompt.clone(), cx);
                });
                store.update(cx, |st, cx| st.mark(&id, now, Some(s.id), None, cx));
            }
            Err(e) => store.update(cx, |st, cx| st.mark(&id, now, None, Some(e), cx)),
        });
    })
    .detach();
}

/// Detak penjadwal: periksa tugas jatuh tempo setiap 30 detik saat engine siap.
pub fn start_scheduler(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_secs(30)).await;
            cx.update(|cx| {
                if super::projects::ProjectStore::global(cx).read(cx).api().is_none() {
                    return;
                }
                let now = super::projects::now_ms();
                let due: Vec<String> = AutomationStore::global(cx).read(cx).tasks.iter().filter(|t| t.enabled && is_due(t, now)).map(|t| t.id.clone()).collect();
                for id in due {
                    run_task(id, cx);
                }
            });
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task(s: Schedule, created: f64, last: Option<f64>) -> Task {
        Task {
            id: "a".into(),
            name: "n".into(),
            project_id: None,
            prompt: "p".into(),
            schedule: s,
            enabled: true,
            created_at: created,
            last_run_at: last,
            last_session_id: None,
            last_error: None,
        }
    }
    #[test]
    fn interval_dan_sekali() {
        let t = task(Schedule::Interval { every_minutes: 10 }, 0., None);
        assert!(!is_due(&t, 9. * 60_000.));
        assert!(is_due(&t, 10. * 60_000.));
        let o = task(Schedule::Once { at: 1000. }, 0., None);
        assert!(is_due(&o, 1000.));
        assert!(!is_due(&task(Schedule::Once { at: 1000. }, 0., Some(1000.)), 5000.));
        assert!(validate_schedule(&Schedule::Interval { every_minutes: 4 }, 0.).is_some());
        assert!(validate_schedule(&Schedule::Daily { time: "25:00".into(), days: vec![] }, 0.).is_some());
    }
    #[test]
    fn harian() {
        let now = Local::now().timestamp_millis() as f64;
        let t = task(Schedule::Daily { time: "00:00".into(), days: vec![] }, now - 3. * 86_400_000., None);
        assert!(is_due(&t, now));
        assert!(next_run(&t, now).is_some_and(|n| n > now));
    }
    #[test]
    fn serde_sama_dengan_ts() {
        let j = r#"{"version":1,"tasks":[{"id":"at_1","name":"x","projectId":null,"prompt":"p","schedule":{"kind":"interval","everyMinutes":5},"enabled":true,"createdAt":1,"lastRunAt":null,"lastSessionId":null,"lastError":null}]}"#;
        let p: Persisted = serde_json::from_str(j).unwrap();
        assert_eq!(p.tasks[0].schedule, Schedule::Interval { every_minutes: 5 });
    }
}
