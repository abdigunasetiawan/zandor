//! SettingsStore — port packages/core/src/settings.ts. Skema diekspor dari TS (scripts/export-settings-gpui.ts) dan
//! menjadi satu-satunya sumber: default, validasi, grup, dan label UI. Nilai eksplisit disimpan di <data>/settings.json.
use gpui::{App, AppContext, Context, Entity, Global};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::OnceLock;

pub const GROUPS: [&str; 6] = ["general", "appearance", "chat", "notifications", "tools", "layout"];

pub fn schema() -> &'static Value {
    static S: OnceLock<Value> = OnceLock::new();
    S.get_or_init(|| serde_json::from_str(include_str!("../../assets/settings.schema.json")).expect("skema pengaturan"))
}

pub fn properties() -> &'static Map<String, Value> {
    schema()["properties"].as_object().expect("properties")
}

/// Kunci pengaturan berurutan sesuai skema.
pub fn keys() -> Vec<&'static str> {
    properties().keys().map(String::as_str).collect()
}

pub fn group_of(key: &str) -> &'static str {
    properties().get(key).and_then(|p| p["x-zandor"]["group"].as_str()).unwrap_or("general")
}

/// Validasi satu nilai terhadap skema; None = valid.
pub fn validate(key: &str, v: &Value) -> Option<String> {
    let Some(p) = properties().get(key) else { return Some(format!("kunci tidak dikenal: {key}")) };
    let ty = p["type"].as_str().unwrap_or("");
    let ok_type = match ty {
        "string" => v.is_string(),
        "number" => v.as_f64().is_some_and(f64::is_finite),
        "boolean" => v.is_boolean(),
        _ => false,
    };
    if !ok_type {
        return Some(format!("{key}: tipe harus {ty}"));
    }
    if let Some(e) = p["enum"].as_array() {
        let hit = e.iter().any(|x| match (x.as_f64(), v.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            _ => x == v,
        });
        if !hit {
            return Some(format!("{key}: nilai tidak diizinkan"));
        }
    }
    if let Some(n) = v.as_f64() {
        if p["minimum"].as_f64().is_some_and(|m| n < m) || p["maximum"].as_f64().is_some_and(|m| n > m) {
            return Some(format!("{key}: di luar rentang"));
        }
    }
    None
}

pub struct SettingsStore {
    path: PathBuf,
    explicit: Map<String, Value>,
}

struct Handle(Entity<SettingsStore>);
impl Global for Handle {}

impl SettingsStore {
    pub fn init(path: PathBuf, cx: &mut App) -> Entity<Self> {
        let mut explicit = Map::new();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(Value::Object(o)) = serde_json::from_str::<Value>(&text) {
                for (k, v) in o {
                    if validate(&k, &v).is_none() {
                        explicit.insert(k, v);
                    } else {
                        eprintln!("[zandor] pengaturan tidak valid diabaikan: {k}");
                    }
                }
            }
        }
        let e = cx.new(|_| Self { path, explicit });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<Handle>().0.clone()
    }
    pub fn get(&self, key: &str) -> Value {
        self.explicit.get(key).cloned().unwrap_or_else(|| properties().get(key).map(|p| p["default"].clone()).unwrap_or(Value::Null))
    }
    pub fn str(&self, key: &str) -> String {
        self.get(key).as_str().unwrap_or_default().to_string()
    }
    pub fn bool(&self, key: &str) -> bool {
        self.get(key).as_bool().unwrap_or(false)
    }
    pub fn num(&self, key: &str) -> f64 {
        self.get(key).as_f64().unwrap_or(0.)
    }
    pub fn is_explicit(&self, key: &str) -> bool {
        self.explicit.contains_key(key)
    }
    /// Semua nilai efektif (untuk tampilan JSON).
    pub fn all(&self) -> Map<String, Value> {
        keys().into_iter().map(|k| (k.to_string(), self.get(k))).collect()
    }
    pub fn explicit_json(&self) -> String {
        serde_json::to_string_pretty(&Value::Object(self.explicit.clone())).unwrap_or_default()
    }
    fn save(&self) {
        if let Some(d) = self.path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = std::fs::write(&self.path, format!("{}\n", self.explicit_json())) {
            eprintln!("[zandor] gagal menyimpan pengaturan: {e}");
        }
    }
    pub fn set(&mut self, key: &str, v: Value, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(e) = validate(key, &v) {
            return Err(e);
        }
        self.explicit.insert(key.into(), v);
        self.save();
        cx.notify();
        Ok(())
    }
    pub fn reset(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.explicit.remove(key).is_some() {
            self.save();
            cx.notify();
        }
    }
    pub fn reset_all(&mut self, cx: &mut Context<Self>) {
        self.explicit.clear();
        self.save();
        cx.notify();
    }
    /// Impor JSON (hanya kunci valid); kembalikan daftar galat.
    pub fn import(&mut self, text: &str, cx: &mut Context<Self>) -> Vec<String> {
        let Ok(Value::Object(o)) = serde_json::from_str::<Value>(text) else { return vec!["JSON tidak valid".into()] };
        let mut errs = vec![];
        for (k, v) in o {
            match validate(&k, &v) {
                None => {
                    self.explicit.insert(k, v);
                }
                Some(e) => errs.push(e),
            }
        }
        self.save();
        cx.notify();
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn skema_terbaca_dan_validasi() {
        assert!(keys().contains(&"theme"));
        assert!(validate("theme", &json!("dark")).is_none());
        assert!(validate("theme", &json!("ungu")).is_some());
        assert!(validate("uiScale", &json!(1.25)).is_none());
        assert!(validate("uiScale", &json!(3)).is_some());
        assert!(validate("onboarding", &json!(1)).is_some());
        for k in keys() {
            assert!(GROUPS.contains(&group_of(k)), "{k}");
            let d = &properties()[k]["default"];
            assert!(validate(k, d).is_none(), "default {k}");
        }
    }
    #[test]
    fn semua_label_ada_di_i18n() {
        let i = crate::i18n::I18n::new(crate::i18n::Locale::Id);
        for k in keys() {
            let m = &properties()[k]["x-zandor"];
            let t = m["titleKey"].as_str().unwrap();
            assert_ne!(i.t(t), t, "{t}");
        }
    }
}
