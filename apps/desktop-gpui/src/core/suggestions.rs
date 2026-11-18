//! Saran prompt dinamis — port packages/core/src/suggestions.ts: murni, dihitung dari konteks nyata (project, perubahan
//! git, berkas yang sedang dibuka, berkas di akar project). Tidak ada model yang dipanggil dan tidak ada saran "pengisi".

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SuggestionContext {
    pub has_project: bool,
    /// Nama berkas/folder di akar project, bila sudah dibaca.
    pub root_files: Option<Vec<String>>,
    /// Path berkas yang berubah menurut git (kosong bila bukan repo / bersih).
    pub changed: Vec<String>,
    pub staged: usize,
    /// Berkas yang sedang dibuka di Panel 3: (path, jenis "code" | "markdown" | lainnya).
    pub active_doc: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub id: &'static str,
    pub params: Vec<(&'static str, String)>,
}

fn is_code(path: &str) -> bool {
    let re = regex::Regex::new(r"(?i)\.(?:[cm]?[jt]sx?|py|rs|go|java|kt|cs|cpp|c|h|rb|php|swift|dart|vue|svelte)$").expect("regex");
    re.is_match(path)
}

/// Maks `max` saran, berurutan menurut relevansi (yang paling spesifik dulu).
pub fn suggest_prompts(ctx: &SuggestionContext, max: usize) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = vec![];
    let mut add = |id: &'static str, params: Vec<(&'static str, String)>| {
        if !out.iter().any(|s| s.id == id) {
            out.push(Suggestion { id, params });
        }
    };
    if !ctx.has_project {
        add("plan-week", vec![]);
        add("summarize-text", vec![]);
        add("explain-concept", vec![]);
        out.truncate(max);
        return out;
    }
    if !ctx.changed.is_empty() {
        add("review-changes", vec![("n", ctx.changed.len().to_string())]);
        if ctx.staged > 0 || ctx.changed.len() <= 20 {
            add("commit-message", vec![]);
        }
    }
    if let Some((path, kind)) = &ctx.active_doc {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        if kind == "code" || kind == "markdown" {
            add("explain-file", vec![("name", name.clone()), ("path", path.clone())]);
        }
        let test_re = regex::Regex::new(r"(?i)\.(?:test|spec)\.").expect("regex");
        if kind == "code" && is_code(path) && !test_re.is_match(path) {
            add("write-tests", vec![("name", name), ("path", path.clone())]);
        }
    }
    if let Some(root) = &ctx.root_files {
        let readme = regex::Regex::new(r"(?i)^readme(?:\.[a-z]+)?$").expect("regex");
        if !root.iter().any(|f| readme.is_match(f)) {
            add("write-readme", vec![]);
        }
        let manifest = regex::Regex::new(r"(?i)^(?:package\.json|cargo\.toml|pyproject\.toml|go\.mod|pom\.xml)$").expect("regex");
        if root.iter().any(|f| manifest.is_match(f)) {
            add("run-tests", vec![]);
        }
    }
    add("explain-project", vec![]);
    add("find-todos", vec![]);
    out.truncate(max);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(v: &[Suggestion]) -> Vec<&str> {
        v.iter().map(|s| s.id).collect()
    }
    #[test]
    fn tanpa_project() {
        assert_eq!(ids(&suggest_prompts(&SuggestionContext::default(), 4)), vec!["plan-week", "summarize-text", "explain-concept"]);
    }
    #[test]
    fn dengan_project() {
        let ctx = SuggestionContext {
            has_project: true,
            root_files: Some(vec!["Cargo.toml".into(), "src".into()]),
            changed: vec!["a.rs".into(), "b.rs".into()],
            staged: 0,
            active_doc: Some(("src/main.rs".into(), "code".into())),
        };
        let v = suggest_prompts(&ctx, 4);
        assert_eq!(ids(&v), vec!["review-changes", "commit-message", "explain-file", "write-tests"]);
        assert_eq!(v[0].params, vec![("n", "2".to_string())]);
        let ctx2 = SuggestionContext { has_project: true, root_files: Some(vec!["README.md".into(), "package.json".into()]), ..Default::default() };
        assert_eq!(ids(&suggest_prompts(&ctx2, 4)), vec!["run-tests", "explain-project", "find-todos"]);
    }
}
