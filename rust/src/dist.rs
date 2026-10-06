//! Distributed job protocol. Heuristics order work; only full-depth searches prove bounds.
use crate::{board::*, book, json, search::*};
use std::path::Path;

pub fn inspect(p: Position) -> String {
    let mut groups: Vec<(Position, Vec<String>, i32)> = vec![];
    for m in p.moves() {
        let q = p.play(m);
        let c = book::canonical(q);
        if let Some(g) = groups.iter_mut().find(|g| book::canonical(g.0) == c) {
            g.1.push(m.to_string());
        } else {
            groups.push((q, vec![m.to_string()], -evaluate(q, Weights::default())));
        }
    }
    let terminal = groups.is_empty() && p.pass().moves().is_empty();
    if groups.is_empty() && !terminal {
        groups.push((p.pass(), vec!["pass".into()], -evaluate(p.pass(), Weights::default())));
    }
    groups.sort_by_key(|g| -g.2);
    let rows = groups.iter().map(|(q, names, estimate)| format!(
        "{{\"position\":{},\"moves\":[{}],\"estimate\":{estimate}}}",
        json::position_json(*q), names.iter().map(|s|json::quote(s)).collect::<Vec<_>>().join(",")
    )).collect::<Vec<_>>().join(",");
    format!("{{\"position\":{},\"empties\":{},\"terminal\":{terminal},\"terminalValue\":{},\"estimate\":{},\"children\":[{rows}]}}",
        json::position_json(p), p.empty().count_ones(), p.diff()*p.side as i32, evaluate(p,Weights::default()))
}
fn checksum(s: &str) -> String {
    format!("{:016x}", s.bytes().fold(0xcbf29ce484222325u64, |h,b|(h^b as u64).wrapping_mul(0x100000001b3)))
}

pub fn solve(p: Position, o: &Options, kind: &str, threshold: i32, path: &Path) -> Result<String,String> {
    if !["exact", "bound"].contains(&kind) || !(-64..=65).contains(&threshold) || o.threads == 0 || o.threads > 256 {
        return Err("invalid job type, threshold (-64..65), or threads (1..256)".into());
    }
    let path = std::env::current_dir().map_err(|e|e.to_string())?.join(path);
    let (mut lo,mut hi)=(-64,64);
    if path.exists() {
        let raw=std::fs::read_to_string(&path).map_err(|e|e.to_string())?;
        let envelope=json::parse(&raw)?;
        let data=envelope.get("data")?.str()?;
        if envelope.get("checksum")?.str()? != checksum(data) {return Err("job checkpoint checksum".into())}
        let v=json::parse(data)?;
        if v.get("version")?.num()?!=1 || json::position(v.get("position")?)?!=p {return Err("job checkpoint identity".into())}
        lo=i32::try_from(v.get("lower")?.num()?).map_err(|_|"bound overflow")?;
        hi=i32::try_from(v.get("upper")?.num()?).map_err(|_|"bound overflow")?;
        if lo < -64 || hi > 64 || lo>hi {return Err("job checkpoint bounds".into())}
    }
    let save=|lo,hi| {
        let data=format!("{{\"version\":1,\"position\":{},\"lower\":{lo},\"upper\":{hi}}}",json::position_json(p));
        book::atomic_write(&path,&format!("{{\"data\":{},\"checksum\":{}}}\n",json::quote(&data),json::quote(&checksum(&data))))
    };
    save(lo,hi)?;
    let start=now();
    let (saved,nodes)=crate::search::distributed_execute(o,start+o.time_ms as f64,|s| -> Result<(),String> {
        let mut guess=evaluate(p,Weights::default()).clamp(lo,hi);
        while lo<hi && (kind=="exact" || (lo<threshold && hi>=threshold)) {
            if now()>=s.deadline {break}
            let beta=if kind=="bound" {threshold} else {guess.clamp(lo+1,hi)};
            let Ok(v)=s.run(p,(p.empty().count_ones()/2) as u8,beta-1,beta) else {break};
            if v>=beta {lo=lo.max(v)} else {hi=hi.min(v)}
            if lo>hi {return Err("contradictory search bounds".into())}
            save(lo,hi)?;
            eprintln!("checkpoint lower={lo} upper={hi} nodes={} elapsedMs={:.0}",s.nodes,now()-start);
            guess=v;
        }
        Ok(())
    });
    saved?;
    let complete=if kind=="exact" {lo==hi} else {lo>=threshold || hi<threshold};
    let outcome=if !complete {"incomplete"} else if kind=="exact" {"exact"} else if lo>=threshold {"fail-high"} else {"fail-low"};
    Ok(format!("{{\"version\":1,\"position\":{},\"type\":{},\"threshold\":{threshold},\"complete\":{complete},\"outcome\":{},\"lower\":{lo},\"upper\":{hi},\"value\":{},\"nodes\":{nodes},\"elapsedMs\":{:.3},\"perspective\":\"side-to-move\"}}",json::position_json(p),json::quote(kind),json::quote(outcome),if lo==hi {lo.to_string()} else {"null".into()},now()-start))
}
