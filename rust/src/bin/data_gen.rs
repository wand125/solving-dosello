use dosello_ai::{board::*, book::*, dataset::*, search::*};
use std::{
    io::{BufWriter, Write},
    sync::atomic::{AtomicUsize, Ordering},
};
fn main() {
    dosello_ai::cli::main_result(|| {
        let args: Vec<_> = std::env::args().collect();
        let arg = |k: &str, d: &str| {
            args.windows(2)
                .find(|a| a[0] == k)
                .map_or(d.to_owned(), |a| a[1].clone())
        };
        let output = arg("--output", "rust/data/train.bin");
        let games: usize = arg("--games", "12000").parse().map_err(|_| "games")?;
        let threads: usize = arg("--threads", "14").parse().map_err(|_| "threads")?;
        let budget: f64 = arg("--exact-ms", "100").parse().map_err(|_| "exact-ms")?;
        let label_ms: f64 = arg("--label-ms", "8").parse().map_err(|_| "label-ms")?;
        let learned = arg("--learned", "0") == "1";
        if games == 0
            || !(1..=64).contains(&threads)
            || !budget.is_finite()
            || budget < 0.0
            || !label_ms.is_finite()
            || label_ms <= 0.0
        {
            return Err("games > 0, threads 1..64, exact-ms >= 0 and label-ms > 0 required".into());
        }
        let mut out = BufWriter::new(
            std::fs::File::create(format!("{output}.tmp")).map_err(|e| e.to_string())?,
        );
        out.write_all(b"DSDATA02").map_err(|e| e.to_string())?;
        if arg("--book", "1") == "1" {
            let book = Book::load(std::path::Path::new("rust/book/book.jsonl"), false)?;
            let mut n = 0;
            let mut exact = 0;
            for row in book.nodes.values() {
                if row.exact() || row.depth >= 4 {
                    let s = Sample {
                        p: row.p,
                        value: row.value as i8,
                        depth: row.depth,
                        source: if row.exact() {
                            0
                        } else if row.eval_version == dosello_ai::pattern::revision() {
                            2
                        } else {
                            1
                        },
                        group: group(row.p),
                    };
                    write(&mut out, s).map_err(|e| e.to_string())?;
                    n += 1;
                    exact += usize::from(row.exact());
                }
            }
            eprintln!("book samples={n} exact={exact}");
        }
        let counter = AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::sync_channel(threads.max(1) * 4);
        std::thread::scope(|scope| {
            for worker in 0..threads {
                let counter = &counter;
                let tx = tx.clone();
                scope.spawn(move || {
                    let mut seed = 0x845f7a123 + worker as u64;
                    let mut s = Search::new(1 << 18, f64::INFINITY);
                    let mut old = dosello_ai::legacy_search::Search::new(1 << 18, f64::INFINITY);
                    loop {
                        let game = counter.fetch_add(1, Ordering::Relaxed);
                        if game >= games {
                            break;
                        }
                        let mut p = Position::initial();
                        let mut samples = vec![];
                        let split = (game as u32).wrapping_mul(2654435761);
                        loop {
                            let ms = p.moves();
                            if ms.is_empty() {
                                p = p.pass();
                                if p.mobility() == 0 {
                                    break;
                                }
                                continue;
                            }
                            let e = p.empty().count_ones();
                            if (20..=54).contains(&e) && e % 4 == 0 || e == 54 {
                                let max = (e / 2) as u8;
                                let mut value = if learned {
                                    evaluate(p, Weights::default())
                                } else {
                                    hand_evaluate(p, Weights::default())
                                };
                                let mut depth = 0;
                                let mut source = if learned { 2 } else { 1 };
                                if e <= 40 {
                                    s.deadline = now() + budget;
                                    s.aborted = false;
                                    if let Ok(v) = s.exact(p, false) {
                                        value = v;
                                        depth = max;
                                        source = 0;
                                    }
                                }
                                if source != 0 {
                                    let deadline = now() + label_ms;
                                    s.deadline = deadline;
                                    s.aborted = false;
                                    old.deadline = deadline;
                                    old.aborted = false;
                                    for d in 1..=max {
                                        let r = if learned {
                                            s.run(p, d, -65, 65)
                                        } else {
                                            old.run(p, d, -65, 65)
                                        };
                                        if let Ok(v) = r {
                                            value = v;
                                            depth = d;
                                            if d == max {
                                                source = 0;
                                                break;
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                samples.push(Sample {
                                    p,
                                    value: value as i8,
                                    depth,
                                    source,
                                    group: split,
                                });
                            }
                            // Softmax over depth-one values, with 25% uniformly random exploration.
                            let m = if rng(&mut seed) % 4 == 0 {
                                ms[rng(&mut seed) as usize % ms.len()]
                            } else {
                                let scores: Vec<f64> = ms
                                    .iter()
                                    .map(|m| {
                                        let v = if learned {
                                            evaluate(p.play(*m), Weights::default())
                                        } else {
                                            hand_evaluate(p.play(*m), Weights::default())
                                        };
                                        (-v as f64 / 6.0).exp()
                                    })
                                    .collect();
                                let mut r = (rng(&mut seed) as f64 / u64::MAX as f64)
                                    * scores.iter().sum::<f64>();
                                let mut choice = ms[ms.len() - 1];
                                for (m, v) in ms.iter().zip(scores) {
                                    r -= v;
                                    if r <= 0.0 {
                                        choice = *m;
                                        break;
                                    }
                                }
                                choice
                            };
                            p = p.play(m);
                            if e <= 20 {
                                break;
                            }
                        }
                        tx.send(samples).unwrap();
                    }
                });
            }
            drop(tx);
            let mut count = 0;
            let mut exact = 0;
            for batch in rx {
                for s in batch {
                    write(&mut out, s).unwrap();
                    count += 1;
                    exact += usize::from(s.source == 0);
                }
                if counter.load(Ordering::Relaxed) % 100 == 0 {
                    eprintln!(
                        "games={} generated={count} exact={exact}",
                        counter.load(Ordering::Relaxed)
                    );
                }
            }
            eprintln!("generated={count} exact={exact}");
        });
        out.flush().map_err(|e| e.to_string())?;
        out.get_ref().sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(format!("{output}.tmp"), output).map_err(|e| e.to_string())?;
        Ok(())
    })
}
