//! DSDATA02: 8-byte magic then 40-byte LE records (4 u64 boards, i8 side,
//! i8 value, u8 search depth, u8 source: 0 exact / 1 legacy search / 2 new search,
//! u32 split group). No symmetry duplicates are intentionally generated.
use crate::{board::Position, book};
use std::io::{Read, Write};
#[derive(Clone, Copy)]
pub struct Sample {
    pub p: Position,
    pub value: i8,
    pub depth: u8,
    pub source: u8,
    pub group: u32,
}
pub fn group(p: Position) -> u32 {
    let mut h = book::canonical(p).hash();
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58476d1ce4e5b9);
    h ^= h >> 27;
    h as u32
}
pub fn write(mut w: impl Write, s: Sample) -> std::io::Result<()> {
    for n in [s.p.black, s.p.white, s.p.hleft, s.p.vtop] {
        w.write_all(&n.to_le_bytes())?;
    }
    w.write_all(&[s.p.side as u8, s.value as u8, s.depth, s.source])?;
    w.write_all(&s.group.to_le_bytes())
}
pub fn read(path: &str) -> Result<Vec<Sample>, String> {
    let mut b = vec![];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_to_end(&mut b))
        .map_err(|e| e.to_string())?;
    if b.len() < 8 || &b[..8] != b"DSDATA02" || (b.len() - 8) % 40 != 0 {
        return Err("Invalid dataset".into());
    }
    b[8..]
        .chunks_exact(40)
        .map(|r| {
            let n = |i| u64::from_le_bytes(r[i..i + 8].try_into().unwrap());
            let p = Position {
                black: n(0),
                white: n(8),
                hleft: n(16),
                vtop: n(24),
                side: r[32] as i8,
            }
            .validate()?;
            if r[35] > 2 || !(-64..=64).contains(&(r[33] as i8)) {
                return Err("Invalid target".into());
            }
            Ok(Sample {
                p,
                value: r[33] as i8,
                depth: r[34],
                source: r[35],
                group: u32::from_le_bytes(r[36..40].try_into().unwrap()),
            })
        })
        .collect()
}
pub fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
