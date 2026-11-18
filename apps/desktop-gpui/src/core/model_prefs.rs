//! Port packages/core/src/models/prefs.ts: model bawaan (chat, gambar), model tersembunyi dari pemilih, dan logo kustom
//! provider/model. Disimpan di prefs.json (kunci yang sama dengan aplikasi TS).
use super::chat_store::{read_pref, write_pref};
use gpui::{App, AppContext, Context, Entity, Global};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const KEY: &str = "zandor.modelPrefs";
pub const MAX_LOGO_CHARS: usize = 96_000;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Ref {
    pub provider_id: String,
    pub model_id: String,
}
impl Ref {
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider_id, self.model_id)
    }
}

pub fn is_valid_logo(url: &str) -> bool {
    let re = regex::Regex::new(r"^data:image/(png|jpeg|webp|gif|svg\+xml);base64,[A-Za-z0-9+/=]+$").expect("regex");
    url.len() <= MAX_LOGO_CHARS && re.is_match(url)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelPrefs {
    pub default_chat: Option<Ref>,
    pub default_image: Option<Ref>,
    pub hidden: Vec<String>,
    pub provider_logos: BTreeMap<String, String>,
    pub model_logos: BTreeMap<String, String>,
}

fn as_ref(v: Option<&Value>) -> Option<Ref> {
    let o = v?.as_object()?;
    Some(Ref { provider_id: o.get("providerID")?.as_str()?.to_string(), model_id: o.get("modelID")?.as_str()?.to_string() })
}
fn logo_map(v: Option<&Value>) -> BTreeMap<String, String> {
    v.and_then(Value::as_object)
        .map(|o| o.iter().filter_map(|(k, x)| x.as_str().filter(|u| is_valid_logo(u)).map(|u| (k.clone(), u.to_string()))).collect())
        .unwrap_or_default()
}

pub fn parse(raw: Option<&str>) -> ModelPrefs {
    let Some(v) = raw.and_then(|r| serde_json::from_str::<Value>(r).ok()) else { return ModelPrefs::default() };
    ModelPrefs {
        default_chat: as_ref(v.get("defaultChat")),
        default_image: as_ref(v.get("defaultImage")),
        hidden: v.get("hidden").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        provider_logos: logo_map(v.get("providerLogos")),
        model_logos: logo_map(v.get("modelLogos")),
    }
}

fn ref_json(r: &Option<Ref>) -> Value {
    r.as_ref().map(|r| json!({"providerID": r.provider_id, "modelID": r.model_id})).unwrap_or(Value::Null)
}

pub fn serialize(p: &ModelPrefs) -> String {
    let m = |b: &BTreeMap<String, String>| Value::Object(b.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect::<Map<_, _>>());
    json!({
        "defaultChat": ref_json(&p.default_chat),
        "defaultImage": ref_json(&p.default_image),
        "hidden": p.hidden,
        "providerLogos": m(&p.provider_logos),
        "modelLogos": m(&p.model_logos),
    })
    .to_string()
}

pub struct ModelPrefsStore {
    pub prefs: ModelPrefs,
}
struct Handle(Entity<ModelPrefsStore>);
impl Global for Handle {}

impl ModelPrefsStore {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let e = cx.new(|_| Self { prefs: parse(read_pref(KEY).as_deref()) });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<Handle>().map(|h| h.0.clone())
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        write_pref(KEY, &serialize(&self.prefs));
        cx.notify();
    }
    pub fn set_default_chat(&mut self, r: Option<Ref>, cx: &mut Context<Self>) {
        self.prefs.default_chat = r;
        self.save(cx);
    }
    pub fn set_default_image(&mut self, r: Option<Ref>, cx: &mut Context<Self>) {
        self.prefs.default_image = r;
        self.save(cx);
    }
    pub fn is_hidden(&self, r: &Ref) -> bool {
        self.prefs.hidden.contains(&r.key())
    }
    pub fn set_visible(&mut self, refs: &[Ref], visible: bool, cx: &mut Context<Self>) {
        let keys: Vec<String> = refs.iter().map(Ref::key).collect();
        self.prefs.hidden.retain(|k| !keys.contains(k));
        if !visible {
            self.prefs.hidden.extend(keys);
        }
        self.save(cx);
    }
    pub fn set_provider_logo(&mut self, provider: &str, url: Option<String>, cx: &mut Context<Self>) -> bool {
        match url {
            None => {
                self.prefs.provider_logos.remove(provider);
            }
            Some(u) if is_valid_logo(&u) => {
                self.prefs.provider_logos.insert(provider.into(), u);
            }
            _ => return false,
        }
        self.save(cx);
        true
    }
    pub fn set_model_logo(&mut self, r: &Ref, url: Option<String>, cx: &mut Context<Self>) -> bool {
        match url {
            None => {
                self.prefs.model_logos.remove(&r.key());
            }
            Some(u) if is_valid_logo(&u) => {
                self.prefs.model_logos.insert(r.key(), u);
            }
            _ => return false,
        }
        self.save(cx);
        true
    }
    /// Logo kustom (model dulu, lalu provider) — ModelLogoBridge.
    pub fn override_for(&self, provider: Option<&str>, model: Option<&str>) -> Option<String> {
        if let (Some(p), Some(m)) = (provider, model) {
            if let Some(u) = self.prefs.model_logos.get(&format!("{p}/{m}")) {
                return Some(u.clone());
            }
        }
        provider.and_then(|p| self.prefs.provider_logos.get(p).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_validation() {
        let mut p = parse(Some(r#"{"defaultChat":{"providerID":"a","modelID":"b"},"hidden":["x/y",3],"providerLogos":{"a":"data:image/png;base64,AAAA","b":"http://x"}}"#));
        assert_eq!(p.default_chat, Some(Ref { provider_id: "a".into(), model_id: "b".into() }));
        assert_eq!(p.hidden, vec!["x/y".to_string()]);
        assert_eq!(p.provider_logos.len(), 1);
        p.default_image = None;
        assert_eq!(parse(Some(&serialize(&p))), p);
        assert_eq!(parse(Some("rusak")), ModelPrefs::default());
        assert!(!is_valid_logo(&format!("data:image/png;base64,{}", "A".repeat(MAX_LOGO_CHARS))));
    }
}
