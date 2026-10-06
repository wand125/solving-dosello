//! Durable root proof intervals. Only completed terminal searches are saved;
//! TT contents and heuristic predictions are never persisted as proof bounds.
use super::*;
use std::path::Path;
struct Checkpoint {
    p: Position,
    lo: i32,
    hi: i32,
    children: Vec<(u16, i32, i32)>,
}
fn checksum(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}
impl Checkpoint {
    fn load(p: Position, path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self {
                p,
                lo: -64,
                hi: 64,
                children: vec![],
            });
        }
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let envelope = crate::json::parse(&raw)?;
        let data = envelope.get("data")?.str()?;
        if envelope.get("checksum")?.str()? != format!("{:016x}", checksum(data)) {
            return Err("Proof checkpoint checksum mismatch".into());
        }
        let v = crate::json::parse(data)?;
        if v.get("version")?.num()? != 1 || crate::json::position(v.get("position")?)? != p {
            return Err("Proof checkpoint version/position mismatch".into());
        }
        let bound = |v: &crate::json::Json, k: &str| -> Result<i32, String> {
            let n = v.get(k)?.num()?;
            if !(-64..=64).contains(&n) {
                return Err("Invalid proof bound".into());
            }
            Ok(n as i32)
        };
        let lo = bound(&v, "lower")?;
        let hi = bound(&v, "upper")?;
        if lo > hi {
            return Err("Contradictory root proof".into());
        }
        let legal = p.moves();
        let mut children = vec![];
        for child in v.get("children")?.arr()? {
            let id = child.get("id")?.num()?;
            if !legal.iter().any(|m| m.id() as i64 == id)
                || children.iter().any(|(m, _, _)| *m == id as u16)
            {
                return Err("Invalid checkpoint move".into());
            }
            let a = bound(child, "lower")?;
            let b = bound(child, "upper")?;
            if a > b {
                return Err("Contradictory child proof".into());
            }
            children.push((id as u16, a, b));
        }
        Ok(Self {
            p,
            lo,
            hi,
            children,
        })
    }
    fn save(&self, path: &Path) -> Result<(), String> {
        let children = self
            .children
            .iter()
            .map(|(id, lo, hi)| format!("{{\"id\":{id},\"lower\":{lo},\"upper\":{hi}}}"))
            .collect::<Vec<_>>()
            .join(",");
        let data = format!(
            "{{\"version\":1,\"position\":{},\"lower\":{},\"upper\":{},\"children\":[{}]}}",
            crate::json::position_json(self.p),
            self.lo,
            self.hi,
            children
        );
        crate::book::atomic_write(
            path,
            &format!(
                "{{\"checksum\":\"{:016x}\",\"data\":{}}}\n",
                checksum(&data),
                crate::json::quote(&data)
            ),
        )
    }
    fn prove(
        &mut self,
        s: &mut Search,
        path: &Path,
    ) -> Result<Option<(i32, Option<Move>)>, String> {
        let d = (self.p.empty().count_ones() / 2) as u8;
        let mut guess = evaluate(self.p, s.weights).clamp(self.lo, self.hi);
        guess = guess / 2 * 2;
        while self.lo < self.hi {
            let b = if guess == self.lo { guess + 2 } else { guess };
            let a = b - 1;
            let Ok(v) = s.run(self.p, d, a, b) else {
                return Ok(None);
            };
            if v <= a {
                self.hi = self.hi.min(v)
            } else if v >= b {
                self.lo = self.lo.max(v)
            } else {
                self.lo = v;
                self.hi = v;
            }
            self.save(path)?;
            guess = v;
        }
        let value = self.lo;
        let mut moves = self.p.moves();
        // A saved attaining child is already a complete best-move proof;
        // TT replacement / a cold restart must not change that fact.
        if let Some(m) = moves.iter().find(|m| {
            self.children
                .iter()
                .any(|(id, _, hi)| *id == m.id() && *hi <= -value)
        }) {
            return Ok(Some((value, Some(*m))));
        }
        let preferred = s.entry(self.p).best;
        moves.sort_by_key(|m| m.id() != preferred);
        for m in moves {
            if let Some((_, lo, hi)) = self.children.iter().find(|(id, _, _)| *id == m.id()) {
                if *hi <= -value {
                    return Ok(Some((value, Some(m))));
                }
                if *lo > -value {
                    continue;
                }
            }
            let Ok(v) = s.run(self.p.play(m), d.saturating_sub(1), -value, -value + 1) else {
                return Ok(None);
            };
            let (lo, hi) = if v <= -value { (-value, v) } else { (v, 64) };
            self.children.retain(|(id, _, _)| *id != m.id());
            self.children.push((m.id(), lo, hi));
            self.save(path)?;
            if v <= -value {
                return Ok(Some((value, Some(m))));
            }
        }
        if self.p.mobility() == 0 {
            Ok(Some((value, None)))
        } else {
            Err("No child attains the proven root value".into())
        }
    }
}
pub(super) fn prove(p: Position, o: &Options, path: &Path) -> Result<String, String> {
    // A bare filename has an empty Path::parent(). Resolve it so the durable
    // writer can fsync the actual containing directory after rename.
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut checkpoint = Checkpoint::load(p, &path)?;
    // Establish a reviewable valid checkpoint even before the first bound.
    checkpoint.save(&path)?;
    let start = now();
    let (result, nodes) =
        parallel::execute(o.tt_entries, o.threads, start + o.time_ms as f64, |s| {
            checkpoint.prove(s, &path)
        });
    let result = result?;
    Ok(format!("{{\"mode\":\"prove-best\",\"proofComplete\":{},\"complete\":false,\"exact\":{},\"value\":{},\"bestMove\":{},\"lower\":{},\"upper\":{},\"nodes\":{nodes},\"elapsedMs\":{:.3},\"perspective\":\"side-to-move\"}}",result.is_some(),result.is_some(),result.map_or("null".into(),|(v,_)|v.to_string()),result.and_then(|(_,m)|m).map_or("null".into(),|m|crate::json::quote(&m.to_string())),checkpoint.lo,checkpoint.hi,now()-start))
}
