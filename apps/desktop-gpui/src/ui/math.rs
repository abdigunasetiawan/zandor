//! Rumus LaTeX — pengganti KaTeX versi React (KaTeX hanya menghasilkan HTML/MathML, yang tidak bisa dirender GPUI).
//! Parser subset LaTeX yang umum dipakai model: grup, ^/_, \frac/\dfrac/\tfrac/\binom, \sqrt[n]{}, huruf Yunani & simbol,
//! operator (\sum \int \lim …), fungsi tegak (\sin \log …), \text/\mathrm/\mathbf/\mathbb/\mathcal, aksen (\hat \bar \vec
//! \dot \tilde), \left \right, spasi, dan lingkungan matrix/pmatrix/bmatrix/vmatrix/cases/aligned.
//! Rumus blok dirender sebagai tata letak (pecahan bertumpuk, akar bergaris atas, matriks); rumus sebaris sebagai teks
//! Unicode terdekat (superskrip/subskrip Unicode bila ada). Perbedaan jujur: bukan mesin TeX penuh — perintah yang tidak
//! dikenal ditampilkan apa adanya (berwarna) dan tata letaknya lebih sederhana dari KaTeX.
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, FontStyle, FontWeight, IntoElement};

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// Simbol/huruf: (teks, miring?).
    Sym(String, bool),
    /// Teks tegak (\text, fungsi, angka).
    Text(String),
    Bold(Vec<Node>),
    Group(Vec<Node>),
    Sup(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    SubSup(Box<Node>, Box<Node>, Box<Node>),
    Frac(Vec<Node>, Vec<Node>),
    Binom(Vec<Node>, Vec<Node>),
    Sqrt(Option<Vec<Node>>, Vec<Node>),
    /// Aksen: (karakter penanda, isi).
    Accent(char, Vec<Node>),
    /// Operator besar (∑ ∫ …) atau lim: (simbol, tegak?).
    BigOp(String),
    Space(f32),
    /// Lingkungan: (kiri, kanan, baris → sel).
    Matrix(String, String, Vec<Vec<Vec<Node>>>),
    /// Perintah tak dikenal.
    Unknown(String),
}

fn symbol(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "alpha" => "α", "beta" => "β", "gamma" => "γ", "delta" => "δ", "epsilon" => "ϵ", "varepsilon" => "ε", "zeta" => "ζ", "eta" => "η", "theta" => "θ", "vartheta" => "ϑ", "iota" => "ι", "kappa" => "κ", "lambda" => "λ", "mu" => "μ", "nu" => "ν", "xi" => "ξ", "pi" => "π", "varpi" => "ϖ", "rho" => "ρ", "varrho" => "ϱ", "sigma" => "σ", "varsigma" => "ς", "tau" => "τ", "upsilon" => "υ", "phi" => "ϕ", "varphi" => "φ", "chi" => "χ", "psi" => "ψ", "omega" => "ω",
        "Gamma" => "Γ", "Delta" => "Δ", "Theta" => "Θ", "Lambda" => "Λ", "Xi" => "Ξ", "Pi" => "Π", "Sigma" => "Σ", "Upsilon" => "Υ", "Phi" => "Φ", "Psi" => "Ψ", "Omega" => "Ω",
        "times" => "×", "cdot" => "⋅", "div" => "÷", "pm" => "±", "mp" => "∓", "ast" => "∗", "star" => "⋆", "circ" => "∘", "bullet" => "∙",
        "leq" | "le" => "≤", "geq" | "ge" => "≥", "neq" | "ne" => "≠", "approx" => "≈", "equiv" => "≡", "sim" => "∼", "simeq" => "≃", "cong" => "≅", "propto" => "∝", "ll" => "≪", "gg" => "≫",
        "in" => "∈", "notin" => "∉", "ni" => "∋", "subset" => "⊂", "subseteq" => "⊆", "supset" => "⊃", "supseteq" => "⊇", "cup" => "∪", "cap" => "∩", "emptyset" | "varnothing" => "∅", "setminus" => "∖",
        "forall" => "∀", "exists" => "∃", "nexists" => "∄", "neg" | "lnot" => "¬", "land" | "wedge" => "∧", "lor" | "vee" => "∨", "oplus" => "⊕", "otimes" => "⊗",
        "to" | "rightarrow" => "→", "leftarrow" | "gets" => "←", "leftrightarrow" => "↔", "Rightarrow" | "implies" => "⇒", "Leftarrow" => "⇐", "Leftrightarrow" | "iff" => "⇔", "mapsto" => "↦", "uparrow" => "↑", "downarrow" => "↓", "longrightarrow" => "⟶",
        "infty" => "∞", "partial" => "∂", "nabla" => "∇", "hbar" => "ℏ", "ell" => "ℓ", "Re" => "ℜ", "Im" => "ℑ", "aleph" => "ℵ", "angle" => "∠", "perp" => "⊥", "parallel" => "∥", "degree" => "°", "prime" => "′",
        "ldots" | "dots" => "…", "cdots" => "⋯", "vdots" => "⋮", "ddots" => "⋱",
        "langle" => "⟨", "rangle" => "⟩", "lfloor" => "⌊", "rfloor" => "⌋", "lceil" => "⌈", "rceil" => "⌉", "lbrace" | "{" => "{", "rbrace" | "}" => "}", "vert" | "|" => "|", "Vert" => "‖", "backslash" => "\\",
        "%" => "%", "$" => "$", "&" => "&", "#" => "#", "_" => "_",
        _ => return None,
    })
}

