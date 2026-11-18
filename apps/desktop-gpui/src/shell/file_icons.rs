//! Ikon tipe berkas/folder — port packages/core/src/file-icons.ts + shell/FileTypeIcon.tsx. Data Material Icon Theme (MIT,
//! ADR-031) diekspor scripts/export-file-icons-gpui.ts ke assets/file-icons.json dan di-embed; diurai sekali saat dipakai.
use crate::theme::Theme;
use crate::ui::icon;
use gpui::{div, prelude::*, AnyElement};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

struct Manifest {
    file: String,
    folder: String,
    folder_expanded: String,
    file_names: HashMap<String, String>,
    file_exts: HashMap<String, String>,
    folder_names: HashMap<String, String>,
    folder_names_exp: HashMap<String, String>,
    defs: HashMap<String, String>,
    svgs: HashMap<String, String>,
}

fn map(v: &Value, k: &str) -> HashMap<String, String> {
    v.get(k).and_then(Value::as_object).map(|o| o.iter().filter_map(|(a, b)| b.as_str().map(|s| (a.clone(), s.to_string()))).collect()).unwrap_or_default()
}

fn manifest() -> &'static Manifest {
    static M: OnceLock<Manifest> = OnceLock::new();
    M.get_or_init(|| {
        let v: Value = serde_json::from_str(include_str!("../../assets/file-icons.json")).unwrap_or_default();
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Manifest {
            file: s("file"),
            folder: s("folder"),
            folder_expanded: s("folderExpanded"),
            file_names: map(&v, "fileNames"),
            file_exts: map(&v, "fileExtensions"),
            folder_names: map(&v, "folderNames"),
            folder_names_exp: map(&v, "folderNamesExpanded"),
            defs: map(&v, "defs"),
            svgs: map(&v, "svgs"),
        }
    })
}

/// resolveIconName: nama persis → ekstensi (majemuk dulu) → bawaan; folder: nama → bawaan.
pub fn resolve_icon_name(name: &str, dir: bool, expanded: bool) -> String {
    let m = manifest();
    let n = name.to_lowercase();
    let found = if dir {
        (if expanded { &m.folder_names_exp } else { &m.folder_names }).get(&n).cloned().unwrap_or_else(|| if expanded { m.folder_expanded.clone() } else { m.folder.clone() })
    } else {
        m.file_names
            .get(&n)
            .cloned()
            .or_else(|| n.match_indices('.').find_map(|(i, _)| m.file_exts.get(&n[i + 1..]).cloned()))
            .unwrap_or_else(|| m.file.clone())
    };
    if m.defs.contains_key(&found) {
        found
    } else if dir {
        m.folder.clone()
    } else {
        m.file.clone()
    }
}

thread_local! {
    static CACHE: std::cell::RefCell<HashMap<String, Arc<gpui::Image>>> = Default::default();
}

fn image(icon_name: &str) -> Option<Arc<gpui::Image>> {
    if let Some(i) = CACHE.with(|c| c.borrow().get(icon_name).cloned()) {
        return Some(i);
    }
    let m = manifest();
    let svg = m.svgs.get(m.defs.get(icon_name)?)?;
    let img = Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, svg.clone().into_bytes()));
    CACHE.with(|c| c.borrow_mut().insert(icon_name.to_string(), img.clone()));
    Some(img)
}

/// FileTypeIcon: SVG Material bila ada, selain itu ikon Lucide generik. `size_rem` = sisi ikon.
pub fn file_type_icon(t: &Theme, name: &str, dir: bool, expanded: bool, size_rem: f32) -> AnyElement {
    let s = t.rem(size_rem);
    match image(&resolve_icon_name(name, dir, expanded)) {
        Some(img) => div().flex_none().size(s).child(gpui::img(img).size_full()).into_any_element(),
        None => {
            let ic = if !dir { "file" } else if expanded { "folder_open" } else { "folder" };
            div().flex_none().size(s).flex().items_center().justify_center().child(icon(ic).color(t.colors.fg_muted)).into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve() {
        let f = |n: &str| resolve_icon_name(n, false, false);
        assert_eq!(f("package.json"), "nodejs");
        assert_eq!(f("Dockerfile"), "docker");
        assert_eq!(f("main.test.ts"), "test-ts");
        assert_eq!(f("main.ts"), "typescript");
        assert_eq!(f("a.b.c.md"), "markdown");
        assert_eq!(f("data.xyz"), "file");
        assert_eq!(f("LICENSE"), "license");
        assert_eq!(f("tanpa-ikon"), "file");
        assert_eq!(f("constructor"), "file");
        assert_eq!(resolve_icon_name("src", true, false), "folder-src");
        assert_eq!(resolve_icon_name("src", true, true), "folder-src-open");
        assert_eq!(resolve_icon_name("src2", true, true), "folder-open");
        assert!(image("typescript").is_some());
    }
}
