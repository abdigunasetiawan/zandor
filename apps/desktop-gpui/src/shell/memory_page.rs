//! Halaman Memori & Skill — port MemoryPage.tsx + MemoryStore (packages/core/src/memory/store.ts): tab Pengaturan
//! (memori/skill creator bawaan + override per project), Catatan (edit berkas memori dengan pemindai secret), Antrean
//! skill (edit/setujui/tolak/hapus draf), dan Skill terpasang (global + per project; edit/hapus). Berkas dibaca plugin
//! Zandor di engine setiap giliran; skill yang berubah memuat ulang instance engine terkait.
use super::state::AppState;
use crate::core::mem::{self, parse_settings, serialize_settings, MemorySettings, SkillDraft};
use crate::core::projects::ProjectStore;
use crate::core::services::app_data_dir;
use crate::i18n::I18n;
use crate::memory::SkillInfo;
use crate::theme::{rem, Theme, H3, H4};
use crate::ui::{badge, button, icon_button, select, switch, tabs, BadgeTone, ButtonVariant, ControlSize, InputChanged, TabItem, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

fn mem_dir() -> std::path::PathBuf {
    app_data_dir().join("memory")
}
fn skills_root(scope: &str, project: Option<&str>) -> std::path::PathBuf {
    crate::memory::skills_root(scope, project.map(Path::new), &app_data_dir().join("engine")).unwrap_or_default()
}
pub(crate) fn read(rel: &str) -> String {
    crate::memory::read_memory(&mem_dir(), rel).unwrap_or_default()
}
pub(crate) fn write(rel: &str, text: &str) -> Result<(), String> {
    let _ = std::fs::create_dir_all(mem_dir());
    crate::memory::write_memory(&mem_dir(), rel, text)
}

/// Port parseQueue: baris rusak dilewati, id lama dibuat, status "draft" → "candidate".
pub fn parse_queue(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v.is_object())
        .map(|mut e| {
            if e["id"].as_str().unwrap_or("").is_empty() {
                e["id"] = Value::String(format!("legacy-{}", e["t"]));
            }
            if e["status"] == "draft" {
                e["status"] = "candidate".into();
            }
            e
        })
        .collect()
}
pub fn serialize_queue(q: &[Value]) -> String {
    let mut s: String = q.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
    if !q.is_empty() {
        s.push('\n');
    }
    s
}

#[derive(Clone)]
struct ProjectRef {
    id: String,
    name: String,
    path: String,
    key: String,
}

struct Draft {
    name: Entity<TextInput>,
    desc: Entity<TextInput>,
    body: Entity<TextInput>,
    target: String,
    error: Option<String>,
}

struct SkillEdit {
    editor: Entity<TextInput>,
    error: Option<String>,
}

enum Confirm {
    Note(String),
    Skill { scope: String, project: Option<String>, name: String },
}

pub struct MemoryPage {
    /// Tertanam di halaman Pengaturan: tanpa bingkai halaman, tombol kembali, dan judul (seperti `embedded` di Tauri).
    pub embedded: bool,
    app: Entity<AppState>,
    tab: SharedString,
    settings: MemorySettings,
    known: BTreeMap<String, String>,
    files: Vec<String>,
    queue: Vec<Value>,
    installed_global: Vec<SkillInfo>,
    installed_projects: HashMap<String, Vec<SkillInfo>>,
    note_rel: String,
    note: Entity<TextInput>,
    note_dirty: bool,
    note_error: Option<String>,
    drafts: HashMap<String, Draft>,
    skill_edits: HashMap<String, SkillEdit>,
    confirm: Option<Confirm>,
}

fn multiline(cx: &mut App, text: String) -> Entity<TextInput> {
    cx.new(|cx| {
        let mut ti = TextInput::new(cx, "");
        ti.multiline = true;
        ti.enter_newline = true;
        ti.set_text(text, cx);
        ti
    })
}
fn single(cx: &mut App, text: String) -> Entity<TextInput> {
    cx.new(|cx| {
        let mut ti = TextInput::new(cx, "");
        ti.set_text(text, cx);
        ti
    })
}

fn fail(e: String, cx: &mut App) {
    let title = cx.global::<I18n>().tp("sb.error", &[("error", &e)]);
    Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
}

