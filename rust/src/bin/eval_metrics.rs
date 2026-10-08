use dosello_ai::{eval_lab::*, json, pattern, tree};
use std::collections::BTreeMap;
#[derive(Default)]
struct Metric {
    n: usize,
    correct: usize,
    static_abs: f64,
    bias: f64,
    loss_lo: f64,
    loss_hi: f64,
    max_lo: i32,
    max_hi: i32,
    value_abs: f64,
    depth: usize,
    incomplete: usize,
    nodes: u64,
    elapsed: f64,
}
fn main() {
    dosello_ai::cli::main_result(|| {
        let a = Args::read()?;
        let model = a.model()?;
        let config = a.search_config(&model)?;
        let raw =
            std::fs::read_to_string(a.get("--suite", "suite.jsonl")).map_err(|e| e.to_string())?;
        let mut rows = vec![];
        for l in raw.lines().filter(|l| !l.trim().is_empty()) {
            let v = json::parse(l)?;
            if !v.get("complete")?.boolean()? {
                return Err("suite contains incomplete proof".into());
            }
            let p = tree::position(v.get("key")?.str()?)?;
            let best = v.get("lower")?.num()? as i32;
            if v.get("upper")?.num()? as i32 != best {
                return Err("position not exact".into());
            }
            let mut moves = BTreeMap::new();
            for c in v.get("children")?.arr()? {
                let lo = c.get("lower")?.num()? as i32;
                let hi = c.get("upper")?.num()? as i32;
                if lo > hi || lo < -64 || hi > 64 || !(lo > -best || lo == hi && lo == -best) {
                    return Err("best set not proven".into());
                }
                for m in c.get("moves")?.arr()? {
                    if moves.insert(m.str()?.to_owned(), (lo, hi)).is_some() {
                        return Err("duplicate suite move".into());
                    }
                }
            }
            let groups = tree::groups(p);
            let expected: usize = groups.iter().map(|g| g.1.len()).sum();
            if expected != moves.len() {
                return Err("suite move coverage".into());
            }
            for (child, ms) in groups {
                for m in ms {
                    let c = v
                        .get("children")?
                        .arr()?
                        .iter()
                        .find(|c| {
                            c.get("moves")
                                .unwrap()
                                .arr()
                                .unwrap()
                                .iter()
                                .any(|x| x.str().ok() == Some(&m))
                        })
                        .ok_or("missing move")?;
                    if c.get("key")?.str()? != tree::key(child) {
                        return Err("suite child transition".into());
                    }
                }
            }
            if !moves.is_empty() && !moves.values().any(|b| *b == (-best, -best)) {
                return Err("no attaining move".into());
            }
            rows.push((p, best, moves));
        }
        if rows.is_empty() {
            return Err("empty suite".into());
        }
        let sample: usize = a.number("--sample", "0")?;
        let seed: u64 = a.number("--seed", "1")?;
        if sample > 0 {
            // Stable hash ordering: independent of input order and worker scheduling.
            rows.sort_by_key(|(p, _, _)| {
                (
                    pattern::phase(*p),
                    mix(checksum(tree::key(*p).as_bytes()) ^ seed),
                )
            });
            let mut counts = BTreeMap::new();
            rows.retain(|(p, _, _)| {
                let n = counts.entry(pattern::phase(*p)).or_insert(0);
                *n += 1;
                *n <= sample
            });
        }
        let average = a.number::<u8>("--root-average", "0")? != 0;
        let cold = a.number::<u8>("--cold-tt", "0")? != 0;
        let strict = a.number::<u8>("--strict-depth", "0")? != 0;
        let ordering = a.number::<u8>("--eval-ordering", "0")? != 0;
        if average && (config.table.is_some() || config.ordering) {return Err("root-average cannot be combined with probcut/eval-ordering".into());}
        let max: u8 = a.number("--max-depth", "10")?;
        if max > 28 {
            return Err("depth 0..28 (0: time budgets only)".into());
        }
        let timeout: f64 = a.number("--depth-ms", "60000")?;
        if !timeout.is_finite() || timeout <= 0. {
            return Err("depth-ms positive".into());
        }
        let mut budgets: Vec<(String, u8, f64)> = (1..=max)
            .map(|d| (format!("d{d:02}"), d, timeout))
            .collect();
        for t in a
            .get("--times", "100,500,3000")
            .split(',')
            .filter(|x| !x.is_empty())
        {
            let ms: f64 = t.parse().map_err(|_| "times")?;
            if !ms.is_finite() || ms <= 0. {
                return Err("positive time budget".into());
            }
            budgets.push((format!("t{ms:05}"), 28, ms))
        }
        let threads = a.threads()?;
        let next = std::sync::atomic::AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..threads.min(rows.len()) {
                let next = &next;
                let rows = &rows;
                let budgets = &budgets;
                let tx = tx.clone();
                let model = model.clone();
                let config = &config;
                scope.spawn(move || loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= rows.len() {
                        break;
                    }
                    let (p, best, moves) = &rows[i];
                    let static_v = model
                        .as_ref()
                        .map_or_else(|| pattern::evaluate(*p), |m| m.evaluate(*p));
                    for (name, depth, ms) in budgets {
                        let started = dosello_ai::search::now();
                        let result = (if average {
                            choose_averaged(*p, model.clone(), *depth, *ms)
                        } else {
                            choose_configured(*p, model.clone(), *depth, *ms, cold, strict, config)
                        })
                        .and_then(|(m, v, d, n)| {
                            let key = m.map_or("pass".into(), |m| m.to_string());
                            let interval = if moves.is_empty() {
                                (-best, -best)
                            } else {
                                *moves.get(&key).ok_or("chosen move not in suite")?
                            };
                            Ok((
                                tree::key(*p),
                                key,
                                p.empty().count_ones(),
                                name.clone(),
                                static_v - best,
                                interval.0 + best,
                                interval.1 + best,
                                v - best,
                                d as usize,
                                n,
                                dosello_ai::search::now() - started,
                                p.side,
                                name.starts_with('d')
                                    && d < (*depth).min((p.empty().count_ones() / 2) as u8),
                            ))
                        });
                        tx.send(result).unwrap();
                    }
                });
            }
            drop(tx);
        });
        let mut details = String::new();
        let mut table: BTreeMap<(u32, String), Metric> = BTreeMap::new();
        for r in rx {
            let (key, mv, ph, name, err, lo, hi, value, d, n, elapsed, side, incomplete) = r?;
            if !a.get("--details", "").is_empty() {
                details+=&format!("{{\"key\":{},\"move\":{},\"budget\":{},\"empties\":{ph},\"lossLower\":{lo},\"lossUpper\":{hi},\"valueError\":{value},\"completedDepth\":{d},\"incomplete\":{incomplete},\"staticError\":{err},\"side\":{side},\"remainingParity\":{},\"nodes\":{n},\"elapsedMs\":{elapsed:.3}}}\n",json::quote(&key),json::quote(&mv),json::quote(&name),(ph/2)%2);
            }
            let m = table.entry((ph, name)).or_default();
            m.incomplete += usize::from(incomplete);
            m.n += 1;
            m.correct += usize::from(lo == 0 && hi == 0);
            m.static_abs += err.abs() as f64;
            m.bias += err as f64;
            m.loss_lo += lo as f64;
            m.loss_hi += hi as f64;
            m.max_lo = m.max_lo.max(lo);
            m.max_hi = m.max_hi.max(hi);
            m.value_abs += value.abs() as f64;
            m.depth += d;
            m.nodes += n;
            m.elapsed += elapsed;
        }
        let mut prev: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
        for ((ph, name), m) in table {
            let n = m.n as f64;
            let acc = m.correct as f64 / n;
            let loss = m.loss_hi / n;
            let flag = name.starts_with('d')
                && prev
                    .get(&ph)
                    .is_some_and(|&(a, l)| acc + 0.001 < a || loss > l + 0.01);
            if name.starts_with('d') {
                prev.insert(ph, (acc, loss));
            }
            println!("{{\"empties\":{ph},\"budget\":{},\"count\":{},\"accuracy\":{acc:.5},\"avgLossLower\":{:.5},\"avgLossUpper\":{loss:.5},\"maxLossLower\":{},\"maxLossUpper\":{},\"staticMAE\":{:.5},\"staticBias\":{:.5},\"searchMAE\":{:.5},\"avgCompletedDepth\":{:.3},\"incomplete\":{},\"nodes\":{},\"nonMonotone\":{flag},\"eval\":{},\"threadsAcrossPositions\":{threads},\"elapsedMs\":{:.3},\"nodesPerSecond\":{:.1},\"samplePerPhase\":{sample},\"seed\":{seed},\"coldTT\":{cold},\"strictDepth\":{strict},\"evalOrdering\":{ordering},\"probcut\":{},\"probcutT\":{},\"rootAverage\":{average}}}",json::quote(&name),m.n,m.loss_lo/n,m.max_lo,m.max_hi,m.static_abs/n,m.bias/n,m.value_abs/n,m.depth as f64/n,m.incomplete,m.nodes,json::quote(&a.get("--eval","old")),m.elapsed,m.nodes as f64*1000./m.elapsed.max(0.001),json::quote(&a.get("--probcut","")),config.confidence);
        }
        if !a.get("--details", "").is_empty() {
            atomic(&a.get("--details", ""), details.as_bytes())?;
        }
        Ok(())
    })
}
