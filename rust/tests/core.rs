use dosello_ai::{api, board::*, json, search::*};
fn naive(p: Position) -> i32 {
    let ms = p.moves();
    if ms.is_empty() {
        return if p.pass().moves().is_empty() {
            p.diff() * p.side as i32
        } else {
            -naive(p.pass())
        };
    }
    ms.into_iter().map(|m| -naive(p.play(m))).max().unwrap()
}
#[test]
fn initial() {
    let p = Position::initial();
    assert_eq!(p.diff(), 0);
    assert_eq!(p.empty().count_ones(), 56);
    assert_eq!(p.moves().len(), 20);
    assert_eq!(p.validate().unwrap(), p);
    for m in p.moves() {
        let q = p.play(m);
        assert_eq!(q.validate().unwrap(), q);
        assert_eq!(q.empty().count_ones(), 54);
    }
}
#[test]
fn roundtrip() {
    let mut seed = 23;
    for n in 0..28 {
        let p = random_position(&mut seed, n * 2);
        assert_eq!(
            p,
            json::position(&json::parse(&json::position_json(p)).unwrap()).unwrap()
        );
    }
}
#[test]
fn minimax_and_tt() {
    let mut seed = 8123;
    let mut shared = Search::new(1 << 14, f64::INFINITY);
    for i in 0..120 {
        let empties = if i < 30 { 12 } else { 10 };
        let mut p = random_position(&mut seed, empties);
        while p.empty().count_ones() > empties || p.moves().is_empty() {
            p = random_position(&mut seed, empties);
        }
        assert!(p.empty().count_ones() <= 12);
        let v = naive(p);
        let mut small = Search::new(1, f64::INFINITY);
        assert_eq!(v, small.exact(p, false).unwrap());
        assert_eq!(v, shared.exact(p, false).unwrap());
        assert_eq!(v.signum(), shared.exact(p, true).unwrap());
        assert_eq!(v, shared.exact(p, false).unwrap());
    }
}
#[test]
fn api_shape() {
    let out = api(r#"{"position":"","options":{"timeMs":10,"exactIfPossible":false}}"#).unwrap();
    let parsed = json::parse(&out).unwrap();
    assert_eq!(parsed.get("moves").unwrap().arr().unwrap().len(), 20);
    assert!(out.contains("\"moves\":["));
    assert!(out.contains("\"bestMove\":"));
    assert!(out.contains("\"perspective\":\"side-to-move\""));
    assert!(api(r#"{"position":"z9-z8"}"#).is_err());
}
#[test]
fn terminals_and_pass() {
    let mut seed = 33;
    let mut passes = 0;
    for _ in 0..500 {
        let p = random_position(&mut seed, 4);
        if p.moves().is_empty() && !p.pass().moves().is_empty() {
            passes += 1;
            let mut s = Search::new(1024, f64::INFINITY);
            assert_eq!(s.exact(p, false).unwrap(), naive(p));
            assert_eq!(perft(p, 1), 1);
        }
    }
    assert!(passes > 0);
    let p = Position {
        black: 3,
        hleft: 1,
        side: -1,
        ..Default::default()
    };
    assert_eq!(naive(p), -2);
    assert_eq!(perft(p, 7), 1);
}
#[test]
fn malformed_positions() {
    assert!(json::position(&json::parse(r#"{"board":[],"shape":[],"turn":1}"#).unwrap()).is_err());
    let mut p = Position::initial();
    p.hleft |= 1 << 7;
    assert!(p.validate().is_err());
}

#[test]
fn all_move_exact_values() {
    let mut seed = 983751;
    for _ in 0..20 {
        let p = random_position(&mut seed, 10);
        let a = analyze(
            p,
            &Options {
                time_ms: 5000,
                threads: 2,
                tt_entries: 2048,
                ..Default::default()
            },
        );
        assert!(a.complete);
        for row in a.moves {
            assert!(row.exact);
            assert_eq!(row.value, -naive(p.play(row.mv)));
        }
    }
}
#[test]
fn non_monochrome_pairs() {
    let mut p = Position::initial();
    p.black ^= 1 << 26;
    p.white ^= 1 << 26;
    assert!(p.validate().is_ok());
    assert_eq!(p.partners(1 << 26), 1 << 27);
}

#[test]
fn incremental_keys_and_mobility() {
    let mut seed=99123;
    for empties in (0..=56).step_by(2) {
        for _ in 0..30 {
            let p=random_position(&mut seed,empties);
            assert_eq!(p.pass_hash(p.hash()),p.pass().hash());
            assert_eq!(p.mobility() as usize,p.moves().len());
            for m in p.moves() {
                assert_eq!(p.child_hash(p.hash(),m),p.play(m).hash());
                assert_eq!(m.flips,p.capture((1<<m.a)|(1<<m.b)));
            }
        }
    }
}
#[test]
fn shared_table_parallel_exact() {
    let table=std::sync::Arc::new(SharedTable::new(2));
    std::thread::scope(|scope| {
        for worker in 0..if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { 2 } else { 8 } {
            let table=table.clone();
            scope.spawn(move || {
                let mut seed=12567+worker;
                let mut s=Search::new(2,f64::INFINITY);s.shared=Some(table);
                for _ in 0..50 {
                    let p=random_position(&mut seed,14);
                    assert_eq!(s.exact(p,false).unwrap(),naive(p));
                }
            });
        }
    });
}
#[test]
fn safe_score_bounds() {
    let mut seed=756312;
    for _ in 0..500 {
        let p=random_position(&mut seed,12);
        let max=p.occupancy_bound();
        let lower=2*p.stable_edges(p.own()).count_ones() as i32-max;
        let upper=max-2*p.stable_edges(p.opp()).count_ones() as i32;
        let value=naive(p);
        assert!(lower<=value && value<=upper,"{lower} <= {value} <= {upper}: {p:?}");
    }
}
