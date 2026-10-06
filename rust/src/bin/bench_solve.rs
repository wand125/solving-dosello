use dosello_ai::{cli,json,search::*};
use std::io::Write;
fn main() {cli::main_result(run)}
fn run()->Result<(),String> {
    let mut args=std::env::args().skip(1);
    let mut positional=vec![];
    let mut mb=None;let mut threads=None;let mut leaf=None;
    while let Some(arg)=args.next() {
        match arg.as_str() {
            "--tt-mb"=>mb=Some(args.next().ok_or("Missing tt-mb")?.parse::<usize>().map_err(|_|"Invalid tt-mb")?),
            "--threads"=>threads=Some(args.next().ok_or("Missing threads")?.parse::<usize>().map_err(|_|"Invalid threads")?),
            "--last-empties"=>leaf=Some(args.next().ok_or("Missing last-empties")?.parse::<u32>().map_err(|_|"Invalid last-empties")?),
            "--help"=>{println!("bench_solve [budget-ms=10000] [id-prefix=e] [TT-log2] [old] [--tt-mb 256] [--threads N] [--last-empties 10]\nDefault: serial exact score; --threads uses interior YBWC + null-window root (N=1 uses the same root driver). MiB rounds down to a power of two.");return Ok(())},
            _ if arg.starts_with("--")=>return Err(format!("Unknown option {arg}")),
            _=>positional.push(arg),
        }
    }
    if positional.len()>4 {return Err("Too many positional arguments".into())}
    if threads.is_some_and(|n|!(1..=64).contains(&n)) {return Err("threads must be 1..64".into())}
    if leaf.is_some_and(|n|n>16) {return Err("last-empties must be 0..16".into())}
    if threads.is_some() && leaf.is_some() {return Err("Leaf experiments are serial only".into())}
    let budget=positional.first().map(|s|s.parse::<u64>()).transpose().map_err(|_|"Invalid budget")?.unwrap_or(10000);
    let filter=positional.get(1).map(String::as_str).unwrap_or("e");
    let mut entries=Options::default().tt_entries;
    if let Some(n)=positional.get(2) {let n=n.parse::<u32>().map_err(|_|"Invalid TT log2")?;
        if !(10..=30).contains(&n) {return Err("TT log2 must be 10..30".into())}entries=1usize<<n;}
    if let Some(mb)=mb {entries=entries_for_mb(mb)?;}
    let old=positional.get(3).is_some_and(|s|s=="old");
    let suite=json::parse(include_str!("../../bench/positions.json"))?;
    let mut matched=0;
    for row in suite.get("positions")?.arr()? {
        let id=row.get("id")?.str()?;if !id.starts_with(filter) {continue}matched+=1;
        let p=json::position(row.get("position")?)?;let t=now();
        let (value,nodes)=if old {
            let mut s=dosello_ai::legacy_search::Search::new(entries,t+budget as f64);
            let v=s.exact(p,false).ok();(v,s.nodes)
        }else if let Some(threads)=threads {parallel_exact(p,entries,threads,t+budget as f64)}else {
            let mut s=Search::new(entries,t+budget as f64);
            if let Some(leaf)=leaf {s.last_empties=leaf;}
            let v=s.exact(p,false).ok();(v,s.nodes)
        };
        let ms=now()-t;
        if let (Some(v),Ok(expected))=(value,row.get("expected")?.num()) {assert_eq!(v as i64,expected,"{id}");}
        println!("{{\"id\":\"{id}\",\"empties\":{},\"value\":{},\"exact\":{},\"nodes\":{nodes},\"elapsedMs\":{ms:.3},\"nps\":{:.0},\"budgetMs\":{budget},\"ttEntries\":{entries},\"threads\":{},\"parallelRoot\":{}}}",p.empty().count_ones(),value.map_or("null".into(),|v|v.to_string()),value.is_some(),nodes as f64*1000.0/ms,threads.unwrap_or(1),threads.is_some());
        std::io::stdout().flush().map_err(|e|e.to_string())?;
    }
    if matched==0 {return Err(format!("No benchmark position matches {filter}"))}Ok(())
}
