use dosello_ai::{board::*, cli, json, search::*};
use std::io::Write;
fn arg(rest: &[String], name: &str, default: &str) -> String {
    rest.chunks(2)
        .find(|x| x[0] == name)
        .and_then(|x| x.get(1))
        .cloned()
        .unwrap_or(default.into())
}
fn write_book(path: &str, entries: &[String]) -> Result<(), String> {
    let text = format!(
        "{{\"version\":1,\"perspective\":\"side-to-move\",\"entries\":[{}]}}\n",
        entries.join(",")
    );
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}
fn main() {
    cli::main_result(|| {
        let a = cli::args()?;
        if a.root_score {
            if a.wld {return Err("--root-score returns full scores; omit --wld".into())}
            let start=now();let (v,nodes)=parallel_exact(a.position,a.opts.tt_entries,a.opts.threads,start+a.opts.time_ms as f64);
            println!("{{\"mode\":\"root-score\",\"value\":{},\"exact\":{},\"nodes\":{nodes},\"elapsedMs\":{},\"perspective\":\"side-to-move\"}}",v.map_or("null".into(),|x|x.to_string()),v.is_some(),now()-start);
            return Ok(())
        }
        if a.prove_best {if a.wld{return Err("--prove-best requires disc scores, not --wld".into())}
            if a.rest.iter().any(|s|s=="--checkpoint") {
                println!("{}",prove_checkpoint_json(a.position,&a.opts,std::path::Path::new(&arg(&a.rest,"--checkpoint","rust/book/search-checkpoint.json")))?);
            } else {println!("{}",prove_json(a.position,&a.opts));}
            return Ok(())}
        if a.rest.iter().any(|x| x == "--measure") {
            let budget: u64 = arg(&a.rest, "--measure", "3000")
                .parse()
                .map_err(|_| "measure")?;
            let samples: usize = arg(&a.rest, "--samples", "3")
                .parse()
                .map_err(|_| "samples")?;
            let mut seed = 0x58af71;
            let mut measured = vec![];
            for empties in [18, 22, 26, 30, 34, 38, 42, 46] {
                for sample in 0..samples {
                    let mut p = random_position(&mut seed, empties);
                    for _ in 0..100 {
                        if p.empty().count_ones() == empties && !p.moves().is_empty() {
                            break;
                        }
                        p = random_position(&mut seed, empties)
                    }
                    let t = now();
                    let mut s = Search::new(a.opts.tt_entries, t + budget as f64);
                    let value = s.exact(p, a.wld).ok();
                    let elapsed = now() - t;
                    println!("{{\"requestedEmpties\":{empties},\"empties\":{},\"sample\":{sample},\"exact\":{},\"value\":{},\"elapsedMs\":{elapsed:.3},\"nodes\":{},\"nps\":{:.0},\"position\":{}}}",p.empty().count_ones(),value.is_some(),value.map_or("null".into(),|v|v.to_string()),s.nodes,s.nodes as f64/(elapsed/1000.0).max(0.000001),json::position_json(p));
                    std::io::stdout().flush().map_err(|e| e.to_string())?;
                    measured.push((empties, value.is_some(), elapsed));
                }
            }
            let good: Vec<_> = measured
                .iter()
                .filter(|(_, ok, ms)| *ok && *ms > 1.0)
                .collect();
            if good.len() >= 2 {
                let n = good.len() as f64;
                let x = good.iter().map(|v| v.0 as f64).sum::<f64>() / n;
                let y = good.iter().map(|v| v.2.ln()).sum::<f64>() / n;
                let den = good.iter().map(|v| (v.0 as f64 - x).powi(2)).sum::<f64>();
                if den > 0.0 {
                    let slope = good
                        .iter()
                        .map(|v| (v.0 as f64 - x) * (v.2.ln() - y))
                        .sum::<f64>()
                        / den;
                    let projected = (y + slope * (56.0 - x)).exp() / 1000.0;
                    eprintln!("Completed-sample log-linear estimate (optimistic; censored failures excluded): start {projected:.1} single-thread seconds, {:.1} hours / ideal 14 threads. NOT a bound; root splitting is not ideal scaling.",projected/14.0/3600.0);
                }
            }
            return Ok(());
        }
        let path = arg(&a.rest, "--book", "rust/book/checkpoint.json");
        let line: usize = arg(&a.rest, "--line", "0").parse().map_err(|_| "line")?;
        let mut entries = vec![];
        if line > 0 {
            let mut p = a.position;
            let mut sequence: Vec<String> = vec![];
            for ply in 0..=line {
                let r = analyze(p, &a.opts);
                let best = r.best().map(|x| x.mv);
                eprintln!(
                    "line ply {ply}: empties {}, exact {}/{}, depth {}, nodes {}",
                    p.empty().count_ones(),
                    r.moves.iter().filter(|x| x.exact).count(),
                    r.moves.len(),
                    r.moves.iter().map(|x| x.depth).min().unwrap_or(0),
                    r.nodes
                );
                let sequence_field = if a.position == Position::initial() {
                    format!(",\"sequence\":{}", json::quote(&sequence.join(" ")))
                } else {
                    String::new()
                };
                entries.push(format!(
                    "{{\"position\":{},\"analysis\":{}{sequence_field}}}",
                    json::position_json(p),
                    r.json()
                ));
                write_book(&path, &entries)?;
                if let Some(m) = best {
                    sequence.push(m.to_string());
                    p = p.play(m)
                } else if r.pass {
                    sequence.push("pass".into());
                    p = p.pass()
                } else {
                    break;
                }
            }
            return Ok(());
        }
        // Direct terminal searches split at the root. Each completed child is atomically checkpointed.
        let p = a.position;
        let ms = p.moves();
        if ms.is_empty() {
            let mut s = Search::new(a.opts.tt_entries, now() + a.opts.time_ms as f64);
            let v = s.exact(p, a.wld).ok();
            println!(
                "{{\"value\":{},\"complete\":{},\"nodes\":{}}}",
                v.map_or("null".into(), |v| v.to_string()),
                v.is_some(),
                s.nodes
            );
            return Ok(());
        }
        let threads = a.opts.threads.max(1).min(ms.len());
        let deadline = now() + a.opts.time_ms as f64;
        let start = now();
        let index = std::sync::atomic::AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut solved = 0;
        let mut best = -65;
        let mut child_rows = vec![];
        let shared=std::sync::Arc::new(SharedTable::new(a.opts.tt_entries));
        let counters: Vec<_> = (0..threads)
            .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
            .collect();
        write_book(&path, &[])?;
        std::thread::scope(|scope| {
            for counter in &counters {
                let counter = std::sync::Arc::clone(counter);
                let tx = tx.clone();
                let ms = &ms;
                let index = &index;
                let opts = &a.opts;
                let shared=shared.clone();
                scope.spawn(move || {
                    let mut s = Search::new(4, deadline);
                    s.progress = Some(counter);
                    s.shared=Some(shared);
                    loop {
                        let i = index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= ms.len() {
                            break;
                        }
                        let m = ms[i];
                        let n = s.nodes;
                        let value = if now() < deadline {
                            (if opts.selective_exact&&!a.wld{s.selective_exact(p.play(m))}else{s.exact(p.play(m), a.wld)}).ok().map(|v| -v)
                        } else {
                            None
                        };
                        let pv = s.pv(p.play(m), 64);
                        tx.send((m, value, s.nodes - n, pv)).unwrap();
                    }
                });
            }
            drop(tx);
            loop {
                let (m, v, n, pv) = match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                    Ok(v) => v,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        let nodes: usize = counters
                            .iter()
                            .map(|n| n.load(std::sync::atomic::Ordering::Relaxed))
                            .sum();
                        let seconds = (now() - start) / 1000.0;
                        eprintln!(
                            "progress elapsed={seconds:.1}s nodes={nodes} nps={:.0} solved={}/{}",
                            nodes as f64 / seconds.max(0.001),
                            solved,
                            ms.len()
                        );
                        continue;
                    }
                    Err(_) => break,
                };
                if let Some(v) = v {
                    solved += 1;
                    best = best.max(v)
                }
                eprintln!(
                    "{}: {:?}, solved {}/{}, nodes {}, {:.1}s",
                    m,
                    v,
                    solved,
                    ms.len(),
                    n,
                    (now() - start) / 1000.0
                );
                let row=format!("{{\"move\":{},\"value\":{},\"exact\":{},\"depth\":{},\"pv\":[{}],\"nodes\":{n}}}",json::quote(&m.to_string()),v.map_or("null".into(),|v|v.to_string()),v.is_some()&&!a.wld,p.empty().count_ones()/2,std::iter::once(m.to_string()).chain(pv).map(|x|json::quote(&x)).collect::<Vec<_>>().join(","));
                child_rows.push(row);
                entries=vec![format!("{{\"position\":{},\"mode\":{},\"analysis\":{{\"moves\":[{}],\"complete\":{}}}}}",json::position_json(p),json::quote(if a.wld{"wld"}else{"score"}),child_rows.join(","),solved==ms.len())];
                if let Err(e) = write_book(&path, &entries) {
                    eprintln!("Checkpoint error: {e}")
                }
            }
        });
        println!("{{\"complete\":{},\"solvedChildren\":{solved},\"children\":{},\"value\":{},\"mode\":{}}}",solved==ms.len(),ms.len(),if solved==ms.len(){best.to_string()}else{"null".into()},json::quote(if a.wld{"wld"}else{"score"}));
        Ok(())
    })
}
