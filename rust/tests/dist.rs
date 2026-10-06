use dosello_ai::{board::*, book, dist, json, search::*};

#[test]
fn initial_symmetry_groups() {
    let p = Position::initial();
    assert_eq!(book::transform(p, 2), p);
    let v = json::parse(&dist::inspect(p)).unwrap();
    let children = v.get("children").unwrap().arr().unwrap();
    assert_eq!(children.len(), 10);
    assert!(children.iter().all(|c| c.get("moves").unwrap().arr().unwrap().len() == 2));
}

#[test]
fn jobs_bound_exact_resume_and_corruption() {
    let path = std::env::temp_dir().join(format!("dosello-dist-test-{}.json", std::process::id()));
    let mut seed = 856431;
    let o = Options { threads: 2, time_ms: 10000, tt_entries: 1 << 16, ..Default::default() };
    for i in 0..16 {
        let p = random_position(&mut seed, 26 + 2*(i%3));
        let expected = Search::new(1 << 16, now()+10000.).exact(p,false).unwrap();
        for t in [-64, expected-1, expected, expected+1, 65].into_iter().filter(|t|(-64..=65).contains(t)) {
            let _ = std::fs::remove_file(&path);
            let r = json::parse(&dist::solve(p,&o,"bound",t,&path).unwrap()).unwrap();
            let lo = r.get("lower").unwrap().num().unwrap() as i32;
            let hi = r.get("upper").unwrap().num().unwrap() as i32;
            assert!(lo<=expected && expected<=hi);
            assert!(r.get("complete").unwrap().boolean().unwrap());
            assert_eq!(r.get("outcome").unwrap().str().unwrap(), if expected>=t {"fail-high"} else {"fail-low"});
            let r = json::parse(&dist::solve(p,&o,"exact",0,&path).unwrap()).unwrap();
            assert_eq!(r.get("value").unwrap().num().unwrap(),expected as i64);
            let resumed = json::parse(&dist::solve(p,&o,"exact",0,&path).unwrap()).unwrap();
            assert_eq!(resumed.get("nodes").unwrap().num().unwrap(),0);
            assert!(dist::solve(p.pass(),&o,"exact",0,&path).is_err());
        }
    }
    std::fs::write(&path,"{}\n").unwrap();
    assert!(dist::solve(Position::initial(),&o,"exact",0,&path).is_err());
    std::fs::remove_file(path).unwrap();
}