fn big_op(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "sum" => "∑", "prod" => "∏", "coprod" => "∐", "int" => "∫", "iint" => "∬", "iiint" => "∭", "oint" => "∮", "bigcup" => "⋃", "bigcap" => "⋂", "bigoplus" => "⨁", "bigotimes" => "⨂",
        "lim" => "lim", "limsup" => "lim sup", "liminf" => "lim inf", "max" => "max", "min" => "min", "sup" => "sup", "inf" => "inf", "det" => "det", "arg" => "arg",
        _ => return None,
    })
}

const FUNCS: &[&str] = &["sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "log", "ln", "lg", "exp", "deg", "dim", "ker", "hom", "gcd", "Pr", "mod", "bmod"];

fn double_struck(c: char) -> char {
    match c {
        'R' => 'ℝ', 'N' => 'ℕ', 'Z' => 'ℤ', 'Q' => 'ℚ', 'C' => 'ℂ', 'P' => 'ℙ', 'H' => 'ℍ',
        c if c.is_ascii_uppercase() => char::from_u32(0x1D538 + (c as u32 - 'A' as u32)).unwrap_or(c),
        c => c,
    }
}
fn calligraphic(c: char) -> char {
    match c {
        'B' => 'ℬ', 'E' => 'ℰ', 'F' => 'ℱ', 'H' => 'ℋ', 'I' => 'ℐ', 'L' => 'ℒ', 'M' => 'ℳ', 'R' => 'ℛ',
        c if c.is_ascii_uppercase() => char::from_u32(0x1D49C + (c as u32 - 'A' as u32)).unwrap_or(c),
        c => c,
    }
}

struct P<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }
    fn ws(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
    }
    fn cmd(&mut self) -> String {
        // setelah '\'
        let start = self.i;
        if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                self.i += 1;
            }
        } else {
            self.bump();
        }
        self.src[start..self.i].to_string()
    }
    /// Raw `{...}` (untuk \text, \begin{nama}).
    fn raw_group(&mut self) -> String {
        self.ws();
        if self.peek() != Some('{') {
            return self.bump().map(|c| c.to_string()).unwrap_or_default();
        }
        self.i += 1;
        let start = self.i;
        let mut depth = 1;
        while let Some(c) = self.peek() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let out = self.src[start..self.i].to_string();
                        self.i += 1;
                        return out;
                    }
                }
                '\\' => {
                    self.i += 1;
                }
                _ => {}
            }
            self.bump();
        }
        self.src[start..].to_string()
    }
    /// Satu argumen: grup `{...}` atau satu atom.
    fn arg(&mut self) -> Vec<Node> {
        self.ws();
        if self.peek() == Some('{') {
            self.i += 1;
            let v = self.list(&['}']);
            if self.peek() == Some('}') {
                self.i += 1;
            }
            v
        } else {
            self.atom().map(|a| vec![a]).unwrap_or_default()
        }
    }
    fn opt_arg(&mut self) -> Option<Vec<Node>> {
        self.ws();
        if self.peek() != Some('[') {
            return None;
        }
        self.i += 1;
        let v = self.list(&[']']);
        if self.peek() == Some(']') {
            self.i += 1;
        }
        Some(v)
    }
    fn list(&mut self, stop: &[char]) -> Vec<Node> {
        let mut out: Vec<Node> = vec![];
        loop {
            self.ws();
            let Some(c) = self.peek() else { break };
            if stop.contains(&c) {
                break;
            }
            if c == '\\' && (self.src[self.i..].starts_with("\\end") || self.src[self.i..].starts_with("\\\\") || self.src[self.i..].starts_with("\\right")) && stop.contains(&'&') {
                break;
            }
            if c == '\\' && self.src[self.i..].starts_with("\\right") && stop.contains(&'R') {
                break;
            }
            if c == '^' || c == '_' {
                self.i += 1;
                let script = Node::Group(self.arg());
                let base = out.pop().unwrap_or(Node::Text(String::new()));
                let n = match (c, base) {
                    ('^', Node::Sub(b, s)) => Node::SubSup(b, s, Box::new(script)),
                    ('_', Node::Sup(b, s)) => Node::SubSup(b, Box::new(script), s),
                    ('^', b) => Node::Sup(Box::new(b), Box::new(script)),
                    (_, b) => Node::Sub(Box::new(b), Box::new(script)),
                };
                out.push(n);
                continue;
            }
            if c == '\'' {
                self.i += 1;
                let base = out.pop().unwrap_or(Node::Text(String::new()));
                out.push(Node::Sup(Box::new(base), Box::new(Node::Text("′".into()))));
                continue;
            }
            match self.atom() {
                Some(a) => out.push(a),
                None => break,
            }
        }
        out
    }
    fn atom(&mut self) -> Option<Node> {
        self.ws();
        let c = self.peek()?;
        match c {
            '{' => Some(Node::Group(self.arg())),
            '\\' => {
                self.i += 1;
                let cmd = self.cmd();
                Some(self.command(&cmd))
            }
            '}' | ']' => {
                self.i += 1;
                Some(Node::Text(c.to_string()))
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = self.i;
                while self.peek().is_some_and(|x| x.is_ascii_digit() || x == '.') {
                    self.i += 1;
                }
                Some(Node::Text(self.src[start..self.i].to_string()))
            }
            c if c.is_alphabetic() => {
                self.bump();
                Some(Node::Sym(c.to_string(), true))
            }
            '-' => {
                self.i += 1;
                Some(Node::Text(" − ".into()))
            }
            '+' | '=' | '<' | '>' => {
                self.i += 1;
                Some(Node::Text(format!(" {c} ")))
            }
            ',' => {
                self.i += 1;
                Some(Node::Text(", ".into()))
            }
            '~' => {
                self.i += 1;
                Some(Node::Space(0.3))
            }
            _ => {
                self.bump();
                Some(Node::Text(c.to_string()))
            }
        }
    }
    fn command(&mut self, cmd: &str) -> Node {
        if let Some(s) = symbol(cmd) {
            let spaced = matches!(cmd, "times" | "cdot" | "div" | "pm" | "mp" | "leq" | "le" | "geq" | "ge" | "neq" | "ne" | "approx" | "equiv" | "sim" | "simeq" | "cong" | "propto" | "in" | "notin" | "subset" | "subseteq" | "supset" | "supseteq" | "cup" | "cap" | "to" | "rightarrow" | "leftarrow" | "gets" | "Rightarrow" | "implies" | "Leftarrow" | "Leftrightarrow" | "iff" | "mapsto" | "land" | "lor" | "wedge" | "vee" | "oplus" | "otimes" | "leftrightarrow" | "longrightarrow" | "ll" | "gg" | "setminus");
            let greek_lower = s.chars().next().is_some_and(|c| ('α'..='ω').contains(&c) || "ϵϑϖϱςϕ".contains(c));
            return if spaced { Node::Text(format!(" {s} ")) } else if greek_lower { Node::Sym(s.into(), true) } else { Node::Text(s.into()) };
        }
        if let Some(op) = big_op(cmd) {
            return Node::BigOp(op.into());
        }
        if FUNCS.contains(&cmd) {
            return Node::Text(format!("{cmd}\u{2009}"));
        }
        match cmd {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let a = self.arg();
                let b = self.arg();
                Node::Frac(a, b)
            }
            "binom" | "dbinom" | "tbinom" => {
                let a = self.arg();
                let b = self.arg();
                Node::Binom(a, b)
            }
            "sqrt" => {
                let n = self.opt_arg();
                Node::Sqrt(n, self.arg())
            }
            "text" | "textrm" | "mathrm" | "operatorname" | "textnormal" | "mbox" | "textit" => Node::Text(self.raw_group()),
            "mathbf" | "textbf" | "boldsymbol" | "bm" => Node::Bold(self.arg()),
            "mathbb" => Node::Text(self.raw_group().chars().map(double_struck).collect()),
            "mathcal" | "mathscr" => Node::Text(self.raw_group().chars().map(calligraphic).collect()),
            "mathit" => Node::Group(self.arg()),
            "hat" | "widehat" => Node::Accent('\u{0302}', self.arg()),
            "bar" | "overline" => Node::Accent('\u{0305}', self.arg()),
            "vec" | "overrightarrow" => Node::Accent('\u{20D7}', self.arg()),
            "dot" => Node::Accent('\u{0307}', self.arg()),
            "ddot" => Node::Accent('\u{0308}', self.arg()),
            "tilde" | "widetilde" => Node::Accent('\u{0303}', self.arg()),
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" => {
                self.ws();
                if self.peek() == Some('\\') {
                    self.i += 1;
                    let c = self.cmd();
                    Node::Text(symbol(&c).unwrap_or("").to_string())
                } else {
                    let c = self.bump().unwrap_or('.');
                    Node::Text(if c == '.' { String::new() } else { c.to_string() })
                }
            }
            "," | ":" | ">" => Node::Space(0.17),
            ";" => Node::Space(0.28),
            "!" => Node::Space(0.),
            " " => Node::Space(0.25),
            "quad" => Node::Space(1.),
            "qquad" => Node::Space(2.),
            "displaystyle" | "textstyle" | "limits" | "nolimits" => Node::Group(vec![]),
            "\\" => Node::Text("\n".into()),
            "begin" => self.env(),
            _ => Node::Unknown(format!("\\{cmd} ")),
        }
    }
    fn env(&mut self) -> Node {
        let name = self.raw_group();
        let name = name.trim_end_matches('*');
        if matches!(name, "array") {
            let _ = self.raw_group(); // spesifikasi kolom
        }
        let (l, r) = match name {
            "pmatrix" => ("(", ")"),
            "bmatrix" => ("[", "]"),
            "Bmatrix" => ("{", "}"),
            "vmatrix" => ("|", "|"),
            "Vmatrix" => ("‖", "‖"),
            "cases" | "dcases" => ("{", ""),
            _ => ("", ""),
        };
        let mut rows: Vec<Vec<Vec<Node>>> = vec![];
        let mut row: Vec<Vec<Node>> = vec![];
        loop {
            let cell = self.list(&['&']);
            row.push(cell);
            self.ws();
            let rest = &self.src[self.i..];
            if rest.starts_with('&') {
                self.i += 1;
                continue;
            }
            if rest.starts_with("\\\\") {
                self.i += 2;
                rows.push(std::mem::take(&mut row));
                continue;
            }
            if rest.starts_with("\\end") {
                self.i += 4;
                let _ = self.raw_group();
                break;
            }
            if rest.starts_with("\\right") {
                // \right di dalam sel: lewati agar tidak macet
                self.i += 6;
                let _ = self.bump();
                continue;
            }
            if self.i >= self.s.len() {
                break;
            }
            self.bump();
        }
        if !row.is_empty() && !(row.len() == 1 && row[0].is_empty()) {
            rows.push(row);
        }
        Node::Matrix(l.into(), r.into(), rows)
    }
}

