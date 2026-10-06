use dosello_ai::{dataset::*, pattern, search::*};
use std::{
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let data = read(args.get(1).map_or("rust/data/train.bin", String::as_str)).unwrap();
    let index = AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..14 {
            let data = &data;
            let index = &index;
            let tx = tx.clone();
            scope.spawn(move || {
                let mut s = Search::new(1 << 17, f64::INFINITY);
                loop {
                    let i = index.fetch_add(1, Ordering::Relaxed);
                    if i >= 9000 {
                        break;
                    }
                    let p = data[(i * 7919) % data.len()].p;
                    if group(p) % 10 == 0 {
                        continue;
                    }
                    for (shallow, deep) in [(2, 4), (3, 6), (4, 8)] {
                        if deep as u32 >= p.empty().count_ones() / 2 {
                            continue;
                        }
                        s.clear();
                        s.deadline = now() + 100.0;
                        s.aborted = false;
                        if let (Ok(x), Ok(y)) =
                            (s.run(p, shallow, -65, 65), s.run(p, deep, -65, 65))
                        {
                            tx.send((pattern::phase(p), shallow, deep, x as f64, y as f64))
                                .unwrap();
                        }
                    }
                }
            });
        }
        drop(tx);
        let mut groups = std::collections::BTreeMap::<_, Vec<(f64, f64)>>::new();
        for (ph, sh, deep, x, y) in rx {
            groups.entry((ph, sh, deep)).or_default().push((x, y));
        }
        let mut f = std::fs::File::create("rust/data/probcut.csv").unwrap();
        writeln!(f, "# phase,shallow,deep,slope,intercept,sigma,n").unwrap();
        for ((ph, sh, deep), v) in groups {
            let n = v.len() as f64;
            let x = v.iter().map(|r| r.0).sum::<f64>() / n;
            let y = v.iter().map(|r| r.1).sum::<f64>() / n;
            let variance = v.iter().map(|r| (r.0 - x).powi(2)).sum::<f64>();
            if variance < 1. || n < 30. {
                continue;
            }
            let slope = v.iter().map(|r| (r.0 - x) * (r.1 - y)).sum::<f64>() / variance;
            let intercept = y - slope * x;
            let sigma = (v
                .iter()
                .map(|r| (r.1 - slope * r.0 - intercept).powi(2))
                .sum::<f64>()
                / (n - 2.))
                .sqrt();
            writeln!(
                f,
                "{ph},{sh},{deep},{slope:.6},{intercept:.6},{sigma:.6},{}",
                v.len()
            )
            .unwrap();
            println!("phase={ph} {sh}->{deep} n={} sigma={sigma:.3}", v.len());
        }
    });
}
