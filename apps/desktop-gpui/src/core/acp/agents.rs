//! Agent ACP yang didukung: definisi, konfigurasi pengguna, dan pencarian cara menjalankannya.
//!
//! Zandor tidak memegang kredensial agent. Tiap agent memakai login CLI resminya sendiri (langganan Claude, ChatGPT,
//! atau akun Google). Claude Code dan Codex butuh adaptor ACP resmi dari npm (versi dikunci, dipasang ke folder data
//! Zandor atas permintaan pengguna); Antigravity punya server ACP bawaan (`agy acp`).
use super::rpc::Launch;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct AgentDef {
    /// Kunci stabil (konfigurasi, nama berkas).
    pub id: &'static str,
    /// `provider_id` model di pemilih model (`acp-<id>`).
    pub provider_id: &'static str,
    pub name: &'static str,
    /// CLI resmi yang harus terpasang dan sudah login (untuk petunjuk dan pemeriksaan).
    pub cli: &'static str,
    pub login: &'static str,
    /// Adaptor npm (paket, versi, berkas skrip di dalam paket). None = CLI punya mode ACP sendiri.
    pub adapter: Option<(&'static str, &'static str, &'static str)>,
    /// Argumen untuk CLI ber-ACP bawaan (mis. `agy acp`).
    pub native_args: &'static [&'static str],
    /// Lokasi tambahan biner CLI (relatif ke folder profil pengguna) bila tidak ada di PATH.
    pub home_bins: &'static [&'static str],
    /// Variabel lingkungan kredensial yang dibuang agar agent memakai login langganan, bukan kunci API ambien
    /// (mis. kunci provider OpenCode yang ikut terwarisi). Pengguna bisa memakai kunci lewat login CLI itu sendiri.
    pub strip_env: &'static [&'static str],
}

pub const AGENTS: [AgentDef; 3] = [
    AgentDef {
        id: "claude",
        provider_id: "acp-claude",
        name: "Claude Code",
        cli: "claude",
        login: "claude auth login",
        adapter: Some(("@agentclientprotocol/claude-agent-acp", "0.109.1", "dist/index.js")),
        native_args: &[],
        home_bins: &[".local/bin/claude.exe", ".local/bin/claude"],
        strip_env: &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"],
    },
    AgentDef {
        id: "codex",
        provider_id: "acp-codex",
        name: "Codex",
        cli: "codex",
        login: "codex login",
        adapter: Some(("@agentclientprotocol/codex-acp", "2.9.0", "dist/index.js")),
        native_args: &[],
        home_bins: &["AppData/Local/Programs/OpenAI/Codex/bin/codex.exe"],
        strip_env: &["OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL"],
    },
    AgentDef {
        id: "antigravity",
        provider_id: "acp-antigravity",
        name: "Gemini (Antigravity)",
        cli: "agy",
        login: "agy",
        adapter: None,
        native_args: &["acp"],
        home_bins: &[".gemini/bin/agy.exe", ".gemini/bin/agy"],
        strip_env: &["GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_GENAI_USE_VERTEXAI"],
    },
];

pub fn by_id(id: &str) -> Option<&'static AgentDef> {
    AGENTS.iter().find(|a| a.id == id)
}

pub fn by_provider(provider_id: &str) -> Option<&'static AgentDef> {
    AGENTS.iter().find(|a| a.provider_id == provider_id)
}

pub fn is_acp_provider(provider_id: &str) -> bool {
    by_provider(provider_id).is_some()
}

/// Konfigurasi satu agent (berkas `acp/agents.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentConfig {
    /// Model agent tampil di pemilih model dan bisa dipakai. Bawaan mati sampai pengguna mengaktifkan.
    pub enabled: bool,
    /// Perintah ACP kustom (menggantikan deteksi otomatis), mis. jalur `claude-agent-acp` buatan sendiri.
    pub command: Option<String>,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    pub agents: BTreeMap<String, AgentConfig>,
}

impl AgentsConfig {
    pub fn get(&self, id: &str) -> AgentConfig {
        self.agents.get(id).cloned().unwrap_or_default()
    }
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| e.to_string())
    }
}

/// Mengapa agent belum bisa dijalankan.
#[derive(Clone, Debug, PartialEq)]
pub enum Missing {
    /// CLI resmi tidak ditemukan (pengguna perlu memasang dan login).
    Cli,
    /// Adaptor npm belum dipasang (bisa dipasang dari Pengaturan bila npm ada).
    Adapter { npm: bool },
    /// Node.js tidak ditemukan (adaptor npm butuh node).
    Node,
    /// Perintah kustom tidak ada.
    Command(String),
}

/// Cari program di PATH (Windows: ikut PATHEXT).
pub fn which(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.components().count() > 1 {
        return p.is_file().then(|| p.to_path_buf());
    }
    let exts: Vec<String> = if cfg!(windows) {
        let mut v: Vec<String> = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into()).split(';').filter(|e| !e.is_empty()).map(|e| e.to_lowercase()).collect();
        v.insert(0, String::new());
        v
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for e in &exts {
            let c = dir.join(format!("{name}{e}"));
            // Di Windows berkas tanpa ekstensi (skrip sh dari npm) tidak bisa dijalankan langsung.
            if c.is_file() && (!cfg!(windows) || !e.is_empty() || Path::new(name).extension().is_some()) {
                return Some(c);
            }
        }
    }
    None
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

