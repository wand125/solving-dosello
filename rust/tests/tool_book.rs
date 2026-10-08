use dosello_ai::{board::Position, book::transform, match_book::MatchBook};
#[test]
fn shipped_tool_roundtrip_and_validation() {
    let bytes = include_bytes!("../../docs/play/wasm/tool-book.bin");
    let book = MatchBook::decode(bytes).unwrap();
    assert_eq!(book.len(), 10977);
    assert_eq!(book.encode_tool().unwrap(), bytes);
    for t in 0..8 {
        let p = transform(Position::initial(), t);
        assert!(p.moves().contains(&book.best(p).unwrap()));
        assert_eq!(book.value(p), Some(2));
    }
    assert!(MatchBook::decode(&bytes[..bytes.len()-1]).is_err());
    let mut bad = bytes.to_vec();
    bad[45] = 64;
    assert!(MatchBook::decode(&bad).is_err());
}
