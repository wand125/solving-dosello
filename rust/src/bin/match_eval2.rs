use dosello_ai::{board::Position, dataset::rng, eval_lab::*, search::now};
fn main() {
    dosello_ai::cli::main_result(|| {
        let a = Args::read()?;
        let games: usize = a.number("--games", "200")?;
        let ms: f64 = a.number("--time", "100")?;
        let opponent = a.get("--opponent", "old");
        let model = a.model()?;
        let config = a.search_config(&model)?;
        let book_side = a.get("--book-side", "none");
        // Random opening plies (default 4); with an opening book use fewer so play stays inside the book.
        let random_plies: usize = a.number("--random-plies", "4")?;
        // Deviation test: the opponent plays one random move at a seeded ply in [random_plies+1, random_plies+N]
        // (0 = off), so book play can be compared on the same deviations.
        let deviate_max: usize = a.number("--deviate-max", "0")?;
        if !["ai","both","none"].contains(&book_side.as_str()) {return Err("book-side ai|both|none".into());}
        let book_path = a.get("--book", "");
        if book_side != "none" && book_path.is_empty() {return Err("book-side requires --book FILE".into());}
        let book = if book_path.is_empty() {None} else {Some(dosello_ai::match_book::MatchBook::decode(&std::fs::read(&book_path).map_err(|e|e.to_string())?)?)};
        let opponent_model = if opponent == "eval3" {
            let weights = a.get("--opponent-weights", &a.get("--weights", "rust/data/eval3-r2.bin"));
            Some(std::sync::Arc::new(Evaluator::Eval3(std::sync::Arc::new(dosello_ai::eval3::Model::decode(&std::fs::read(weights).map_err(|e|e.to_string())?)?))))
        } else {None};
        let average = a.number::<u8>("--root-average", "0")? != 0;
        if average && (config.table.is_some() || config.ordering) {return Err("root-average cannot be combined with probcut/eval-ordering".into());}
        let threads = a.threads()?;
        if games == 0
            || games % 2 != 0
            || !ms.is_finite()
            || ms <= 0.
            || !["old", "eval3"].contains(&opponent.as_str())
        {
            return Err("even games, positive time, opponent old|eval3; site5 uses the optional test/match_eval.js adapter".into());
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..threads.min(games / 2) {
                let next = &next;
                let model = model.clone();
                let tx = tx.clone();
                let config = &config;
                let book = &book;
                let book_side = &book_side;
                let opponent_model = opponent_model.clone();
                scope.spawn(move || loop {
                    let pair = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if pair >= games / 2 {
                        break;
                    }
                    let mut pair_score = 0.;
                    let mut wdl = [0; 3];
                    let mut times = [0.; 2];
                    let mut hits = [0usize; 2];
                    for color in [1, -1] {
                        let mut p = Position::initial();
                        let mut seed = mix(5111 + pair as u64);
                        let mut ply = 0;
                        let mut record: Vec<String> = vec![];
                        let deviate_at = if deviate_max > 0 { random_plies + 1 + (mix(9001 + pair as u64) as usize % deviate_max) } else { usize::MAX };
                        let mut deviated = false;
                        loop {
                            let moves = p.moves();
                            if moves.is_empty() {
                                p = p.pass();
                                record.push("pass".into());
                                if p.mobility() == 0 {
                                    break;
                                }
                                continue;
                            }
                            let start = now();
                            let book_move = if ply >= random_plies && (book_side == "both" || book_side == "ai" && p.side == color) {
                                book.as_ref().and_then(|book|book.best(p))
                            } else {None};
                            let m = if ply < random_plies {
                                moves[rng(&mut seed) as usize % moves.len()]
                            } else if !deviated && p.side != color && ply + 1 >= deviate_at {
                                deviated = true;
                                let mut dseed = mix(7919 + pair as u64);
                                moves[rng(&mut dseed) as usize % moves.len()]
                            } else if let Some(m) = book_move {
                                hits[usize::from(p.side != color)] += 1; m
                            } else if p.side == color {
                                (if average {
                                    choose_averaged(p, model.clone(), 28, ms)
                                } else {
                                    choose_configured(p, model.clone(), 28, ms, false, false, config)
                                })
                                .unwrap()
                                .0
                                .unwrap()
                            } else {
                                choose(p, opponent_model.clone(), 28, ms).unwrap().0.unwrap()
                            };
                            if ply >= random_plies {
                                times[usize::from(p.side != color)] += now() - start
                            }
                            record.push(m.to_string());
                            p = p.play(m);
                            ply += 1;
                        }
                        let score = p.diff() * color as i32;
                        // One JSON line per game on stderr (AI colour, final disc diff for the AI, moves).
                        eprintln!("{{\"pair\":{pair},\"aiColor\":{color},\"score\":{score},\"moves\":\"{}\"}}", record.join(" "));
                        wdl[if score > 0 {
                            0
                        } else if score == 0 {
                            1
                        } else {
                            2
                        }] += 1;
                        pair_score += if score > 0 {
                            1.
                        } else if score == 0 {
                            0.5
                        } else {
                            0.
                        };
                    }
                    tx.send((wdl, times, pair_score / 2., hits)).unwrap();
                });
            }
            drop(tx);
        });
        let mut wdl = [0; 3];
        let mut times = [0.; 2];
        let mut pairs = vec![];
        let mut hits = [0usize;2];
        for (w, t, p, h) in rx {
            for i in 0..2 {hits[i]+=h[i];}
            for i in 0..3 {
                wdl[i] += w[i]
            }
            for i in 0..2 {
                times[i] += t[i]
            }
            pairs.push(p)
        }
        let score = pairs.iter().sum::<f64>() / pairs.len() as f64;
        let elo =
            |s: f64| 400. * (s.clamp(0.0001, 0.9999) / (1. - s.clamp(0.0001, 0.9999))).log10();
        let se = if pairs.len() > 1 {
            (pairs.iter().map(|v| (v - score).powi(2)).sum::<f64>()
                / (pairs.len() * (pairs.len() - 1)) as f64)
                .sqrt()
        } else {
            0.5
        };
        println!("{{\"bookHitsAI\":{},\"bookHitsOpponent\":{},\"bookSide\":{},\"probcut\":{},\"probcutT\":{},\"evalOrdering\":{},\"eval\":{},\"games\":{games},\"wins\":{},\"draws\":{},\"losses\":{},\"score\":{score},\"eloMethod\":\"half-win-half-loss prior\",\"eloApprox\":{},\"pairClusterScore95\":[{},{}],\"newMs\":{},\"opponentMs\":{},\"opponent\":{},\"timeMs\":{ms},\"site5FixedDepthNotEqualTime\":{}}}",hits[0],hits[1],dosello_ai::json::quote(&book_side),dosello_ai::json::quote(&a.get("--probcut","")),config.confidence,config.ordering,dosello_ai::json::quote(&a.get("--eval","old")),wdl[0],wdl[1],wdl[2],elo((wdl[0] as f64 + wdl[1] as f64 / 2. + 0.5) / (games as f64 + 1.)),if pairs.len()<20 {0.} else {(score-1.96*se).max(0.)},if pairs.len()<20 {1.} else {(score+1.96*se).min(1.)},times[0],times[1],dosello_ai::json::quote(&opponent),opponent=="site5");
        eprintln!("config eval={} probcut={} probcut-t={} eval-ordering={} book={} book-side={} bookHitsAI={} bookHitsOpponent={}",a.get("--eval","old"),a.get("--probcut","off"),config.confidence,config.ordering,book_path,book_side,hits[0],hits[1]);
        Ok(())
    })
}
