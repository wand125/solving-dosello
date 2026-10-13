use dosello_ai::{board::*, dataset::rng, eval3};
fn reference_targets(p: Position) -> u64 {
    let (own, opp) = (p.own(), p.opp());
    let mut out = 0;
    for d in 0..8 {
        let mut x = shift(own, d) & opp;
        for _ in 0..5 {
            x |= shift(x, d) & opp;
        }
        out |= shift(x, d);
    }
    out & p.empty()
}
fn reference_moves(p: Position) -> Vec<Move> {
    let mut out = vec![];
    for a in 0..64u8 {
        for b in [a + 1, a + 8] {
            if b >= 64 || (b == a + 1 && a % 8 == 7) || p.empty() & (1 << a) == 0 || p.empty() & (1 << b) == 0 {
                continue;
            }
            let m = p.with_flips(Move { a, b, flips: 0 });
            if m.flips != 0 {
                out.push(m);
            }
        }
    }
    out
}
#[test]
fn kernels_match_reference_on_random_playouts() {
    let mut seed = 12345u64;
    let mut checked = 0;
    for _ in 0..3000 {
        let mut p = Position::initial();
        let mut state = eval3::State::new(p);
        loop {
            assert_eq!(p.targets(), reference_targets(p));
            assert_eq!(p.pass().targets(), reference_targets(p.pass()));
            let ms = p.moves();
            assert_eq!(ms, reference_moves(p));
            assert_eq!(p.has_moves(), !ms.is_empty());
            assert_eq!(p.lazy_moves().iter().map(|m| (m.a, m.b)).collect::<Vec<_>>(), ms.iter().map(|m| (m.a, m.b)).collect::<Vec<_>>());
            state.update(p);
            assert_eq!(state, eval3::State::new(p));
            checked += 1;
            if ms.is_empty() {
                if !p.pass().has_moves() { break; }
                p = p.pass();
                continue;
            }
            p = p.play(ms[rng(&mut seed) as usize % ms.len()]);
        }
    }
    assert!(checked > 50000);
}