pub fn parse(src: &str) -> Vec<Node> {
    let mut p = P { s: src.as_bytes(), src, i: 0 };
    let mut out = vec![];
    while p.i < src.len() {
        let mut v = p.list(&[]);
        out.append(&mut v);
        if p.i < src.len() {
            // karakter penutup liar
            p.bump();
        }
    }
    out
}

// ---------- sebaris: teks Unicode ----------

fn sup_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
        '+' => '⁺', '-' | '−' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾', 'n' => 'ⁿ', 'i' => 'ⁱ', 'x' => 'ˣ', 'y' => 'ʸ', 'a' => 'ᵃ', 'b' => 'ᵇ', 'c' => 'ᶜ', 'd' => 'ᵈ', 'e' => 'ᵉ', 'k' => 'ᵏ', 'm' => 'ᵐ', 'o' => 'ᵒ', 'p' => 'ᵖ', 't' => 'ᵗ', 'T' => 'ᵀ', '′' => '′', ' ' => ' ',
        _ => return None,
    })
}
fn sub_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄', '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
        '+' => '₊', '-' | '−' => '₋', '=' => '₌', '(' => '₍', ')' => '₎', 'a' => 'ₐ', 'e' => 'ₑ', 'i' => 'ᵢ', 'j' => 'ⱼ', 'k' => 'ₖ', 'n' => 'ₙ', 'o' => 'ₒ', 'r' => 'ᵣ', 't' => 'ₜ', 'x' => 'ₓ', 'm' => 'ₘ', 'p' => 'ₚ', 's' => 'ₛ', ' ' => ' ',
        _ => return None,
    })
}

