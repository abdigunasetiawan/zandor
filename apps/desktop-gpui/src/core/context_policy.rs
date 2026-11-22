//! Kebijakan konteks: batas konteks yang bisa diatur, ambang auto compact, dan pengecekan muat-tidaknya riwayat
//! sesi ke jendela konteks model baru. Murni (tanpa UI/engine) agar mudah diuji.

/// Batas konteks efektif: `max` (0 = ikut model) membatasi, tidak pernah melampaui batas asli model.
/// Model tanpa batas yang diketahui + `max` > 0 memakai `max`.
pub fn effective_limit(model_limit: Option<u64>, max: u64) -> Option<u64> {
    match (model_limit.filter(|l| *l > 0), max) {
        (l, 0) => l,
        (Some(l), m) => Some(l.min(m)),
        (None, m) => Some(m),
    }
}

/// Auto compact dipicu bila token konteks terakhir mencapai `threshold_pct`% dari batas.
pub fn should_auto_compact(used: Option<f64>, limit: Option<u64>, threshold_pct: f64) -> bool {
    match (used, limit.filter(|l| *l > 0)) {
        (Some(u), Some(l)) => u / l as f64 * 100. >= threshold_pct,
        _ => false,
    }
}

/// Hasil pengecekan saat model diganti di tengah sesi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchFit {
    /// Riwayat masih muat di model baru (di bawah ambang).
    Fits,
    /// Riwayat tidak muat / terlalu mepet di model baru: ringkas dulu (dengan model lama yang lebih besar).
    NeedsCompact,
    /// Batas model baru atau pemakaian sesi belum diketahui: tidak ada yang bisa dinilai.
    Unknown,
}

pub fn switch_fit(used: Option<f64>, new_limit: Option<u64>, threshold_pct: f64) -> SwitchFit {
    match (used, new_limit.filter(|l| *l > 0)) {
        (Some(_), Some(_)) if should_auto_compact(used, new_limit, threshold_pct) => SwitchFit::NeedsCompact,
        (Some(_), Some(_)) => SwitchFit::Fits,
        _ => SwitchFit::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_limit_caps_but_never_raises() {
        assert_eq!(effective_limit(Some(1_000_000), 0), Some(1_000_000));
        assert_eq!(effective_limit(Some(1_000_000), 128_000), Some(128_000));
        assert_eq!(effective_limit(Some(64_000), 128_000), Some(64_000));
        assert_eq!(effective_limit(None, 0), None);
        assert_eq!(effective_limit(None, 128_000), Some(128_000));
        assert_eq!(effective_limit(Some(0), 0), None);
    }

    #[test]
    fn auto_compact_threshold() {
        assert!(should_auto_compact(Some(90_000.), Some(100_000), 90.));
        assert!(!should_auto_compact(Some(89_999.), Some(100_000), 90.));
        assert!(!should_auto_compact(None, Some(100_000), 90.));
        assert!(!should_auto_compact(Some(1.), None, 90.));
        assert!(!should_auto_compact(Some(1.), Some(0), 90.));
    }

    #[test]
    fn switch_to_smaller_model() {
        // 300K di sesi, pindah dari model 1M ke model 128K: harus diringkas dulu.
        assert_eq!(switch_fit(Some(300_000.), Some(128_000), 90.), SwitchFit::NeedsCompact);
        // 50K masih muat.
        assert_eq!(switch_fit(Some(50_000.), Some(128_000), 90.), SwitchFit::Fits);
        // Mepet ambang juga dianggap perlu.
        assert_eq!(switch_fit(Some(120_000.), Some(128_000), 90.), SwitchFit::NeedsCompact);
        assert_eq!(switch_fit(None, Some(128_000), 90.), SwitchFit::Unknown);
        assert_eq!(switch_fit(Some(10.), None, 90.), SwitchFit::Unknown);
    }
}
