use dosello_ai::{board::*,search::*};
#[test]
fn parallel_proofs_and_leaf_thresholds() {
    let mut seed=123789;
    for _ in 0..24 {
        let p=random_position(&mut seed,16);
        let mut reference=Search::new(4096,f64::INFINITY);
        reference.last_empties=4;
        let expected=reference.exact(p,false).unwrap();
        for threshold in [6,8,10] {
            let mut s=Search::new(4,f64::INFINITY);s.last_empties=threshold;
            assert_eq!(s.exact(p,false).unwrap(),expected);
        }
        for threads in if std::env::var_os("DOSELLO_TEST_SMALL").is_some() {[1,2,2]} else {[1,2,4]} {assert_eq!(parallel_exact(p,16,threads,f64::INFINITY).0,Some(expected));}
    }
}
#[test]
fn persistent_scores_restart_and_corruption() {
    use dosello_ai::exact_cache::ExactCache;
    use std::io::Write;
    let path=std::env::temp_dir().join(format!("dosello-speed-cache-{}.bin",std::process::id()));
    let _=std::fs::remove_file(&path);
    let mut seed=891234;
    let p=random_position(&mut seed,16);
    let v=Search::new(1024,f64::INFINITY).exact(p,false).unwrap();
    let cache=std::sync::Arc::new(ExactCache::load(&path,10).unwrap());
    cache.insert(p,v);assert_eq!(cache.checkpoint().unwrap(),1);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"torn").unwrap();
    let restored=std::sync::Arc::new(ExactCache::load(&path,10).unwrap());
    let mut search=Search::new(4,f64::INFINITY);search.exact_cache=Some(restored.clone());
    assert_eq!(search.run(p,(p.empty().count_ones()/2) as u8,-65,65).unwrap(),v);
    assert_eq!(search.nodes,1);
    assert_eq!(restored.hits.load(std::sync::atomic::Ordering::Relaxed),1);
    let mut bytes=std::fs::read(&path).unwrap();bytes[12]^=1;std::fs::write(&path,bytes).unwrap();
    assert!(ExactCache::load(&path,10).is_err());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn lazy_flips_agree() {
    let mut seed=768234;
    for empties in (0..=56).step_by(2) {for _ in 0..20 {
        let p=random_position(&mut seed,empties);
        assert_eq!(p.moves(),p.lazy_moves().iter().map(|m|p.with_flips(*m)).collect::<Vec<_>>());
    }}
}