impl MemoryPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let note = multiline(cx, String::new());
        cx.subscribe(&note, |s, _, _: &InputChanged, cx| {
            s.note_dirty = true;
            cx.notify();
        })
        .detach();
        let store = ProjectStore::global(cx);
        cx.observe(&store, |s, _, cx| s.refresh(cx)).detach();
        let mut me = Self {
            embedded: false,
            app,
            tab: "settings".into(),
            settings: MemorySettings::default(),
            known: BTreeMap::new(),
            files: vec![],
            queue: vec![],
            installed_global: vec![],
            installed_projects: HashMap::new(),
            note_rel: mem::FILE_GLOBAL.into(),
            note,
            note_dirty: false,
            note_error: None,
            drafts: HashMap::new(),
            skill_edits: HashMap::new(),
            confirm: None,
        };
        me.refresh(cx);
        me.load_note(cx);
        me
    }

    fn refs(cx: &App) -> Vec<ProjectRef> {
        ProjectStore::global(cx).read(cx).projects.iter().map(|p| ProjectRef { id: p.id.clone(), name: p.name.clone(), path: p.path.clone(), key: mem::project_key(&p.path) }).collect()
    }

    /// Port MemoryStore.refresh.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.settings = parse_settings(&read(mem::FILE_SETTINGS));
        let meta: Value = serde_json::from_str(&read(mem::FILE_META)).unwrap_or(Value::Null);
        self.known = meta["projects"].as_object().map(|o| o.iter().filter_map(|(k, v)| v["path"].as_str().map(|p| (k.clone(), p.to_string()))).collect()).unwrap_or_default();
        self.files = crate::memory::list_memory(&mem_dir()).into_iter().map(|f| f.rel).collect();
        self.queue = parse_queue(&read(mem::FILE_QUEUE));
        self.installed_global = crate::memory::list_skills(&skills_root("global", None), "global");
        self.installed_projects = Self::refs(cx).into_iter().map(|r| (r.path.clone(), crate::memory::list_skills(&skills_root("project", Some(&r.path)), "project"))).collect();
        cx.notify();
    }

    fn patch_settings(&mut self, f: impl FnOnce(&mut MemorySettings), cx: &mut Context<Self>) {
        let mut cur = parse_settings(&read(mem::FILE_SETTINGS));
        f(&mut cur);
        match write(mem::FILE_SETTINGS, &serialize_settings(&cur)) {
            Ok(()) => self.settings = cur,
            Err(e) => fail(e, cx),
        }
        cx.notify();
    }

    fn load_note(&mut self, cx: &mut Context<Self>) {
        let text = read(&self.note_rel);
        self.note.update(cx, |n, cx| n.set_text(text, cx));
        self.note_dirty = false;
        self.note_error = None;
        cx.notify();
    }

    fn mutate_queue(&mut self, f: impl FnOnce(Vec<Value>) -> Vec<Value>, cx: &mut Context<Self>) {
        // baca ulang tepat sebelum menulis: plugin bisa menambah baris kapan saja
        let q = f(parse_queue(&read(mem::FILE_QUEUE)));
        match write(mem::FILE_QUEUE, &serialize_queue(&q)) {
            Ok(()) => self.queue = q,
            Err(e) => fail(e, cx),
        }
        cx.notify();
    }

    fn skills_changed(&self, scope: &str, project: Option<&str>, cx: &mut Context<Self>) {
        let st = ProjectStore::global(cx).read(cx);
        let Some(api) = st.api() else { return };
        let dirs: Vec<String> = if scope == "project" { project.map(|p| vec![p.to_string()]).unwrap_or_default() } else { st.projects.iter().map(|p| p.path.clone()).chain(st.chat_dir.clone()).collect() };
        cx.background_spawn(async move {
            for d in dirs {
                let _ = api.dispose_instance(&d);
            }
        })
        .detach();
    }

    fn draft_of(&mut self, e: &Value, refs: &[ProjectRef], cx: &mut Context<Self>) {
        let id = e["id"].as_str().unwrap_or("").to_string();
        if self.drafts.contains_key(&id) {
            return;
        }
        let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
        let home = refs.iter().find(|r| Some(r.key.as_str()) == e["project"].as_str());
        let d = Draft {
            name: single(cx, s("name")),
            desc: single(cx, s("description")),
            body: multiline(cx, s("body")),
            target: home.map(|h| format!("project:{}", h.path)).unwrap_or_else(|| "global".into()),
            error: None,
        };
        self.drafts.insert(id, d);
    }

    fn draft_values(&self, id: &str, cx: &App) -> Option<SkillDraft> {
        let d = self.drafts.get(id)?;
        Some(SkillDraft { name: d.name.read(cx).text().to_string(), description: d.desc.read(cx).text().to_string(), body: d.body.read(cx).text().to_string() })
    }

    /// Port editDraft.
    fn edit_draft(&mut self, id: &str, d: &SkillDraft, cx: &mut Context<Self>) -> Option<String> {
        if let Some(err) = mem::validate_skill_draft(d) {
            return Some(err);
        }
        let (id, d) = (id.to_string(), d.clone());
        self.mutate_queue(
            move |q| {
                q.into_iter()
                    .map(|mut e| {
                        if e["id"] == id.as_str() {
                            e["name"] = d.name.clone().into();
                            e["description"] = d.description.clone().into();
                            e["body"] = d.body.clone().into();
                            e["status"] = "pending".into();
                        }
                        e
                    })
                    .collect()
            },
            cx,
        );
        None
    }

    fn set_status(&mut self, id: &str, status: &str, cx: &mut Context<Self>) {
        let (id, status) = (id.to_string(), status.to_string());
        self.mutate_queue(
            move |q| {
                q.into_iter()
                    .map(|mut e| {
                        if e["id"] == id.as_str() {
                            e["status"] = status.clone().into();
                        }
                        e
                    })
                    .collect()
            },
            cx,
        );
    }

    /// Port EntryCard.approve + MemoryStore.approve.
    fn approve(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(d) = self.draft_values(id, cx) else { return };
        let entry = self.queue.iter().find(|e| e["id"] == id).cloned().unwrap_or(Value::Null);
        let edited = ["name", "description", "body"].iter().zip([&d.name, &d.description, &d.body]).any(|(k, v)| entry[*k].as_str().unwrap_or("") != v.as_str());
        let mut err = if edited { self.edit_draft(id, &d, cx) } else { None };
        if err.is_none() {
            let target = self.drafts.get(id).map(|x| x.target.clone()).unwrap_or_default();
            let (scope, project) = match target.strip_prefix("project:") {
                Some(p) => ("project", Some(p.to_string())),
                None => ("global", None),
            };
            let cur = parse_queue(&read(mem::FILE_QUEUE)).into_iter().find(|e| e["id"] == id);
            err = if cur.as_ref().map(|e| e["status"] != "pending").unwrap_or(true) {
                Some("Draf tidak ditemukan atau belum siap disetujui.".into())
            } else if let Some(e) = mem::validate_skill_draft(&d) {
                Some(e)
            } else if scope == "project" && project.is_none() {
                Some("Pilih project untuk skill ini.".into())
            } else {
                match crate::memory::write_skill(&skills_root(scope, project.as_deref()), &d.name, &mem::skill_markdown(&d), false) {
                    Err(e) => Some(e),
                    Ok(_) => {
                        self.set_status(id, "approved", cx);
                        self.skills_changed(scope, project.as_deref(), cx);
                        self.refresh(cx);
                        let title = cx.global::<I18n>().tp("mem.queue.installed", &[("path", &d.name)]);
                        Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(title) }, cx);
                        None
                    }
                }
            };
        }
        if let Some(x) = self.drafts.get_mut(id) {
            x.error = err;
        }
        cx.notify();
    }

    fn save_note(&mut self, cx: &mut Context<Self>) {
        let text = self.note.read(cx).text().to_string();
        let found = mem::scan_secrets(&text);
        if !found.is_empty() {
            self.note_error = Some(cx.global::<I18n>().tp("mem.notes.secret", &[("kinds", &found.join(", "))]));
            cx.notify();
            return;
        }
        self.note_error = None;
        match write(&self.note_rel, &text) {
            Ok(()) => {
                self.note_dirty = false;
                self.files = crate::memory::list_memory(&mem_dir()).into_iter().map(|f| f.rel).collect();
                let title = cx.global::<I18n>().t("mem.notes.saved");
                Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(title) }, cx);
            }
            Err(e) => fail(e, cx),
        }
        cx.notify();
    }

    fn do_confirm(&mut self, cx: &mut Context<Self>) {
        match self.confirm.take() {
            Some(Confirm::Note(rel)) => match crate::memory::delete_memory(&mem_dir(), &rel) {
                Ok(()) => {
                    self.note.update(cx, |n, cx| n.set_text("", cx));
                    self.note_dirty = false;
                    self.files = crate::memory::list_memory(&mem_dir()).into_iter().map(|f| f.rel).collect();
                }
                Err(e) => fail(e, cx),
            },
            Some(Confirm::Skill { scope, project, name }) => match crate::memory::delete_skill(&skills_root(&scope, project.as_deref()), &name) {
                Ok(()) => {
                    self.skills_changed(&scope, project.as_deref(), cx);
                    self.refresh(cx);
                }
                Err(e) => fail(e, cx),
            },
            None => {}
        }
        cx.notify();
    }

    fn skill_save(&mut self, k: &str, scope: &str, project: Option<&str>, name: &str, cx: &mut Context<Self>) {
        let Some(md) = self.skill_edits.get(k).map(|e| e.editor.read(cx).text().to_string()) else { return };
        let err = match mem::parse_skill_markdown(&md) {
            Some(p) => mem::validate_skill_draft(&p).or_else(|| (p.name != name).then(|| "Nama di frontmatter harus sama dengan nama folder skill.".to_string())),
            None => {
                let f = mem::scan_secrets(&md);
                (!f.is_empty()).then(|| format!("Ditolak: teks tampak memuat secret ({}).", f.join(", ")))
            }
        };
        let err = err.or_else(|| crate::memory::write_skill(&skills_root(scope, project), name, &md, true).err());
        match err {
            Some(e) => {
                if let Some(x) = self.skill_edits.get_mut(k) {
                    x.error = Some(e);
                }
            }
            None => {
                self.skill_edits.remove(k);
                self.skills_changed(scope, project, cx);
                self.refresh(cx);
            }
        }
        cx.notify();
    }
}

