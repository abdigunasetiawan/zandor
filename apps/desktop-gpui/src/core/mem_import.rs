//! Impor memori dari AI lain (onboarding) — port packages/core/src/memory/import.ts. Fakta `- kunci: nilai` dari
//! beberapa AI digabung dengan memori global; kunci tunggal dengan nilai berbeda menjadi KONFLIK yang wajib dipilih.
use super::mem::scan_secrets;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ImportSource {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedLine {
    pub key: Option<String>,
    pub label: Option<String>,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FactOption {
    pub value: String,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyedFact {
    pub key: String,
    pub label: String,
    pub options: Vec<FactOption>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergedMemory {
    pub keyed: Vec<KeyedFact>,
    pub free: Vec<FactOption>,
    pub secrets: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Resolution {
    Pick(usize),
    Custom(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListPick {
    pub excluded: Vec<String>,
    pub extra: Vec<String>,
}

pub const MAX_IMPORT_FACTS: usize = 300;
const MAX_LINE: usize = 500;
const MAX_KEY: usize = 40;

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn value_id(v: &str) -> String {
    let s = squash(&v.to_lowercase());
    s.trim_end_matches(['.', '。']).trim().to_string()
}
pub fn norm_key(k: &str) -> String {
    let s: String = k.to_lowercase().chars().filter(|c| !matches!(c, '*' | '_' | '`' | '"')).collect();
    squash(&s)
}

fn strip_bullet(line: &str) -> &str {
    for p in ["- ", "* ", "• "] {
        if let Some(r) = line.strip_prefix(p) {
            return r;
        }
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &line[digits..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return r;
        }
    }
    line
}

pub fn parse_memory_text(text: &str) -> Vec<ParsedLine> {
    let mut out = vec![];
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") || line.starts_with('#') {
            continue;
        }
        let mut line = strip_bullet(line).trim_start().to_string();
        // **x**… → x…
        if let Some(rest) = line.strip_prefix("**") {
            if let Some(end) = rest.find("**").filter(|e| *e > 0) {
                line = format!("{}{}", &rest[..end], &rest[end + 2..]);
            }
        }
        let line = line.trim().to_string();
        if line.is_empty() || line.chars().count() > MAX_LINE {
            continue;
        }
        let sep = line.char_indices().find(|(_, c)| *c == ':' || *c == '：');
        let parsed = sep.and_then(|(at, c)| {
            let label_raw = &line[..at];
            let n = label_raw.chars().count();
            if n == 0 || n > 40 {
                return None;
            }
            let value = line[at + c.len_utf8()..].trim();
            if value.is_empty() {
                return None;
            }
            let label: String = label_raw.chars().filter(|c| !matches!(c, '*' | '_' | '`' | '"')).collect::<String>().trim().to_string();
            (!label.is_empty() && label.chars().count() <= MAX_KEY && !label.eq_ignore_ascii_case("http") && !label.eq_ignore_ascii_case("https")).then(|| ParsedLine { key: Some(norm_key(&label)), label: Some(label), value: value.to_string() })
        });
        out.push(parsed.unwrap_or(ParsedLine { key: None, label: None, value: line }));
    }
    out
}

pub fn merge_memory(sources: &[ImportSource]) -> MergedMemory {
    let mut keyed: Vec<KeyedFact> = vec![];
    let mut kidx: HashMap<String, usize> = HashMap::new();
    let mut free: Vec<FactOption> = vec![];
    let mut fidx: HashMap<String, usize> = HashMap::new();
    let (mut secrets, mut count) = (0, 0);
    for src in sources {
        for f in parse_memory_text(&src.text) {
            if !scan_secrets(&format!("{} {}", f.label.clone().unwrap_or_default(), f.value)).is_empty() {
                secrets += 1;
                continue;
            }
            if count >= MAX_IMPORT_FACTS {
                break;
            }
            let nv = value_id(&f.value);
            if nv.is_empty() {
                continue;
            }
            match (f.key, f.label) {
                (Some(key), Some(label)) => {
                    let ix = *kidx.entry(key.clone()).or_insert_with(|| {
                        keyed.push(KeyedFact { key, label, options: vec![] });
                        count += 1;
                        keyed.len() - 1
                    });
                    let k = &mut keyed[ix];
                    match k.options.iter_mut().find(|o| value_id(&o.value) == nv) {
                        Some(o) => {
                            if !o.sources.contains(&src.id) {
                                o.sources.push(src.id.clone());
                            }
                        }
                        None => k.options.push(FactOption { value: f.value, sources: vec![src.id.clone()] }),
                    }
                }
                _ => match fidx.get(&nv) {
                    Some(&ix) => {
                        if !free[ix].sources.contains(&src.id) {
                            free[ix].sources.push(src.id.clone());
                        }
                    }
                    None => {
                        fidx.insert(nv, free.len());
                        free.push(FactOption { value: f.value, sources: vec![src.id.clone()] });
                        count += 1;
                    }
                },
            }
        }
    }
    MergedMemory { keyed, free, secrets }
}

pub const SINGLE_KEYS: [&str; 18] = ["nama", "nama lengkap", "name", "full name", "panggilan", "nickname", "pekerjaan", "occupation", "job", "perusahaan", "company", "lokasi", "location", "zona waktu", "time zone", "timezone", "gaya jawaban", "answer style"];
pub fn is_single_key(key: &str) -> bool {
    SINGLE_KEYS.contains(&norm_key(key).as_str())
}

pub fn conflicts_of(m: &MergedMemory) -> Vec<&KeyedFact> {
    m.keyed.iter().filter(|k| is_single_key(&k.key) && k.options.len() > 1).collect()
}
pub fn lists_of(m: &MergedMemory) -> Vec<&KeyedFact> {
    m.keyed.iter().filter(|k| !is_single_key(&k.key)).collect()
}

pub fn unresolved<'a>(m: &'a MergedMemory, res: &HashMap<String, Resolution>) -> Vec<&'a KeyedFact> {
    conflicts_of(m)
        .into_iter()
        .filter(|k| match res.get(&k.key) {
            None => true,
            Some(Resolution::Custom(c)) => c.trim().is_empty(),
            _ => false,
        })
        .collect()
}

pub fn build_memory(m: &MergedMemory, res: &HashMap<String, Resolution>, lists: &HashMap<String, ListPick>) -> String {
    let mut lines: Vec<String> = vec![];
    for k in &m.keyed {
        if !is_single_key(&k.key) {
            let pick = lists.get(&k.key);
            let mut seen: Vec<String> = vec![];
            let values: Vec<String> = k.options.iter().map(|o| o.value.clone()).chain(pick.map(|p| p.extra.iter().map(|x| squash(x)).collect::<Vec<_>>()).unwrap_or_default()).collect();
            for v in values {
                let id = value_id(&v);
                if id.is_empty() || seen.contains(&id) || pick.is_some_and(|p| p.excluded.contains(&id)) {
                    continue;
                }
                seen.push(id);
                lines.push(format!("- {}: {v}", k.label));
            }
            continue;
        }
        if k.options.len() == 1 {
            lines.push(format!("- {}: {}", k.label, k.options[0].value));
            continue;
        }
        match res.get(&k.key) {
            None => {}
            Some(Resolution::Custom(c)) => {
                let v = squash(c);
                if !v.is_empty() {
                    lines.push(format!("- {}: {v}", k.label));
                }
            }
            Some(Resolution::Pick(i)) => {
                if let Some(o) = k.options.get(*i) {
                    lines.push(format!("- {}: {}", k.label, o.value));
                }
            }
        }
    }
    for f in &m.free {
        lines.push(format!("- {}", f.value));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn src(id: &str, text: &str) -> ImportSource {
        ImportSource { id: id.into(), label: id.to_uppercase(), text: text.into() }
    }
    #[test]
    fn urai() {
        let p = parse_memory_text("```\n- nama: Abdi\n* **Pekerjaan**: developer\n1. suka jawaban singkat\n# Judul\n```");
        assert_eq!(p[0], ParsedLine { key: Some("nama".into()), label: Some("nama".into()), value: "Abdi".into() });
        assert_eq!(p[1], ParsedLine { key: Some("pekerjaan".into()), label: Some("Pekerjaan".into()), value: "developer".into() });
        assert_eq!(p[2], ParsedLine { key: None, label: None, value: "suka jawaban singkat".into() });
        assert_eq!(p.len(), 3);
    }
    #[test]
    fn konflik() {
        let m = merge_memory(&[src("gemini", "- nama: Adi\n- bahasa: Indonesia"), src("chatgpt", "- Nama: Abdi\n- bahasa: indonesia.")]);
        assert_eq!(conflicts_of(&m).iter().map(|k| k.key.as_str()).collect::<Vec<_>>(), vec!["nama"]);
        let b = m.keyed.iter().find(|k| k.key == "bahasa").unwrap();
        assert_eq!(b.options, vec![FactOption { value: "Indonesia".into(), sources: vec!["gemini".into(), "chatgpt".into()] }]);
        let none = HashMap::new();
        assert_eq!(unresolved(&m, &none).len(), 1);
        assert_eq!(build_memory(&m, &none, &HashMap::new()), "- bahasa: Indonesia\n");
        let r = HashMap::from([("nama".to_string(), Resolution::Pick(1))]);
        assert_eq!(build_memory(&m, &r, &HashMap::new()), "- nama: Abdi\n- bahasa: Indonesia\n");
        let c = HashMap::from([("nama".to_string(), Resolution::Custom(" Abdi\n Guna ".into()))]);
        assert_eq!(build_memory(&m, &c, &HashMap::new()), "- nama: Abdi Guna\n- bahasa: Indonesia\n");
    }
    #[test]
    fn daftar() {
        let m = merge_memory(&[src("a", "- proyek: Zandor\n- alat: Git"), src("b", "- proyek: Toko Online\n- kebiasaan unik: kopi pagi")]);
        assert!(conflicts_of(&m).is_empty());
        let picks = HashMap::from([("proyek".to_string(), ListPick { excluded: vec![value_id("Toko Online")], extra: vec!["Blog  pribadi".into(), "zandor".into(), " ".into()] })]);
        assert_eq!(build_memory(&m, &HashMap::new(), &picks), "- proyek: Zandor\n- proyek: Blog pribadi\n- alat: Git\n- kebiasaan unik: kopi pagi\n");
    }
    #[test]
    fn secret_dan_bebas() {
        let m = merge_memory(&[src("x", "- kunci: sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789\n- nama: Rina")]);
        assert_eq!(m.secrets, 1);
        let m = merge_memory(&[src("a", "- suka kopi"), src("b", "- Suka kopi.")]);
        assert_eq!(m.free, vec![FactOption { value: "suka kopi".into(), sources: vec!["a".into(), "b".into()] }]);
        assert!(is_single_key("Time Zone") && !is_single_key("proyek"));
    }
}
