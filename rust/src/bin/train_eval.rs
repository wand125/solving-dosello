//! Deterministic normalized SGD, L2 shrinkage, canonical-position holdout.
use dosello_ai::{
    dataset::*,
    pattern::*,
    search::{hand_evaluate, Weights},
};
use std::io::Write;
struct Row {
    ids: [usize; INSTANCES],
    global: [f32; GLOBAL],
    phase: usize,
    y: f32,
    test: bool,
    old: f32,
    exact: bool,
}
fn predict(w: &[f32], r: &Row) -> f32 {
    let w = &w[r.phase * FEATURES..(r.phase + 1) * FEATURES];
    r.ids.iter().map(|i| w[*i]).sum::<f32>()
        + r.global
            .iter()
            .enumerate()
            .map(|(i, v)| v * w[FEATURES - GLOBAL + i])
            .sum::<f32>()
}
fn report(w: &[f32], data: &[Row], epoch: usize) {
    for phase in 0..PHASES {
        for only_exact in [false, true] {
            let mut n = 0;
            let mut before = 0.;
            let mut after = 0.;
            for r in data
                .iter()
                .filter(|r| r.test && r.phase == phase && (!only_exact || r.exact))
            {
                n += 1;
                before += (r.old - r.y).abs();
                after += (predict(w, r).round().clamp(-63., 63.) - r.y).abs();
            }
            if n > 0 {
                println!("{{\"epoch\":{epoch},\"phase\":{phase},\"emptiesMin\":{},\"emptiesMax\":{},\"exactOnly\":{only_exact},\"holdout\":{n},\"oldMAE\":{:.4},\"newMAE\":{:.4}}}",phase*4,phase*4+3,before/n as f32,after/n as f32);
            }
        }
    }
}
fn main() {
    dosello_ai::cli::main_result(|| {
        let args: Vec<_> = std::env::args().collect();
        let arg = |k: &str, d: &str| {
            args.windows(2)
                .find(|a| a[0] == k)
                .map_or(d.to_owned(), |a| a[1].clone())
        };
        let paths = arg("--data", "rust/data/train.bin");
        let epochs: usize = arg("--epochs", "25").parse().map_err(|_| "epochs")?;
        if epochs == 0 {
            return Err("--epochs must be positive".into());
        }
        let mut seen = std::collections::BTreeMap::new();
        for path in paths.split(',') {
            for s in read(path)? {
                let k = dosello_ai::book::key(dosello_ai::book::canonical(s.p));
                if seen.get(&k).is_none_or(|old: &Sample| {
                    (s.source == 0, s.depth) > (old.source == 0, old.depth)
                }) {
                    seen.insert(k, s);
                }
            }
        }
        if seen.is_empty() {
            return Err("No training positions".into());
        }
        let data: Vec<_> = seen
            .values()
            .map(|s| Row {
                ids: indices(s.p),
                global: globals(s.p),
                phase: phase(s.p),
                y: s.value as f32,
                test: group(s.p) % 10 == 0,
                old: hand_evaluate(s.p, Weights::default()) as f32,
                exact: s.source == 0,
            })
            .collect();
        drop(seen);
        eprintln!(
            "unique={} train={} holdout={} exact={}",
            data.len(),
            data.iter().filter(|r| !r.test).count(),
            data.iter().filter(|r| r.test).count(),
            data.iter().filter(|r| r.exact).count()
        );
        let mut w = vec![0f32; PHASES * FEATURES];
        for ph in 0..PHASES {
            w[ph * FEATURES + FEATURES - GLOBAL + 1] = 16.0;
        }
        let mut order: Vec<_> = (0..data.len()).filter(|i| !data[*i].test).collect();
        let mut seed = 78321;
        report(&w, &data, 0);
        for epoch in 1..=epochs {
            for i in (1..order.len()).rev() {
                let j = rng(&mut seed) as usize % (i + 1);
                order.swap(i, j);
            }
            for (ph, weights) in w.chunks_mut(FEATURES).enumerate() {
                if data.iter().any(|r| r.phase == ph && !r.test) {
                    for v in weights {
                        *v *= 0.995;
                    }
                }
            }
            let rate = 0.45 / (1.0 + epoch as f32 * 0.08);
            for &i in &order {
                let r = &data[i];
                let error = (r.y - predict(&w, r)).clamp(-40., 40.);
                let norm = 128.0 + r.global.iter().map(|v| v * v).sum::<f32>();
                let step = rate * error / norm;
                let base = r.phase * FEATURES;
                for id in r.ids {
                    w[base + id] += step;
                }
                for (j, v) in r.global.iter().enumerate() {
                    w[base + FEATURES - GLOBAL + j] += step * v;
                }
            }
            if epoch % 5 == 0 || epoch == epochs {
                report(&w, &data, epoch);
                eprintln!("epoch={epoch}");
            }
        }
        for (ph, weights) in w.chunks_mut(FEATURES).enumerate() {
            if !data.iter().any(|r| r.phase == ph && !r.test) {
                weights.fill(0.0);
                weights[FEATURES - GLOBAL + 1] = 16.0;
            }
            for v in weights {
                *v = (*v * SCALE).round().clamp(i16::MIN as f32, i16::MAX as f32) / SCALE;
            }
        }
        report(&w, &data, epochs + 1);
        let output = arg("--output", "rust/data/eval.bin");
        let mut f = std::io::BufWriter::new(
            std::fs::File::create(format!("{output}.tmp")).map_err(|e| e.to_string())?,
        );
        f.write_all(b"DSEVAL02").unwrap();
        f.write_all(&(FEATURES as u32).to_le_bytes()).unwrap();
        f.write_all(&(PHASES as u32).to_le_bytes()).unwrap();
        for v in &w {
            let q = (v * SCALE).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            f.write_all(&q.to_le_bytes()).unwrap();
        }
        f.flush().map_err(|e| e.to_string())?;
        f.get_ref().sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(format!("{output}.tmp"), output).map_err(|e| e.to_string())?;
        Ok(())
    })
}