fn muted(t: &Theme, s: String) -> gpui::Div {
    div().small(t).text_color(t.colors.fg_muted).child(s)
}
fn err_note(t: &Theme, s: String) -> gpui::Div {
    div().small(t).text_color(t.colors.danger).child(s)
}
fn field(t: &Theme, label: String, el: impl IntoElement) -> gpui::Div {
    div().flex().flex_col().gap(t.space(1)).child(div().small(t).font_weight(FontWeight::MEDIUM).child(label)).child(el)
}
fn card(t: &Theme) -> gpui::Div {
    div().flex().flex_col().gap(t.space(2)).p(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border_soft)
}
fn tri(on: Option<bool>) -> &'static str {
    match on {
        Some(true) => "on",
        Some(false) => "off",
        None => "follow",
    }
}
fn untri(v: &str) -> Option<bool> {
    match v {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

impl MemoryPage {
    fn settings_tab(&mut self, t: &Theme, i: &I18n, refs: &[ProjectRef], cx: &mut Context<Self>) -> AnyElement {
        let s = self.settings.clone();
        let me = cx.entity().downgrade();
        let sw = |id: &'static str, label: String, on: bool, which: u8| {
            let me = me.clone();
            switch(id, label, on).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.patch_settings(
                        |x| {
                            if which == 0 {
                                x.default_enabled = v
                            } else {
                                x.skill_creator = v
                            }
                        },
                        cx,
                    )
                })
                .ok();
            })
        };
        let mut col = div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(muted(t, i.t("mem.settings.intro")))
            .child(sw("mem-def", i.t("mem.settings.default"), s.default_enabled, 0))
            .child(muted(t, i.t("mem.settings.defaultHint")))
            .child(sw("mem-sc", i.t("mem.settings.skillCreator"), s.skill_creator, 1))
            .child(muted(t, i.t("mem.settings.skillCreatorHint")))
            .child(div().typo(t, H4).child(i.t("mem.settings.projects")));
        if refs.is_empty() {
            col = col.child(muted(t, i.t("mem.settings.projectsEmpty")));
        }
        for r in refs {
            let mem_items: Vec<(SharedString, SharedString)> = [("follow", "mem.settings.follow"), ("on", "mem.settings.on"), ("off", "mem.settings.off")].iter().map(|(v, k)| ((*v).into(), i.t(k).into())).collect();
            let sk_items: Vec<(SharedString, SharedString)> = [("follow", "mem.settings.skillFollow"), ("on", "mem.settings.skillOn"), ("off", "mem.settings.skillOff")].iter().map(|(v, k)| ((*v).into(), i.t(k).into())).collect();
            let (k1, k2, m1, m2) = (r.key.clone(), r.key.clone(), me.clone(), me.clone());
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_1().min_w_0().truncate().child(r.name.clone()))
                    .child(select(SharedString::from(format!("mem-p-{}", r.id)), mem_items, tri(s.projects.get(&r.key).copied())).on_change(move |v, _, cx| {
                        let (k, on) = (k1.clone(), untri(v.as_ref()));
                        m1.update(cx, |s, cx| {
                            s.patch_settings(
                                move |x| match on {
                                    Some(b) => {
                                        x.projects.insert(k, b);
                                    }
                                    None => {
                                        x.projects.remove(&k);
                                    }
                                },
                                cx,
                            )
                        })
                        .ok();
                    }))
                    .child(select(SharedString::from(format!("mem-s-{}", r.id)), sk_items, tri(s.skill_creator_projects.get(&r.key).copied())).on_change(move |v, _, cx| {
                        let (k, on) = (k2.clone(), untri(v.as_ref()));
                        m2.update(cx, |s, cx| {
                            s.patch_settings(
                                move |x| match on {
                                    Some(b) => {
                                        x.skill_creator_projects.insert(k, b);
                                    }
                                    None => {
                                        x.skill_creator_projects.remove(&k);
                                    }
                                },
                                cx,
                            )
                        })
                        .ok();
                    })),
            );
        }
        col.child(muted(t, i.t("mem.settings.sessionHint"))).into_any_element()
    }

    fn notes_tab(&mut self, t: &Theme, i: &I18n, refs: &[ProjectRef], cx: &mut Context<Self>) -> AnyElement {
        let label = |rel: &str| -> String {
            if rel == mem::FILE_GLOBAL {
                return i.t("mem.notes.global");
            }
            if let Some(id) = rel.strip_prefix("pins/").and_then(|r| r.strip_suffix(".md")) {
                return i.tp("mem.notes.pin", &[("id", id)]);
            }
            let key = rel.strip_prefix("projects/").and_then(|r| r.strip_suffix(".md")).unwrap_or("");
            let name = refs.iter().find(|r| r.key == key).map(|r| r.name.clone()).or_else(|| self.known.get(key).and_then(|p| p.split(['\\', '/']).filter(|s| !s.is_empty()).last().map(str::to_string)));
            match name {
                Some(n) => i.tp("mem.notes.project", &[("name", &n)]),
                None => i.tp("mem.notes.projectUnknown", &[("key", key)]),
            }
        };
        let mut rels: Vec<String> = vec![mem::FILE_GLOBAL.into()];
        for r in refs {
            let f = mem::project_file(&r.key);
            if !rels.contains(&f) {
                rels.push(f);
            }
        }
        for f in &self.files {
            if (f.starts_with("projects/") || f.starts_with("pins/") || f == mem::FILE_GLOBAL) && !rels.contains(f) {
                rels.push(f.clone());
            }
        }
        let items: Vec<(SharedString, SharedString)> = rels.iter().map(|r| (r.clone().into(), label(r).into())).collect();
        let me = cx.entity().downgrade();
        let empty = self.note.read(cx).text().is_empty();
        let rel = self.note_rel.clone();
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(field(
                t,
                i.t("mem.notes.file"),
                div().flex().flex_col().gap(t.space(1)).child(select("mem-file", items, self.note_rel.clone()).on_change(move |v, _, cx| {
                    let v = v.to_string();
                    me.update(cx, |s, cx| {
                        s.note_rel = v;
                        s.load_note(cx);
                    })
                    .ok();
                })).child(muted(t, i.t("mem.notes.hint"))),
            ))
            .child(
                div()
                    .id("mem-note")
                    .min_h(t.rem(14.))
                    .max_h(t.rem(32.))
                    .overflow_y_scroll()
                    .p(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .border_1()
                    .border_color(if self.note_error.is_some() { t.colors.danger } else { t.colors.border })
                    .code(t)
                    .child(self.note.clone()),
            )
            .children(self.note_error.clone().map(|e| err_note(t, e)))
            .child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .child(button("mem-save").variant(ButtonVariant::Solid).disabled(!self.note_dirty).label(i.t("mem.notes.save")).on_click(cx.listener(|s, _, _, cx| s.save_note(cx))))
                    .child(button("mem-del").variant(ButtonVariant::Danger).disabled(rel == mem::FILE_GLOBAL && empty).label(i.t("mem.notes.delete")).on_click(cx.listener(move |s, _, _, cx| {
                        s.confirm = Some(Confirm::Note(rel.clone()));
                        cx.notify();
                    }))),
            )
            .into_any_element()
    }

    fn queue_tab(&mut self, t: &Theme, i: &I18n, refs: &[ProjectRef], cx: &mut Context<Self>) -> AnyElement {
        let mut entries = self.queue.clone();
        entries.sort_by(|a, b| b["t"].as_f64().unwrap_or(0.).partial_cmp(&a["t"].as_f64().unwrap_or(0.)).unwrap_or(std::cmp::Ordering::Equal));
        if entries.is_empty() {
            return muted(t, i.t("mem.queue.empty")).into_any_element();
        }
        let mut col = div().flex().flex_col().gap(t.space(2));
        for e in entries {
            let id = e["id"].as_str().unwrap_or("").to_string();
            let status = e["status"].as_str().unwrap_or("candidate").to_string();
            let trigger = e["trigger"].as_str().unwrap_or("manual");
            let pending = status == "pending";
            let tone = match status.as_str() {
                "pending" => BadgeTone::Warning,
                "approved" => BadgeTone::Success,
                _ => BadgeTone::Neutral,
            };
            let when = e["t"].as_f64().and_then(|ms| chrono::DateTime::from_timestamp_millis(ms as i64)).map(|d| d.with_timezone(&chrono::Local).format("%-d/%-m/%Y %H.%M.%S").to_string()).unwrap_or_default();
            let mut c = card(t).child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(badge(i.t(&format!("mem.queue.status.{status}"))).tone(tone))
                    .child(div().small(t).font_weight(FontWeight::MEDIUM).child(i.t(&format!("mem.queue.trigger.{trigger}"))))
                    .child(muted(t, when)),
            );
            if let Some(x) = e["excerpt"].as_str().filter(|x| !x.is_empty()) {
                c = c.child(muted(t, i.tp("mem.queue.excerpt", &[("text", x)])));
            }
            if pending {
                self.draft_of(&e, refs, cx);
                let d = &self.drafts[&id];
                let mut scope_items: Vec<(SharedString, SharedString)> = vec![("global".into(), i.t("mem.queue.scopeGlobal").into())];
                scope_items.extend(refs.iter().map(|r| (format!("project:{}", r.path).into(), i.tp("mem.notes.project", &[("name", &r.name)]).into())));
                let (me, id2) = (cx.entity().downgrade(), id.clone());
                let boxed = |el: Entity<TextInput>, tall: bool| div().p(t.space(1)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).when(tall, |d| d.min_h(t.rem(10.)).code(t)).child(el);
                c = c
                    .child(field(t, i.t("mem.queue.name"), boxed(d.name.clone(), false)))
                    .child(field(t, i.t("mem.queue.description"), boxed(d.desc.clone(), false)))
                    .child(field(t, i.t("mem.queue.body"), boxed(d.body.clone(), true)))
                    .child(field(
                        t,
                        i.t("mem.queue.scope"),
                        select(SharedString::from(format!("mq-scope-{id}")), scope_items, d.target.clone()).on_change(move |v, _, cx| {
                            let v = v.to_string();
                            me.update(cx, |s, cx| {
                                if let Some(d) = s.drafts.get_mut(&id2) {
                                    d.target = v;
                                }
                                cx.notify();
                            })
                            .ok();
                        }),
                    ))
                    .children(d.error.clone().map(|x| err_note(t, x)));
            } else if let Some(n) = e["name"].as_str() {
                c = c.child(div().small(t).child(div().flex().gap(t.space(1)).child(div().font_weight(FontWeight::SEMIBOLD).child(n.to_string())).child(format!("— {}", e["description"].as_str().unwrap_or("")))));
            }
            let mut actions = div().flex().flex_wrap().gap(t.space(2));
            if pending {
                let (a, b, r) = (id.clone(), id.clone(), id.clone());
                actions = actions
                    .child(button(SharedString::from(format!("mq-ok-{id}"))).variant(ButtonVariant::Solid).label(i.t("mem.queue.approve")).on_click(cx.listener(move |s, _, _, cx| s.approve(&a, cx))))
                    .child(button(SharedString::from(format!("mq-ed-{id}"))).variant(ButtonVariant::Ghost).label(i.t("mem.queue.saveEdit")).on_click(cx.listener(move |s, _, _, cx| {
                        if let Some(d) = s.draft_values(&b, cx) {
                            let err = s.edit_draft(&b, &d, cx);
                            if let Some(x) = s.drafts.get_mut(&b) {
                                x.error = err;
                            }
                            cx.notify();
                        }
                    })))
                    .child(button(SharedString::from(format!("mq-no-{id}"))).variant(ButtonVariant::Ghost).label(i.t("mem.queue.reject")).on_click(cx.listener(move |s, _, _, cx| s.set_status(&r, "rejected", cx))));
            }
            let rm = id.clone();
            actions = actions.child(button(SharedString::from(format!("mq-rm-{id}"))).variant(ButtonVariant::Danger).label(i.t("mem.queue.remove")).on_click(cx.listener(move |s, _, _, cx| {
                let rm = rm.clone();
                s.drafts.remove(&rm);
                s.mutate_queue(move |q| q.into_iter().filter(|e| e["id"] != rm.as_str()).collect(), cx);
            })));
            col = col.child(c.child(actions));
        }
        col.into_any_element()
    }

    fn skills_tab(&mut self, t: &Theme, i: &I18n, refs: &[ProjectRef], cx: &mut Context<Self>) -> AnyElement {
        let mut groups: Vec<(String, String, Option<String>, Vec<SkillInfo>)> = vec![("global".into(), i.t("mem.skills.global"), None, self.installed_global.clone())];
        for r in refs {
            groups.push(("project".into(), i.tp("mem.skills.project", &[("name", &r.name)]), Some(r.path.clone()), self.installed_projects.get(&r.path).cloned().unwrap_or_default()));
        }
        if groups.iter().all(|g| g.3.is_empty()) {
            return muted(t, i.t("mem.skills.empty")).into_any_element();
        }
        let mut col = div().flex().flex_col().gap(t.space(4));
        for (scope, title, project, skills) in groups.into_iter().filter(|g| !g.3.is_empty()) {
            let mut sec = div().flex().flex_col().gap(t.space(2)).child(div().typo(t, H4).child(title));
            for s in skills {
                let k = format!("{scope}|{}|{}", project.clone().unwrap_or_default(), s.name);
                let editing = self.skill_edits.contains_key(&k);
                let mut c = card(t);
                let mut head = div().flex().items_center().gap(t.space(2)).child(div().flex_1().font_weight(FontWeight::MEDIUM).child(s.name.clone()));
                if !editing {
                    let (k2, sc, pj, nm) = (k.clone(), scope.clone(), project.clone(), s.name.clone());
                    head = head.child(button(SharedString::from(format!("sk-ed-{k}"))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("mem.skills.edit")).on_click(cx.listener(move |me, _, _, cx| {
                        match crate::memory::read_skill(&skills_root(&sc, pj.as_deref()), &nm) {
                            Ok(text) => {
                                let editor = multiline(cx, text);
                                me.skill_edits.insert(k2.clone(), SkillEdit { editor, error: None });
                            }
                            Err(e) => fail(e, cx),
                        }
                        cx.notify();
                    })));
                }
                let (sc, pj, nm) = (scope.clone(), project.clone(), s.name.clone());
                head = head.child(button(SharedString::from(format!("sk-del-{k}"))).size(ControlSize::Sm).variant(ButtonVariant::Danger).label(i.t("mem.skills.delete")).on_click(cx.listener(move |me, _, _, cx| {
                    me.confirm = Some(Confirm::Skill { scope: sc.clone(), project: pj.clone(), name: nm.clone() });
                    cx.notify();
                })));
                c = c.child(head);
                if let Some(ed) = self.skill_edits.get(&k) {
                    let (k1, k2, sc, pj, nm) = (k.clone(), k.clone(), scope.clone(), project.clone(), s.name.clone());
                    c = c
                        .child(div().id(SharedString::from(format!("sk-tx-{k}"))).min_h(t.rem(14.)).max_h(t.rem(32.)).overflow_y_scroll().p(t.space(2)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).code(t).child(ed.editor.clone()))
                        .children(ed.error.clone().map(|e| err_note(t, e)))
                        .child(
                            div()
                                .flex()
                                .gap(t.space(2))
                                .child(button(SharedString::from(format!("sk-sv-{k}"))).variant(ButtonVariant::Solid).label(i.t("mem.skills.save")).on_click(cx.listener(move |me, _, _, cx| me.skill_save(&k1, &sc, pj.as_deref(), &nm, cx))))
                                .child(button(SharedString::from(format!("sk-x-{k}"))).variant(ButtonVariant::Ghost).label("✕").on_click(cx.listener(move |me, _, _, cx| {
                                    me.skill_edits.remove(&k2);
                                    cx.notify();
                                }))),
                        );
                }
                sec = sec.child(c);
            }
            col = col.child(sec);
        }
        col.into_any_element()
    }
}

