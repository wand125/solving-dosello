//! Residual pairing-aware evaluator. Five states in the pattern's own frame:
//! empty, own-H, own-V, opponent-H, opponent-V. D4 instances share tables.
use crate::{board::*, pattern};
pub const PHASES: usize = 15;
pub const TABLE: usize = 15625; // 5^6
pub const GLOBAL: usize = 8;
pub const FEATURES: usize = 4 * TABLE + GLOBAL;
pub const INSTANCES: usize = 32;
const BASE: [[u8; 6]; 4] = [
    [0, 1, 2, 8, 9, 10],
    [0, 1, 2, 3, 4, 5],
    [9, 10, 11, 17, 18, 19],
    [18, 19, 20, 26, 27, 28],
];
const fn cell(a: u8, t: usize) -> u8 {
    let (mut r, mut c) = (a / 8, a % 8);
    if t >= 4 {
        c = 7 - c
    }
    let mut i = 0;
    while i < t % 4 {
        let z = r;
        r = c;
        c = 7 - z;
        i += 1
    }
    r * 8 + c
}
const fn maps() -> [[[u8; 6]; 8]; 4] {
    let mut out = [[[0; 6]; 8]; 4];
    let mut k = 0;
    while k < 4 {
        let mut t = 0;
        while t < 8 {
            let mut j = 0;
            while j < 6 {
                out[k][t][j] = cell(BASE[k][j], t);
                j += 1
            }
            t += 1
        }
        k += 1
    }
    out
}
const MAPS: [[[u8; 6]; 8]; 4] = maps();
pub fn indices(p: Position) -> [usize; INSTANCES] {
    let horizontal = p.hleft | (p.hleft << 1);
    let own = p.own();
    let occupied = p.black | p.white;
    let mut out = [0; INSTANCES];
    for k in 0..4 {
        for t in 0..8 {
            let mut n = 0;
            for a in MAPS[k][t] {
                let bit = 1u64 << a;
                let state = if occupied & bit == 0 {
                    0
                } else {
                    1 + 2 * usize::from(own & bit == 0)
                        + usize::from((horizontal & bit == 0) ^ (t % 2 == 1))
                };
                n = n * 5 + state;
            }
            out[k * 8 + t] = k * TABLE + n;
        }
    }
    out
}
pub fn globals(p: Position) -> [f32; GLOBAL] {
    let e = p.empty();
    let own = p.own();
    let opp = p.opp();
    let starts = p.hleft | p.vtop;
    let diff =
        |mask: u64| ((mask & own).count_ones() as f32 - (mask & opp).count_ones() as f32) / 16.;
    let stable = |color| {
        let cells = p.stable_edges(color);
        cells & p.partners(cells) & starts
    };
    let live = e & (shift(e, 1) | shift(e, 3) | shift(e, 4) | shift(e, 6));
    let mut remain = e;
    let mut odd = 0.;
    while remain != 0 {
        let mut region = remain & remain.wrapping_neg();
        loop {
            let next = region
                | ((shift(region, 1) | shift(region, 3) | shift(region, 4) | shift(region, 6))
                    & remain);
            if next == region {
                break;
            }
            region = next
        }
        odd += (placements(region) % 2) as f32;
        remain &= !region;
    }
    let corners = 0x8100000000000081;
    let edges = 0xff818181818181ff;
    [
        diff(stable(own) | stable(opp)),
        placements(e) as f32 / 32.,
        odd / 8.,
        (e & !live).count_ones() as f32 / 8.,
        (p.mobility() - p.pass().mobility()) as f32 / 16.,
        (placements(e & adjacent(opp)) - placements(e & adjacent(own))) as f32 / 32.,
        diff((corners | p.partners(corners)) & starts),
        diff((edges | p.partners(edges)) & starts),
    ]
}
#[derive(Clone)]
pub struct Model {
    pub weights: Vec<i16>,
}
impl Model {
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() != 24 + 2 * PHASES * FEATURES
            || &b[..8] != b"DSEVAL03"
            || u32::from_le_bytes(b[8..12].try_into().unwrap()) != FEATURES as u32
            || u32::from_le_bytes(b[12..16].try_into().unwrap()) != PHASES as u32
            || i64::from_le_bytes(b[16..24].try_into().unwrap()) != pattern::revision()
        {
            return Err("eval2 schema/base revision mismatch".into());
        }
        Ok(Self {
            weights: b[24..]
                .chunks_exact(2)
                .map(|x| i16::from_le_bytes([x[0], x[1]]))
                .collect(),
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut b = b"DSEVAL03".to_vec();
        b.extend((FEATURES as u32).to_le_bytes());
        b.extend((PHASES as u32).to_le_bytes());
        b.extend(pattern::revision().to_le_bytes());
        for v in &self.weights {
            b.extend(v.to_le_bytes())
        }
        b
    }
    pub fn evaluate(&self, p: Position) -> i32 {
        let base = pattern::phase(p) * FEATURES;
        let sum: i32 = indices(p)
            .iter()
            .map(|i| self.weights[base + i] as i32)
            .sum();
        let g = globals(p);
        let correction = sum as f32
            + g.iter()
                .enumerate()
                .map(|(i, v)| v * self.weights[base + 4 * TABLE + i] as f32)
                .sum::<f32>();
        (pattern::evaluate(p) as f32 + correction / 256.)
            .round()
            .clamp(-63., 63.) as i32
    }
}