fn script(s: &str, f: fn(char) -> Option<char>, mark: char) -> String {
    let compact: String = s.split_whitespace().collect();
    let t = compact.as_str();
    match t.chars().map(f).collect::<Option<String>>() {
        Some(u) => u,
        None if t.chars().count() == 1 => format!("{mark}{t}"),
        None => format!("{mark}({t})"),
    }
}

pub fn to_text(nodes: &[Node]) -> String {
    nodes.iter().map(node_text).collect::<String>().replace("  ", " ")
}

fn node_text(n: &Node) -> String {
    match n {
        Node::Sym(s, _) | Node::Text(s) | Node::BigOp(s) => s.clone(),
        Node::Bold(v) | Node::Group(v) => to_text(v),
        Node::Sup(b, s) => format!("{}{}", node_text(b), script(&node_text(s), sup_char, '^')),
        Node::Sub(b, s) => format!("{}{}{}", node_text(b), script(&node_text(s), sub_char, '_'), if matches!(**b, Node::BigOp(_)) { " " } else { "" }),
        Node::SubSup(b, d, u) => {
            let op = if matches!(**b, Node::BigOp(_)) { " " } else { "" };
            format!("{}{}{}{op}", node_text(b), script(&node_text(d), sub_char, '_'), script(&node_text(u), sup_char, '^'))
        }
        Node::Frac(a, b) => {
            let (a, b) = (to_text(a), to_text(b));
            let wrap = |s: String| if s.trim().chars().count() > 1 && s.contains([' ', '+', '−']) { format!("({})", s.trim()) } else { s.trim().to_string() };
            format!("{}⁄{}", wrap(a), wrap(b))
        }
        Node::Binom(a, b) => format!("C({}, {})", to_text(a).trim(), to_text(b).trim()),
        Node::Sqrt(n, v) => {
            let inner = to_text(v);
            let root = match n.as_ref().map(|x| to_text(x)) {
                Some(k) if k.trim() == "3" => "∛".to_string(),
                Some(k) if k.trim() == "4" => "∜".to_string(),
                Some(k) => format!("{}√", script(&k, sup_char, '^')),
                None => "√".into(),
            };
            if inner.trim().chars().count() > 1 { format!("{root}({})", inner.trim()) } else { format!("{root}{}", inner.trim()) }
        }
        Node::Accent(m, v) => {
            let s = to_text(v);
            if s.chars().count() == 1 { format!("{s}{m}") } else { format!("{s}") }
        }
        Node::Space(w) => if *w >= 1. { "  ".into() } else if *w > 0. { " ".into() } else { String::new() },
        Node::Matrix(l, r, rows) => format!("{l}{}{r}", rows.iter().map(|row| row.iter().map(|c| to_text(c).trim().to_string()).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("; ")),
        Node::Unknown(s) => s.clone(),
    }
}

// ---------- blok: tata letak ----------

fn row(t: &Theme) -> gpui::Div {
    let _ = t;
    div().flex().flex_row().items_center()
}

fn render_list(nodes: &[Node], t: &Theme, size: f32) -> gpui::Div {
    let mut r = row(t).flex_wrap();
    for n in nodes {
        r = r.child(render_node(n, t, size));
    }
    r
}

fn render_node(n: &Node, t: &Theme, size: f32) -> AnyElement {
    let txt = |s: &str| div().text_size(px(size)).child(s.to_string());
    match n {
        Node::Sym(s, italic) => txt(s).when(*italic, |d| d.italic()).into_any_element(),
        Node::Text(s) if s == "\n" => div().w_full().into_any_element(),
        Node::Text(s) => txt(&s.replace(' ', "\u{2009}")).into_any_element(),
        Node::BigOp(s) => {
            let big = s.chars().count() == 1;
            txt(s).when(big, |d| d.text_size(px(size * 1.6))).px(px(size * 0.1)).into_any_element()
        }
        Node::Bold(v) => render_list(v, t, size).font_weight(FontWeight::BOLD).into_any_element(),
        Node::Group(v) => render_list(v, t, size).into_any_element(),
        Node::Sup(b, s) => row(t).items_start().child(render_node(b, t, size)).child(div().mt(px(-size * 0.15)).child(render_node(s, t, size * 0.7))).into_any_element(),
        Node::Sub(b, s) => {
            let op = matches!(**b, Node::BigOp(_));
            if op {
                // batas di bawah operator (∑_{i=1}, lim_{x→0})
                div().flex().flex_col().items_center().child(render_node(b, t, size)).child(render_node(s, t, size * 0.7)).into_any_element()
            } else {
                row(t).items_end().child(render_node(b, t, size)).child(div().mb(px(-size * 0.25)).child(render_node(s, t, size * 0.7))).into_any_element()
            }
        }
        Node::SubSup(b, d, u) => {
            if matches!(**b, Node::BigOp(ref s) if s.chars().count() == 1 && s != "∫" && s != "∮") {
                div().flex().flex_col().items_center().child(render_node(u, t, size * 0.7)).child(render_node(b, t, size)).child(render_node(d, t, size * 0.7)).into_any_element()
            } else {
                row(t)
                    .child(render_node(b, t, size))
                    .child(div().flex().flex_col().justify_between().child(render_node(u, t, size * 0.7)).child(render_node(d, t, size * 0.7)))
                    .into_any_element()
            }
        }
        Node::Frac(a, b) => div()
            .flex()
            .flex_col()
            .items_center()
            .px(px(size * 0.15))
            .child(render_list(a, t, size * 0.9).justify_center())
            .child(div().h(px(1.)).w_full().bg(t.colors.fg).my(px(size * 0.1)))
            .child(render_list(b, t, size * 0.9).justify_center())
            .into_any_element(),
        Node::Binom(a, b) => row(t)
            .child(txt("(").text_size(px(size * 1.8)))
            .child(div().flex().flex_col().items_center().child(render_list(a, t, size * 0.9)).child(render_list(b, t, size * 0.9)))
            .child(txt(")").text_size(px(size * 1.8)))
            .into_any_element(),
        Node::Sqrt(k, v) => row(t)
            .items_end()
            .children(k.as_ref().map(|k| div().mb(px(size * 0.5)).child(render_list(k, t, size * 0.55))))
            .child(txt("√").text_size(px(size * 1.25)))
            .child(render_list(v, t, size).border_t_1().border_color(t.colors.fg).pt(px(1.)))
            .into_any_element(),
        Node::Accent(m, v) => {
            let s = to_text(v);
            if s.chars().count() == 1 {
                txt(&format!("{s}{m}")).italic().into_any_element()
            } else {
                render_list(v, t, size).border_t_1().border_color(t.colors.fg).into_any_element()
            }
        }
        Node::Space(w) => div().w(px(size * w)).into_any_element(),
        Node::Matrix(l, r, rows) => {
            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let cases = l == "{" && r.is_empty();
            let mut grid = div().flex().flex_row().gap(px(size * 0.8));
            for c in 0..cols {
                let mut col = div().flex().flex_col().gap(px(size * 0.25)).when(!cases, |d| d.items_center());
                for rw in rows {
                    col = col.child(rw.get(c).map(|cell| render_list(cell, t, size).into_any_element()).unwrap_or_else(|| div().h(px(size)).into_any_element()));
                }
                grid = grid.child(col);
            }
            let h = (rows.len().max(1) as f32) * size * 1.4;
            let delim = |s: &str| (!s.is_empty()).then(|| div().text_size(px(h.min(size * 4.))).line_height(px(h)).child(s.to_string()));
            row(t).children(delim(l)).child(grid).children(delim(r)).into_any_element()
        }
        Node::Unknown(s) => txt(s).text_color(t.colors.code_attr).into_any_element(),
    }
}

/// Rumus blok (port MathBlock tanpa ekspor gambar): tata letak + gaya.
pub fn render_block(latex: &str, t: &Theme) -> AnyElement {
    let nodes = parse(latex);
    let size = f32::from(t.rem(1.)) * 1.15;
    div()
        .id(gpui::ElementId::Name(format!("math-{:x}", super::rich::block_key(&["math", latex])).into()))
        .w_full()
        .overflow_x_scroll()
        .py(t.space(2))
        .flex()
        .justify_center()
        .font_family(t.font_ui.clone())
        .child(render_list(&nodes, t, size).justify_center().font_style_math())
        .into_any_element()
}

trait MathStyle {
    fn font_style_math(self) -> Self;
}
impl MathStyle for gpui::Div {
    fn font_style_math(self) -> Self {
        // Cambria Math (Windows) bila ada; jatuh ke huruf UI
        self.font_family("Cambria Math")
    }
}

/// Teks rumus sebaris (Unicode).
pub fn inline_text(latex: &str) -> String {
    to_text(&parse(latex)).trim().to_string()
}


// ---------- MathML (port "Salin MathML" KaTeX; juga sumber gambar rumus) ----------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn ml_list(nodes: &[Node]) -> String {
    let inner: String = nodes.iter().map(ml_node).collect();
    if nodes.len() == 1 { inner } else { format!("<mrow>{inner}</mrow>") }
}

