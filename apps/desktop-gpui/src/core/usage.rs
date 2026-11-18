//! Pemakaian sesi — port packages/core/src/tools/usage.ts (sessionUsage, formatTokens, usageLevel).
use super::types::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionUsage {
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_total: usize,
    pub tool_completed: usize,
    pub tool_failed: usize,
    pub tool_running: usize,
    pub input: f64,
    pub output: f64,
    pub reasoning: f64,
    pub total: f64,
    pub cost: Option<f64>,
    pub context_tokens: Option<f64>,
    pub context_limit: Option<f64>,
    pub context_percent: Option<f64>,
    pub context_remaining: Option<f64>,
    pub output_limit: Option<f64>,
    pub work_ms: f64,
}

pub fn session_usage(messages: &[ChatMessage], context_limit: Option<f64>, output_limit: Option<f64>) -> SessionUsage {
    let mut u = SessionUsage { context_limit, output_limit, ..Default::default() };
    for m in messages {
        if m.info.role == Role::User {
            u.user_messages += 1;
            continue;
        }
        u.assistant_messages += 1;
        if let (Some(c), true) = (m.info.completed, m.info.created > 0.) {
            u.work_ms += (c - m.info.created).max(0.);
        }
        if let Some(k) = &m.info.tokens {
            u.input += k.input;
            u.output += k.output;
            u.reasoning += k.reasoning;
            u.total += k.total;
            if k.input > 0. {
                u.context_tokens = Some(k.input);
            }
        }
        if let Some(c) = m.info.cost {
            u.cost = Some(u.cost.unwrap_or(0.) + c);
        }
        for p in &m.parts {
            if let PartKind::Tool { state, .. } = &p.kind {
                u.tool_total += 1;
                match state.status {
                    ToolStatus::Completed => u.tool_completed += 1,
                    ToolStatus::Error => u.tool_failed += 1,
                    _ => u.tool_running += 1,
                }
            }
        }
    }
    if let (Some(lim), Some(ct)) = (context_limit.filter(|l| *l > 0.), u.context_tokens) {
        u.context_percent = Some((ct / lim * 100.).min(100.));
        u.context_remaining = Some((lim - ct).max(0.));
    }
    u
}

/// 950 → "950", 12400 → "12,4k" (id) / "12.4k" (en), 2_300_000 → "2,3M".
pub fn format_tokens(n: f64, locale_id: bool) -> String {
    let f = |v: f64| {
        let r = (v * 10.).round() / 10.;
        let s = if r.fract() == 0. { format!("{}", r as i64) } else { format!("{r:.1}") };
        if locale_id { s.replace('.', ",") } else { s }
    };
    if n >= 1_000_000. {
        format!("{}M", f(n / 1_000_000.))
    } else if n >= 10_000. {
        format!("{}k", f(n / 1000.))
    } else if n >= 1000. {
        format!("{}k", f((n / 100.).round() / 10.))
    } else {
        format!("{}", n.round() as i64)
    }
}

pub fn usage_level(percent: Option<f64>) -> &'static str {
    match percent {
        Some(p) if p >= 95. => "high",
        Some(p) if p >= 80. => "warn",
        _ => "ok",
    }
}

pub fn duration(ms: f64) -> String {
    let s = (ms / 1000.).round() as i64;
    if s < 60 { format!("{s} dtk") } else { format!("{} mnt {} dtk", s / 60, s % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angka() {
        assert_eq!(format_tokens(950., true), "950");
        assert_eq!(format_tokens(12400., true), "12,4k");
        assert_eq!(format_tokens(2_300_000., false), "2.3M");
        assert_eq!(format_tokens(1234., false), "1.2k");
        assert_eq!(usage_level(Some(81.)), "warn");
        assert_eq!(usage_level(None), "ok");
        assert_eq!(duration(75_000.), "1 mnt 15 dtk");
        let u = session_usage(&[], Some(1000.), None);
        assert_eq!(u.context_percent, None);
    }
}
