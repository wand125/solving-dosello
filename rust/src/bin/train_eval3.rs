//! Normalized SGD minimizes squared values, with L2 and adjacent-phase smoothing.
use dosello_ai::{
    book,
    eval3::*,
    eval_lab::{atomic, labels, Args},
    pattern,
};
use std::collections::BTreeSet;
fn main() {
    dosello_ai::cli::main_result(|| {
        let a = Args::read()?;
        let epochs: usize = a.number("--epochs", "20")?;
        let l2: f32 = a.number("--l2", "0.001")?;
        let smooth: f32 = a.number("--smooth", "0.05")?;
        if epochs == 0
            || !l2.is_finite()
            || !(0.0..=1.0).contains(&l2)
            || !smooth.is_finite()
            || !(0.0..=0.5).contains(&smooth)
        {
            return Err("invalid trainer parameters".into());
        }
        if !["all", "tables", "no-potential"].contains(&a.get("--features", "all").as_str()) {
            return Err("features all|tables|no-potential".into());
        }
        let feature_mode = a.get("--features", "all");
        let side_bias = a.number::<u8>("--side-bias", "0")?;
        if side_bias > 1 {
            return Err("side-bias 0|1".into());
        }
        let drop_global: usize = a.number("--drop-global", "8")?;
        if drop_global > 8 {
            return Err("drop-global 0..7, or 8 for none".into());
        }
        let mut globals_mask = if feature_mode == "tables" { 0u8 } else { 255 };
        if feature_mode == "no-potential" {
            globals_mask &= !(1 << 5);
        }
        if side_bias == 0 {
            globals_mask &= !(1 << 7);
        }
        if drop_global < 8 {
            globals_mask &= !(1 << drop_global);
        }
        let mut data = vec![];
        for path in a.get("--data", "labels.bin").split(',') {
            data.extend(labels(path)?)
        }
        let test_keys: BTreeSet<_> = data
            .iter()
            .filter(|r| r.game % 10 == 0)
            .map(|r| book::key(r.p))
            .collect();
        let mut seen = BTreeSet::new();
        data.sort_by_key(|r| (r.kind, std::cmp::Reverse(r.depth)));
        data.retain(|r| seen.insert(book::key(r.p)));
        // If any trajectory assigned this position to holdout, exclude it from train.
        let train: Vec<_> = data
            .iter()
            .filter(|r| r.game % 10 != 0 && !test_keys.contains(&book::key(r.p)))
            .collect();
        let test: Vec<_> = data
            .iter()
            .filter(|r| test_keys.contains(&book::key(r.p)))
            .collect();
        if train.is_empty() {
            return Err("empty training split".into());
        }
        // Cache extraction and duplicate-aware norms once; bounded by input size.
        struct Example {
            ids: [u16; INSTANCES],
            g: [f32; GLOBAL],
            base: usize,
            target: f32,
            norm: f32,
        }
        let examples: Vec<_> = train
            .iter()
            .map(|r| {
                let ids = indices(r.p);
                let mut g = globals(r.p);
                for i in 0..GLOBAL {
                    if globals_mask & (1 << i) == 0 {
                        g[i] = 0.;
                    }
                }
                let norm = ids
                    .iter()
                    .map(|i| ids.iter().filter(|j| *j == i).count() as f32)
                    .sum::<f32>()
                    + g.iter().map(|v| v * v).sum::<f32>();
                Example {
                    ids: ids.map(|i| i as u16),
                    g,
                    base: pattern::phase(r.p) * FEATURES,
                    target: r.value as f32,
                    norm: norm.max(1.),
                }
            })
            .collect();
        let mut w = vec![0f32; PHASES * FEATURES];
        let mut order: Vec<_> = (0..train.len()).collect();
        let mut seed = a.number::<u64>("--seed", "97123")?;
        for epoch in 0..epochs {
            for i in (1..order.len()).rev() {
                let j = dosello_ai::dataset::rng(&mut seed) as usize % (i + 1);
                order.swap(i, j)
            }
            for v in &mut w {
                *v *= 1. - l2
            }
            for &i in &order {
                let r = &examples[i];
                let ids = r.ids;
                let g = r.g;
                let base = r.base;
                let pred = ids.iter().map(|i| w[base + *i as usize]).sum::<f32>()
                    + g.iter()
                        .enumerate()
                        .map(|(i, v)| v * w[base + 4 * TABLE + i])
                        .sum::<f32>();
                let step = 0.4 / (1. + epoch as f32 * 0.1) * (r.target - pred) / r.norm;
                for id in ids {
                    w[base + id as usize] += step
                }
                for (j, v) in g.iter().enumerate() {
                    w[base + 4 * TABLE + j] += step * v
                }
            }
            let previous = w.clone();
            for ph in 0..PHASES {
                for i in 0..FEATURES {
                    let at = ph * FEATURES + i;
                    if ph > 0 {
                        w[at] += smooth * (previous[at - FEATURES] - previous[at])
                    }
                    if ph + 1 < PHASES {
                        w[at] += smooth * (previous[at + FEATURES] - previous[at])
                    }
                }
            }
        }
        // Existing shards cover only part of the game. Do not leave unseen
        // opening/endgame phases at zero: explicitly extrapolate nearest fitted
        // phase. This is a fallback, not a substitute for labels at those phases.
        let mut phase_counts = [0usize; PHASES];
        for r in &train {
            phase_counts[pattern::phase(r.p)] += 1;
        }
        let fitted = w.clone();
        for ph in 0..PHASES {
            if phase_counts[ph] == 0 {
                let nearest = (0..PHASES)
                    .filter(|&j| phase_counts[j] > 0)
                    .min_by_key(|&j| j.abs_diff(ph))
                    .unwrap();
                w[ph * FEATURES..(ph + 1) * FEATURES]
                    .copy_from_slice(&fitted[nearest * FEATURES..(nearest + 1) * FEATURES]);
            }
        }
        let model = Model {
            globals_mask,
            weights: w
                .iter()
                .map(|v| (v * 256.).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16)
                .collect(),
        };
        for ph in 0..PHASES {
            for exact in [false, true] {
                let rows: Vec<_> = test
                    .iter()
                    .filter(|r| pattern::phase(r.p) == ph && (!exact || r.kind == 0))
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                let old = rows
                    .iter()
                    .map(|r| (pattern::evaluate(r.p) - r.value as i32).abs() as f64)
                    .sum::<f64>()
                    / rows.len() as f64;
                let new = rows
                    .iter()
                    .map(|r| (model.evaluate(r.p) - r.value as i32).abs() as f64)
                    .sum::<f64>()
                    / rows.len() as f64;
                println!("{{\"phase\":{ph},\"exactOnly\":{exact},\"holdout\":{},\"oldMAE\":{old:.4},\"newMAE\":{new:.4}}}",rows.len());
            }
        }
        let output = a.get("--output", "eval3.bin");
        atomic(&output, &model.encode())?;
        let mut bias = std::collections::BTreeMap::new();
        for r in &test {
            let e = r.p.empty().count_ones();
            let stats = bias
                .entry((e, r.p.side, r.kind))
                .or_insert((0usize, 0i64, 0i64, 0i64));
            let old = pattern::evaluate(r.p) - r.value as i32;
            let new = model.evaluate(r.p) - r.value as i32;
            stats.0 += 1;
            stats.1 += old as i64;
            stats.2 += new as i64;
            stats.3 += new.abs() as i64;
        }
        let mut diagnostics = String::new();
        for ((e, side, kind), (n, old, new, abs)) in bias {
            diagnostics+=&format!("{{\"empties\":{e},\"phase\":{},\"remainingParity\":{},\"side\":{side},\"labelKind\":{kind},\"holdout\":{n},\"oldBias\":{},\"newBias\":{},\"newMAE\":{}}}\n",(e/4).min(14),(e/2)%2,old as f64/n as f64,new as f64/n as f64,abs as f64/n as f64);
        }
        atomic(&format!("{output}.bias.jsonl"), diagnostics.as_bytes())?;
        eprintln!(
            "features={feature_mode} side_bias={side_bias}; one SGD thread; cached examples={}",
            examples.len()
        );
        let meta=format!("{{\"train\":{},\"holdout\":{},\"epochs\":{epochs},\"l2\":{l2},\"smoothing\":{smooth},\"bytes\":{},\"holdoutSplit\":\"game mod 10; holdout canonical keys excluded from train\",\"featureMode\":{},\"sideBias\":{side_bias},\"globalsMask\":{globals_mask},\"standalone\":true,\"phaseTrainCounts\":{phase_counts:?},\"unseenPhasePolicy\":\"nearest fitted phase\",\"schema\":\"DSEVAL04\",\"referenceRevision\":{},\"data\":{}}}\n",train.len(),test.len(),model.encode().len(),dosello_ai::json::quote(&feature_mode),pattern::revision(),dosello_ai::json::quote(&a.get("--data","labels.bin")));
        atomic(&format!("{output}.json"), meta.as_bytes())?;
        eprintln!("{meta}");
        Ok(())
    })
}
