//! Standalone quantized pairing tables. No call to the old evaluator.
use crate::{board::*, pattern};
pub const PHASES: usize = 15;
pub const TABLE: usize = 15625;
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
pub(crate) const MAPS: [[[u8; 6]; 8]; 4] = maps();
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

// Integer numerators, all divided by 16. Bias and remaining-placement parity
// deliberately need not be antisymmetric under pass: tempo has value.
#[inline]
fn global_feature(p: Position, i: usize) -> i32 {
    let own = p.own();
    let opp = p.opp();
    let e = p.empty();
    let diff = |mask: u64| (own & mask).count_ones() as i32 - (opp & mask).count_ones() as i32;
    match i {
        0 => 16,
        1 => {
            if e.count_ones() / 2 % 2 == 0 {
                16
            } else {
                -16
            }
        }
        2 => diff(!0),
        3 => diff(0x8100000000000081),
        4 => diff(adjacent(e)),
        5 => (e & adjacent(opp)).count_ones() as i32 - (e & adjacent(own)).count_ones() as i32,
        6 => placements(e) / 2,
        7 => 16 * p.side as i32,
        _ => unreachable!(),
    }
}
pub fn global_numerators(p: Position) -> [i32; GLOBAL] {
    std::array::from_fn(|i| global_feature(p, i))
}
pub fn globals(p: Position) -> [f32; GLOBAL] {
    global_numerators(p).map(|x| x as f32 / 16.)
}
#[derive(Clone, Copy)]
struct Link {
    instance: u8,
    power: i16,
}
const EMPTY_LINK: Link = Link {
    instance: 0,
    power: 0,
};
const fn links() -> ([[Link; INSTANCES]; 64], [usize; 64]) {
    let mut out = [[EMPTY_LINK; INSTANCES]; 64];
    let mut sizes = [0; 64];
    let mut k = 0;
    while k < 4 {
        let mut t = 0;
        while t < 8 {
            let mut j = 0;
            while j < 6 {
                let a = MAPS[k][t][j] as usize;
                let mut power = 1;
                let mut n = j + 1;
                while n < 6 {
                    power *= 5;
                    n += 1;
                }
                out[a][sizes[a]] = Link {
                    instance: (k * 8 + t) as u8,
                    power,
                };
                sizes[a] += 1;
                j += 1;
            }
            t += 1;
        }
        k += 1;
    }
    (out, sizes)
}
const LINKS: ([[Link; INSTANCES]; 64], [usize; 64]) = links();
const fn swaps() -> [u16; TABLE] {
    let mut out = [0; TABLE];
    let mut i = 0;
    while i < TABLE {
        let mut n = i;
        let mut v = 0;
        let mut power = 1;
        let mut j = 0;
        while j < 6 {
            let x = n % 5;
            n /= 5;
            v += if x == 0 {
                0
            } else if x <= 2 {
                (x + 2) * power
            } else {
                (x - 2) * power
            };
            power *= 5;
            j += 1;
        }
        out[i] = v as u16;
        i += 1;
    }
    out
}
static SWAP: [u16; TABLE] = swaps();
fn state(p: Position, a: usize, rotated: bool) -> i32 {
    let b = 1u64 << a;
    if (p.black | p.white) & b == 0 {
        0
    } else {
        1 + 2 * i32::from(p.white & b != 0)
            + i32::from((((p.hleft | (p.hleft << 1)) & b) == 0) ^ rotated)
    }
}
/// Absolute-colour indices avoid rewriting every pattern on a pass.
/// Search uses copy/make with stack restoration (including aborted searches).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub position: Position,
    pub indices: [u16; INSTANCES],
}
impl State {
    pub fn new(p: Position) -> Self {
        let absolute = indices(Position { side: 1, ..p });
        Self {
            position: p,
            indices: std::array::from_fn(|i| (absolute[i] - (i / 8) * TABLE) as u16),
        }
    }
    pub fn update(&mut self, p: Position) {
        let old = self.position;
        let mut changed = (old.black ^ p.black)
            | (old.white ^ p.white)
            | (old.hleft ^ p.hleft)
            | ((old.hleft ^ p.hleft) << 1);
        while changed != 0 {
            let a = changed.trailing_zeros() as usize;
            changed &= changed - 1;
            let delta = [
                state(p, a, false) - state(old, a, false),
                state(p, a, true) - state(old, a, true),
            ];
            for link in &LINKS.0[a][..LINKS.1[a]] {
                let i = link.instance as usize;
                self.indices[i] =
                    (self.indices[i] as i32 + delta[i % 2] * link.power as i32) as u16;
            }
        }
        self.position = p;
    }
}
#[derive(Clone)]
pub struct Model {
    pub weights: Vec<i16>,
    pub globals_mask: u8,
}
impl Model {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.weights.len(), PHASES * FEATURES);
        let mut b = b"DSEVAL04".to_vec();
        b.extend((FEATURES as u32).to_le_bytes());
        b.extend((PHASES as u32).to_le_bytes());
        b.extend((self.globals_mask as u32).to_le_bytes());
        for w in &self.weights {
            b.extend(w.to_le_bytes());
        }
        b
    }
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() != 20 + 2 * PHASES * FEATURES
            || &b[..8] != b"DSEVAL04"
            || u32::from_le_bytes(b[8..12].try_into().unwrap()) != FEATURES as u32
            || u32::from_le_bytes(b[12..16].try_into().unwrap()) != PHASES as u32
            || u32::from_le_bytes(b[16..20].try_into().unwrap()) > 255
        {
            return Err("eval3 schema mismatch".into());
        }
        Ok(Self {
            globals_mask: b[16],
            weights: b[20..]
                .chunks_exact(2)
                .map(|x| i16::from_le_bytes([x[0], x[1]]))
                .collect(),
        })
    }
    pub fn evaluate_state(&self, s: &State) -> i32 {
        let p = s.position;
        let base = pattern::phase(p) * FEATURES;
        let mut sum = 0i32;
        for (i, &raw) in s.indices.iter().enumerate() {
            let id = if p.side == 1 { raw } else { SWAP[raw as usize] };
            sum += self.weights[base + i / 8 * TABLE + id as usize] as i32;
        }
        let mut score = 16 * sum;
        for i in 0..GLOBAL {
            if self.globals_mask & (1 << i) != 0 {
                score += global_feature(p, i) * self.weights[base + 4 * TABLE + i] as i32;
            }
        }
        // Round away from zero, exactly and independently of symmetry order.
        ((score + score.signum() * 2048) / 4096).clamp(-63, 63)
    }
    pub fn evaluate(&self, p: Position) -> i32 {
        self.evaluate_state(&State::new(p))
    }
}
