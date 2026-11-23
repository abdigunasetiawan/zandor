//! Agent Client Protocol (ACP): menjalankan agent CLI resmi (Claude Code, Codex, Antigravity) yang sudah login
//! dengan langganan pengguna, di samping engine OpenCode. Pola diambil dari t3code (MIT): Zandor tidak menjual atau
//! memegang token; ia mengendalikan agent yang sudah login di mesin pengguna. Lihat ADR-083.
pub mod agents;
pub mod convert;
pub mod host;
pub mod rpc;

pub use agents::is_acp_provider;
pub use host::host;