fn ml_node(n: &Node) -> String {
    match n {
        Node::Sym(s, italic) => {
            let c = s.chars().next().unwrap_or(' ');
            if c.is_ascii_digit() || c == '.' {
                format!("<mn>{}</mn>", esc(s))
            } else if *italic || c.is_alphabetic() {
                format!("<mi>{}</mi>", esc(s))
            } else {
                format!("<mo>{}</mo>", esc(s))
            }
        }
        Node::Text(s) if s == "\n" => "<mspace linebreak=\"newline\"/>".into(),
        Node::Text(s) => {
            if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.') {
                format!("<mn>{}</mn>", esc(s))
            } else {
                format!("<mtext>{}</mtext>", esc(s))
            }
        }
        Node::BigOp(s) => {
            if s.chars().count() == 1 { format!("<mo>{}</mo>", esc(s)) } else { format!("<mi>{}</mi>", esc(s)) }
        }
        Node::Bold(v) => format!("<mstyle mathvariant=\"bold\">{}</mstyle>", ml_list(v)),
        Node::Group(v) => format!("<mrow>{}</mrow>", v.iter().map(ml_node).collect::<String>()),
        Node::Sup(b, s) => format!("<msup>{}{}</msup>", ml_node(b), ml_node(s)),
        Node::Sub(b, s) => {
            let tag = if matches!(**b, Node::BigOp(_)) { "munder" } else { "msub" };
            format!("<{tag}>{}{}</{tag}>", ml_node(b), ml_node(s))
        }
        Node::SubSup(b, d, u) => {
            let tag = if matches!(**b, Node::BigOp(ref s) if s.chars().count() == 1 && s != "∫" && s != "∮") { "munderover" } else { "msubsup" };
            format!("<{tag}>{}{}{}</{tag}>", ml_node(b), ml_node(d), ml_node(u))
        }
        Node::Frac(a, b) => format!("<mfrac>{}{}</mfrac>", ml_list(a), ml_list(b)),
        Node::Binom(a, b) => format!("<mrow><mo>(</mo><mfrac linethickness=\"0\">{}{}</mfrac><mo>)</mo></mrow>", ml_list(a), ml_list(b)),
        Node::Sqrt(None, v) => format!("<msqrt>{}</msqrt>", ml_list(v)),
        Node::Sqrt(Some(k), v) => format!("<mroot>{}{}</mroot>", ml_list(v), ml_list(k)),
        Node::Accent(m, v) => format!("<mover accent=\"true\">{}<mo>{}</mo></mover>", ml_list(v), esc(&m.to_string())),
        Node::Space(w) => format!("<mspace width=\"{w}em\"/>"),
        Node::Matrix(l, r, rows) => {
            let body: String = rows.iter().map(|rw| format!("<mtr>{}</mtr>", rw.iter().map(|c| format!("<mtd>{}</mtd>", ml_list(c))).collect::<String>())).collect();
            let d = |s: &str| if s.is_empty() { String::new() } else { format!("<mo>{}</mo>", esc(s)) };
            format!("<mrow>{}<mtable>{body}</mtable>{}</mrow>", d(l), d(r))
        }
        Node::Unknown(s) => format!("<merror><mtext>{}</mtext></merror>", esc(s)),
    }
}

