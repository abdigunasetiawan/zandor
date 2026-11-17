//! Logika murni panel Trajectory — port trajectory.ts (ADR-073): buku besar per giliran dari pesan sesi (pengguna,
//! konteks, panggilan model, berpikir, jawaban, tool call, batas langkah, part lain), total, pencarian, dan proyeksi
//! ikhtisar tiga jalur. Hanya data yang benar-benar ada: rekaman tanpa waktu dibiarkan tanpa waktu.
use super::types::{ChatMessage, PartKind, Role, TokenUsage, ToolStatus};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    User,
    Context,
    Model,
    Thinking,
    Assistant,
    Tool,
    Step,
    Error,
    Other,
}
impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::User => "user",
            Kind::Context => "context",
            Kind::Model => "model",
            Kind::Thinking => "thinking",
            Kind::Assistant => "assistant",
            Kind::Tool => "tool",
            Kind::Step => "step",
            Kind::Error => "error",
            Kind::Other => "other",
        }
    }
    pub fn lane(self) -> Lane {
        match self {
            Kind::Tool => Lane::Tools,
            Kind::User | Kind::Context => Lane::Input,
            _ => Lane::Model,
        }
    }
    /// Kelompok yang bisa disembunyikan dari daftar ("other" ikut "step").
    pub fn hide_key(self) -> Option<&'static str> {
        match self {
            Kind::Thinking => Some("thinking"),
            Kind::Context => Some("context"),
            Kind::Tool => Some("tool"),
            Kind::Step | Kind::Other => Some("step"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Input,
    Model,
    Tools,
}
impl Lane {
    pub fn key(self) -> &'static str {
        match self {
            Lane::Input => "input",
            Lane::Model => "model",
            Lane::Tools => "tools",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub id: String,
    pub index: usize,
    pub turn: usize,
    pub kind: Kind,
    pub name: Option<String>,
    pub summary: String,
    pub preview: Option<String>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    /// "pending" | "running" | "completed" | "error"
    pub status: Option<&'static str>,
    pub message_id: String,
    pub tokens: Option<TokenUsage>,
    pub cost: Option<f64>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub raw: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turn {
    pub turn: usize,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub records: Vec<Record>,
    pub tokens: TokenUsage,
    pub cost: f64,
    pub calls: usize,
    pub model_calls: usize,
    pub errors: usize,
}

const PREVIEW_MAX: usize = 200;

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn clip(s: &str) -> String {
    if s.chars().count() > PREVIEW_MAX {
        format!("{}…", s.chars().take(PREVIEW_MAX).collect::<String>())
    } else {
        s.to_string()
    }
}
fn add(a: &TokenUsage, b: Option<&TokenUsage>) -> TokenUsage {
    match b {
        Some(b) => TokenUsage { input: a.input + b.input, output: a.output + b.output, reasoning: a.reasoning + b.reasoning, total: a.total + b.total },
        None => a.clone(),
    }
}
fn tok_json(t: &TokenUsage) -> Value {
    json!({ "input": t.input, "output": t.output, "reasoning": t.reasoning, "total": t.total })
}

pub fn duration_of(r: &Record) -> Option<f64> {
    match (r.start, r.end) {
        (Some(a), Some(b)) if b >= a => Some(b - a),
        _ => None,
    }
}

pub fn format_ms(ms: f64) -> String {
    if ms < 1000. {
        format!("{} ms", ms.round())
    } else if ms < 60_000. {
        if ms < 10_000. { format!("{:.2} s", ms / 1000.) } else { format!("{:.1} s", ms / 1000.) }
    } else {
        let m = (ms / 60_000.).floor();
        format!("{m}m {}s", ((ms % 60_000.) / 1000.).round())
    }
}

pub fn format_tokens(n: f64) -> String {
    if n >= 10_000. { format!("{:.1}k", n / 1000.) } else { format!("{}", n as i64) }
}

/// Port toolSummary (process-label.ts).
pub fn tool_summary(tool: &str, input: &serde_json::Map<String, Value>, title: Option<&str>) -> String {
    let pick = |keys: &[&str]| keys.iter().filter_map(|k| input.get(*k).and_then(Value::as_str)).find(|s| !s.is_empty()).map(String::from);
    let title = title.filter(|s| !s.is_empty()).map(String::from);
    let r = match tool {
        "websearch" | "codesearch" => pick(&["query"]).or(title),
        "webfetch" => pick(&["url"]).or(title),
        "bash" => pick(&["command", "description"]).or(title),
        "glob" | "grep" => pick(&["pattern"]).or(title),
        _ => title.or_else(|| pick(&["filePath", "path", "pattern", "command", "url", "query"])).or_else(|| input.values().find_map(|v| v.as_str().map(String::from))),
    };
    r.unwrap_or_default()
}

fn status_of(s: ToolStatus) -> &'static str {
    match s {
        ToolStatus::Pending => "pending",
        ToolStatus::Running => "running",
        ToolStatus::Completed => "completed",
        ToolStatus::Error => "error",
    }
}

struct Builder {
    turns: Vec<Turn>,
    index: usize,
}

impl Builder {
    fn open(&mut self, start: Option<f64>) {
        let n = self.turns.len() + 1;
        self.turns.push(Turn { turn: n, start, ..Default::default() });
    }
    fn cur(&mut self) -> &mut Turn {
        self.turns.last_mut().expect("turn")
    }
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, mut r: Record) {
        r.index = self.index;
        self.index += 1;
        let t = self.cur();
        r.turn = t.turn;
        for s in [r.start, r.end].into_iter().flatten() {
            if t.start.is_none_or(|x| s < x) {
                t.start = Some(s);
            }
            if t.end.is_none_or(|x| s > x) {
                t.end = Some(s);
            }
        }
        if r.status == Some("error") || r.kind == Kind::Error {
            t.errors += 1;
        }
        t.records.push(r);
    }
}

fn rec(id: String, kind: Kind, summary: String, message_id: &str, raw: Value) -> Record {
    Record { id, index: 0, turn: 0, kind, name: None, summary, preview: None, start: None, end: None, status: None, message_id: message_id.into(), tokens: None, cost: None, input: None, output: None, raw }
}

fn short_url(u: &str) -> String {
    if u.starts_with("data:") { format!("{}…", u.chars().take(60).collect::<String>()) } else { u.to_string() }
}

/// Port buildTrajectory: giliran baru di setiap pesan pengguna.
pub fn build(messages: &[ChatMessage]) -> Vec<Turn> {
    let mut b = Builder { turns: vec![], index: 0 };
    for m in messages {
        let info = &m.info;
        if info.role == Role::User {
            b.open(Some(info.created));
            let text: Vec<&str> = m.parts.iter().filter_map(|p| if let PartKind::Text { text, synthetic: false } = &p.kind { Some(text.as_str()) } else { None }).collect();
            let text = text.join("\n");
            let files = m.parts.iter().filter(|p| matches!(p.kind, PartKind::File { .. })).count();
            let mut summary = clip(&one_line(&text));
            if summary.is_empty() && files > 0 {
                summary = format!("[{files}]");
            }
            let mut r = rec(info.id.clone(), Kind::User, summary, &info.id, json!({ "id": info.id, "role": "user", "time": { "created": info.created }, "text": text }));
            r.start = Some(info.created);
            r.input = Some(text);
            b.push(r);
            for p in &m.parts {
                match &p.kind {
                    PartKind::Text { text, synthetic: true } => {
                        let mut r = rec(p.id.clone(), Kind::Context, clip(&one_line(text)), &info.id, json!({ "id": p.id, "type": "text", "synthetic": true, "text": text }));
                        r.start = Some(info.created);
                        r.input = Some(text.clone());
                        b.push(r);
                    }
                    PartKind::File { mime, filename, url } => {
                        let label = filename.clone().unwrap_or_else(|| url.chars().take(80).collect());
                        let mut r = rec(p.id.clone(), Kind::Context, format!("{label} · {mime}"), &info.id, json!({ "id": p.id, "type": "file", "mime": mime, "filename": filename, "url": short_url(url) }));
                        r.name = Some("file".into());
                        r.start = Some(info.created);
                        b.push(r);
                    }
                    PartKind::Other { raw_type } => {
                        let mut r = rec(p.id.clone(), Kind::Other, raw_type.clone(), &p.message_id, json!({ "id": p.id, "type": raw_type }));
                        r.name = Some(raw_type.clone());
                        r.start = Some(info.created);
                        b.push(r);
                    }
                    _ => {}
                }
            }
            continue;
        }
        if b.turns.is_empty() {
            b.open(Some(info.created));
        }
        {
            let t = b.cur();
            t.model_calls += 1;
            t.tokens = add(&t.tokens, info.tokens.as_ref());
            t.cost += info.cost.unwrap_or(0.);
        }
        let status = if info.error.is_some() { "error" } else if info.completed.is_some() { "completed" } else { "running" };
        let name = [info.provider_id.clone(), info.model_id.clone()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/");
        let summary = [info.agent.clone(), info.finish.clone()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        let raw = json!({
            "id": info.id, "role": "assistant", "agent": info.agent, "providerID": info.provider_id, "modelID": info.model_id,
            "finish": info.finish, "cost": info.cost, "tokens": info.tokens.as_ref().map(tok_json),
            "time": { "created": info.created, "completed": info.completed },
            "error": info.error.as_ref().map(|e| json!({ "name": e.name, "message": e.message })),
        });
        let mut r = rec(format!("{}:model", info.id), Kind::Model, summary, &info.id, raw);
        r.name = (!name.is_empty()).then_some(name);
        r.start = Some(info.created);
        r.end = info.completed;
        r.status = Some(status);
        r.tokens = info.tokens.clone();
        r.cost = info.cost;
        b.push(r);

        let mut step = 1;
        for p in &m.parts {
            match &p.kind {
                PartKind::Reasoning { text } if !text.trim().is_empty() => {
                    let mut r = rec(p.id.clone(), Kind::Thinking, clip(&one_line(text)), &info.id, json!({ "id": p.id, "type": "reasoning", "text": text }));
                    r.output = Some(text.clone());
                    b.push(r);
                }
                PartKind::Text { text, synthetic } if !text.trim().is_empty() => {
                    let mut r = rec(p.id.clone(), if *synthetic { Kind::Context } else { Kind::Assistant }, clip(&one_line(text)), &info.id, json!({ "id": p.id, "type": "text", "synthetic": synthetic, "text": text }));
                    r.output = Some(text.clone());
                    b.push(r);
                }
                PartKind::Tool { tool, call_id, state } => {
                    b.cur().calls += 1;
                    let out = state.error.clone().or_else(|| state.output.clone()).unwrap_or_default();
                    let input = serde_json::to_string_pretty(&Value::Object(state.input.clone())).unwrap_or_default();
                    let raw = json!({
                        "id": p.id, "type": "tool", "tool": tool, "callID": call_id,
                        "state": { "status": status_of(state.status), "input": state.input, "output": state.output, "error": state.error, "title": state.title, "metadata": state.metadata,
                                   "time": state.time.map(|(a, b)| json!({ "start": a, "end": b })) },
                    });
                    let mut r = rec(p.id.clone(), Kind::Tool, clip(&one_line(&tool_summary(tool, &state.input, state.title.as_deref()))), &info.id, raw);
                    r.name = Some(tool.clone());
                    r.preview = (!out.is_empty()).then(|| clip(&one_line(&out)));
                    r.start = state.time.map(|t| t.0);
                    r.end = state.time.and_then(|t| t.1);
                    r.status = Some(status_of(state.status));
                    r.input = Some(input);
                    r.output = (!out.is_empty()).then_some(out);
                    b.push(r);
                }
                PartKind::StepFinish { reason, cost, tokens } => {
                    let mut r = rec(p.id.clone(), Kind::Step, reason.clone(), &info.id, json!({ "id": p.id, "type": "step-finish", "reason": reason, "cost": cost, "tokens": tok_json(tokens) }));
                    r.name = Some(format!("#{step}"));
                    step += 1;
                    r.tokens = Some(tokens.clone());
                    r.cost = Some(*cost);
                    b.push(r);
                }
                PartKind::File { mime, filename, url } => {
                    let mut r = rec(p.id.clone(), Kind::Assistant, format!("{} · {mime}", filename.clone().unwrap_or_default()), &info.id, json!({ "id": p.id, "type": "file", "mime": mime, "filename": filename, "url": short_url(url) }));
                    r.name = Some("file".into());
                    b.push(r);
                }
                PartKind::Other { raw_type } => {
                    let mut r = rec(p.id.clone(), Kind::Other, raw_type.clone(), &p.message_id, json!({ "id": p.id, "type": raw_type }));
                    r.name = Some(raw_type.clone());
                    b.push(r);
                }
                _ => {}
            }
        }
        if let Some(e) = &info.error {
            let mut r = rec(format!("{}:error", info.id), Kind::Error, clip(&one_line(&e.message)), &info.id, json!({ "name": e.name, "message": e.message }));
            r.name = Some(e.name.clone());
            r.start = info.completed;
            r.status = Some("error");
            r.output = Some(e.message.clone());
            b.push(r);
        }
    }
    b.turns
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub tokens: TokenUsage,
    pub cost: f64,
    pub calls: usize,
    pub model_calls: usize,
    pub errors: usize,
    pub records: usize,
}

pub fn totals(turns: &[Turn]) -> Totals {
    let mut s = Totals::default();
    for t in turns {
        s.tokens = add(&s.tokens, Some(&t.tokens));
        s.cost += t.cost;
        s.calls += t.calls;
        s.model_calls += t.model_calls;
        s.errors += t.errors;
        s.records += t.records.len();
    }
    s
}

pub fn matches_query(r: &Record, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let n = q.to_lowercase();
    [Some(&r.summary), r.name.as_ref(), r.input.as_ref(), r.output.as_ref(), r.preview.as_ref()].into_iter().flatten().any(|s| s.to_lowercase().contains(&n))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub index: usize,
    pub lane: Lane,
    pub kind: Kind,
    pub left: f32,
    pub width: f32,
    pub error: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub spans: Vec<Span>,
    pub boundaries: Vec<(usize, f32)>,
}

/// Port buildTimeline (persen 0–100). `time`: posisi dari waktu tercatat; selain itu satu slot per rekaman.
pub fn timeline(turns: &[Turn], time: bool, now: f64) -> Option<Timeline> {
    let recs: Vec<&Record> = turns.iter().flat_map(|t| &t.records).collect();
    if recs.is_empty() {
        return None;
    }
    let err = |r: &Record| r.status == Some("error") || r.kind == Kind::Error;
    if !time {
        let n = recs.len() as f32;
        return Some(Timeline {
            boundaries: turns.iter().filter_map(|t| t.records.first().map(|r| (t.turn, r.index as f32 / n * 100.))).collect(),
            spans: recs.iter().map(|r| Span { index: r.index, lane: r.kind.lane(), kind: r.kind, left: r.index as f32 / n * 100., width: 100. / n, error: err(r) }).collect(),
        });
    }
    let timed: Vec<&&Record> = recs.iter().filter(|r| r.start.is_some()).collect();
    if timed.is_empty() {
        return None;
    }
    let start = timed.iter().filter_map(|r| r.start).fold(f64::INFINITY, f64::min);
    let ends: Vec<f64> = timed.iter().map(|r| r.end.unwrap_or(if r.status == Some("running") { now } else { r.start.unwrap_or(start) })).collect();
    let end = ends.iter().copied().fold(start + 1., f64::max);
    let span = end - start;
    let pct = |x: f64| ((x - start) / span * 100.) as f32;
    Some(Timeline {
        boundaries: turns.iter().filter_map(|t| t.start.map(|s| (t.turn, pct(s)))).collect(),
        spans: timed
            .iter()
            .zip(&ends)
            .map(|(r, e)| {
                let l = pct(r.start.unwrap_or(start));
                Span { index: r.index, lane: r.kind.lane(), kind: r.kind, left: l, width: (pct(*e) - l).max(0.), error: err(r) }
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{MessageError, MessageInfo, MessagePart, ToolState};

    fn info(id: &str, role: Role, created: f64, completed: Option<f64>) -> MessageInfo {
        MessageInfo { id: id.into(), session_id: "s".into(), role, created, completed, parent_id: None, agent: Some("build".into()), provider_id: Some("p".into()), model_id: Some("m".into()), finish: None, error: None, tokens: Some(TokenUsage { input: 10., output: 5., reasoning: 0., total: 15. }), cost: Some(0.01) }
    }
    fn part(id: &str, mid: &str, kind: PartKind) -> MessagePart {
        MessagePart { id: id.into(), message_id: mid.into(), session_id: "s".into(), kind }
    }

    #[test]
    fn builds_turns() {
        let mut input = serde_json::Map::new();
        input.insert("command".into(), json!("ls -la"));
        let mut a = info("a1", Role::Assistant, 1100., Some(1500.));
        a.error = Some(MessageError { name: "E".into(), message: "boom".into() });
        let msgs = vec![
            ChatMessage { info: info("u1", Role::User, 1000., None), parts: vec![part("p1", "u1", PartKind::Text { text: "halo  dunia".into(), synthetic: false })] },
            ChatMessage {
                info: a,
                parts: vec![
                    part("p2", "a1", PartKind::Reasoning { text: "pikir".into() }),
                    part("p3", "a1", PartKind::Tool { tool: "bash".into(), call_id: "c".into(), state: ToolState { status: ToolStatus::Completed, input, output: Some("ok".into()), error: None, title: None, metadata: None, time: Some((1200., Some(1300.))) } }),
                    part("p4", "a1", PartKind::Text { text: "selesai".into(), synthetic: false }),
                ],
            },
        ];
        let turns = build(&msgs);
        assert_eq!(turns.len(), 1);
        let t = &turns[0];
        let kinds: Vec<_> = t.records.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![Kind::User, Kind::Model, Kind::Thinking, Kind::Tool, Kind::Assistant, Kind::Error]);
        assert_eq!(t.records[0].summary, "halo dunia");
        assert_eq!(t.records[3].summary, "ls -la");
        assert_eq!(t.records[3].preview.as_deref(), Some("ok"));
        assert_eq!(duration_of(&t.records[3]), Some(100.));
        assert_eq!((t.calls, t.model_calls, t.errors), (1, 1, 2));
        let s = totals(&turns);
        assert_eq!(s.records, 6);
        assert!(matches_query(&t.records[3], "LS"));
        let tl = timeline(&turns, true, 2000.).unwrap();
        assert!(tl.spans.iter().all(|s| (0. ..=100.).contains(&s.left)));
        assert_eq!(timeline(&turns, false, 0.).unwrap().spans.len(), 6);
    }

    #[test]
    fn formats() {
        assert_eq!(format_ms(250.), "250 ms");
        assert_eq!(format_ms(2500.), "2.50 s");
        assert_eq!(format_ms(125_000.), "2m 5s");
        assert_eq!(format_tokens(12_345.), "12.3k");
        assert_eq!(format_tokens(42.), "42");
    }
}
