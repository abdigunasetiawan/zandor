//! Konversi ACP → bentuk pesan Zandor (JSON berbentuk pesan engine opencode, dibaca lagi oleh `types::to_message`/
//! `to_part`). Dengan begitu transkrip agent ACP tampil memakai komponen chat yang sama (langkah tool, diff, rencana).
//! Semua fungsi murni; input dari agent dianggap tidak tepercaya.
use crate::core::types::{ChatMessage, PartKind, PermissionRequest, Role, ToolRef};
use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// ID menaik seperti opencode (`msg_`/`prt_` + 12 heksa waktu·4096+urut + 14 acak), supaya pesan ACP terurut benar
/// bila digabung dengan pesan engine (ChatState mengurutkan menurut id).
pub fn ascending(prefix: &str) -> String {
    static LAST: Mutex<(u64, u64)> = Mutex::new((0, 0));
    static RND: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
    let now = crate::core::projects::now_ms() as u64;
    let n = {
        let mut g = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if now > g.0 {
            *g = (now, 0);
        } else {
            g.1 += 1;
        }
        g.0 * 0x1000 + (g.1 & 0xfff)
    };
    const B62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut x = RND.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed) ^ n.rotate_left(17);
    let mut tail = String::with_capacity(14);
    for _ in 0..14 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        tail.push(B62[(x % 62) as usize] as char);
    }
    format!("{prefix}_{:012x}{tail}", n & 0xffff_ffff_ffff)
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(String::from).filter(|x| !x.is_empty())
}

/// Info pesan berbentuk engine.
pub fn message_info(id: &str, sid: &str, role: Role, created: f64, parent: Option<&str>, provider: Option<&str>, model: Option<&str>) -> Value {
    let mut v = json!({ "id": id, "sessionID": sid, "role": if role == Role::User { "user" } else { "assistant" }, "time": { "created": created } });
    if let Some(p) = parent {
        v["parentID"] = json!(p);
    }
    if let Some(p) = provider {
        v["providerID"] = json!(p);
    }
    if let Some(m) = model {
        v["modelID"] = json!(m);
    }
    v
}

pub fn text_part(id: &str, mid: &str, sid: &str, kind: &str, text: &str, synthetic: bool) -> Value {
    let mut p = json!({ "id": id, "messageID": mid, "sessionID": sid, "type": kind, "text": text });
    if synthetic {
        p["synthetic"] = json!(true);
    }
    p
}

/// Token dari `usage` balasan `session/prompt` (ACP: inputTokens, outputTokens, cachedReadTokens, cachedWriteTokens,
/// thoughtTokens, totalTokens). Input = semua token masuk termasuk cache = ukuran konteks permintaan terakhir.
pub fn tokens_from_usage(u: &Value) -> Option<Value> {
    let n = |k: &str| u.get(k).and_then(Value::as_f64).unwrap_or(0.);
    let input = n("inputTokens") + n("cachedReadTokens") + n("cachedWriteTokens");
    let (output, reasoning) = (n("outputTokens"), n("thoughtTokens"));
    if input == 0. && output == 0. {
        return None;
    }
    let total = u.get("totalTokens").and_then(Value::as_f64).unwrap_or(input + output + reasoning);
    Some(json!({ "input": input, "output": output, "reasoning": reasoning, "total": total }))
}

/// Nama tool Zandor untuk `kind` ACP (read, edit, delete, move, search, execute, think, fetch, switch_mode, other).
fn tool_name(kind: &str, raw: &Value) -> &'static str {
    match kind {
        "read" => "read",
        "edit" | "delete" | "move" => "edit",
        "execute" => "bash",
        "search" => {
            if raw.get("pattern").is_some() && raw.get("path").is_none() && raw.get("glob").is_none() && s(raw, "pattern").is_some_and(|p| p.contains('*')) {
                "glob"
            } else {
                "grep"
            }
        }
        "fetch" => {
            if raw.get("url").is_none() && raw.get("query").is_some() {
                "websearch"
            } else {
                "webfetch"
            }
        }
        "think" => "task",
        _ => "tool",
    }
}

