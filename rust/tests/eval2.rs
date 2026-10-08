use dosello_ai::{board::*, book, eval2::*, eval_lab::*, pattern};
#[test]
fn d4_features_and_predictions() {
    let mut seed = 1701;
    let model = Model {
        weights: (0..PHASES * FEATURES)
            .map(|i| (i % 101) as i16 - 50)
            .collect(),
    };
    for e in [12, 22, 34, 46] {
        let p = random_position(&mut seed, e);
        let mut ids = indices(p);
        ids.sort();
        let mut old = pattern::indices(p);
        old.sort();
        for t in 0..8 {
            let q = book::transform(p, t);
            let mut other = indices(q);
            other.sort();
            assert_eq!(ids, other);
            let mut old2 = pattern::indices(q);
            old2.sort();
            assert_eq!(old, old2);
            assert_eq!(globals(p), globals(q));
            assert_eq!(model.evaluate(p), model.evaluate(q));
        }
    }
}
#[test]
fn pairing_changes_features() {
    let p = Position {
        black: 0x303,
        white: 0,
        hleft: 0x101,
        vtop: 0,
        side: 1,
    }
    .validate()
    .unwrap();
    let q = Position {
        hleft: 0,
        vtop: 3,
        ..p
    }
    .validate()
    .unwrap();
    assert_eq!(pattern::indices(p), pattern::indices(q));
    assert_ne!(indices(p), indices(q));
    for t in 0..8 {
        assert_ne!(
            indices(book::transform(p, t)),
            indices(book::transform(q, t))
        );
    }
}
#[test]
fn schema_and_budget() {
    let m = Model {
        weights: vec![0; PHASES * FEATURES],
    };
    let b = m.encode();
    assert!(b.len() + pattern::MODEL.len() + 2_000_000 < 12_000_000);
    assert!(Model::decode(&b).is_ok());
    assert!(Model::decode(&b[..b.len() - 1]).is_err());
    let mut wrong = b;
    wrong[16] ^= 1;
    assert!(Model::decode(&wrong).is_err());
    assert_eq!(
        m.evaluate(Position::initial()),
        pattern::evaluate(Position::initial())
    );
}
#[test]
fn sampling_repeatable_and_game_groups() {
    for i in 0..6 {
        let (p, g) = sample(123, i, 26, None, &[], 1).unwrap();
        let (q, h) = sample(123, i, 26, None, &[], 1).unwrap();
        assert_eq!(p, q);
        assert_eq!(g, h);
        assert_eq!(p.empty().count_ones(), 26);
        assert_eq!(book::canonical(p), p);
    }
}
#[test]
fn exact_choice_agrees_with_all_moves() {
    let mut seed = 777;
    let p = random_position(&mut seed, 16);
    let (m, v, d, _) = choose(p, None, 12, 10000.).unwrap();
    assert_eq!(d, 8);
    let mut s = search(None, 10000.);
    let best = p
        .moves()
        .iter()
        .map(|m| -s.exact(p.play(*m), false).unwrap())
        .max()
        .unwrap();
    assert_eq!(v, best);
    assert_eq!(-s.exact(p.play(m.unwrap()), false).unwrap(), best);
}
#[test]
fn custom_eval_tt_isolation() {
    let mut seed = 789;
    let p = random_position(&mut seed, 26);
    let mut s = search(None, 10000.);
    let old = s.run(p, 2, -65, 65).unwrap();
    let m = std::sync::Arc::new(Model {
        weights: vec![0; PHASES * FEATURES],
    });
    s.set_evaluator(Some(m));
    assert_eq!(old, s.run(p, 2, -65, 65).unwrap());
}