impl Render for MemoryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let refs = Self::refs(cx);
        let body = match self.tab.as_ref() {
            "settings" => self.settings_tab(&t, &i, &refs, cx),
            "notes" => self.notes_tab(&t, &i, &refs, cx),
            "queue" => self.queue_tab(&t, &i, &refs, cx),
            _ => self.skills_tab(&t, &i, &refs, cx),
        };
        let pending = self.queue.iter().filter(|e| e["status"] == "pending").count();
        let confirm = self.confirm.as_ref().map(|c| {
            let (title, desc, label) = match c {
                Confirm::Note(rel) => (i.tp("mem.notes.deleteTitle", &[("name", rel)]), i.t("mem.notes.deleteBody"), i.t("mem.notes.delete")),
                Confirm::Skill { name, .. } => (i.tp("mem.skills.deleteTitle", &[("name", name)]), i.t("mem.skills.deleteBody"), i.t("mem.skills.delete")),
            };
            crate::ui::dialog(title)
                .description(desc)
                .on_dismiss(cx.listener(|s, _, _, cx| {
                    s.confirm = None;
                    cx.notify();
                }))
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap(t.space(2))
                        .child(button("mem-cf-no").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                            s.confirm = None;
                            cx.notify();
                        })))
                        .child(button("mem-cf-yes").variant(ButtonVariant::Danger).label(label).on_click(cx.listener(|s, _, _, cx| s.do_confirm(cx)))),
                )
        });
        let embedded = self.embedded;
        let col = div()
                    .flex()
                    .flex_col()
                    .gap(t.space(4))
                    .w_full()
                    .when(!embedded, |d| d.max_w(t.rem(rem::SPACE[7] * 16.)).mx_auto().px(t.space(4)).py(t.space(5)))
                    .when(!embedded, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(t.space(2))
                                .child(icon_button("mem-back", "arrow_left", i.t("mem.back")).on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/", cx)))))
                                .child(div().typo(&t, H3).child(i.t("mem.title"))),
                        )
                    })
                    .child(
                        tabs(
                            "mem-tabs",
                            ["settings", "notes", "queue", "skills"]
                                .iter()
                                .map(|v| {
                                    let mut label = i.t(&format!("mem.tab.{v}"));
                                    if *v == "queue" && pending > 0 {
                                        label.push_str(&format!(" ({pending})"));
                                    }
                                    TabItem { value: (*v).into(), label: label.into(), icon: None, disabled: false }
                                })
                                .collect(),
                            self.tab.clone(),
                        )
                        .on_change({
                            let me = cx.entity().downgrade();
                            move |v, _, cx| {
                                me.update(cx, |s, cx| {
                                    s.tab = v;
                                    s.refresh(cx);
                                })
                                .ok();
                            }
                        }),
                    )
                    .child(div().p(t.space(4)).rounded(t.rem(rem::RADIUS_LG)).bg(t.colors.surface).child(body));
        if embedded {
            return div().w_full().child(col).children(confirm).into_any_element();
        }
        div().id("mem-page").flex().flex_col().size_full().overflow_y_scroll().bg(t.colors.frame).child(col).children(confirm).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn antrean() {
        let q = super::parse_queue("{\"t\":5,\"status\":\"draft\",\"trigger\":\"manual\"}\nrusak\n\n{\"id\":\"a\",\"status\":\"pending\"}\n");
        assert_eq!(q.len(), 2);
        assert_eq!(q[0]["id"], "legacy-5");
        assert_eq!(q[0]["status"], "candidate");
        let s = super::serialize_queue(&q);
        assert!(s.ends_with('\n'));
        assert_eq!(super::parse_queue(&s), q);
        assert_eq!(super::serialize_queue(&[]), "");
    }
}