fn status_of(st: &str) -> Option<&'static str> {
    Some(match st {
        "pending" => "pending",
        "in_progress" => "running",
        "completed" => "completed",
        "failed" => "error",
        _ => return None,
    })
}

/// Teks dari blok konten ACP (`{type:"content", content:{type:"text", text}}` atau blok teks langsung).
fn content_text(c: &Value) -> Option<String> {
    let inner = if c.get("type").and_then(Value::as_str) == Some("content") { c.get("content")? } else { c };
    match inner.get("type").and_then(Value::as_str)? {
        "text" => s(inner, "text"),
        "resource" => inner.get("resource").and_then(|r| s(r, "text")),
        "resource_link" => s(inner, "uri"),
        _ => None,
    }
}

/// Diff unified sederhana (LCS baris, konteks 3). Berkas sangat besar → satu hunk ganti-semua.
pub fn unified_diff(path: &str, old: &str, new: &str) -> (String, u32, u32) {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    // ops: (' ' | '-' | '+', baris)
    let mut ops: Vec<(char, &str)> = vec![];
    if a.len().saturating_mul(b.len()) > 4_000_000 {
        ops.extend(a.iter().map(|l| ('-', *l)));
        ops.extend(b.iter().map(|l| ('+', *l)));
    } else {
        let (n, m) = (a.len(), b.len());
        let mut dp = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[i * (m + 1) + j] = if a[i] == b[j] { dp[(i + 1) * (m + 1) + j + 1] + 1 } else { dp[(i + 1) * (m + 1) + j].max(dp[i * (m + 1) + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && a[i] == b[j] {
                ops.push((' ', a[i]));
                i += 1;
                j += 1;
            } else if j < m && (i == n || dp[i * (m + 1) + j + 1] > dp[(i + 1) * (m + 1) + j]) {
                ops.push(('+', b[j]));
                j += 1;
            } else {
                ops.push(('-', a[i]));
                i += 1;
            }
        }
    }
    let adds = ops.iter().filter(|o| o.0 == '+').count() as u32;
    let dels = ops.iter().filter(|o| o.0 == '-').count() as u32;
    let mut out = format!("Index: {path}\n===================================================================\n--- {path}\n+++ {path}\n");
    const CTX: usize = 3;
    let changed: Vec<usize> = ops.iter().enumerate().filter(|(_, o)| o.0 != ' ').map(|(k, _)| k).collect();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(CTX);
        let mut end = changed[k];
        while k < changed.len() && changed[k] <= end + 2 * CTX {
            end = changed[k];
            k += 1;
        }
        let end = (end + CTX + 1).min(ops.len());
        let old_before = ops[..start].iter().filter(|o| o.0 != '+').count();
        let new_before = ops[..start].iter().filter(|o| o.0 != '-').count();
        let seg = &ops[start..end];
        let ol = seg.iter().filter(|o| o.0 != '+').count();
        let nl = seg.iter().filter(|o| o.0 != '-').count();
        let os = if ol == 0 { old_before } else { old_before + 1 };
        let ns = if nl == 0 { new_before } else { new_before + 1 };
        out.push_str(&format!("@@ -{os},{ol} +{ns},{nl} @@\n"));
        for (c, l) in seg {
            out.push(*c);
            out.push_str(l);
            out.push('\n');
        }
    }
    (out, adds, dels)
}

/// Gabungkan `tool_call` / `tool_call_update` ACP ke part tool berbentuk engine. `part` = part lama (atau baru kosong).
pub fn merge_tool(part: &mut Value, upd: &Value, now: f64) {
    let mut state = part.get("state").cloned().unwrap_or_else(|| json!({ "status": "pending", "input": {}, "time": { "start": now } }));
    let mut meta: Map<String, Value> = state.get("metadata").and_then(Value::as_object).cloned().unwrap_or_default();
    if let Some(k) = s(upd, "kind") {
        meta.insert("acpKind".into(), json!(k));
    }
    let kind = meta.get("acpKind").and_then(Value::as_str).unwrap_or("other").to_string();
    if let Some(raw) = upd.get("rawInput").filter(|r| !r.is_null()) {
        meta.insert("rawInput".into(), raw.clone());
    }
    let raw = meta.get("rawInput").cloned().unwrap_or(Value::Null);
    if let Some(t) = s(upd, "title") {
        state["title"] = json!(t);
    }
    let title = state.get("title").and_then(Value::as_str).unwrap_or("").to_string();

    // ---- input ----
    let mut input: Map<String, Value> = raw.as_object().cloned().unwrap_or_default();
    let loc = upd.get("locations").and_then(Value::as_array).and_then(|l| l.first()).and_then(|l| s(l, "path"));
    if let Some(l) = &loc {
        meta.insert("location".into(), json!(l));
    }
    let path = meta.get("location").and_then(Value::as_str).map(String::from).or_else(|| s(&raw, "file_path")).or_else(|| s(&raw, "filePath")).or_else(|| s(&raw, "path")).or_else(|| s(&raw, "abs_path"));
    if let Some(p) = &path {
        input.insert("filePath".into(), json!(p));
    }
    let mut tool = tool_name(&kind, &raw);
    match tool {
        "bash" => {
            let cmd = match raw.get("command") {
                Some(Value::String(c)) => c.clone(),
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "),
                _ => title.clone(),
            };
            input.insert("command".into(), json!(cmd));
            if !title.is_empty() && title != cmd {
                input.insert("description".into(), json!(title));
            }
        }
        "grep" | "glob" => {
            let pat = s(&raw, "pattern").or_else(|| s(&raw, "query")).unwrap_or_else(|| title.clone());
            input.insert("pattern".into(), json!(pat));
        }
        "webfetch" => {
            input.insert("url".into(), json!(s(&raw, "url").unwrap_or_else(|| title.clone())));
        }
        "websearch" => {
            input.insert("query".into(), json!(s(&raw, "query").unwrap_or_else(|| title.clone())));
        }
        "task" => {
            input.insert("description".into(), json!(s(&raw, "description").unwrap_or_else(|| title.clone())));
        }
        _ => {}
    }

    // ---- konten: teks keluaran, diff ----
    if let Some(content) = upd.get("content").and_then(Value::as_array) {
        let mut texts = vec![];
        let mut files = vec![];
        for c in content {
            match c.get("type").and_then(Value::as_str) {
                Some("diff") => {
                    let p = s(c, "path").unwrap_or_default();
                    let old = c.get("oldText").and_then(Value::as_str);
                    let new = c.get("newText").and_then(Value::as_str).unwrap_or("");
                    let (diff, adds, dels) = unified_diff(&p, old.unwrap_or(""), new);
                    files.push(json!({ "filePath": p, "type": if old.is_none() { "add" } else { "update" }, "diff": diff, "additions": adds, "deletions": dels, "newText": new }));
                }
                Some("terminal") => {}
                _ => texts.extend(content_text(c)),
            }
        }
        if !texts.is_empty() {
            state["output"] = json!(texts.join("\n"));
        }
        if files.len() == 1 {
            let f = &files[0];
            input.insert("filePath".into(), f["filePath"].clone());
            if f["type"] == "add" {
                tool = "write";
                input.insert("content".into(), f["newText"].clone());
                meta.insert("exists".into(), json!(false));
            } else {
                meta.insert("diff".into(), f["diff"].clone());
                meta.insert("filediff".into(), json!({ "patch": f["diff"], "additions": f["additions"], "deletions": f["deletions"] }));
            }
        } else if files.len() > 1 {
            tool = "apply_patch";
            meta.insert("files".into(), json!(files.iter().map(|f| json!({ "filePath": f["filePath"], "type": f["type"], "diff": f["diff"], "additions": f["additions"], "deletions": f["deletions"] })).collect::<Vec<_>>()));
        }
    }
    // Keluaran terminal yang dialirkan agent (codex-acp: _meta.terminal_output_delta) bila client tidak memegang terminal.
    if let Some(d) = upd.pointer("/_meta/terminal_output_delta/data").and_then(Value::as_str) {
        let prev = state.get("output").and_then(Value::as_str).unwrap_or("").to_string();
        state["output"] = json!(format!("{prev}{d}"));
    }
    let exit = upd.pointer("/_meta/terminal_exit/exit_code").or_else(|| upd.pointer("/rawOutput/exit_code")).and_then(Value::as_i64);
    if let Some(e) = exit {
        meta.insert("exit".into(), json!(e));
    }
    match upd.get("rawOutput") {
        Some(Value::String(o)) if state.get("output").is_none() => state["output"] = json!(o),
        Some(o @ Value::Object(_)) if state.get("output").is_none() => {
            if let Some(t) = s(o, "output").or_else(|| s(o, "stdout")).or_else(|| s(o, "content")) {
                state["output"] = json!(t);
            }
        }
        _ => {}
    }

    // ---- status ----
    if let Some(st) = s(upd, "status").and_then(|x| status_of(&x)) {
        state["status"] = json!(st);
        if matches!(st, "completed" | "error") {
            state["time"]["end"] = json!(now);
        }
        if st == "error" {
            let why = state.get("output").and_then(Value::as_str).map(String::from).filter(|o| !o.trim().is_empty()).unwrap_or_else(|| if title.is_empty() { "gagal".into() } else { title.clone() });
            state["error"] = json!(why);
        }
    }
    // Part yang sudah bernama "write"/"apply_patch" tetap begitu walau pembaruan berikutnya tanpa konten.
    let prev_tool = part.get("tool").and_then(Value::as_str).unwrap_or("");
    if matches!(prev_tool, "write" | "apply_patch") && tool == "edit" {
        tool = if prev_tool == "write" { "write" } else { "apply_patch" };
    }
    if tool == "tool" {
        // Tool lain: pakai nama dari agent (mis. codex `name`), selain itu "tool".
        if let Some(n) = s(upd, "name").or_else(|| part.get("tool").and_then(Value::as_str).filter(|t| *t != "tool").map(String::from)) {
            part["tool"] = json!(n);
        } else {
            part["tool"] = json!("tool");
        }
    } else {
        part["tool"] = json!(tool);
    }
    state["input"] = Value::Object(input);
    state["metadata"] = Value::Object(meta);
    part["state"] = state;
}

/// Izin ACP (`session/request_permission`) → izin Zandor. `permission` mengikuti nama engine agar mode izin
/// otomatis (auto-edit = `edit`, auto-safe memeriksa perintah) berlaku sama.
pub fn permission_request(id: &str, zsid: &str, params: &Value, tool: Option<ToolRef>) -> PermissionRequest {
    let tc = params.get("toolCall").cloned().unwrap_or(Value::Null);
    let mut part = json!({});
    merge_tool(&mut part, &tc, 0.);
    let tool_kind = part["tool"].as_str().unwrap_or("tool").to_string();
    let input = &part["state"]["input"];
    let permission = match tool_kind.as_str() {
        "edit" | "write" | "apply_patch" => "edit",
        "bash" => "bash",
        "read" => "read",
        "webfetch" | "websearch" => "webfetch",
        "grep" | "glob" => "grep",
        other => other,
    }
    .to_string();
    let pattern = input.get("command").or_else(|| input.get("filePath")).or_else(|| input.get("url")).or_else(|| input.get("pattern")).and_then(Value::as_str).map(String::from).or_else(|| s(&tc, "title"));
    let options = params.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
    let has_always = options.iter().any(|o| o.get("kind").and_then(Value::as_str) == Some("allow_always"));
    let mut metadata = Map::new();
    if let Some(t) = s(&tc, "title") {
        metadata.insert("title".into(), json!(t));
    }
    if let Some(c) = input.get("command") {
        metadata.insert("command".into(), c.clone());
    }
    if let Some(d) = part["state"]["metadata"].get("diff") {
        metadata.insert("diff".into(), d.clone());
    }
    PermissionRequest {
        id: id.to_string(),
        session_id: zsid.to_string(),
        permission,
        patterns: pattern.into_iter().collect(),
        metadata,
        always: if has_always { vec!["*".into()] } else { vec![] },
        tool,
    }
}

/// Pilih opsi ACP untuk jawaban Zandor. Tidak ada opsi yang cocok → None (dibalas `cancelled`).
pub fn pick_option(options: &[Value], reply: crate::core::types::PermissionReply) -> Option<String> {
    use crate::core::types::PermissionReply as R;
    let want: &[&str] = match reply {
        R::Once => &["allow_once", "allow_always"],
        R::Always => &["allow_always", "allow_once"],
        R::Reject => &["reject_once", "reject_always"],
    };
    want.iter().find_map(|k| options.iter().find(|o| o.get("kind").and_then(Value::as_str) == Some(k)).and_then(|o| s(o, "optionId")))
}

/// Teks riwayat percakapan untuk diberikan ke agent yang baru bergabung (ganti model di tengah sesi).
/// Hanya teks pengguna/asisten dan ringkasan langkah tool; diambil dari akhir sampai `max_chars`.
pub fn history_text(msgs: &[&ChatMessage], max_chars: usize) -> Option<String> {
    let mut blocks: Vec<String> = vec![];
    let mut used = 0;
    for m in msgs.iter().rev() {
        let mut lines = vec![];
        for p in &m.parts {
            match &p.kind {
                PartKind::Text { text, synthetic: false } if !text.trim().is_empty() => lines.push(text.trim().to_string()),
                PartKind::Tool { tool, state, .. } => {
                    let what = state.title.clone().filter(|t| !t.is_empty()).unwrap_or_default();
                    lines.push(format!("[langkah: {tool}{}{what}]", if what.is_empty() { "" } else { " " }));
                }
                PartKind::File { filename, .. } => lines.push(format!("[lampiran: {}]", filename.clone().unwrap_or_default())),
                _ => {}
            }
        }
        if lines.is_empty() {
            continue;
        }
        let who = if m.info.role == Role::User { "Pengguna" } else { "Asisten" };
        let block = format!("{who}:\n{}", lines.join("\n"));
        if used + block.len() > max_chars {
            if blocks.is_empty() {
                let cut: String = block.chars().rev().take(max_chars).collect::<Vec<_>>().into_iter().rev().collect();
                blocks.push(cut);
            }
            break;
        }
        used += block.len();
        blocks.push(block);
    }
    if blocks.is_empty() {
        return None;
    }
    blocks.reverse();
    Some(format!(
        "<riwayat_percakapan>\nIni riwayat percakapan sebelumnya di sesi ini (sebagian dijawab model lain). Pakai sebagai konteks, jangan ulangi pekerjaan yang sudah selesai, dan langsung tanggapi pesan pengguna setelah blok ini.\n\n{}\n</riwayat_percakapan>",
        blocks.join("\n\n")
    ))
}

/// Judul sementara sesi dari pesan pertama (baris pertama yang berisi, maks 50 karakter, dipotong di batas kata).
/// None bila pesan kosong atau hanya perintah slash.
pub fn provisional_title(text: &str) -> Option<String> {
    const MAX: usize = 50;
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    if line.starts_with('/') {
        return None;
    }
    let line: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX {
        return Some(line);
    }
    let cut: String = line.chars().take(MAX).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > MAX / 2 => &cut[..i],
        _ => cut.as_str(),
    };
    Some(format!("{}…", cut.trim_end_matches(|c: char| c.is_ascii_punctuation() || c == ' ')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{to_message, to_part, ToolStatus};

    #[test]
    fn judul_sementara_dari_pesan_pertama() {
        assert_eq!(provisional_title("  \n Halo  dunia \nbaris dua").as_deref(), Some("Halo dunia"));
        assert_eq!(provisional_title("/compact"), None);
        assert_eq!(provisional_title("   "), None);
        let t = provisional_title("Tolong buatkan rencana belajar Rust selama tiga bulan untuk pemula, lengkap").unwrap();
        assert!(t.ends_with('…') && t.chars().count() <= 51, "{t}");
        assert!(!t.contains("pemula,"), "{t}");
    }

    fn part_of(upd: Value) -> Value {
        let mut p = json!({ "id": "prt_1", "messageID": "msg_1", "sessionID": "s", "type": "tool", "callID": "c" });
        merge_tool(&mut p, &upd, 1.);
        p
    }

    #[test]
    fn id_menaik_dan_berprefiks() {
        let a = ascending("msg");
        let b = ascending("msg");
        assert!(a.starts_with("msg_") && a.len() == 4 + 12 + 14);
        assert!(b > a, "{b} > {a}");
    }

    #[test]
    fn eksekusi_codex_jadi_bash_dengan_keluaran_dan_kode() {
        let mut p = part_of(json!({"toolCallId":"c","kind":"execute","title":"dir","status":"in_progress","rawInput":{"command":"dir; type a.txt","cwd":"C:\\w"}}));
        merge_tool(&mut p, &json!({"toolCallId":"c","_meta":{"terminal_output_delta":{"data":"halo\r\n"}}}), 2.);
        merge_tool(&mut p, &json!({"toolCallId":"c","status":"completed","_meta":{"terminal_exit":{"exit_code":0}}}), 3.);
        assert_eq!(p["tool"], "bash");
        let part = to_part(Some(&p)).unwrap();
        let PartKind::Tool { state, .. } = part.kind else { panic!() };
        assert_eq!(state.status, ToolStatus::Completed);
        assert_eq!(state.input["command"], "dir; type a.txt");
        assert_eq!(state.output.as_deref(), Some("halo\r\n"));
        assert_eq!(state.metadata.unwrap()["exit"], 0);
    }

    #[test]
    fn berkas_baru_jadi_write_dan_edit_membawa_diff() {
        let w = part_of(json!({"kind":"edit","title":"Editing files","status":"in_progress","content":[{"type":"diff","oldText":null,"newText":"halo\n","path":"C:\\w\\halo.txt"}]}));
        assert_eq!(w["tool"], "write");
        assert_eq!(w["state"]["input"]["content"], "halo\n");
        assert_eq!(w["state"]["metadata"]["exists"], false);
        // pembaruan status tanpa konten tidak mengubah nama tool
        let mut w2 = w.clone();
        merge_tool(&mut w2, &json!({"status":"completed"}), 2.);
        assert_eq!(w2["tool"], "write");

        let e = part_of(json!({"kind":"edit","title":"Edit a.rs","content":[{"type":"diff","oldText":"a\nb\nc\n","newText":"a\nB\nc\n","path":"a.rs"}]}));
        assert_eq!(e["tool"], "edit");
        assert_eq!(e["state"]["metadata"]["filediff"]["additions"], 1);
        assert_eq!(e["state"]["metadata"]["filediff"]["deletions"], 1);
        let diff = e["state"]["metadata"]["diff"].as_str().unwrap();
        assert!(diff.contains("@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n"), "{diff}");
        let hunks = crate::core::patch::parse_patch(diff);
        assert_eq!(hunks.len(), 1);
    }

    #[test]
    fn banyak_berkas_jadi_apply_patch() {
        let p = part_of(json!({"kind":"edit","content":[{"type":"diff","oldText":"x\n","newText":"y\n","path":"a"},{"type":"diff","oldText":null,"newText":"z\n","path":"b"}]}));
        assert_eq!(p["tool"], "apply_patch");
        assert_eq!(p["state"]["metadata"]["files"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn baca_dari_lokasi_dan_gagal_membawa_alasan() {
        let mut p = part_of(json!({"kind":"read","title":"Read a.md","status":"in_progress","locations":[{"path":"C:\\a.md"}]}));
        merge_tool(&mut p, &json!({"status":"failed","content":[{"type":"content","content":{"type":"text","text":"Access denied"}}]}), 2.);
        assert_eq!(p["tool"], "read");
        assert_eq!(p["state"]["input"]["filePath"], "C:\\a.md");
        assert_eq!(p["state"]["status"], "error");
        assert_eq!(p["state"]["error"], "Access denied");
    }

    #[test]
    fn diff_tambah_hapus_dan_kosong() {
        let (d, a, r) = unified_diff("f", "", "1\n2\n");
        assert_eq!((a, r), (2, 0));
        assert!(d.contains("@@ -0,0 +1,2 @@"));
        let (_, a, r) = unified_diff("f", "x\n", "x\n");
        assert_eq!((a, r), (0, 0));
    }

    #[test]
    fn izin_dipetakan_ke_nama_engine() {
        let params = json!({"sessionId":"a","toolCall":{"toolCallId":"c","kind":"execute","title":"npm test","rawInput":{"command":"npm test"}},
            "options":[{"optionId":"y","name":"Allow","kind":"allow_once"},{"optionId":"ya","name":"Always","kind":"allow_always"},{"optionId":"n","name":"No","kind":"reject_once"}]});
        let r = permission_request("acpperm_1", "ses", &params, None);
        assert_eq!(r.permission, "bash");
        assert_eq!(r.patterns, vec!["npm test"]);
        assert_eq!(r.always, vec!["*"]);
        let opts = params["options"].as_array().unwrap();
        use crate::core::types::PermissionReply as R;
        assert_eq!(pick_option(opts, R::Once).as_deref(), Some("y"));
        assert_eq!(pick_option(opts, R::Always).as_deref(), Some("ya"));
        assert_eq!(pick_option(opts, R::Reject).as_deref(), Some("n"));
        assert_eq!(pick_option(&[], R::Once), None);
        let e = permission_request("p", "s", &json!({"toolCall":{"kind":"edit","content":[{"type":"diff","oldText":"a","newText":"b","path":"x"}]}}), None);
        assert_eq!(e.permission, "edit");
    }

    #[test]
    fn token_dari_usage() {
        let t = tokens_from_usage(&json!({"inputTokens":2,"outputTokens":4,"cachedReadTokens":10,"cachedWriteTokens":39139,"totalTokens":39155})).unwrap();
        assert_eq!(t["input"], 39151.0);
        assert_eq!(t["output"], 4.0);
        assert!(tokens_from_usage(&json!({})).is_none());
    }

    #[test]
    fn riwayat_dipotong_dari_akhir() {
        let mk = |id: &str, role: &str, text: &str| {
            to_message(&json!({"info":{"id":id,"sessionID":"s","role":role,"time":{"created":1}},"parts":[{"id":format!("p{id}"),"messageID":id,"sessionID":"s","type":"text","text":text}]})).unwrap()
        };
        let msgs = [mk("a", "user", "pertama"), mk("b", "assistant", "jawaban pertama"), mk("c", "user", "kedua")];
        let refs: Vec<&ChatMessage> = msgs.iter().collect();
        let h = history_text(&refs, 10_000).unwrap();
        assert!(h.contains("Pengguna:\npertama") && h.contains("Asisten:\njawaban pertama"));
        let short = history_text(&refs, 30).unwrap();
        assert!(short.contains("kedua") && !short.contains("pertama"));
        assert!(history_text(&[], 100).is_none());
    }
}
