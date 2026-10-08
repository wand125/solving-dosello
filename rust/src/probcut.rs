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

/// Calibration is bound to the complete encoded DSEVAL04 model, not its filename.
#[derive(Clone)]
pub struct Table {
    revision: u64,
    rows: Vec<Calibration>,
}
pub fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14695981039346656037, |h, b| {
        (h ^ *b as u64).wrapping_mul(1099511628211)
    })
}
impl Table {
    pub fn decode(csv: &str, model: &crate::eval3::Model) -> Result<Self, String> {
        let revision = checksum(&model.encode());
        let mut found = false;
        let mut rows = Vec::new();
        let mut keys = std::collections::BTreeSet::new();
        for line in csv.lines().map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(header) = line.strip_prefix("# evalRevision=") {
                let id = u64::from_str_radix(header.split(';').next().unwrap().trim(), 16)
                    .map_err(|_| "invalid ProbCut checksum")?;
                if found || id != revision {
                    return Err("ProbCut model checksum mismatch or duplicate header".into());
                }
                found = true;
            } else if !line.starts_with('#') {
                let v: Vec<_> = line.split(',').map(str::trim).collect();
                if v.len() != 7 {
                    return Err("invalid ProbCut row".into());
                }
                let c = Calibration {
                    phase: v[0].parse().map_err(|_| "ProbCut phase")?,
                    shallow: v[1].parse().map_err(|_| "ProbCut shallow")?,
                    deep: v[2].parse().map_err(|_| "ProbCut deep")?,
                    slope: v[3].parse().map_err(|_| "ProbCut slope")?,
                    intercept: v[4].parse().map_err(|_| "ProbCut intercept")?,
                    sigma: v[5].parse().map_err(|_| "ProbCut sigma")?,
                    n: v[6].parse().map_err(|_| "ProbCut n")?,
                };
                if c.phase >= crate::pattern::PHASES
                    || c.shallow == 0
                    || c.shallow >= c.deep
                    || c.deep > 28
                    || !c.slope.is_finite()
                    || !c.intercept.is_finite()
                    || !c.sigma.is_finite()
                    || c.sigma < 0.
                    || !keys.insert((c.phase, c.shallow, c.deep))
                {
                    return Err("invalid/duplicate ProbCut coefficients".into());
                }
                rows.push(c);
            }
        }
        if !found {
            return Err("missing ProbCut model checksum".into());
        }
        Ok(Self { revision, rows })
    }
    pub fn matches(&self, model: &crate::eval3::Model) -> bool {
        self.revision == checksum(&model.encode())
    }
    pub fn rows(&self) -> &[Calibration] {
        &self.rows
    }
}
#[derive(Clone)]
pub struct Config {
    pub table: Option<std::sync::Arc<Table>>,
    pub confidence: f64,
    pub ordering: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            table: None,
            confidence: 5.,
            ordering: false,
        }
    }
}
