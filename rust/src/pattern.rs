//! Phase-dependent ternary patterns. Eight D4 instances share every table.
//! Integer accumulation makes evaluation exactly invariant to rotations/reflections.
use crate::board::*;
pub const PHASES: usize = 15;
pub const SIZES: [usize; 8] = [59049, 19683, 59049, 6561, 6561, 6561, 2187, 729];
pub const GLOBAL: usize = 12;
pub const FEATURES: usize = 160380 + GLOBAL;
pub const INSTANCES: usize = 64;
pub const SCALE: f32 = 256.0;
pub const VERSION: i64 = 2;
const BASE: [[u8; 10]; 8] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 9, 14],
    [0, 1, 2, 8, 9, 10, 16, 17, 18, 0],
    [0, 1, 2, 3, 4, 8, 9, 10, 11, 12],
    [8, 9, 10, 11, 12, 13, 14, 15, 0, 0],
    [16, 17, 18, 19, 20, 21, 22, 23, 0, 0],
    [0, 9, 18, 27, 36, 45, 54, 63, 0, 0],
    [1, 10, 19, 28, 37, 46, 55, 0, 0, 0],
    [2, 11, 20, 29, 38, 47, 0, 0, 0, 0],
];
const LENS: [usize; 8] = [10, 9, 10, 8, 8, 8, 7, 6];
const fn map(a: u8, t: usize) -> u8 {
    let mut r = a / 8;
    let mut c = a % 8;
    if t >= 4 {
        c = 7 - c;
    }
    let mut i = 0;
    while i < t % 4 {
        let z = r;
        r = c;
        c = 7 - z;
        i += 1;
    }
    r * 8 + c
}
const fn maps() -> [[[u8; 10]; 8]; 8] {
    let mut out = [[[0; 10]; 8]; 8];
    let mut k = 0;
    while k < 8 {
        let mut t = 0;
        while t < 8 {
            let mut j = 0;
            while j < LENS[k] {
                out[k][t][j] = map(BASE[k][j], t);
                j += 1;
            }
            t += 1;
        }
        k += 1;
    }
    out
}
const MAPS: [[[u8; 10]; 8]; 8] = maps();
pub fn phase(p: Position) -> usize {
    (p.empty().count_ones() as usize / 4).min(PHASES - 1)
}
pub fn indices(p: Position) -> [usize; INSTANCES] {
    let own = p.own();
    let opp = p.opp();
    let mut out = [0; INSTANCES];
    let mut offset = 0;
    for k in 0..8 {
        for t in 0..8 {
            let mut index = 0;
            for j in 0..LENS[k] {
                let bit = 1u64 << MAPS[k][t][j];
                index = index * 3 + usize::from(own & bit != 0) + 2 * usize::from(opp & bit != 0);
            }
            out[k * 8 + t] = offset + index;
        }
        offset += SIZES[k];
    }
    out
}
pub fn globals(p: Position) -> [f32; GLOBAL] {
    let e = p.empty();
    let own = p.own();
    let opp = p.opp();
    let diff = |m: u64| ((own & m).count_ones() as f32 - (opp & m).count_ones() as f32) / 16.0;
    let live = e & (shift(e, 1) | shift(e, 3) | shift(e, 4) | shift(e, 6));
    let edge_pairs = (p.hleft & 0xff000000000000ff)
        | ((p.hleft & 0xff000000000000ff) << 1)
        | (p.vtop & 0x8181818181818181)
        | ((p.vtop & 0x8181818181818181) << 8);
    let mut remaining = e;
    let mut odd = 0;
    while remaining != 0 {
        let mut region = remaining & remaining.wrapping_neg();
        loop {
            let next = region
                | ((shift(region, 1) | shift(region, 3) | shift(region, 4) | shift(region, 6))
                    & remaining);
            if next == region {
                break;
            }
            region = next;
        }
        odd += (region.count_ones() / 2) % 2;
        remaining &= !region;
    }
    [
        1.0,
        crate::search::hand_evaluate(p, crate::search::Weights::default()) as f32 / 16.0,
        diff(!0),
        (p.mobility() - p.pass().mobility()) as f32 / 16.0,
        diff(0x8100000000000081),
        diff(adjacent(e)),
        diff(edge_pairs),
        placements(e) as f32 / 32.0,
        (e & !live).count_ones() as f32 / 8.0,
        odd as f32 / 8.0,
        (p.stable_edges(own).count_ones() as f32 - p.stable_edges(opp).count_ones() as f32) / 16.0,
        (live.count_ones() / 2 % 2) as f32,
    ]
}
pub static MODEL: &[u8] = include_bytes!("../data/eval.bin");
pub fn validate_model(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != 16 + PHASES * FEATURES * 2 || &bytes[..8] != b"DSEVAL02" {
        return Err("Invalid pattern model size or magic".into());
    }
    if u32::from_le_bytes(bytes[8..12].try_into().unwrap()) != FEATURES as u32
        || u32::from_le_bytes(bytes[12..16].try_into().unwrap()) != PHASES as u32
    {
        return Err("Invalid model schema".into());
    }
    Ok(())
}
#[inline]
fn weight(base: usize, i: usize) -> i32 {
    let at = 16 + (base + i) * 2;
    i16::from_le_bytes([MODEL[at], MODEL[at + 1]]) as i32
}
pub fn evaluate(p: Position) -> i32 {
    let base = phase(p) * FEATURES;
    let mut sum = 0;
    for i in indices(p) {
        sum += weight(base, i);
    }
    let mut score = sum as f32;
    for (i, v) in globals(p).iter().enumerate() {
        score += *v * weight(base, FEATURES - GLOBAL + i) as f32;
    }
    (score / SCALE).round().clamp(-63.0, 63.0) as i32
}
/// Weight-content revision, independent of the binary schema version. A new
/// training run automatically invalidates old heuristic journal leaves.
pub fn revision() -> i64 {
    static REV: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *REV.get_or_init(|| {
        MODEL
            .iter()
            .fold(2166136261u32, |h, b| (h ^ *b as u32).wrapping_mul(16777619)) as i64
    })
}
