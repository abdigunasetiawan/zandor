//! Rewind: memutar balik sesi ke sebuah pesan pengguna. Logika murni (daftar berkas, pemotongan pesan, pemulihan teks
//! berkas dari patch) ada di sini supaya teruji; I/O ke engine dan disk ada di `chat_store`.
//!
//! Dukungan engine: `POST /session/:id/revert` memulihkan berkas **dan** menyembunyikan pesan (mode Kode+percakapan).
//! Engine tidak punya "hanya percakapan" maupun "hanya kode", jadi:
//! - Hanya percakapan = hapus pesan (`DELETE /session/:id/message/:mid`) tanpa menyentuh berkas.
//! - Hanya kode = balikkan patch tiap giliran (terbaru dulu) pada berkas di disk; gagal jujur bila isi berkas sudah
//!   berubah sehingga patch tidak cocok (berkas itu tidak disentuh, galat dilaporkan).
use super::engine::SessionFileDiff;
use super::patch::{parse_patch, reverse_apply};
use super::tool_view::{describe_tool, ToolKind};
use super::types::{ChatMessage, PartKind, ToolStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RewindMode {
    #[default]
    Both,
    ConversationOnly,
    CodeOnly,
}

impl RewindMode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::ConversationOnly => "conversation",
            Self::CodeOnly => "code",
        }
    }
    pub fn from_key(k: &str) -> Self {
        match k {
            "conversation" => Self::ConversationOnly,
            "code" => Self::CodeOnly,
            _ => Self::Both,
        }
    }
    pub fn touches_code(self) -> bool {
        self != Self::ConversationOnly
    }
    pub fn touches_conversation(self) -> bool {
        self != Self::CodeOnly
    }
}

/// Perubahan satu berkas sejak pesan tujuan (gabungan semua giliran).
#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    pub file: String,
    pub additions: u32,
    pub deletions: u32,
    /// "added" bila berkas dibuat sejak pesan itu, "deleted" bila dihapus, selain itu "modified".
    pub status: String,
    /// Patch tiap giliran, urutan kronologis.
    pub patches: Vec<String>,
}

/// Gabungkan diff per giliran (kronologis) menjadi satu entri per berkas, urutan kemunculan pertama.
pub fn merge_changes(turns: Vec<Vec<SessionFileDiff>>) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = vec![];
    for d in turns.into_iter().flatten() {
        match out.iter_mut().find(|c| c.file == d.file) {
            Some(c) => {
                c.additions += d.additions;
                c.deletions += d.deletions;
                if d.status == "deleted" {
                    c.status = if c.status == "added" { "modified".into() } else { "deleted".into() };
                } else if c.status == "deleted" {
                    c.status = "modified".into();
                }
                if !d.patch.is_empty() {
                    c.patches.push(d.patch);
                }
            }
            None => out.push(FileChange { file: d.file, additions: d.additions, deletions: d.deletions, status: d.status, patches: if d.patch.is_empty() { vec![] } else { vec![d.patch] } }),
        }
    }
    out
}

/// (jumlah tambah, jumlah hapus) seluruh berkas.
pub fn totals(changes: &[FileChange]) -> (u32, u32) {
    changes.iter().fold((0, 0), |(a, d), c| (a + c.additions, d + c.deletions))
}

/// Id pesan dari `target` ke belakang (id engine terurut leksikografis), dalam urutan masukan.
pub fn ids_from<'a>(ids: impl IntoIterator<Item = &'a str>, target: &str) -> Vec<String> {
    ids.into_iter().filter(|id| *id >= target).map(String::from).collect()
}

/// Pulihkan teks berkas sebelum giliran-giliran itu. `current` None = berkas tidak ada. Hasil `Ok(None)` = berkas
/// harus dihapus (dulu belum ada). Galat bila patch tidak cocok dengan isi sekarang.
pub fn restore_text(current: Option<&str>, change: &FileChange) -> Result<Option<String>, String> {
    let mut text = current.unwrap_or("").to_string();
    if change.patches.is_empty() {
        return Err(format!("{}: engine tidak mengirim patch untuk berkas ini", change.file));
    }
    for p in change.patches.iter().rev() {
        let hunks = parse_patch(p);
        text = reverse_apply(&text, &hunks).ok_or_else(|| format!("{}: isi berkas sudah berubah, patch tidak cocok", change.file))?;
    }
    if change.status == "added" && text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(text))
}


