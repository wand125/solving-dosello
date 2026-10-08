//! Read-only DSBOOK02/03 match book. Only fully covered, proven best moves play.
use crate::{
    board::{Move, Position},
    book::{key, transform, transform_move, Key},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub struct MatchBook {
    proven: BTreeMap<Key, Move>,
    values: BTreeMap<Key, i8>,
}
impl MatchBook {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let version = bytes.get(..8).ok_or("truncated book")?;
        if version == b"DSTOOL01" {
            if bytes.len() < 12 {
                return Err("truncated tool book".into());
            }
            let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
            if n > 1000000 || bytes.len() != 12 + n * 36 {
                return Err("tool size".into());
            }
            let mut out = Self::default();
            for r in bytes[12..].chunks_exact(36) {
                let bits = |i| u64::from_le_bytes(r[i..i + 8].try_into().unwrap());
                let p = Position {
                    black: bits(0),
                    white: bits(8),
                    hleft: bits(16),
                    vtop: bits(24),
                    side: r[32] as i8,
                }
                .validate()?;
                if crate::book::canonical(p) != p || !(-64..=64).contains(&(r[35] as i8)) {
                    return Err("invalid tool entry".into());
                }
                let m = p
                    .moves()
                    .into_iter()
                    .find(|m| m.a == r[33] && m.b == r[34])
                    .ok_or("illegal tool move")?;
                if out.proven.insert(key(p), m).is_some() {
                    return Err("duplicate tool position".into());
                }
                out.values.insert(key(p), r[35] as i8);
            }
            return Ok(out);
        }
        if version != b"DSBOOK02" && version != b"DSBOOK03" {
            return Err("expected DSBOOK02/03 book".into());
        }
        let count = u32::from_le_bytes(
            bytes
                .get(8..12)
                .ok_or("truncated book")?
                .try_into()
                .unwrap(),
        );
        if count > 100000 {
            return Err("book count exceeds limit".into());
        }
        let mut at = 12;
        let mut out = Self::default();
        let mut positions = BTreeSet::new();
        for _ in 0..count {
            let header = if version == b"DSBOOK03" { 37 } else { 34 };
            let row = bytes
                .get(at..at + header)
                .ok_or("truncated book position")?;
            at += header;
            let bits = |i| u64::from_le_bytes(row[i..i + 8].try_into().unwrap());
            let p = Position {
                black: bits(0),
                white: bits(8),
                hleft: bits(16),
                vtop: bits(24),
                side: row[32] as i8,
            }
            .validate()?;
            if !positions.insert(key(p)) {
                return Err("duplicate book position".into());
            }
            let n = row[33] as usize;
            if n > 112 {
                return Err("book move count".into());
            }
            if header == 37 {
                let (lo, hi, v) = (row[34] as i8, row[35] as i8, row[36] as i8);
                if lo < -64 || hi > 64 || lo > hi || v < lo || v > hi {
                    return Err("invalid position proof".into());
                }
            }
            let legal = p.moves();
            let mut seen = BTreeSet::new();
            let mut candidates = Vec::new();
            let mut upper = -64;
            for _ in 0..n {
                let r = bytes.get(at..at + 7).ok_or("truncated book move")?;
                at += 7;
                let (v, lo, hi, exact) = (r[2] as i8, r[3] as i8, r[4] as i8, r[6]);
                let m = legal
                    .iter()
                    .find(|m| m.a == r[0] && m.b == r[1])
                    .ok_or("illegal book move")?;
                if !seen.insert(m.id())
                    || lo < -64
                    || hi > 64
                    || lo > hi
                    || !(-64..=64).contains(&v)
                    || exact > 1
                    || exact == 1 && (lo != hi || v != lo)
                {
                    return Err("invalid book move proof".into());
                }
                upper = upper.max(hi);
                if exact == 1 {
                    candidates.push((*m, v));
                }
            }
            // A partial entry cannot prove optimality against omitted legal moves.
            if seen.len() == legal.len() {
                if let Some((m, value)) = candidates
                    .into_iter()
                    .filter(|(_, v)| *v >= upper)
                    .min_by_key(|(m, _)| m.id())
                {
                    out.proven.insert(key(p), m);
                    out.values.insert(key(p), value);
                }
            }
        }
        if at != bytes.len() {
            return Err("trailing book data".into());
        }
        Ok(out)
    }
    /// Export only entries for which decode verified complete legal-move coverage.
    pub fn encode_tool(&self) -> Result<Vec<u8>, String> {
        let mut rows = BTreeMap::new();
        for (k, m) in &self.proven {
            let p = Position {
                black: k.0,
                white: k.1,
                hleft: k.2,
                vtop: k.3,
                side: k.4,
            };
            let q = crate::book::canonical(p);
            let t = (0..8).find(|t| transform(p, *t) == q).unwrap();
            let m = transform_move(*m, t);
            let v = self.values[k];
            if let Some((_, old)) = rows.insert(key(q), (m, v)) {
                if old != v {
                    return Err("conflicting proof values".into());
                }
            }
        }
        let mut out = b"DSTOOL01".to_vec();
        out.extend((rows.len() as u32).to_le_bytes());
        for (k, (m, v)) in rows {
            for b in [k.0, k.1, k.2, k.3] {
                out.extend(b.to_le_bytes())
            }
            out.extend([k.4 as u8, m.a, m.b, v as u8]);
        }
        Ok(out)
    }
    pub fn best(&self, p: Position) -> Option<Move> {
        // Exported small books include symmetries; this also handles sparse books.
        for t in 0..8 {
            if let Some(m) = self.proven.get(&key(transform(p, t))) {
                return p
                    .moves()
                    .into_iter()
                    .find(|candidate| transform_move(*candidate, t).id() == m.id());
            }
        }
        None
    }
    pub fn value(&self, p: Position) -> Option<i32> {
        (0..8).find_map(|t| self.values.get(&key(transform(p,t))).map(|v| *v as i32))
    }
    pub fn len(&self) -> usize {
        self.proven.len()
    }
    pub fn is_empty(&self) -> bool {
        self.proven.is_empty()
    }
}
