//! Port 1:1 apps/desktop/src/shell/tool-view.ts + process-label.ts: logika murni tampilan langkah tool (berkas, baris,
//! perubahan +/−, perintah dan kode keluar, hasil pencarian, sub-agen) serta label proses jawaban agent. Tanpa UI.
use super::types::{MessagePart, Obj, PartKind, ToolState, ToolStatus};
use serde_json::Value;

fn s(o: &Obj, k: &str) -> String {
    o.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}
fn n(o: &Obj, k: &str) -> Option<f64> {
    o.get(k).and_then(Value::as_f64).filter(|x| x.is_finite())
}
fn r(o: &Obj, k: &str) -> Obj {
    o.get(k).and_then(Value::as_object).cloned().unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Edit,
    Write,
    Shell,
    Search,
    Web,
    Task,
    Skill,
    Plan,
    Ask,
    Memory,
    Generic,
}

pub fn tool_kind(tool: &str) -> ToolKind {
    let t = tool.to_lowercase();
    match t.as_str() {
        "read" => ToolKind::Read,
        "edit" | "multiedit" | "patch" | "apply_patch" => ToolKind::Edit,
        "write" => ToolKind::Write,
        "bash" => ToolKind::Shell,
        "grep" | "glob" | "list" => ToolKind::Search,
        "websearch" | "codesearch" | "webfetch" => ToolKind::Web,
        "task" => ToolKind::Task,
        "skill" => ToolKind::Skill,
        "todowrite" | "todoread" => ToolKind::Plan,
        "question" => ToolKind::Ask,
        _ if t.contains("memory") => ToolKind::Memory,
        _ => ToolKind::Generic,
    }
}

