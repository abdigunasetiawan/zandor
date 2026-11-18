//! Diff unified → hunk yang bisa ditinjau di Panel 3 — port packages/core/src/docs/patch.ts (ADR-075). Isi hunk
//! dicocokkan dengan teks berkas saat ini (dekat posisi yang diharapkan dulu, lalu seluruh berkas bila tepat satu kali).

#[derive(Clone, Debug, PartialEq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    /// Baris dengan awalan ` `, `-`, atau `+`.
    pub lines: Vec<String>,
}

fn header(l: &str) -> Option<(usize, usize, usize, usize)> {
    let rest = l.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let pair = |s: &str| -> Option<(usize, usize)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (a, b) = pair(old)?;
    let (c, d) = pair(new)?;
    Some((a, b, c, d))
}

pub fn parse_patch(patch: &str) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = vec![];
    for raw in patch.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some((a, b, c, d)) = header(raw) {
            hunks.push(Hunk { old_start: a, old_lines: b, new_start: c, new_lines: d, lines: vec![] });
            continue;
        }
        let Some(cur) = hunks.last_mut() else { continue };
        let old_seen = cur.lines.iter().filter(|l| !l.starts_with('+')).count();
        let new_seen = cur.lines.iter().filter(|l| !l.starts_with('-')).count();
        match raw.chars().next() {
            Some('-' | '+' | ' ') => cur.lines.push(raw.to_string()),
            None if old_seen < cur.old_lines && new_seen < cur.new_lines => cur.lines.push(" ".into()),
            _ => {}
        }
    }
    hunks.retain(|h| h.lines.iter().any(|l| !l.starts_with(' ')));
    hunks
}

pub fn hunk_sides(h: &Hunk) -> (Vec<String>, Vec<String>) {
    let (mut before, mut after) = (vec![], vec![]);
    for l in &h.lines {
        let body = l.get(1..).unwrap_or("").to_string();
        if !l.starts_with('+') {
            before.push(body.clone());
        }
        if !l.starts_with('-') {
            after.push(body);
        }
    }
    (before, after)
}

fn split_lines(text: &str) -> (Vec<String>, &'static str) {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    (text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect(), eol)
}

fn matches_at(lines: &[String], at: i64, want: &[String]) -> bool {
    at >= 0 && (at as usize) + want.len() <= lines.len() && lines[at as usize..at as usize + want.len()] == *want
}

fn locate(lines: &[String], want: &[String], hint: i64) -> Option<usize> {
    if want.is_empty() {
        return Some(hint.clamp(0, lines.len() as i64) as usize);
    }
    for d in 0..=50i64 {
        if matches_at(lines, hint - d, want) {
            return Some((hint - d) as usize);
        }
        if d > 0 && matches_at(lines, hint + d, want) {
            return Some((hint + d) as usize);
        }
    }
    let mut found = None;
    for i in 0..=(lines.len().saturating_sub(want.len())) {
        if matches_at(lines, i as i64, want) {
            if found.is_some() {
                return None;
            }
            found = Some(i);
        }
    }
    found
}

/// Baris 1-based awal + jumlah baris sisi "sesudah".
pub fn locate_hunk(text: &str, h: &Hunk) -> Option<(usize, usize)> {
    let (lines, _) = split_lines(text);
    let (_, after) = hunk_sides(h);
    locate(&lines, &after, h.new_start as i64 - 1).map(|a| (a + 1, after.len()))
}

pub fn revert_hunk(text: &str, h: &Hunk) -> Option<String> {
    let (mut lines, eol) = split_lines(text);
    let (before, after) = hunk_sides(h);
    let at = locate(&lines, &after, h.new_start as i64 - 1)?;
    lines.splice(at..at + after.len(), before);
    Some(lines.join(eol))
}

pub fn reverse_apply(text: &str, hunks: &[Hunk]) -> Option<String> {
    let mut v: Vec<&Hunk> = hunks.iter().collect();
    v.sort_by(|a, b| b.new_start.cmp(&a.new_start));
    let mut out = text.to_string();
    for h in v {
        out = revert_hunk(&out, h)?;
    }
    Some(out)
}

pub fn drop_hunk(hunks: &[Hunk], i: usize, reverted: bool) -> Vec<Hunk> {
    let Some(h) = hunks.get(i) else { return hunks.to_vec() };
    let shift = if reverted { h.old_lines as i64 - h.new_lines as i64 } else { 0 };
    hunks
        .iter()
        .enumerate()
        .filter(|(k, _)| *k != i)
        .map(|(_, x)| {
            let mut x = x.clone();
            if shift != 0 && x.new_start > h.new_start {
                x.new_start = (x.new_start as i64 + shift).max(1) as usize;
            }
            x
        })
        .collect()
}

pub fn format_hunks(hunks: &[Hunk]) -> String {
    hunks.iter().map(|h| format!("@@ -{},{} +{},{} @@\n{}\n", h.old_start, h.old_lines, h.new_start, h.new_lines, h.lines.join("\n"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const P: &str = "--- a\n+++ b\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n@@ -5,2 +5,3 @@\n e\n+x\n f\n";
    #[test]
    fn bolak_balik() {
        let cur = "a\nB\nc\nd\ne\nx\nf";
        let h = parse_patch(P);
        assert_eq!(h.len(), 2);
        assert_eq!(parse_patch(&format_hunks(&h)), h);
        assert_eq!(reverse_apply(cur, &h).unwrap(), "a\nb\nc\nd\ne\nf");
        assert_eq!(revert_hunk(cur, &h[0]).unwrap(), "a\nb\nc\nd\ne\nx\nf");
        assert_eq!(locate_hunk(cur, &h[1]), Some((5, 3)));
        let rest = drop_hunk(&h, 1, true);
        assert_eq!(rest.len(), 1);
        assert!(revert_hunk("zzz", &h[0]).is_none());
    }
}
