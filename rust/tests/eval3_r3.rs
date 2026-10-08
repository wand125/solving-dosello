use dosello_ai::{
    board::*,
    eval3,
    match_book::MatchBook,
    probcut::{checksum, Config, Table},
    search::Search,
};
use std::sync::Arc;
fn model() -> Arc<eval3::Model> {
    Arc::new(eval3::Model {
        globals_mask: 255,
        weights: (0..eval3::PHASES * eval3::FEATURES)
            .map(|i| ((i * 7919 % 1009) as i16) - 504)
            .collect(),
    })
}
fn csv(m: &eval3::Model) -> String {
    let mut csv = format!("# evalRevision={:016x}; test fixture, deliberately aggressive\n# phase,shallow,deep,slope,intercept,sigma,n\n",checksum(&m.encode()));
    for phase in 0..15 {
        for (sh, deep) in [(1, 2), (2, 4), (3, 6), (4, 8)] {
            csv += &format!("{phase},{sh},{deep},1,100,0,100\n");
        }
    }
    csv
}
fn search(m: Arc<eval3::Model>) -> Search {
    let mut s = Search::new(1 << 15, dosello_ai::search::now() + 10000.);
    s.set_eval3(m);
    s.strict_depth = true;
    s
}
#[test]
fn checksum_and_malformed_table_rejected_and_policy_reset() {
    let m = model();
    let text = csv(&m);
    let table = Arc::new(Table::decode(&text, &m).unwrap());
    let mut other = (*m).clone();
    other.weights[0] += 1;
    assert!(Table::decode(&text, &other).is_err());
    assert!(Table::decode("0,2,4,1,0,2,100", &m).is_err());
    for replacement in ["NaN", "inf", "-1"] {
        assert!(Table::decode(
            &text.replace(",100,0,100", &format!(",100,{replacement},100")),
            &m
        )
        .is_err());
    }
    assert!(Table::decode(&text.replace("0,1,2,", "0,2,2,"), &m).is_err());
    let mut s = search(Arc::new(other));
    assert!(s
        .set_probcut3(Config {
            table: Some(table.clone()),
            ..Default::default()
        })
        .is_err());
    s.set_eval3(m.clone());
    s.set_probcut3(Config {
        table: Some(table),
        ..Default::default()
    })
    .unwrap();
    s.run(Position::initial(), 4, -1, 0).unwrap();
    assert!(s.probcut_probes > 0);
    assert!(s.probcut_cuts > 0);
    s.eval_ordering = true;
    s.set_eval3(m);
    assert!(!s.eval_ordering);
    let before = s.probcut_probes;
    s.run(Position::initial(), 2, -1, 0).unwrap();
    assert_eq!(s.probcut_probes, before);
}
#[test]
fn exact_endgames_ignore_probcut_even_after_selective_search() {
    let m = model();
    let table = Arc::new(Table::decode(&csv(&m), &m).unwrap());
    let mut seed = 75391;
    for _ in 0..6 {
        let p = random_position(&mut seed, 14);
        let mut plain = search(m.clone());
        let expected = plain.exact(p, false).unwrap();
        let mut selective = search(m.clone());
        selective
            .set_probcut3(Config {
                table: Some(table.clone()),
                confidence: 0.1,
                ordering: true,
            })
            .unwrap();
        selective.run(p, 2, -1, 0).unwrap();
        let before = selective.probcut_probes;
        assert_eq!(selective.exact(p, false).unwrap(), expected);
        assert_eq!(selective.probcut_probes, before);
        assert_eq!(selective.exact(p, true).unwrap(), expected.signum());
        assert_eq!(selective.probcut_probes, before);
    }
}
#[test]
fn ordering_preserves_cold_and_iterative_fixed_depth_values() {
    let m = model();
    let mut seed = 87512;
    for _ in 0..4 {
        let p = random_position(&mut seed, 26);
        let mut a = search(m.clone());
        let mut b = search(m.clone());
        b.eval_ordering = true;
        for d in 1..=4 {
            assert_eq!(a.run(p, d, -65, 65).unwrap(), b.run(p, d, -65, 65).unwrap());
        }
        a.clear();
        b.clear();
        a.nodes = 0;
        b.nodes = 0;
        let av = a.run(p, 4, -65, 65).unwrap();
        let bv = b.run(p, 4, -65, 65).unwrap();
        assert_eq!(av, bv);
        eprintln!(
            "ordering fixture depth=4 value={av} plain={} ordered={}",
            a.nodes, b.nodes
        );
    }
}
fn book_bytes(version: u8, omitted: bool, competing_upper: i8) -> Vec<u8> {
    let p = Position::initial();
    let moves = p.moves();
    let mut bytes = format!("DSBOOK0{version}").into_bytes();
    bytes.extend(1u32.to_le_bytes());
    for n in [p.black, p.white, p.hleft, p.vtop] {
        bytes.extend(n.to_le_bytes());
    }
    bytes.extend([p.side as u8, (moves.len() - usize::from(omitted)) as u8]);
    if version == 3 {
        bytes.extend([4, 4, 4]);
    }
    for (i, m) in moves
        .iter()
        .take(moves.len() - usize::from(omitted))
        .enumerate()
    {
        let (v, lo, hi, exact) = if i == 0 {
            (4, 4, 4, 1)
        } else {
            (0, -64, competing_upper, 0)
        };
        bytes.extend([m.a, m.b, v as i8 as u8, lo as i8 as u8, hi as u8, 8, exact]);
    }
    bytes
}
#[test]
fn book_only_plays_proven_and_fully_covered_moves() {
    let p = Position::initial();
    for version in [2, 3] {
        let book = MatchBook::decode(&book_bytes(version, false, 4)).unwrap();
        assert_eq!(book.best(p).unwrap(), p.moves()[0]);
        for t in 0..8 {
            assert!(book.best(dosello_ai::book::transform(p, t)).is_some());
        }
        assert!(MatchBook::decode(&book_bytes(version, true, 4))
            .unwrap()
            .best(p)
            .is_none());
        assert!(MatchBook::decode(&book_bytes(version, false, 5))
            .unwrap()
            .best(p)
            .is_none());
        let bytes = book_bytes(version, false, 4);
        assert!(MatchBook::decode(&bytes[..bytes.len() - 1]).is_err());
    }
}
#[test]
fn shipped_small_book_proves_initial_move() {
    let book = MatchBook::decode(include_bytes!("../../docs/play/wasm/display-book.bin")).unwrap();
    assert!(book.best(Position::initial()).is_some());
    assert!(!book.is_empty());
}
