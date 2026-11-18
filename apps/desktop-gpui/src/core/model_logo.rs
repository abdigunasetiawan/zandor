//! Port packages/ui/src/atoms/ModelLogo.tsx (deteksi): logo dipilih dari keluarga model dulu, lalu provider, lalu nama
//! tampilan. Data logo (lobehub icons, MIT) diekspor oleh scripts/export-model-logos-gpui.ts ke assets/model-logos.json.
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

static DATA: OnceLock<(HashMap<String, (String, Vec<(String, bool)>)>, HashMap<String, String>)> = OnceLock::new();

fn data() -> &'static (HashMap<String, (String, Vec<(String, bool)>)>, HashMap<String, String>) {
    DATA.get_or_init(|| {
        let v: Value = serde_json::from_str(include_str!("../../assets/model-logos.json")).unwrap_or_default();
        let mono = v
            .get("mono")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .map(|(k, x)| {
                        let vb = x.get("v").and_then(Value::as_str).unwrap_or("0 0 24 24").to_string();
                        let paths = x
                            .get("p")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().map(|p| (p.get("d").and_then(Value::as_str).unwrap_or("").to_string(), p.get("e").and_then(Value::as_bool).unwrap_or(false))).collect())
                            .unwrap_or_default();
                        (k.clone(), (vb, paths))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let color = v.get("color").and_then(Value::as_object).map(|o| o.iter().filter_map(|(k, x)| x.as_str().map(|s| (k.clone(), s.to_string()))).collect()).unwrap_or_default();
        (mono, color)
    })
}

fn has(key: &str) -> bool {
    data().0.contains_key(key)
}

/// MODEL_PATTERNS (urutan penting). `(?<!o)llama` dicek manual karena regex Rust tanpa lookbehind.
fn model_family(t: &str) -> Option<&'static str> {
    let re = |p: &str| regex::Regex::new(p).expect("regex").is_match(t);
    let llama = t.match_indices("llama").any(|(i, _)| i == 0 || t.as_bytes()[i - 1] != b'o') || t.contains("llava");
    let pats: [(&dyn Fn() -> bool, &str); 31] = [
        (&|| t.contains("gpt-oss"), "openai"),
        (&|| t.contains("codex"), "codex"),
        (&|| re(r"claude|\bhaiku\b|\bopus\b|\bsonnet\b"), "claude"),
        (&|| re(r"gpt|chatgpt|(^|[/:-])o[1-9](-|$)|davinci|whisper|dall-?e"), "openai"),
        (&|| t.contains("gemma"), "gemma"),
        (&|| re(r"gemini|bard|palm"), "gemini"),
        (&|| t.contains("deepseek"), "deepseek"),
        (&|| re(r"qwen|qwq|qvq"), "qwen"),
        (&|| llama, "meta"),
        (&|| re(r"mistral|mixtral|codestral|ministral|magistral|devstral|pixtral"), "mistral"),
        (&|| t.contains("grok"), "grok"),
        (&|| re(r"glm|chatglm"), "zhipu"),
        (&|| t.contains("kimi"), "kimi"),
        (&|| t.contains("moonshot"), "moonshot"),
        (&|| re(r"minimax|abab"), "minimax"),
        (&|| re(r"command|cohere|\baya\b"), "cohere"),
        (&|| re(r"sonar|perplexity"), "perplexity"),
        (&|| re(r"phi-?\d|\bphi\b"), "microsoft"),
        (&|| re(r"ernie|wenxin"), "wenxin"),
        (&|| re(r"doubao|seed-"), "doubao"),
        (&|| t.contains("hunyuan"), "hunyuan"),
        (&|| t.contains("step-"), "stepfun"),
        (&|| t.contains("nemotron"), "nvidia"),
        (&|| t.contains("granite"), "ibm"),
        (&|| t.contains("internlm"), "internlm"),
        (&|| t.contains("baichuan"), "baichuan"),
        (&|| t.contains("spark"), "spark"),
        (&|| t.contains("dbrx"), "dbrx"),
        (&|| t.contains("hermes"), "nousresearch"),
        (&|| t.contains("jamba"), "ai21"),
        (&|| t.contains("nova-"), "nova"),
    ];
    pats.iter().find(|(f, k)| f() && has(k)).map(|(_, k)| *k)
}

const ALIASES: [(&str, &str); 33] = [
    ("googlevertex", "vertexai"),
    ("googleaistudio", "aistudio"),
    ("amazonbedrock", "bedrock"),
    ("zhipuai", "zhipu"),
    ("zhipuaicodingplan", "zhipu"),
    ("zai", "zai"),
    ("zaicodingplan", "zai"),
    ("moonshotai", "moonshot"),
    ("moonshotaicn", "moonshot"),
    ("kimiforcoding", "kimi"),
    ("xai", "xai"),
    ("togetherai", "together"),
    ("fireworksai", "fireworks"),
    ("deepinfra", "deepinfra"),
    ("githubmodels", "github"),
    ("githubcopilot", "githubcopilot"),
    ("cloudflareworkersai", "workersai"),
    ("cloudflareaigateway", "cloudflare"),
    ("opencodezen", "opencode"),
    ("azurecognitiveservices", "azure"),
    ("huggingface", "huggingface"),
    ("lmstudio", "lmstudio"),
    ("llamacpp", "meta"),
    ("alibabacn", "alibabacloud"),
    ("alibaba", "alibabacloud"),
    ("dashscope", "bailian"),
    ("siliconflow", "siliconcloud"),
    ("volcengine", "volcengine"),
    ("minimaxcn", "minimax"),
    ("minimaxcodingplan", "minimax"),
    ("nvidia", "nvidia"),
    ("stepfun", "stepfun"),
    ("", ""),
];

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

