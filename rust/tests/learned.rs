use dosello_ai::{board::*, book, pattern, search::*};
#[test]
fn model_and_d4_invariance() {
    pattern::validate_model(pattern::MODEL).unwrap();
    let mut seed = 87239;
    for e in (0..=56).step_by(2) {
        for _ in 0..8 {
            let p = random_position(&mut seed, e);
            let value = evaluate(p, Weights::default());
            let mut features = pattern::indices(p);
            features.sort();
            for t in 0..8 {
                let q = book::transform(p, t);
                let mut other = pattern::indices(q);
                other.sort();
                assert_eq!(features, other);
                assert_eq!(value, evaluate(q, Weights::default()));
                assert_eq!(pattern::globals(p), pattern::globals(q));
            }
        }
    }
}
#[test]
fn proof_agrees_with_full_exact_30_to_36() {
    let mut seed = 77591;
    for e in if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { [26, 28, 30, 30] } else { [30, 32, 34, 36] } {
        let p = random_position(&mut seed, e);
        let mut full = dosello_ai::legacy_search::Search::new(1 << 19, f64::INFINITY);
        let expected = full.exact(p, false).unwrap();
        let mut s = Search::new(1 << 19, f64::INFINITY);
        s.set_selectivity(3);
        let (value, m) = s.prove_best(p).unwrap();
        assert_eq!(value, expected);
        if let Some(m) = m {
            assert_eq!(-full.exact(p.play(m), false).unwrap(), value);
        }
    }
}
#[test]
fn statistical_policy_cannot_contaminate_proofs() {
    let mut seed = 198243;
    let mut s = Search::new(4096, f64::INFINITY);
    s.set_selectivity(3);
    let mut old = dosello_ai::legacy_search::Search::new(4096, f64::INFINITY);
    for _ in 0..100 {
        let p = random_position(&mut seed, 20);
        let _ = s.run(p, 6, -8, 8);
        assert_eq!(s.exact(p, false), old.exact(p, false));
    }
}
#[test]
fn malformed_model_rejected() {
    assert!(pattern::validate_model(b"DSEVAL02").is_err());
    let mut v = pattern::MODEL.to_vec();
    v[8] ^= 1;
    assert!(pattern::validate_model(&v).is_err());
}
#[test]
fn verified_selective_seed_matches_exact() {
    let mut seed = 731;
    for _ in 0..20 {
        let p = random_position(&mut seed, 18);
        let mut s = Search::new(4096, now() + 10000.0);
        let mut old = dosello_ai::legacy_search::Search::new(4096, f64::INFINITY);
        assert_eq!(s.selective_exact(p).unwrap(), old.exact(p, false).unwrap());
    }
}
#[test]
fn choose_returns_legal_optimal_endgame() {
    let mut seed = 33419;
    for _ in 0..20 {
        let p = random_position(&mut seed, 12);
        if p.mobility() == 0 {
            continue;
        }
        let r = choose(
            p,
            &Options {
                time_ms: 1000,
                tt_entries: 4096,
                ..Default::default()
            },
        );
        let m = r.best().unwrap();
        assert!(m.exact);
        let mut s = Search::new(4096, f64::INFINITY);
        assert_eq!(m.value, s.exact(p, false).unwrap());
        assert_eq!(m.value, -s.exact(p.play(m.mv), false).unwrap());
    }
}

#[test]
fn opening_coverage_includes_every_two_ply_reply() {
    let p = Position::initial();
    let mut b = book::Book::new(p);
    b.expand_coverage(3, 2, 3);
    for a in p.moves() {
        let child = p.play(a);
        assert!(b.nodes[&book::key(book::canonical(child))].expanded);
        for reply in child.moves() {
            assert!(b.nodes[&book::key(book::canonical(child.play(reply)))].expanded);
        }
    }
}

#[test]
fn model_migration_keeps_exact_records_and_requeues_old_leaves() {
    let mut b = book::Book::new(Position::initial());
    b.expand(b.root);
    let leaf = b.nodes[&b.root].edges[0].1;
    {
        let n = b.nodes.get_mut(&leaf).unwrap();
        n.eval_version = 0;
        n.depth = 6;
        n.evaluated = true;
        n.value = 12;
    }
    let mut seed = 23811;
    let p = random_position(&mut seed, 12);
    let mut end = book::Book::new(p);
    let mut s = Search::new(1024, f64::INFINITY);
    let value = s.exact(p, false).unwrap();
    let k = end.root;
    let mut n = end.nodes.remove(&k).unwrap();
    n.value = value;
    n.lo = value;
    n.hi = value;
    n.depth = 6;
    n.eval_version = 0;
    n.evaluated = true;
    b.nodes.insert(k, n);
    let path = std::env::temp_dir().join(format!("dosello-migration-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    b.checkpoint(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let loaded = book::Book::load(&path, false).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(loaded.nodes[&k].exact());
    assert_eq!(loaded.nodes[&k].value, value);
    assert!(!loaded.nodes[&leaf].evaluated);
    assert_eq!(loaded.nodes[&leaf].depth, 0);
    assert_eq!(loaded.nodes[&leaf].eval_version, pattern::revision());
    std::fs::remove_file(path).unwrap();
}