/// Perubahan berkas dari part tool edit/write yang sudah selesai pada pesan dengan id >= `target`, urutan kronologis.
/// Dipakai bila engine tidak punya snapshot (mis. folder bukan git) sehingga `diff?messageID=` kosong. Hanya mencakup
/// edit/write; perubahan lewat shell/bash tidak terlihat di sini.
pub fn changes_from_messages(msgs: &[ChatMessage], target: &str) -> Vec<SessionFileDiff> {
    let mut out = vec![];
    for m in msgs.iter().filter(|m| m.info.id.as_str() >= target) {
        for p in &m.parts {
            let PartKind::Tool { tool, state, .. } = &p.kind else { continue };
            if state.status != ToolStatus::Completed {
                continue;
            }
            // apply_patch: banyak berkas di metadata.files [{filePath, type, diff, additions, deletions}].
            if let Some(files) = state.metadata.as_ref().and_then(|m| m.get("files")).and_then(|f| f.as_array()) {
                for f in files {
                    let s = |k: &str| f.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let n = |k: &str| f.get(k).and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                    let file = if s("filePath").is_empty() { s("relativePath") } else { s("filePath") };
                    if file.is_empty() {
                        continue;
                    }
                    let status = match s("type").as_str() {
                        "add" => "added",
                        "delete" => "deleted",
                        _ => "modified",
                    };
                    out.push(SessionFileDiff { file, patch: s("diff"), additions: n("additions"), deletions: n("deletions"), status: status.into() });
                }
                continue;
            }
            let v = describe_tool(tool, state);
            if !matches!(v.kind, ToolKind::Edit | ToolKind::Write) || v.target.is_empty() {
                continue;
            }
            let mut patch = v.diff.unwrap_or_default();
            let mut status = "modified";
            // Tool write tidak mengirim patch. Berkas baru (metadata.exists=false): susun patch tambah-semua dari input.content
            // supaya bisa dibalik (dihapus). Berkas yang sudah ada tidak bisa dipulihkan karena isi lamanya tidak diketahui.
            let created = state.metadata.as_ref().and_then(|m| m.get("exists")).and_then(|e| e.as_bool()) == Some(false);
            if v.kind == ToolKind::Write && patch.is_empty() && created {
                if let Some(content) = state.input.get("content").and_then(|c| c.as_str()) {
                    let lines: Vec<&str> = content.lines().collect();
                    patch = format!("@@ -0,0 +1,{} @@\n{}\n", lines.len(), lines.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"));
                    status = "added";
                }
            }
            out.push(SessionFileDiff { file: v.target, patch, additions: v.adds.unwrap_or(0), deletions: v.dels.unwrap_or(0), status: status.into() });
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;

    fn d(file: &str, patch: &str, a: u32, del: u32, status: &str) -> SessionFileDiff {
        SessionFileDiff { file: file.into(), patch: patch.into(), additions: a, deletions: del, status: status.into() }
    }

    #[test]
    fn gabung_menjumlah_dan_menjaga_urutan() {
        let m = merge_changes(vec![vec![d("a.rs", "p1", 2, 1, "modified"), d("b.rs", "p2", 5, 0, "added")], vec![d("a.rs", "p3", 1, 1, "modified")]]);
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].file.as_str(), m[0].additions, m[0].deletions), ("a.rs", 3, 2));
        assert_eq!(m[0].patches, vec!["p1", "p3"]);
        assert_eq!(totals(&m), (8, 2));
    }

    #[test]
    fn status_dibuat_lalu_dihapus_jadi_modified() {
        let m = merge_changes(vec![vec![d("a", "x", 1, 0, "added")], vec![d("a", "y", 0, 1, "deleted")]]);
        assert_eq!(m[0].status, "modified");
    }

    #[test]
    fn pemotongan_id_dari_pesan_tujuan() {
        let ids = ["msg_001", "msg_002", "msg_003"];
        assert_eq!(ids_from(ids, "msg_002"), vec!["msg_002", "msg_003"]);
        assert!(ids_from(ids, "msg_999").is_empty());
    }

    #[test]
    fn pulihkan_membalik_patch_terbaru_dulu() {
        let p1 = "@@ -1,2 +1,2 @@\n a\n-b\n+B\n";
        let p2 = "@@ -1,2 +1,2 @@\n a\n-B\n+C\n";
        let c = FileChange { file: "f".into(), additions: 2, deletions: 2, status: "modified".into(), patches: vec![p1.into(), p2.into()] };
        assert_eq!(restore_text(Some("a\nC"), &c).unwrap().as_deref(), Some("a\nb"));
    }

    #[test]
    fn pulihkan_berkas_baru_berarti_hapus() {
        let c = FileChange { file: "n".into(), additions: 1, deletions: 0, status: "added".into(), patches: vec!["@@ -0,0 +1 @@\n+halo\n".into()] };
        assert_eq!(restore_text(Some("halo"), &c).unwrap(), None);
    }

    #[test]
    fn pulihkan_gagal_jujur_bila_berkas_berubah() {
        let c = FileChange { file: "f".into(), additions: 1, deletions: 1, status: "modified".into(), patches: vec!["@@ -1,2 +1,2 @@\n a\n-b\n+B\n".into()] };
        assert!(restore_text(Some("zzz"), &c).unwrap_err().contains("tidak cocok"));
        let kosong = FileChange { patches: vec![], ..c };
        assert!(restore_text(Some("a\nB"), &kosong).is_err());
    }

    #[test]
    fn mode_kunci_bolak_balik() {
        for m in [RewindMode::Both, RewindMode::ConversationOnly, RewindMode::CodeOnly] {
            assert_eq!(RewindMode::from_key(m.key()), m);
        }
        assert!(RewindMode::CodeOnly.touches_code() && !RewindMode::CodeOnly.touches_conversation());
    }
}