pub fn detect_logo_key(text: Option<&str>) -> Option<String> {
    let t = text.unwrap_or("").to_lowercase();
    if t.is_empty() {
        return None;
    }
    let n = norm(&t);
    let mut hits: Vec<&String> = data().0.keys().filter(|k| k.len() >= 5 && n.contains(k.as_str())).collect();
    hits.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    if let Some(h) = hits.first() {
        return Some((*h).clone());
    }
    model_family(&t).map(str::to_string)
}

pub fn model_logo_key(provider: Option<&str>, model: Option<&str>, name: Option<&str>) -> Option<String> {
    let m = model.unwrap_or("").to_lowercase();
    if !m.is_empty() {
        if let Some(k) = model_family(&m) {
            return Some(k.into());
        }
    }
    let p = norm(provider.unwrap_or(""));
    if !p.is_empty() {
        if let Some((_, a)) = ALIASES.iter().find(|(k, _)| !k.is_empty() && *k == p) {
            if has(a) {
                return Some(a.to_string());
            }
        }
        if has(&p) {
            return Some(p);
        }
        let trimmed = regex::Regex::new(r"(ai|cn|api)$").expect("regex").replace(&p, "").to_string();
        if !trimmed.is_empty() && has(&trimmed) {
            return Some(trimmed);
        }
    }
    detect_logo_key(name)
}

thread_local! {
    static CACHE: std::cell::RefCell<HashMap<String, Arc<gpui::Image>>> = Default::default();
}

fn cached(key: String, make: impl FnOnce() -> Option<Vec<u8>>) -> Option<Arc<gpui::Image>> {
    if let Some(i) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(i);
    }
    let img = Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, make()?));
    CACHE.with(|c| c.borrow_mut().insert(key, img.clone()));
    Some(img)
}

/// Gambar logo: varian berwarna bila ada, selain itu monokrom diwarnai `color` (pengganti currentColor).
pub fn logo_image(key: &str, color: gpui::Hsla) -> Option<Arc<gpui::Image>> {
    if let Some(svg) = data().1.get(key) {
        return cached(format!("c:{key}"), || Some(svg.clone().into_bytes()));
    }
    let (vb, paths) = data().0.get(key)?;
    let rgb = gpui::Rgba::from(color);
    let hex = format!("#{:02x}{:02x}{:02x}", (rgb.r * 255.) as u8, (rgb.g * 255.) as u8, (rgb.b * 255.) as u8);
    cached(format!("m:{key}:{hex}:{}", rgb.a), || {
        let body: String = paths.iter().map(|(d, e)| format!("<path d=\"{d}\"{}/>", if *e { " fill-rule=\"evenodd\"" } else { "" })).collect();
        Some(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vb}\" fill=\"{hex}\" fill-opacity=\"{}\">{body}</svg>", rgb.a).into_bytes())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deteksi() {
        assert_eq!(model_logo_key(Some("my-ollama"), None, Some("My Ollama")).as_deref(), Some("ollama"));
        assert_eq!(model_logo_key(Some("x"), None, Some("LM Studio lokal")).as_deref(), Some("lmstudio"));
        assert_eq!(model_logo_key(Some("x"), None, Some("Qwen Gateway")).as_deref(), Some("qwen"));
        assert_eq!(model_logo_key(Some("x"), Some("llama3.2"), None).as_deref(), Some("meta"));
        assert_eq!(model_logo_key(Some("x"), None, Some("Server Saya")), None);
        assert_eq!(model_logo_key(Some("antigravity"), Some("gemini-3.8-flash"), None).as_deref(), Some("gemini"));
        assert_eq!(model_logo_key(Some("openrouter"), Some("anthropic/claude-sonnet-4"), None).as_deref(), Some("claude"));
        assert_eq!(model_logo_key(Some("opencode"), Some("gpt-5-codex"), None).as_deref(), Some("codex"));
        assert_eq!(model_logo_key(Some("x"), Some("o3-mini"), None).as_deref(), Some("openai"));
        assert_eq!(model_logo_key(Some("x"), Some("Qwen3-Coder"), None).as_deref(), Some("qwen"));
        assert_eq!(model_logo_key(Some("x"), Some("glm-4.6"), None).as_deref(), Some("zhipu"));
        assert_eq!(model_logo_key(Some("openrouter"), Some("model-aneh"), None).as_deref(), Some("openrouter"));
        assert_eq!(model_logo_key(Some("fireworks-ai"), Some("model-aneh"), None).as_deref(), Some("fireworks"));
        assert_eq!(model_logo_key(Some("amazon-bedrock"), Some("x"), None).as_deref(), Some("bedrock"));
        assert_eq!(model_logo_key(Some("github-copilot"), Some("x"), None).as_deref(), Some("githubcopilot"));
        assert_eq!(model_logo_key(Some("anthropic"), None, None).as_deref(), Some("anthropic"));
        assert_eq!(model_logo_key(Some("zzz-tidak-ada"), Some("model-aneh"), None), None);
        assert!(logo_image("anthropic", gpui::black()).is_some());
    }
}