/// MathML lengkap untuk LaTeX (seperti elemen `<math>` KaTeX, dengan anotasi sumber).
pub fn to_mathml(latex: &str, display: bool) -> String {
    let nodes = parse(latex);
    format!(
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"{}><semantics><mrow>{}</mrow><annotation encoding=\"application/x-tex\">{}</annotation></semantics></math>",
        if display { " display=\"block\"" } else { "" },
        nodes.iter().map(ml_node).collect::<String>(),
        esc(latex)
    )
}

#[allow(dead_code)]
fn _unused(_: FontStyle) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sebaris() {
        assert_eq!(inline_text("x^2 + y_1"), "x² + y₁");
        assert_eq!(inline_text("\\alpha \\leq \\beta"), "α ≤ β");
        assert_eq!(inline_text("\\frac{a+b}{2}"), "(a + b)⁄2");
        assert_eq!(inline_text("\\sqrt{x}"), "√x");
        assert_eq!(inline_text("\\mathbb{R}^n"), "ℝⁿ");
        assert_eq!(inline_text("e^{i\\pi}"), "e^(iπ)");
        assert_eq!(inline_text("\\sum_{i=1}^{n} i"), "∑ᵢ₌₁ⁿ i");
        assert_eq!(inline_text("\\foo x"), "\\foo x");
    }
    #[test]
    fn mathml() {
        let m = to_mathml(r"\frac{a}{2} + \sqrt{x}", true);
        assert!(m.starts_with("<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">"));
        assert!(m.contains("<mfrac><mi>a</mi><mn>2</mn></mfrac>"), "{m}");
        assert!(m.contains("<msqrt><mi>x</mi></msqrt>"), "{m}");
        assert!(m.contains("<annotation encoding=\"application/x-tex\">"));
        assert!(to_mathml("x < y", false).contains("&lt;"));
    }
    #[test]
    fn struktur() {
        assert!(matches!(parse("\\frac{1}{2}")[0], Node::Frac(_, _)));
        let m = parse("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}");
        match &m[0] {
            Node::Matrix(l, r, rows) => {
                assert_eq!((l.as_str(), r.as_str()), ("(", ")"));
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 2);
            }
            o => panic!("{o:?}"),
        }
        assert!(matches!(parse("\\left( x \\right)")[0], Node::Text(_)));
        // input rusak tidak macet/panic
        let _ = parse("\\frac{a");
        let _ = parse("x^");
        let _ = parse("\\begin{matrix} a & b");
        let _ = parse("}}{{\\\\");
    }
}