/// Biner CLI resmi agent (PATH lalu lokasi pemasangan bawaan).
pub fn find_cli(def: &AgentDef) -> Option<PathBuf> {
    which(def.cli).or_else(|| {
        let h = home()?;
        def.home_bins.iter().map(|b| h.join(b)).find(|p| p.is_file())
    })
}

/// Folder pemasangan adaptor milik Zandor: `<data>/acp/tools/<id>@<versi>`.
pub fn adapter_dir(tools: &Path, def: &AgentDef) -> Option<PathBuf> {
    let (_, ver, _) = def.adapter?;
    Some(tools.join(format!("{}@{ver}", def.id)))
}

/// Skrip adaptor yang sudah terpasang (None = belum).
pub fn adapter_script(tools: &Path, def: &AgentDef) -> Option<PathBuf> {
    let (pkg, _, script) = def.adapter?;
    let p = adapter_dir(tools, def)?.join("node_modules").join(pkg).join(script);
    p.is_file().then_some(p)
}

/// Tentukan cara menjalankan agent. `tools` = folder adaptor milik Zandor.
pub fn resolve(def: &AgentDef, cfg: &AgentConfig, tools: &Path) -> Result<Launch, Missing> {
    let env_remove: Vec<String> = def.strip_env.iter().map(|s| s.to_string()).collect();
    if let Some(cmd) = cfg.command.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        let program = which(cmd).ok_or_else(|| Missing::Command(cmd.to_string()))?;
        return Ok(Launch { program, args: cfg.args.clone(), env_remove });
    }
    match def.adapter {
        None => {
            let program = find_cli(def).ok_or(Missing::Cli)?;
            Ok(Launch { program, args: def.native_args.iter().map(|s| s.to_string()).collect(), env_remove })
        }
        Some((pkg, ..)) => {
            // Adaptor tetap butuh CLI yang sudah login (kredensial dan konfigurasi milik CLI itu).
            if find_cli(def).is_none() {
                return Err(Missing::Cli);
            }
            let Some(script) = adapter_script(tools, def) else {
                // Adaptor global yang dipasang pengguna sendiri (`npm i -g ...`).
                let bin = pkg.rsplit('/').next().unwrap_or(pkg);
                if let Some(program) = which(bin) {
                    return Ok(Launch { program, args: vec![], env_remove });
                }
                return Err(Missing::Adapter { npm: which("npm").is_some() });
            };
            let node = which("node").ok_or(Missing::Node)?;
            Ok(Launch { program: node, args: vec![script.display().to_string()], env_remove })
        }
    }
}

/// Pasang adaptor npm ke folder milik Zandor (versi dikunci). Memblokir; panggil dari latar.
pub fn install_adapter(def: &AgentDef, tools: &Path) -> Result<(), String> {
    let (pkg, ver, _) = def.adapter.ok_or("agent ini tidak memakai adaptor")?;
    let dir = adapter_dir(tools, def).ok_or("folder adaptor tidak diketahui")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let npm = which("npm").ok_or("npm tidak ditemukan; pasang Node.js terlebih dahulu")?;
    let launch = Launch {
        program: npm,
        args: vec!["install".into(), "--no-audit".into(), "--no-fund".into(), "--omit=dev".into(), "--prefix".into(), dir.display().to_string(), format!("{pkg}@{ver}")],
        env_remove: vec![],
    };
    let out = super::rpc::command(&launch).current_dir(&dir).output().map_err(|e| format!("gagal menjalankan npm: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).rev().take(4).collect();
        return Err(format!("npm gagal: {}", last.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    if adapter_script(tools, def).is_none() {
        return Err("npm selesai, tetapi berkas adaptor tidak ditemukan".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_provider_unik_dan_terdeteksi() {
        for a in &AGENTS {
            assert!(a.provider_id.starts_with("acp-"));
            assert!(is_acp_provider(a.provider_id));
            assert_eq!(by_id(a.id).unwrap().provider_id, a.provider_id);
        }
        assert!(!is_acp_provider("anthropic"));
        assert!(!is_acp_provider("opencode"));
    }

    #[test]
    fn konfigurasi_bawaan_mati_dan_bertahan() {
        let dir = std::env::temp_dir().join(format!("zandor-acp-cfg-{}", std::process::id()));
        let p = dir.join("agents.json");
        let mut c = AgentsConfig::load(&p);
        assert!(!c.get("claude").enabled);
        c.agents.insert("claude".into(), AgentConfig { enabled: true, ..Default::default() });
        c.save(&p).unwrap();
        assert!(AgentsConfig::load(&p).get("claude").enabled);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn perintah_kustom_tidak_ada() {
        let cfg = AgentConfig { enabled: true, command: Some("zandor-tidak-ada-xyz".into()), args: vec![] };
        assert_eq!(resolve(&AGENTS[0], &cfg, Path::new(".")), Err(Missing::Command("zandor-tidak-ada-xyz".into())));
    }

    #[test]
    fn adaptor_terpasang_dikenali() {
        let tools = std::env::temp_dir().join(format!("zandor-acp-tools-{}", std::process::id()));
        let def = &AGENTS[1];
        assert!(adapter_script(&tools, def).is_none());
        let (pkg, _, script) = def.adapter.unwrap();
        let p = adapter_dir(&tools, def).unwrap().join("node_modules").join(pkg).join(script);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "").unwrap();
        assert_eq!(adapter_script(&tools, def), Some(p));
        let _ = std::fs::remove_dir_all(tools);
    }
}