/// Ikon per jenis (KIND_ICON).
pub fn kind_icon(k: ToolKind) -> &'static str {
    match k {
        ToolKind::Read => "file_text",
        ToolKind::Edit => "diff",
        ToolKind::Write => "file_plus",
        ToolKind::Shell => "terminal",
        ToolKind::Search => "search",
        ToolKind::Web => "globe",
        ToolKind::Task => "bot",
        ToolKind::Skill => "sparkles",
        ToolKind::Plan => "list",
        ToolKind::Ask => "chat",
        ToolKind::Memory => "brain",
        ToolKind::Generic => "wrench",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitPath {
    pub dir: String,
    pub name: String,
    pub full: String,
}

pub fn split_path(path: &str) -> SplitPath {
    let norm = path.replace('\\', "/");
    let parts: Vec<&str> = norm.split('/').filter(|x| !x.is_empty()).collect();
    let name = parts.last().map(|x| x.to_string()).unwrap_or_else(|| norm.clone());
    let parents = if parts.is_empty() { &parts[..] } else { &parts[..parts.len() - 1] };
    let shown = parents[parents.len().saturating_sub(2)..].join("/");
    let dir = if shown.is_empty() { String::new() } else { format!("{}{shown}/", if parents.len() > 2 { "…/" } else { "" }) };
    SplitPath { dir, name, full: path.to_string() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Hunk,
    Add,
    Del,
    Ctx,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffRow {
    pub kind: RowKind,
    pub text: String,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
}

fn row(kind: RowKind, text: &str, old_no: Option<u32>, new_no: Option<u32>) -> DiffRow {
    DiffRow { kind, text: text.to_string(), old_no, new_no }
}

fn is_diff_header(l: &str) -> bool {
    l.starts_with("Index:") || (l.len() >= 5 && l.chars().all(|c| c == '=')) || l.starts_with("--- ") || l.starts_with("+++ ")
}

pub fn parse_diff(text: &str) -> Vec<DiffRow> {
    let re = regex::Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").expect("regex");
    let mut rows: Vec<DiffRow> = vec![];
    let (mut old_no, mut new_no, mut in_hunk) = (0u32, 0u32, false);
    for raw in text.replace("\r\n", "\n").split('\n') {
        if let Some(c) = re.captures(raw) {
            in_hunk = true;
            old_no = c[1].parse().unwrap_or(0);
            new_no = c[2].parse().unwrap_or(0);
            rows.push(row(RowKind::Hunk, raw, None, None));
            continue;
        }
        if !in_hunk && is_diff_header(raw) {
            continue;
        }
        if raw.is_empty() && rows.last().is_some_and(|x| x.text.is_empty()) {
            continue;
        }
        if raw.starts_with('\\') {
            continue;
        }
        if raw.starts_with('+') {
            rows.push(row(RowKind::Add, raw, None, in_hunk.then(|| { new_no += 1; new_no - 1 })));
        } else if raw.starts_with('-') {
            rows.push(row(RowKind::Del, raw, in_hunk.then(|| { old_no += 1; old_no - 1 }), None));
        } else if !raw.is_empty() {
            let (o, nn) = if in_hunk {
                old_no += 1;
                new_no += 1;
                (Some(old_no - 1), Some(new_no - 1))
            } else {
                (None, None)
            };
            rows.push(row(RowKind::Ctx, raw, o, nn));
        }
    }
    while rows.last().is_some_and(|x| x.kind == RowKind::Ctx && x.text.is_empty()) {
        rows.pop();
    }
    rows
}

pub fn diff_stats(rows: &[DiffRow]) -> (u32, u32) {
    (rows.iter().filter(|x| x.kind == RowKind::Add).count() as u32, rows.iter().filter(|x| x.kind == RowKind::Del).count() as u32)
}

pub fn content_rows(content: &str) -> Vec<DiffRow> {
    let c = content.replace("\r\n", "\n");
    let mut lines: Vec<&str> = c.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines.iter().enumerate().map(|(i, l)| row(RowKind::Add, &format!("+{l}"), None, Some(i as u32 + 1))).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumberedLine {
    pub no: Option<u32>,
    pub text: String,
}

pub fn as_rows(lines: &[NumberedLine]) -> Vec<DiffRow> {
    lines.iter().map(|l| row(RowKind::Ctx, &l.text, None, l.no)).collect()
}

pub fn read_lines(output: &str) -> (Vec<NumberedLine>, String) {
    let re = regex::Regex::new(r"(?s)<content>\n?(.*?)\n?</content>").expect("regex");
    let inner = re.captures(output).map(|c| c[1].to_string()).unwrap_or_else(|| output.to_string());
    let num = regex::Regex::new(r"^(\d+): ?(.*)$").expect("regex");
    let (mut lines, mut notes) = (vec![], vec![]);
    for l in inner.split('\n') {
        if let Some(c) = num.captures(l) {
            lines.push(NumberedLine { no: c[1].parse().ok(), text: c[2].to_string() });
        } else if l.starts_with('(') && l.ends_with(')') && l.len() >= 2 {
            notes.push(l[1..l.len() - 1].to_string());
        } else if !l.trim().is_empty() && !l.starts_with('<') {
            lines.push(NumberedLine { no: None, text: l.to_string() });
        }
    }
    (lines, notes.join(" "))
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrepGroup {
    pub file: String,
    pub lines: Vec<NumberedLine>,
}

pub fn grep_groups(output: &str) -> Vec<GrepGroup> {
    let re = regex::Regex::new(r"^\s+Line (\d+): ?(.*)$").expect("regex");
    let mut groups: Vec<GrepGroup> = vec![];
    for l in output.split('\n') {
        if let Some(c) = re.captures(l) {
            if let Some(g) = groups.last_mut() {
                g.lines.push(NumberedLine { no: c[1].parse().ok(), text: c[2].to_string() });
            }
        } else if l.ends_with(':') && !l.starts_with("Found ") && !l.trim().is_empty() {
            groups.push(GrepGroup { file: l[..l.len() - 1].to_string(), lines: vec![] });
        }
    }
    groups.retain(|g| !g.lines.is_empty());
    groups
}

pub fn task_result(output: &str) -> String {
    let re = regex::Regex::new(r"(?s)<task_result>\n?(.*?)\n?</task_result>").expect("regex");
    re.captures(output).map(|c| c[1].trim().to_string()).unwrap_or_else(|| output.trim().to_string())
}

#[derive(Clone, Debug, PartialEq)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolView {
    pub kind: ToolKind,
    pub target: String,
    pub path: Option<SplitPath>,
    pub range: Option<(f64, f64, Option<f64>)>,
    pub adds: Option<u32>,
    pub dels: Option<u32>,
    pub exit: Option<i64>,
    pub count: Option<u32>,
    pub agent: Option<String>,
    pub child_session: Option<String>,
    pub diff: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
    pub todos: Option<Vec<TodoItem>>,
}

fn line_count(s: &str) -> u32 {
    if s.is_empty() {
        return 0;
    }
    let c = s.replace("\r\n", "\n");
    c.strip_suffix('\n').unwrap_or(&c).split('\n').count() as u32
}

fn nz(v: String) -> Option<String> {
    (!v.is_empty()).then_some(v)
}

pub fn describe_tool(tool: &str, state: &ToolState) -> ToolView {
    let input = &state.input;
    let meta = state.metadata.clone().unwrap_or_default();
    let kind = tool_kind(tool);
    let title = state.title.clone().unwrap_or_default();
    let file_path = nz(s(input, "filePath")).or_else(|| nz(s(input, "path"))).or_else(|| nz(s(&meta, "filepath"))).unwrap_or_default();
    let mut v = ToolView {
        kind,
        target: String::new(),
        path: None,
        range: None,
        adds: None,
        dels: None,
        exit: None,
        count: None,
        agent: None,
        child_session: None,
        diff: None,
        command: None,
        description: None,
        todos: None,
    };
    let or_title = |x: String| if x.is_empty() { title.clone() } else { x };
    match kind {
        ToolKind::Read => {
            let display = r(&meta, "display");
            let from = n(&display, "lineStart").or_else(|| n(input, "offset")).unwrap_or(1.);
            let lim = n(input, "limit").filter(|x| *x != 0.);
            let to = n(&display, "lineEnd").or_else(|| lim.map(|l| from + l - 1.));
            let total = n(&display, "totalLines");
            v.target = file_path.clone();
            v.path = nz(file_path).map(|p| split_path(&p));
            if let Some(to) = to {
                if input.contains_key("offset") || input.contains_key("limit") || display.contains_key("lineStart") {
                    v.range = Some((from, to, total));
                }
            }
        }
        ToolKind::Edit | ToolKind::Write => {
            let fd = r(&meta, "filediff");
            let diff = nz(s(&meta, "diff")).or_else(|| nz(s(&fd, "patch")));
            let (mut adds, mut dels) = (n(&fd, "additions").map(|x| x as u32), n(&fd, "deletions").map(|x| x as u32));
            if let Some(d) = &diff {
                if adds.is_none() || dels.is_none() {
                    let (a, b) = diff_stats(&parse_diff(d));
                    adds = adds.or(Some(a));
                    dels = dels.or(Some(b));
                }
            }
            if kind == ToolKind::Write && adds.is_none() {
                adds = Some(line_count(&s(input, "content")));
            }
            v.target = file_path.clone();
            v.path = nz(file_path).map(|p| split_path(&p));
            v.diff = diff;
            v.adds = adds;
            v.dels = dels;
        }
        ToolKind::Shell => {
            let command = s(input, "command");
            v.target = or_title(command.clone());
            v.command = Some(command);
            v.description = nz(s(input, "description"));
            v.exit = n(&meta, "exit").map(|x| x as i64);
        }
        ToolKind::Search => {
            v.target = or_title(nz(s(input, "pattern")).unwrap_or(file_path));
            v.count = n(&meta, "matches").or_else(|| n(&meta, "count")).map(|x| x as u32);
        }
        ToolKind::Web => v.target = or_title(nz(s(input, "url")).unwrap_or_else(|| s(input, "query"))),
        ToolKind::Task => {
            v.target = or_title(s(input, "description"));
            v.agent = nz(s(input, "subagent_type"));
            v.child_session = nz(s(&meta, "sessionId"));
        }
        ToolKind::Skill => v.target = or_title(s(input, "name")),
        ToolKind::Plan => {
            let todos: Vec<TodoItem> = input
                .get("todos")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|x| {
                            let o = x.as_object().cloned().unwrap_or_default();
                            TodoItem { content: s(&o, "content"), status: s(&o, "status") }
                        })
                        .filter(|x| !x.content.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            if !todos.is_empty() {
                v.count = Some(todos.len() as u32);
                v.todos = Some(todos);
            }
        }
        _ => {
            let first = input.values().find_map(|x| x.as_str().map(str::to_string)).unwrap_or_default();
            v.target = if title.is_empty() { first } else { title.clone() };
        }
    }
    v
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProcessTally {
    pub read: u32,
    pub edit: u32,
    pub write: u32,
    pub shell: u32,
    pub search: u32,
    pub web: u32,
    pub task: u32,
    pub other: u32,
    pub adds: u32,
    pub dels: u32,
}

pub fn tally_process(parts: &[&MessagePart]) -> ProcessTally {
    let mut t = ProcessTally::default();
    let mut edited = std::collections::HashSet::new();
    for p in parts {
        let PartKind::Tool { tool, state, .. } = &p.kind else { continue };
        let v = describe_tool(tool, state);
        match v.kind {
            ToolKind::Read => t.read += 1,
            ToolKind::Edit | ToolKind::Write => {
                let key = v.path.as_ref().map(|x| x.full.clone()).unwrap_or_else(|| p.id.clone());
                if edited.insert(key) {
                    if v.kind == ToolKind::Write {
                        t.write += 1
                    } else {
                        t.edit += 1
                    }
                }
                t.adds += v.adds.unwrap_or(0);
                t.dels += v.dels.unwrap_or(0);
            }
            ToolKind::Shell => t.shell += 1,
            ToolKind::Search => t.search += 1,
            ToolKind::Web => t.web += 1,
            ToolKind::Task => t.task += 1,
            _ => t.other += 1,
        }
    }
    t
}

// ---------------- process-label.ts ----------------

pub fn tool_summary(tool: &str, input: &Obj, title: Option<&str>) -> String {
    let pick = |keys: &[&str]| keys.iter().map(|k| s(input, k)).find(|x| !x.is_empty()).unwrap_or_default();
    let title = title.unwrap_or("").to_string();
    let or = |x: String| if x.is_empty() { title.clone() } else { x };
    match tool {
        "websearch" | "codesearch" => or(pick(&["query"])),
        "webfetch" => or(pick(&["url"])),
        "bash" => or(pick(&["command", "description"])),
        "glob" | "grep" => or(pick(&["pattern"])),
        _ => {
            if !title.is_empty() {
                return title;
            }
            let fb = pick(&["filePath", "path", "pattern", "command", "url", "query"]);
            if !fb.is_empty() {
                return fb;
            }
            input.values().find_map(|x| x.as_str().map(str::to_string)).unwrap_or_default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub key: &'static str,
    pub detail: String,
    pub tool: Option<String>,
}

pub fn step_of(p: &MessagePart) -> Option<Step> {
    match &p.kind {
        PartKind::Reasoning { .. } => Some(Step { key: "step.thinking", detail: String::new(), tool: None }),
        PartKind::Tool { tool, state, .. } => {
            let detail = tool_summary(tool, &state.input, state.title.as_deref());
            let t = tool.to_lowercase();
            let key = if t == "skill" {
                "step.skill"
            } else if t.contains("memory") {
                if ["search", "read", "get", "recall", "list", "find"].iter().any(|w| t.contains(w)) { "step.memoryRead" } else { "step.memoryWrite" }
            } else {
                match t.as_str() {
                    "websearch" | "codesearch" => "step.web",
                    "webfetch" => "step.webfetch",
                    "read" => "step.read",
                    "write" => "step.write",
                    "edit" | "multiedit" | "patch" => "step.edit",
                    "bash" => "step.shell",
                    "grep" | "glob" | "list" => "step.search",
                    "todowrite" | "todoread" => "step.plan",
                    "question" => "step.ask",
                    "task" => "step.task",
                    _ => "step.tool",
                }
            };
            Some(Step { key, detail, tool: Some(tool.clone()) })
        }
        _ => None,
    }
}

pub fn is_process_part(p: &MessagePart) -> bool {
    matches!(p.kind, PartKind::Reasoning { .. } | PartKind::Tool { .. })
}

/// (proses, jawaban): semua sampai part proses TERAKHIR adalah proses.
pub fn split_turn<'a>(parts: &[&'a MessagePart]) -> (Vec<&'a MessagePart>, Vec<&'a MessagePart>) {
    let last = parts.iter().rposition(|p| is_process_part(p)).map(|i| i + 1).unwrap_or(0);
    (parts[..last].to_vec(), parts[last..].to_vec())
}

pub fn count_steps(process: &[&MessagePart]) -> usize {
    process.iter().filter(|p| is_process_part(p)).count()
}

pub fn elapsed_seconds(ms: f64) -> u64 {
    (ms / 1000.).floor().max(0.) as u64
}

const DOC_EXTS: [&str; 14] = ["pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "odt", "ods", "odp", "rtf", "md", "csv", "txt"];

pub fn is_document_path(path: &str) -> bool {
    let last = path.rsplit(['\\', '/']).next().unwrap_or("");
    let ext = last.rsplit('.').next().unwrap_or("").to_lowercase();
    path.contains('.') && !ext.is_empty() && DOC_EXTS.contains(&ext.as_str())
}

pub fn created_documents(parts: &[&MessagePart]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for p in parts {
        let PartKind::Tool { tool, state, .. } = &p.kind else { continue };
        if tool != "write" || state.status != ToolStatus::Completed {
            continue;
        }
        let path = nz(s(&state.input, "filePath")).unwrap_or_else(|| s(&state.input, "path"));
        if !path.is_empty() && is_document_path(&path) && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// Dokumen yang dihasilkan satu giliran di sesi Chat (path absolut): yang ditulis lewat tool `write` ditambah berkas dokumen
/// di folder kerja sesi yang berubah selama giliran itu (mis. dibuat lewat skrip bash/python, yang tidak terlihat sebagai
/// `write`). Folder sesi Chat khusus satu sesi, jadi pemindaian satu tingkat cukup; `until_ms` = batas atas waktu ubah.
pub fn turn_documents(process: &[&MessagePart], dir: &str, from_ms: f64, until_ms: Option<f64>) -> Vec<String> {
    let base = std::path::Path::new(dir);
    let mut out: Vec<String> = vec![];
    let key = |p: &str| p.replace('\\', "/").to_lowercase();
    let mut add = |p: String, out: &mut Vec<String>| {
        if !out.iter().any(|x| key(x) == key(&p)) {
            out.push(p);
        }
    };
    for rel in created_documents(process) {
        let p = std::path::Path::new(&rel);
        let abs = if p.is_absolute() || dir.is_empty() { rel.clone() } else { base.join(rel.replace('/', "\\")).to_string_lossy().into_owned() };
        add(abs, &mut out);
    }
    if dir.is_empty() {
        return out;
    }
    let ms = |t: std::time::SystemTime| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.);
    let mut found: Vec<(f64, String)> = std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            let name = e.file_name().to_string_lossy().into_owned();
            if !is_document_path(&name) {
                return None;
            }
            let at = ms(meta.modified().ok()?);
            (at >= from_ms - 2000. && until_ms.is_none_or(|u| at <= u)).then(|| (at, e.path().to_string_lossy().into_owned()))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, p) in found {
        add(p, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dokumen_giliran_dari_folder_sesi() {
        let dir = std::env::temp_dir().join(format!("zandor-td-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.pdf"), b"x").unwrap();
        std::fs::write(dir.join("b.rs"), b"x").unwrap();
        let d = dir.to_string_lossy().into_owned();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
        let found = turn_documents(&[], &d, now - 5000., None);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("a.pdf"));
        assert!(turn_documents(&[], &d, now + 60_000., None).is_empty());
        assert!(turn_documents(&[], &d, 0., Some(1.)).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn st(input: Value, meta: Option<Value>, title: Option<&str>) -> ToolState {
        ToolState {
            status: ToolStatus::Completed,
            input: input.as_object().cloned().unwrap_or_default(),
            output: None,
            error: None,
            title: title.map(str::to_string),
            metadata: meta.and_then(|m| m.as_object().cloned()),
            time: None,
        }
    }
    fn part(id: &str, tool: &str, state: ToolState) -> MessagePart {
        MessagePart { id: id.into(), message_id: "m1".into(), session_id: "s1".into(), kind: PartKind::Tool { tool: tool.into(), call_id: "c1".into(), state } }
    }
    const EDIT_DIFF: &str = "Index: C:\\w\\a.txt\n===================================================================\n--- C:\\w\\a.txt\n+++ C:\\w\\a.txt\n@@ -1,5 +1,5 @@\n satu\n-dua\n+DUA-BARU\n tiga\n empat\n lima\n";

    #[test]
    fn kinds_and_paths() {
        assert_eq!(tool_kind("bash"), ToolKind::Shell);
        assert_eq!(tool_kind("glob"), ToolKind::Search);
        assert_eq!(tool_kind("memory_search"), ToolKind::Memory);
        assert_eq!(tool_kind("todowrite"), ToolKind::Plan);
        assert_eq!(tool_kind("apa-saja"), ToolKind::Generic);
        let p = split_path("C:\\Users\\a\\Code\\Zandor\\apps\\desktop\\src\\shell\\ChatPanel.tsx");
        assert_eq!((p.name.as_str(), p.dir.as_str()), ("ChatPanel.tsx", "…/src/shell/"));
        assert_eq!(split_path("src/a.ts").dir, "src/");
        assert_eq!(split_path("README.md").dir, "");
    }

    #[test]
    fn diffs() {
        let rows = parse_diff(EDIT_DIFF);
        assert_eq!(rows[0], row(RowKind::Hunk, "@@ -1,5 +1,5 @@", None, None));
        let del = rows.iter().find(|x| x.kind == RowKind::Del).unwrap();
        assert_eq!((del.text.as_str(), del.old_no), ("-dua", Some(2)));
        let add = rows.iter().find(|x| x.kind == RowKind::Add).unwrap();
        assert_eq!((add.text.as_str(), add.new_no), ("+DUA-BARU", Some(2)));
        let ctx = rows.iter().find(|x| x.text == " tiga").unwrap();
        assert_eq!((ctx.old_no, ctx.new_no), (Some(3), Some(3)));
        assert_eq!(diff_stats(&rows), (1, 1));
        let r2 = parse_diff("-lama\n+baru\nsama");
        assert_eq!(r2.iter().map(|x| x.kind).collect::<Vec<_>>(), vec![RowKind::Del, RowKind::Add, RowKind::Ctx]);
        assert_eq!(r2[0].old_no, None);
        let r3 = parse_diff("@@ -1,1 +1,1 @@\n-a\n+b\n@@ -10,1 +10,2 @@\n x\n+y");
        assert_eq!(r3.iter().filter(|x| x.kind == RowKind::Hunk).count(), 2);
        assert_eq!(r3.iter().find(|x| x.text == "+y").unwrap().new_no, Some(11));
        assert_eq!(content_rows("# Judul\nisi\n"), vec![row(RowKind::Add, "+# Judul", None, Some(1)), row(RowKind::Add, "+isi", None, Some(2))]);
    }

    #[test]
    fn outputs() {
        let out = "<path>C:\\w\\a.txt</path>\n<type>file</type>\n<content>\n2: dua\n3: tiga\n4: empat\n\n(Showing lines 2-4 of 5. Use offset=5 to continue.)\n</content>";
        let (lines, note) = read_lines(out);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], NumberedLine { no: Some(2), text: "dua".into() });
        assert!(note.contains("Showing lines 2-4 of 5"));
        assert_eq!(read_lines("1: a\n2: b").0.len(), 2);
        let g = grep_groups("Found 2 matches\nC:\\w\\a.txt:\n  Line 3: tiga\nC:\\w\\b.ts:\n  Line 1: tiga lagi\n");
        assert_eq!(g.len(), 2);
        assert_eq!(g[1].file, "C:\\w\\b.ts");
        assert_eq!(g[1].lines[0], NumberedLine { no: Some(1), text: "tiga lagi".into() });
        assert_eq!(task_result("<task id=\"x\">\n<task_result>\nok selesai\n</task_result>\n</task>"), "ok selesai");
        assert_eq!(task_result("teks biasa"), "teks biasa");
    }

    #[test]
    fn describe() {
        let v = describe_tool("read", &st(json!({"filePath": "C:\\w\\src\\a.txt", "offset": 2, "limit": 3}), Some(json!({"display": {"lineStart": 2, "lineEnd": 4, "totalLines": 5}})), None));
        assert_eq!(v.path.unwrap().name, "a.txt");
        assert_eq!(v.range, Some((2., 4., Some(5.))));
        assert_eq!(describe_tool("read", &st(json!({"filePath": "a.ts"}), None, None)).range, None);
        assert_eq!(describe_tool("read", &st(json!({"filePath": "a.ts", "offset": 10, "limit": 5}), None, None)).range, Some((10., 14., None)));
        let v = describe_tool("edit", &st(json!({"filePath": "a.txt"}), Some(json!({"diff": EDIT_DIFF, "filediff": {"additions": 1, "deletions": 1}})), None));
        assert_eq!((v.adds, v.dels), (Some(1), Some(1)));
        assert!(v.diff.unwrap().contains("@@"));
        let v = describe_tool("edit", &st(json!({"filePath": "a.txt"}), Some(json!({"diff": EDIT_DIFF})), None));
        assert_eq!((v.adds, v.dels), (Some(1), Some(1)));
        assert_eq!(describe_tool("write", &st(json!({"filePath": "baru.md", "content": "# Judul\nisi\n"}), None, None)).adds, Some(2));
        let v = describe_tool("bash", &st(json!({"command": "echo halo", "description": "Cetak halo"}), Some(json!({"exit": 0})), None));
        assert_eq!((v.command.as_deref(), v.description.as_deref(), v.exit), (Some("echo halo"), Some("Cetak halo"), Some(0)));
        let v = describe_tool("grep", &st(json!({"pattern": "tiga"}), Some(json!({"matches": 4})), None));
        assert_eq!((v.target.as_str(), v.count), ("tiga", Some(4)));
        assert_eq!(describe_tool("glob", &st(json!({"pattern": "**/*.ts"}), Some(json!({"count": 7})), None)).count, Some(7));
        let v = describe_tool("task", &st(json!({"description": "Cari berkas", "prompt": "x", "subagent_type": "general"}), Some(json!({"sessionId": "ses_anak"})), None));
        assert_eq!((v.target.as_str(), v.agent.as_deref(), v.child_session.as_deref()), ("Cari berkas", Some("general"), Some("ses_anak")));
        let v = describe_tool("todowrite", &st(json!({"todos": [{"content": "A", "status": "completed"}, {"content": "B", "status": "pending"}]}), None, None));
        assert_eq!(v.count, Some(2));
        assert_eq!(v.todos.unwrap()[1], TodoItem { content: "B".into(), status: "pending".into() });
        assert_eq!(describe_tool("aneh", &st(json!({"x": "isi"}), None, None)).target, "isi");
        assert_eq!(describe_tool("aneh", &st(json!({}), None, Some("Judul"))).target, "Judul");
    }

    #[test]
    fn tally_and_labels() {
        let edit = |id: &str, a: u32| part(id, "edit", st(json!({"filePath": "a.ts"}), Some(json!({"filediff": {"patch": "", "additions": a, "deletions": 1}})), None));
        let parts = vec![
            part("1", "read", st(json!({"filePath": "a.ts"}), None, None)),
            part("2", "read", st(json!({"filePath": "b.ts"}), None, None)),
            edit("3", 3),
            edit("4", 2),
            part("5", "bash", st(json!({"command": "ls"}), None, None)),
            part("6", "grep", st(json!({"pattern": "x"}), None, None)),
            part("7", "webfetch", st(json!({"url": "http://x"}), None, None)),
            part("8", "task", st(json!({"description": "d"}), None, None)),
            part("9", "skill", st(json!({"name": "s"}), None, None)),
            MessagePart { id: "r1".into(), message_id: "m1".into(), session_id: "s1".into(), kind: PartKind::Reasoning { text: "hm".into() } },
        ];
        let refs: Vec<&MessagePart> = parts.iter().collect();
        let t = tally_process(&refs);
        assert_eq!((t.read, t.edit, t.shell, t.search, t.web, t.task, t.other, t.adds, t.dels), (2, 1, 1, 1, 1, 1, 1, 5, 2));
        assert_eq!(step_of(&parts[4]).unwrap().key, "step.shell");
        assert_eq!(step_of(&parts[9]).unwrap().key, "step.thinking");
        assert_eq!(step_of(&part("x", "memory_recall", st(json!({}), None, None))).unwrap().key, "step.memoryRead");
        let (p, a) = split_turn(&refs);
        assert_eq!((p.len(), a.len()), (10, 0));
        assert_eq!(count_steps(&refs), 10);
        assert!(is_document_path("C:\\a\\laporan.PDF"));
        assert!(!is_document_path("main.rs"));
        let w = part("w", "write", st(json!({"filePath": "x.md"}), None, None));
        assert_eq!(created_documents(&[&w, &w]), vec!["x.md".to_string()]);
        assert_eq!(tool_summary("bash", &json!({"description": "d"}).as_object().unwrap().clone(), None), "d");
    }
}
