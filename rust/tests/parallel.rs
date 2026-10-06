use dosello_ai::{board::*, search::*};

#[test]
fn exact_and_best_agree_on_204_random_positions_26_to_36() {
    let mut seed = 0x77aab3128;
    for i in 0..204 {
        let small = std::env::var_os("DOSELLO_TEST_SMALL").is_some();
        let empties = 26 + 2 * (i % if small { 3 } else { 6 });
        let p = loop {
            let p = random_position(&mut seed, empties);
            if p.empty().count_ones() == empties {
                break p;
            }
        };
        let expected = Search::new(1 << 20, f64::INFINITY).exact(p, false).unwrap();
        let threads = if small { 2 } else { [2, 4, 8, 14][i as usize % 4] };
        let (proof, _) = parallel_prove_best(p, 1 << 20, threads, f64::INFINITY);
        let (value, mv) = proof.expect("complete parallel proof");
        assert_eq!(
            value, expected,
            "case {i}, {empties} empties, {threads} workers"
        );
        if let Some(m) = mv {
            assert!(p.moves().contains(&m));
            assert_eq!(
                -Search::new(1 << 20, f64::INFINITY)
                    .exact(p.play(m), false)
                    .unwrap(),
                expected,
                "proved child {i}"
            );
        } else {
            assert_eq!(p.mobility(), 0);
        }
        if i % 12 == 0 {
            eprintln!("validated {i}/204");
        }
    }
}

#[test]
fn deadline_cancels_workers_without_publishing_a_proof() {
    for threads in if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { [1, 2] } else { [2, 14] } {
        let start = now();
        let (result, _) = parallel_prove_best(if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { random_position(&mut 923891, 30) } else { Position::initial() }, 1 << 16, threads, start + if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { 0.0 } else { 20.0 });
        assert!(result.is_none());
        assert!(now() - start < 5000.0, "workers must drain promptly");
    }
}

#[test]
fn checkpoint_resumes_bounds_and_rejects_damage_or_wrong_position() {
    let mut seed = 938237;
    let p = random_position(&mut seed, 26);
    let path = std::env::temp_dir().join(format!("dosello-proof-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut opts = Options {
        threads: if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { 2 } else { 4 },
        time_ms: 0,
        tt_entries: 1 << 16,
        ..Default::default()
    };
    let partial = prove_checkpoint_json(p, &opts, &path).unwrap();
    assert!(partial.contains("\"proofComplete\":false"));
    opts.time_ms = 60000;
    let result = prove_checkpoint_json(p, &opts, &path).unwrap();
    let v = dosello_ai::json::parse(&result).unwrap();
    assert!(v.get("proofComplete").unwrap().boolean().unwrap());
    let expected = Search::new(1 << 18, f64::INFINITY).exact(p, false).unwrap();
    assert_eq!(v.get("value").unwrap().num().unwrap(), expected as i64);
    let resumed = prove_checkpoint_json(p, &opts, &path).unwrap();
    let resumed = dosello_ai::json::parse(&resumed).unwrap();
    assert_eq!(resumed.get("nodes").unwrap().num().unwrap(), 0);
    assert_eq!(
        resumed.get("value").unwrap().num().unwrap(),
        expected as i64
    );
    assert!(prove_checkpoint_json(p.pass(), &opts, &path).is_err());
    let mut data = std::fs::read(&path).unwrap();
    let at = data.iter().position(|&b| b == b':').unwrap() + 2;
    data[at] = if data[at] == b'0' { b'1' } else { b'0' };
    std::fs::write(&path, data).unwrap();
    assert!(prove_checkpoint_json(p, &opts, &path).is_err());
    std::fs::remove_file(&path).unwrap();

    let bare = std::path::PathBuf::from(format!(".dosello-proof-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&bare);
    let result = prove_checkpoint_json(p, &opts, &bare).unwrap();
    assert!(result.contains("\"proofComplete\":true"));
    std::fs::remove_file(bare).unwrap();
}