#[cfg(test)]
mod transcript_tests {
    use super::*;
    use crate::core::types::*;
    use serde_json::json;

    fn msg(id: &str, role: Role, parts: Vec<PartKind>) -> ChatMessage {
        let info = MessageInfo { id: id.into(), session_id: "s".into(), role, created: 0., completed: None, parent_id: None, agent: None, provider_id: None, model_id: None, finish: None, error: None, tokens: None, cost: None };
        ChatMessage { info, parts: parts.into_iter().enumerate().map(|(i, kind)| MessagePart { id: format!("{id}p{i}"), message_id: id.into(), session_id: "s".into(), kind }).collect() }
    }
    fn tool(name: &str, path: &str, diff: &str, status: ToolStatus) -> PartKind {
        let state = ToolState { status, input: json!({"filePath": path}).as_object().cloned().unwrap(), output: None, error: None, title: None, metadata: json!({"diff": diff}).as_object().cloned(), time: None };
        PartKind::Tool { tool: name.into(), call_id: "c".into(), state }
    }

    #[test]
    fn ambil_edit_dari_transkrip_sejak_pesan_tujuan() {
        let d = "@@ -1,2 +1,2 @@\n a\n-b\n+B\n";
        let msgs = vec![
            msg("m1", Role::Assistant, vec![tool("edit", "lama.rs", d, ToolStatus::Completed)]),
            msg("m2", Role::Assistant, vec![tool("edit", "baru.rs", d, ToolStatus::Completed), tool("edit", "gagal.rs", d, ToolStatus::Error), tool("read", "x.rs", "", ToolStatus::Completed)]),
        ];
        let c = changes_from_messages(&msgs, "m2");
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].file.as_str(), c[0].additions, c[0].deletions), ("baru.rs", 1, 1));
        assert_eq!(restore_text(Some("a\nB"), &merge_changes(vec![c])[0]).unwrap().as_deref(), Some("a\nb"));
    }
}

#[cfg(test)]
mod apply_patch_tests {
    use super::*;
    use crate::core::types::*;
    use serde_json::json;

    #[test]
    fn apply_patch_membaca_metadata_files() {
        let meta = json!({"files": [{"filePath": "C:/w/a.css", "type": "update", "diff": "@@ -1 +1 @@\n-x\n+y\n", "additions": 1, "deletions": 1}, {"relativePath": "b.css", "type": "add", "diff": "", "additions": 3, "deletions": 0}]});
        let state = ToolState { status: ToolStatus::Completed, input: Default::default(), output: None, error: None, title: None, metadata: meta.as_object().cloned(), time: None };
        let info = MessageInfo { id: "m2".into(), session_id: "s".into(), role: Role::Assistant, created: 0., completed: None, parent_id: None, agent: None, provider_id: None, model_id: None, finish: None, error: None, tokens: None, cost: None };
        let m = ChatMessage { info, parts: vec![MessagePart { id: "p".into(), message_id: "m2".into(), session_id: "s".into(), kind: PartKind::Tool { tool: "apply_patch".into(), call_id: "c".into(), state } }] };
        let c = changes_from_messages(&[m], "m1");
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].additions, c[0].status.as_str()), (1, "modified"));
        assert_eq!((c[1].file.as_str(), c[1].status.as_str()), ("b.css", "added"));
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;
    use crate::core::types::*;
    use serde_json::json;

    #[test]
    fn write_berkas_baru_bisa_dibalik_jadi_hapus() {
        let state = ToolState { status: ToolStatus::Completed, input: json!({"filePath": "rewind-uji.txt", "content": "satu\ndua\ntiga"}).as_object().cloned().unwrap(), output: None, error: None, title: None, metadata: json!({"exists": false}).as_object().cloned(), time: None };
        let info = MessageInfo { id: "m2".into(), session_id: "s".into(), role: Role::Assistant, created: 0., completed: None, parent_id: None, agent: None, provider_id: None, model_id: None, finish: None, error: None, tokens: None, cost: None };
        let m = ChatMessage { info, parts: vec![MessagePart { id: "p".into(), message_id: "m2".into(), session_id: "s".into(), kind: PartKind::Tool { tool: "write".into(), call_id: "c".into(), state } }] };
        let c = merge_changes(vec![changes_from_messages(&[m], "m1")]);
        assert_eq!((c[0].status.as_str(), c[0].additions), ("added", 3));
        assert_eq!(restore_text(Some("satu\ndua\ntiga"), &c[0]).unwrap(), None);
        assert_eq!(restore_text(Some("satu\ndua\ntiga\n"), &c[0]).unwrap(), None);
    }
}
