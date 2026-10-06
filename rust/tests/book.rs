use dosello_ai::{board::*,book::*,search::*};
use std::collections::BTreeSet;
#[test]
fn symmetry_preserves_pairs_moves_flips_and_play(){
    let mut seed=125781;
    for empty in [56,48,40,30,20,10,4]{for _ in 0..15{
        let p=random_position(&mut seed,empty);
        for t in 0..8{
            let q=transform(p,t);assert_eq!(canonical(p),canonical(q));
            let expected:BTreeSet<_>=p.moves().into_iter().map(|m|{let m=transform_move(m,t);(m.a,m.b,m.flips)}).collect();
            let actual:BTreeSet<_>=q.moves().into_iter().map(|m|(m.a,m.b,m.flips)).collect();assert_eq!(expected,actual);
            for m in p.moves(){assert_eq!(transform(p.play(m),t),q.play(transform_move(m,t)));}
            assert_eq!(transform(p.pass(),t),q.pass());
            for a in 0..64 {assert_eq!(transform(Position{black:p.partners(1<<a),..Default::default()},t).black,q.partners(1<<cell(a,t)));}
        }
    }}
}
fn finish(book:&mut Book,k:Key){
    book.expand(k);
    let edges=book.nodes[&k].edges.clone();
    for (_,c) in edges{if !book.nodes[&c].exact(){finish(book,c);}}
    book.propagate();
}
#[test]
fn propagation_matches_plain_solve_and_pass(){
    let mut seed=48291;let mut pass_seen=false;
    for _ in 0..24{
        let p=random_position(&mut seed,8);let mut b=Book::new(p);let root=b.root;finish(&mut b,root);
        for n in b.nodes.values(){assert!(n.exact());let mut search=Search::new(1024,now()+30000.0);assert_eq!(n.value,search.exact(n.p,false).unwrap());pass_seen|=n.p.moves().is_empty()&&!n.p.pass().moves().is_empty();}
    }
    assert!(pass_seen);
}
#[test]
fn exact_child_can_dominate_bounded_alternatives(){
    let mut b=Book::new(Position::initial());b.expand(b.root);
    let edges=b.nodes[&b.root].edges.clone();let winner=edges[0].1;
    for (_,k) in edges {let n=b.nodes.get_mut(&k).unwrap();n.lo=-8;n.hi=64;n.value=0;}
    let n=b.nodes.get_mut(&winner).unwrap();n.lo=-8;n.hi=-8;n.value=-8;
    b.propagate();assert!(b.nodes[&b.root].exact());assert_eq!(b.nodes[&b.root].value,8);
}
#[test]
fn checkpoint_resume_and_torn_tail(){
    let path=std::env::temp_dir().join(format!("dosello-book-test-{}.jsonl",std::process::id()));let _=std::fs::remove_file(&path);
    let root=Position::initial();
    let mut b=Book::new(root);b.expand(b.root);
    let mut k=b.nodes[&b.root].edges[0].1;
    // The book loader is rooted at the initial position. Keep that root, but
    // descend by legal expansion before evaluating in the small test profile.
    if std::env::var_os("DOSELLO_TEST_SMALL").is_some() {
        while b.nodes[&k].p.empty().count_ones()>30 {
            b.expand(k);
            k=b.nodes[&k].edges.first().expect("fixture path remains playable").1;
        }
    }
    let mut s=Search::new(1024,now()+10000.0);let r=evaluate_leaf(&mut s,b.nodes[&k].p,10,false);b.apply(k,r);b.expand(k);b.propagate();
    b.checkpoint(&path).unwrap();let length=std::fs::metadata(&path).unwrap().len();
    use std::io::Write;std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"version\":1,\"pos").unwrap();
    // Reader/exporter ignores incomplete final record without changing the writer's file.
    let read=Book::load(&path,false).unwrap();assert_eq!(read.completed,b.completed);assert!(std::fs::metadata(&path).unwrap().len()>length);
    let mut resumed=Book::load(&path,true).unwrap();assert_eq!(std::fs::metadata(&path).unwrap().len(),length);
    assert_eq!(resumed.nodes.len(),b.nodes.len());assert_eq!(resumed.completed,b.completed);
    for (k,n) in &b.nodes{let r=&resumed.nodes[k];assert_eq!((n.value,n.lo,n.hi,n.depth,n.expanded,n.visits),(r.value,r.lo,r.hi,r.depth,r.expanded,r.visits));assert_eq!(n.edges,r.edges);}
    resumed.checkpoint(&path).unwrap();let again=Book::load(&path,true).unwrap();assert_eq!(again.analysis(root),b.analysis(root));
    std::fs::remove_file(path).unwrap();
}
#[test]
fn terminal_search_bounds_are_sound(){
    let mut seed=857211;for _ in 0..16 {let p=random_position(&mut seed,12);let mut s=Search::new(1024,now()+10000.0);let exact=s.exact(p,false).unwrap();let mut worker=Search::new(1024,0.0);let r=evaluate_leaf(&mut worker,p,1,true);assert!(r.lo<=exact&&exact<=r.hi);if r.lo==r.hi{assert_eq!(r.value,exact);}}
}
#[test]
fn best_first_deepens_equal_cost_lines(){
    let mut b=Book::new(Position::initial());b.expand(b.root);
    let child=b.nodes[&b.root].edges[0].1;b.expand(child);
    for n in b.nodes.values_mut(){n.value=0;}
    let (_,_,std::cmp::Reverse(k))=b.frontier(&BTreeSet::new(),4).pop().unwrap();
    assert!(b.nodes[&k].p.empty().count_ones()<b.nodes[&child].p.empty().count_ones());
}
#[test]
fn proof_import_intersects_symmetry_and_is_atomic_on_conflict(){
    let p=Position::initial();let mut b=Book::new(p);b.expand(b.root);
    let c=b.nodes[&b.root].edges[0].1;let q=b.nodes[&c].p;
    b.import_bounds(&[(q,4,64),(transform(q,2),6,20)]).unwrap();
    assert_eq!((b.nodes[&c].lo,b.nodes[&c].hi),(6,20));
    let snapshot=(b.nodes[&b.root].lo,b.nodes[&b.root].hi,b.nodes.len());
    assert!(b.import_bounds(&[(p,-64,10),(q,-8,-2)]).is_err());
    assert_eq!((b.nodes[&b.root].lo,b.nodes[&b.root].hi,b.nodes.len()),snapshot);
    assert_eq!((b.nodes[&c].lo,b.nodes[&c].hi),(6,20));
}
#[test]
fn solved_root_and_lines_remain_open_for_human_replies(){
    let mut b=Book::new(Position::initial());b.expand(b.root);
    let edges=b.nodes[&b.root].edges.clone();
    let rows:Vec<_>=edges.iter().map(|(_,c)|(b.nodes[c].p,-2,-2)).collect();
    b.import_bounds(&rows).unwrap();
    assert_eq!((b.nodes[&b.root].lo,b.nodes[&b.root].hi),(2,2));
    let q=b.frontier(&BTreeSet::new(),4);
    assert!(!q.is_empty());
    assert!(q.iter().all(|(_,_,k)|!b.nodes[&k.0].exact()));
    assert!(q.iter().all(|(_,_,k)|b.nodes[&k.0].p.empty().count_ones()<=52));
}
