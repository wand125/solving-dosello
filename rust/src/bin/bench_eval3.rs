//! Bounded, single-threaded evaluator/search throughput and parity diagnostic.
use dosello_ai::{eval3, eval_lab::*, json, pattern, search::now, tree};
fn main() {
    dosello_ai::cli::main_result(|| {
        let a = Args::read()?;
        let model = a.model()?;
        let config = a.search_config(&model)?;
        let raw = std::fs::read_to_string(a.get("--suite", "rust/data/solved-suite-1500.jsonl"))
            .map_err(|e| e.to_string())?;
        let count: usize = a.number("--count", "6")?;
        let depth: u8 = a.number("--depth", "6")?;
        let ms: f64 = a.number("--time", "1000")?;
        let mut rows = vec![];
        let mut counts = std::collections::BTreeMap::new();
        for l in raw.lines() {
            let v = json::parse(l)?;
            let p = tree::position(v.get("key")?.str()?)?;
            let n = counts.entry(p.empty().count_ones()).or_insert(0);
            if *n >= count {
                continue;
            }
            *n += 1;
            rows.push((p, v.get("lower")?.num()?));
        }
        let repeats: usize = a.number("--repeats", "16")?;
        if !(1..=1000).contains(&repeats) {
            return Err("repeats 1..1000".into());
        }
        for (p, truth) in rows {
            let mut s = search(model.clone(), ms);
            s.set_probcut3(config.clone())?;
            s.strict_depth = true;
            // Warm code/weights once; measure repeated cold-TT searches.
            let _ = s.run(p, depth, -65, 65);
            let mut elapsed = 0.;
            let mut nodes = 0;
            let mut complete = true;
            let mut value = None;
            for _ in 0..repeats {
                s.clear();
                s.nodes = 0;
                s.aborted = false;
                s.deadline = now() + ms;
                let started = now();
                let result = s.run(p, depth, -65, 65);
                elapsed += now() - started;
                nodes += s.nodes;
                value = result.ok();
                if value.is_none() {
                    complete = false;
                    break;
                }
            }
            let eval = model
                .as_ref()
                .map_or_else(|| pattern::evaluate(p), |m| m.evaluate(p));
            let mut micro = 0.;
            let mut checksum = 0;
            if let Some(Evaluator::Eval3(m)) = model.as_deref() {
                let moves = p.moves();
                let mut state = eval3::State::new(p);
                let start = now();
                for _ in 0..1000 {
                    for mv in &moves {
                        state.update(p.play(*mv));
                        checksum += std::hint::black_box(m.evaluate_state(&state));
                        state.update(p);
                    }
                }
                micro = (now() - start) * 1000000. / (1000 * moves.len()).max(1) as f64;
            }
            println!("{{\"evalOrdering\":{},\"value\":{},\"eval\":{},\"key\":{},\"empties\":{},\"side\":{},\"staticError\":{},\"depth\":{depth},\"repeats\":{repeats},\"complete\":{},\"nodes\":{},\"elapsedMs\":{elapsed:.3},\"nodesPerSecond\":{:.1},\"makeEvalUnmakeNs\":{micro:.1},\"checksum\":{checksum}}}",config.ordering,value.map_or("null".into(),|v|v.to_string()),json::quote(&a.get("--eval","old")),json::quote(&tree::key(p)),p.empty().count_ones(),p.side,eval as i64-truth,complete,nodes,nodes as f64*1000./elapsed.max(0.001));
        }
        Ok(())
    })
}
