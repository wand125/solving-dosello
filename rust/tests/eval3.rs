use dosello_ai::{board::*, book, dataset::rng, eval3::*, eval_lab};
fn model() -> Model {
    Model {
        globals_mask: 255,
        weights: (0..PHASES * FEATURES)
            .map(|i| ((i * 7919 % 1009) as i16) - 504)
            .collect(),
    }
}
#[test]
fn incremental_make_unmake_pass_partner_and_symmetry() {
    let m = model();
    let mut seed = 8741;
    let mut partner_flips = 0;
    let mut forced_passes = 0;
    for _ in 0..24 {
        let mut p = Position::initial();
        let mut s = State::new(p);
        let mut stack = vec![];
        for _ in 0..64 {
            assert_eq!(s, State::new(p));
            assert_eq!(m.evaluate_state(&s), m.evaluate(p));
            let base = dosello_ai::pattern::phase(p) * FEATURES;
            let sum = indices(p)
                .iter()
                .map(|&i| 16 * m.weights[base + i] as i32)
                .sum::<i32>()
                + global_numerators(p)
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| m.globals_mask & (1 << i) != 0)
                    .map(|(i, v)| v * m.weights[base + 4 * TABLE + i] as i32)
                    .sum::<i32>();
            assert_eq!(
                m.evaluate_state(&s),
                ((sum + sum.signum() * 2048) / 4096).clamp(-63, 63)
            );
            for t in 0..8 {
                assert_eq!(m.evaluate(p), m.evaluate(book::transform(p, t)));
            }
            // Artificial pass tests absolute-colour indices even away from forced pass.
            let old = s;
            s.update(p.pass());
            assert_eq!(s, State::new(p.pass()));
            s.update(p);
            assert_eq!(s, old);
            let moves = p.moves();
            if moves.is_empty() {
                if p.pass().moves().is_empty() {
                    break;
                }
                forced_passes += 1;
                stack.push((p, s));
                p = p.pass();
            } else {
                let mv = moves[rng(&mut seed) as usize % moves.len()];
                partner_flips += usize::from(mv.flips != 0 && p.partners(mv.flips) != 0);
                stack.push((p, s));
                p = p.play(mv);
            }
            s.update(p);
        }
        while let Some((q, expected)) = stack.pop() {
            s.update(q);
            assert_eq!(s, expected);
            assert_eq!(m.evaluate_state(&s), m.evaluate(q));
        }
    }
    assert!(partner_flips > 0);
    assert!(forced_passes > 0);
}
#[test]
fn roundtrip_size_and_schema() {
    let m = model();
    let b = m.encode();
    let other = Model::decode(&b).unwrap();
    assert_eq!(m.weights, other.weights);
    assert_eq!(m.globals_mask, other.globals_mask);
    assert!(b.len() + dosello_ai::pattern::MODEL.len() < 12_000_000);
    for n in [0, 8, 15, b.len() - 1] {
        assert!(Model::decode(&b[..n]).is_err());
    }
    let mut bad = b;
    bad[8] ^= 1;
    assert!(Model::decode(&bad).is_err());
}
#[test]
fn incremental_search_matches_scratch_minimax_and_restores_after_abort() {
    fn reference(p: Position, d: u8, m: &Model) -> i32 {
        let moves = p.moves();
        if moves.is_empty() {
            return if p.pass().moves().is_empty() {
                p.diff() * p.side as i32
            } else {
                -reference(p.pass(), d, m)
            };
        }
        if d == 0 {
            return m.evaluate(p);
        }
        moves
            .into_iter()
            .map(|mv| -reference(p.play(mv), d - 1, m))
            .max()
            .unwrap()
    }
    let m = std::sync::Arc::new(model());
    let mut seed = 123;
    for _ in 0..4 {
        let p = random_position(&mut seed, 32);
        let mut s = eval_lab::search(None, 5000.);
        s.set_eval3(m.clone());
        s.strict_depth = true;
        for d in 1..=3 {
            assert_eq!(s.run(p, d, -65, 65).unwrap(), reference(p, d, &m));
        }
        s.deadline = 0.;
        assert!(s.run(p, 10, -65, 65).is_err());
        s.deadline = dosello_ai::search::now() + 5000.;
        s.clear();
        assert_eq!(s.run(p, 2, -65, 65).unwrap(), reference(p, 2, &m));
    }
}
#[test]
fn trainer_and_metrics_tiny_roundtrip() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("dosello-eval3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut seed = 754;
    let mut rows: Vec<_> = (0..32)
        .map(|game| eval_lab::Label {
            p: book::canonical(random_position(&mut seed, 30 + (game % 3) as u32 * 2)),
            value: (game % 9) as i8 - 4,
            depth: 4,
            kind: 1,
            game,
        })
        .collect();
    // A duplicate first seen on a holdout game must stay out of training,
    // even when its preferred exact label has a training game id.
    let mut duplicate = rows[0].clone();
    duplicate.game = 999;
    duplicate.kind = 0;
    duplicate.value = 12;
    rows.push(duplicate);
    let labels = dir.join("labels.bin");
    let weights = dir.join("weights.bin");
    std::fs::write(&labels, eval_lab::shard(&rows, "{}")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_train_eval3"))
        .args([
            "--data",
            labels.to_str().unwrap(),
            "--output",
            weights.to_str().unwrap(),
            "--epochs",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let decoded = Model::decode(&std::fs::read(&weights).unwrap()).unwrap();
    assert!(decoded.weights.iter().any(|&w| w != 0));
    let meta = dosello_ai::json::parse(
        &std::fs::read_to_string(weights.with_extension("bin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(meta.get("holdout").unwrap().num().unwrap(), 4);
    assert_eq!(meta.get("train").unwrap().num().unwrap(), 28);
    let source = include_str!("fixtures/eval3-suite.json");
    let suite = dir.join("suite.jsonl");
    std::fs::write(&suite, source.lines().next().unwrap()).unwrap();
    for evaluator in ["old", "eval3"] {
        let out = Command::new(env!("CARGO_BIN_EXE_eval_metrics"))
            .args([
                "--suite",
                suite.to_str().unwrap(),
                "--eval",
                evaluator,
                "--weights",
                weights.to_str().unwrap(),
                "--sample",
                "1",
                "--seed",
                "42",
                "--max-depth",
                "2",
                "--times",
                "1",
                "--threads",
                "1",
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8(out.stdout).unwrap().lines().count(), 3);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn averaged_iterations_use_exact_scores_and_completed_fallback() {
    let mut seed = 871;
    let p = random_position(&mut seed, 26);
    let moves = p.moves();
    let mut s = eval_lab::search(None, 5000.);
    let scores: Vec<_> = moves
        .iter()
        .map(|m| {
            let q = p.play(*m);
            s.clear();
            let a = -s.run(q, 0, -65, 65).unwrap();
            s.clear();
            let b = -s.run(q, 1, -65, 65).unwrap();
            a + b
        })
        .collect();
    let i = (0..moves.len())
        .max_by_key(|&i| (scores[i], std::cmp::Reverse(moves[i].id())))
        .unwrap();
    let result = eval_lab::choose_averaged(p, None, 2, 5000.).unwrap();
    assert_eq!(result.0, Some(moves[i]));
    assert_eq!(result.1, (scores[i] + scores[i].signum()) / 2);
    assert_eq!(result.2, 2);
    // An already exhausted budget must not claim a completed iteration.
    assert_eq!(eval_lab::choose_averaged(p, None, 8, 0.).unwrap().2, 0);
    assert_eq!(eval_lab::choose(p, None, 8, 0.).unwrap().2, 0);
}
