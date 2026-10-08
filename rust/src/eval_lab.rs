//! Native evaluation experiments; all files are separate from production assets.
use crate::{board::*, book, dataset::rng, eval2, json, pattern, search::*, tree};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
    sync::Arc,
};
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone)]
pub enum Evaluator { Eval2(Arc<eval2::Model>), Eval3(Arc<crate::eval3::Model>) }
impl Evaluator {
    pub fn evaluate(&self,p:Position)->i32 {match self {Self::Eval2(m)=>m.evaluate(p),Self::Eval3(m)=>m.evaluate(p)}}
    pub fn encode(&self)->Vec<u8> {match self {Self::Eval2(m)=>m.encode(),Self::Eval3(m)=>m.encode()}}
}
pub struct Args(pub BTreeMap<String, String>);
impl Args {
    pub fn read() -> Result<Self> {
        let mut out = BTreeMap::new();
        let mut a = std::env::args().skip(1);
        while let Some(k) = a.next() {
            if !k.starts_with("--") {
                return Err(format!("expected option, got {k}"));
            }
            out.insert(k, a.next().ok_or("option needs value")?);
        }
        Ok(Self(out))
    }
    pub fn get(&self, k: &str, d: &str) -> String {
        self.0.get(k).cloned().unwrap_or(d.into())
    }
    pub fn number<T: std::str::FromStr>(&self, k: &str, d: &str) -> Result<T> {
        self.get(k, d).parse().map_err(|_| format!("invalid {k}"))
    }
    pub fn model(&self) -> Result<Option<Arc<Evaluator>>> {
        match self.get("--eval", "old").as_str() {
            "old" => Ok(None),
            "new" => Ok(Some(Arc::new(Evaluator::Eval2(Arc::new(eval2::Model::decode(
                &std::fs::read(self.get("--weights", "rust/data/eval2.bin"))
                    .map_err(|e| e.to_string())?,
            )?))))),
            "eval3" => Ok(Some(Arc::new(Evaluator::Eval3(Arc::new(crate::eval3::Model::decode(&std::fs::read(self.get("--weights", "rust/data/eval3-r2.bin")).map_err(|e|e.to_string())?)?))))),
            _ => Err("--eval old|new|eval3".into()),
        }
    }
    pub fn search_config(&self, model: &Option<Arc<Evaluator>>) -> Result<crate::probcut::Config> {
        let confidence: f64 = self.number("--probcut-t", "5")?;
        let ordering: u8 = self.number("--eval-ordering", "0")?;
        if !confidence.is_finite() || confidence <= 0. || ordering > 1 { return Err("positive finite probcut-t; eval-ordering 0|1".into()); }
        let path = self.get("--probcut", "");
        let table = if path.is_empty() { None } else {
            let Some(Evaluator::Eval3(m)) = model.as_deref() else { return Err("--probcut requires --eval eval3".into()); };
            Some(Arc::new(crate::probcut::Table::decode(&std::fs::read_to_string(path).map_err(|e|e.to_string())?, m)?))
        };
        Ok(crate::probcut::Config { table, confidence, ordering: ordering != 0 })
    }
    pub fn threads(&self) -> Result<usize> {
        let n = self.number("--threads", "1")?;
        if !(1..=256).contains(&n) {
            return Err("threads must be 1..256".into());
        }
        Ok(n)
    }
}
pub fn atomic(path: &str, b: &[u8]) -> Result<()> {
    let tmp = format!("{path}.tmp");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    f.write_all(b)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, path).map_err(|e| e.to_string())
}
pub fn checksum(b: &[u8]) -> u64 {
    b.iter().fold(14695981039346656037u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(1099511628211)
    })
}
pub fn mix(mut n: u64) -> u64 {
    n = n.wrapping_add(0x9e3779b97f4a7c15);
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    (n ^ (n >> 31)).max(1)
}
pub fn search(model: Option<Arc<Evaluator>>, ms: f64) -> Search {
    let mut s = Search::new(1 << 18, now() + ms);
    match model.as_deref() {
        Some(Evaluator::Eval2(m))=>s.set_evaluator(Some(m.clone())),
        Some(Evaluator::Eval3(m))=>s.set_eval3(m.clone()),None=>{}
    }
    s
}
pub fn choose(
    p: Position,
    model: Option<Arc<Evaluator>>,
    depth: u8,
    ms: f64,
) -> Result<(Option<Move>, i32, u8, u64)> {
    choose_options(p,model,depth,ms,false,false,false)
}
pub fn choose_options(p:Position,model:Option<Arc<Evaluator>>,depth:u8,ms:f64,cold:bool,strict:bool,ordering:bool)->Result<(Option<Move>,i32,u8,u64)> {
    choose_configured(p,model,depth,ms,cold,strict,&crate::probcut::Config {ordering,..Default::default()})
}
pub fn choose_configured(p:Position,model:Option<Arc<Evaluator>>,depth:u8,ms:f64,cold:bool,strict:bool,config:&crate::probcut::Config)->Result<(Option<Move>,i32,u8,u64)> {
    let mut s = search(model, ms);
    s.set_probcut3(config.clone())?;
    s.strict_depth=strict;
    let moves = p.moves();
    let mut best = moves
        .iter()
        .copied()
        .max_by_key(|m| -s.static_value(p.play(*m)));
    let mut value = s.static_value(p);
    let mut done = 0;
    let max = depth.min((p.empty().count_ones() / 2) as u8).max(1);
    for d in 1..=max {
        if now() >= s.deadline {
            break;
        }
        if cold {s.clear();}
        let Ok(v) = s.run(p, d, -65, 65) else { break };
        let candidate = if (d as u32) < p.empty().count_ones() / 2 {
            s.best_move(p)
        } else {
            // Full-depth endgame leaves can bypass TT storage. Prove an attaining move.
            let mut found = None;
            for m in &moves {
                if now() >= s.deadline {
                    break;
                }
                let Ok(cv) = s.run(p.play(*m), d.saturating_sub(1), -65, 65) else {
                    break;
                };
                if -cv == v {
                    found = Some(*m);
                    break;
                }
            }
            found
        };
        if !moves.is_empty() && candidate.is_none() {
            break;
        }
        best = candidate;
        value = v;
        done = d;
    }
    Ok((best, value, done, s.nodes))
}
/// Optional horizon-variance reduction. Complete *all* root children before
/// publishing an iteration; blend unrounded scores of adjacent placement depths.
/// More expensive than PVS; benchmark equal-time before adopting it.
pub fn choose_averaged(p:Position,model:Option<Arc<Evaluator>>,depth:u8,ms:f64)->Result<(Option<Move>,i32,u8,u64)> {
    let mut moves=p.moves();moves.sort_by_key(|m|m.id());
    if moves.is_empty(){return choose(p,model,depth,ms)}
    let mut s=search(model,ms);s.strict_depth=true;
    let mut best=Some(*moves.iter().max_by_key(|m|-s.static_value(p.play(**m))).unwrap());
    let mut value=s.static_value(p);let mut done=0;let mut previous:Option<Vec<i32>>=None;
    let terminal=(p.empty().count_ones()/2) as u8;
    for d in 1..=depth.min(terminal).max(1) {
        let mut current=Vec::with_capacity(moves.len());
        for m in &moves {
            if now()>=s.deadline {break}
            match s.run(p.play(*m),d-1,-65,65) {Ok(v)=>current.push(-v),Err(_)=>break}
        }
        if current.len()!=moves.len(){break}
        let scores:Vec<_>=current.iter().enumerate().map(|(i,v)|
            if d==terminal {2*v} else {v+previous.as_ref().map_or(*v,|last|last[i])}).collect();
        // Stable move-id tie break, unaffected by TT ordering.
        let i=(0..moves.len()).max_by_key(|&i|(scores[i],std::cmp::Reverse(moves[i].id()))).unwrap();
        best=Some(moves[i]);value=(scores[i]+scores[i].signum())/2;done=d;previous=Some(current);
    }
    Ok((best,value,done,s.nodes))
}
#[derive(Clone, Debug)]
pub struct Label {
    pub p: Position,
    pub value: i8,
    pub depth: u8,
    pub kind: u8,
    pub game: u64,
}
pub fn shard(rows: &[Label], metadata: &str) -> Vec<u8> {
    let mut b = b"DSLABEL1".to_vec();
    b.extend((metadata.len() as u32).to_le_bytes());
    b.extend(metadata.bytes());
    for r in rows {
        for n in [r.p.black, r.p.white, r.p.hleft, r.p.vtop] {
            b.extend(n.to_le_bytes())
        }
        b.extend([r.p.side as u8, r.value as u8, r.depth, r.kind]);
        b.extend(r.game.to_le_bytes())
    }
    b
}
pub fn labels(path: &str) -> Result<Vec<Label>> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    if b.len() < 12 || &b[..8] != b"DSLABEL1" {
        return Err("label magic".into());
    }
    let start = 12 + u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    if start > b.len() || (b.len() - start) % 44 != 0 {
        return Err("label length".into());
    }
    json::parse(std::str::from_utf8(&b[12..start]).map_err(|e| e.to_string())?)?;
    b[start..]
        .chunks_exact(44)
        .map(|r| {
            let n = |i| u64::from_le_bytes(r[i..i + 8].try_into().unwrap());
            let p = Position {
                black: n(0),
                white: n(8),
                hleft: n(16),
                vtop: n(24),
                side: r[32] as i8,
            }
            .validate()?;
            if book::canonical(p) != p || r[35] > 1 || !(-64..=64).contains(&(r[33] as i8)) {
                return Err("label record".into());
            }
            Ok(Label {
                p,
                value: r[33] as i8,
                depth: r[34],
                kind: r[35],
                game: n(36),
            })
        })
        .collect()
}
pub fn book_seeds(path: &str) -> Result<Vec<Position>> {
    if path.is_empty() {
        return Ok(vec![]);
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let mut out = vec![];
    let mut seed = 98111;
    for (i, l) in std::io::BufReader::new(file.take(len)).lines().enumerate() {
        let l = l.map_err(|e| e.to_string())?;
        let v = json::parse(&l)?;
        let p = json::position(v.get("position")?)?;
        if out.len() < 256 {
            out.push(p)
        } else {
            let j = rng(&mut seed) as usize % (i + 1);
            if j < 256 {
                out[j] = p
            }
        }
    }
    Ok(out)
}
/// One independently seeded game per candidate; provenance is stable across hosts/threads.
pub fn sample(
    seed: u64,
    index: u64,
    empties: u32,
    model: Option<Arc<Evaluator>>,
    books: &[Position],
    sampling_depth: u8,
) -> Result<(Position, u64)> {
    if empties > 54 || empties % 2 != 0 {
        return Err("empties must be even, <=54".into());
    }
    for retry in 0..1000 {
        let game = mix(seed ^ mix(index) ^ mix(retry));
        let mut state = game;
        let source = index % 3;
        let mut p = Position::initial();
        let mut group = game;
        if source == 1 && !books.is_empty() {
            let root = books[rng(&mut state) as usize % books.len()];
            if root.empty().count_ones() >= empties {
                p = root;
                group = mix(book::canonical(root).hash())
            }
        }
        let mut ply = 0;
        while p.empty().count_ones() > empties {
            let moves = p.moves();
            if moves.is_empty() {
                p = p.pass();
                if p.mobility() == 0 {
                    break;
                }
                continue;
            }
            let m = if source == 2 || ply < 4 || rng(&mut state) % 4 == 0 {
                moves[rng(&mut state) as usize % moves.len()]
            } else {
                let mut s = search(model.clone(), f64::INFINITY);
                let mut scores = vec![];
                for m in &moves {
                    let v = s
                        .run(p.play(*m), sampling_depth.saturating_sub(1), -65, 65)
                        .map_err(|_| "sampling search")?;
                    scores.push((-v as f64 / 6.).exp())
                }
                let mut pick =
                    rng(&mut state) as f64 / u64::MAX as f64 * scores.iter().sum::<f64>();
                let mut choice = moves[moves.len() - 1];
                for (m, w) in moves.iter().zip(scores) {
                    pick -= w;
                    if pick <= 0. {
                        choice = *m;
                        break;
                    }
                }
                choice
            };
            p = p.play(m);
            ply += 1;
        }
        if p.empty().count_ones() == empties && !p.moves().is_empty() {
            return Ok((book::canonical(p), group));
        }
    }
    Err("unable to sample phase".into())
}
pub fn label_main(a: &Args) -> Result<()> {
    let count: usize = a.number("--count", "16")?;
    let seed: u64 = a.number("--seed", "1")?;
    let low: u32 = a.number("--min-empties", "22")?;
    let high: u32 = a.number("--max-empties", "34")?;
    let mode = a.get("--label-mode", "mixed");
    let exact_max: u32 = a.number("--exact-max", "34")?;
    let depth: u8 = a.number("--depth", "10")?;
    let budget: u64 = a.number("--label-ms", "5000")?;
    let sample_depth: u8 = a.number("--sampling-depth", "2")?;
    let workers = a.threads()?;
    if count == 0
        || low > high
        || high > 54
        || low % 2 != 0
        || high % 2 != 0
        || depth == 0
        || depth > 28
        || sample_depth == 0
        || sample_depth > 8
        || budget == 0
        || !["mixed", "exact", "deep"].contains(&mode.as_str())
    {
        return Err("invalid generation configuration".into());
    }
    let model = a.model()?;
    let books = book_seeds(&a.get("--book", ""))?;
    let start = now();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..workers.min(count) {
            let tx = tx.clone();
            let next = &next;
            let model = model.clone();
            let books = &books;
            let mode = &mode;
            scope.spawn(move||{loop{let i=next.fetch_add(1,std::sync::atomic::Ordering::Relaxed);if i>=count{break}let result:Result<Label>=(||{let e=low+2*(mix(seed^i as u64)%((high-low)/2+1) as u64) as u32;let (p,game)=sample(seed,i as u64,e,model.clone(),books,sample_depth)?;let exact=mode=="exact"||mode=="mixed"&&e<=exact_max;
        let (value,d,kind)=if exact{let mut s=search(model.clone(),budget as f64);(s.exact(p,false).map_err(|_|format!("exact timeout at candidate {i}; reduce batch/phase or increase label-ms"))?,(e/2) as u8,0)}else{let (_,v,d,_)=choose(p,model.clone(),depth,budget as f64)?;if d==0{return Err("no completed label iteration".into())}(v,d,if d as u32>=e/2{0}else{1})};Ok(Label{p,value:value as i8,depth:d,kind,game})})();if tx.send((i,result)).is_err(){break}}});
        }
        drop(tx);
    });
    let mut rows = BTreeMap::new();
    for (i, r) in rx {
        rows.insert(i, r?);
    }
    let rows: Vec<_> = rows.into_values().collect();
    let revision = model
        .as_ref()
        .map_or(pattern::revision() as u64, |m| checksum(&m.encode()));
    let metadata=format!("{{\"version\":1,\"seed\":{seed},\"evalRevision\":\"{revision:016x}\",\"labelMode\":{},\"minEmpties\":{low},\"maxEmpties\":{high},\"depth\":{depth},\"labelMs\":{budget},\"samplingDepth\":{sample_depth},\"book\":{},\"recordBytes\":44}}",json::quote(&mode),json::quote(&a.get("--book","")));
    let b = shard(&rows, &metadata);
    let output = a.get("--output", "labels.bin");
    atomic(&output, &b)?;
    println!("{{\"count\":{},\"exact\":{},\"elapsedMs\":{:.3},\"checksumFNV64\":\"{:016x}\",\"output\":{}}}",rows.len(),rows.iter().filter(|r|r.kind==0).count(),now()-start,checksum(&b),json::quote(&output));
    Ok(())
}
pub fn suite_main(a: &Args) -> Result<()> {
    if a.get("--inspect-stream", "0") == "1" {
        for line in std::io::stdin().lock().lines() {
            let line = line.map_err(|e| e.to_string())?;
            let request = json::parse(&line)?;
            println!("{}", tree::inspect(tree::position(request.str()?)?));
            std::io::stdout().flush().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let inspect = a.get("--inspect-key", "");
    if !inspect.is_empty() {
        println!("{}", tree::inspect(tree::position(&inspect)?));
        return Ok(());
    }
    let req = a.get("--request-file", "");
    if !req.is_empty() {
        let raw = std::fs::read_to_string(req).map_err(|e| e.to_string())?;
        return tree::suite_batch(
            &raw,
            &Options {
                threads: a.threads()?,
                time_ms: a.number("--time", "120000")?,
                tt_entries: entries_for_mb(a.number("--tt-mb", "64")?)?,
                selectivity: 0,
                ..Default::default()
            },
        );
    }
    let phases = a.get("--phases", "50,46,42,38,34,30,26,22");
    let seed: u64 = a.number("--seed", "20261006")?;
    let books = book_seeds(&a.get("--book", ""))?;
    let model = a.model()?;
    let mut rows = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for phase in phases.split(',') {
        let e: u32 = phase.parse().map_err(|_| "phase")?;
        let count: usize = a.number("--count", if e >= 40 { "300" } else { "2000" })?;
        for i in 0..count {
            let mut attempt = i as u64;
            let p = loop {
                let (p, _) = sample(
                    seed ^ e as u64,
                    attempt,
                    e,
                    model.clone(),
                    &books,
                    a.number("--sampling-depth", "4")?,
                )?;
                if seen.insert(book::key(p)) {
                    break p;
                }
                attempt += count as u64;
            };
            rows.push(format!(
                "{{\"key\":{},\"mode\":{},\"lower\":-64,\"upper\":64,\"children\":[]}}",
                json::quote(&tree::key(p)),
                json::quote(if e >= 40 { "best" } else { "all" })
            ));
        }
    }
    atomic(
        &a.get("--output", "suite-requests.json"),
        format!("[{}]\n", rows.join(",")).as_bytes(),
    )
}
