//! Empirical deep = slope * shallow + intercept; sigma is residual stddev.
//! Statistical cuts are never proofs. Terminal-depth search disables them.
#[derive(Clone, Copy)]
pub struct Calibration {
    pub phase: usize,
    pub shallow: u8,
    pub deep: u8,
    pub slope: f64,
    pub intercept: f64,
    pub sigma: f64,
    pub n: usize,
}
pub fn models() -> &'static [Calibration] {
    static DATA: std::sync::OnceLock<Vec<Calibration>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        include_str!("../data/probcut.csv")
            .lines()
            .filter(|s| !s.starts_with('#'))
            .filter_map(|s| {
                let v: Vec<_> = s.split(',').collect();
                if v.len() != 7 {
                    return None;
                }
                Some(Calibration {
                    phase: v[0].parse().ok()?,
                    shallow: v[1].parse().ok()?,
                    deep: v[2].parse().ok()?,
                    slope: v[3].parse().ok()?,
                    intercept: v[4].parse().ok()?,
                    sigma: v[5].parse().ok()?,
                    n: v[6].parse().ok()?,
                })
            })
            .collect()
    })
}
