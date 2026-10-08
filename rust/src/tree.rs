//! Compact solution-tree protocol. No estimates enter proof intervals.
use crate::{board::*, book, json::{self,Json}, search::*};
use std::{collections::BTreeMap, io::{BufRead,Write}};
pub fn key(p:Position)->String {format!("{:016x}{:016x}{:016x}{:016x}{}",p.black,p.white,p.hleft,p.vtop,if p.side==1{'b'}else{'w'})}
pub fn position(k:&str)->Result<Position,String>{
    if !k.is_ascii() || k.len()!=65 || !k[..64].bytes().all(|c|c.is_ascii_hexdigit()) {return Err("tree key".into())}
    let bits=|i|u64::from_str_radix(&k[i..i+16],16).map_err(|_|"tree key".to_string());
    let p=Position{black:bits(0)?,white:bits(16)?,hleft:bits(32)?,vtop:bits(48)?,side:match &k[64..]{"b"=>1,"w"=>-1,_=>return Err("tree side".into())}};
    p.validate()?;if book::canonical(p)!=p{return Err("noncanonical tree key".into())}Ok(p)
}
pub fn groups(p:Position)->Vec<(Position,Vec<String>)>{
    let mut g:BTreeMap<book::Key,(Position,Vec<String>)>=BTreeMap::new();
    for m in p.moves(){let c=book::canonical(p.play(m));g.entry(book::key(c)).or_insert((c,vec![])).1.push(m.to_string());}
    if g.is_empty()&&!p.pass().moves().is_empty(){let c=book::canonical(p.pass());g.insert(book::key(c),(c,vec!["pass".into()]));}
    let mut g:Vec<_>=g.into_values().collect();g.sort_by_key(|(p,_)|evaluate(*p,Weights::default()));g
}
pub fn inspect(p:Position)->String{
    let p=book::canonical(p);let g=groups(p);
    let children=g.iter().map(|(c,m)|format!("{{\"key\":{},\"moves\":[{}]}}",json::quote(&key(*c)),m.iter().map(|m|json::quote(m)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
    format!("{{\"key\":{},\"side\":{},\"empties\":{},\"terminal\":{},\"terminalValue\":{},\"children\":[{}]}}",json::quote(&key(p)),p.side,p.empty().count_ones(),g.is_empty(),p.diff()*p.side as i32,children)
}
fn interval(v:&Json)->Result<(i32,i32),String>{let l=v.get("lower")?.num()?;let h=v.get("upper")?.num()?;if l< -64||h>64||l>h{return Err("tree interval".into())}Ok((l as i32,h as i32))}
fn narrow(s:&mut Search,p:Position,b:&mut(i32,i32),threshold:Option<i32>){
    narrow_observed(s,p,b,threshold,|_|{});
}
fn narrow_observed(s:&mut Search,p:Position,b:&mut(i32,i32),threshold:Option<i32>,mut progress:impl FnMut((i32,i32))){
    let mut guess=evaluate(p,Weights::default()).clamp(b.0,b.1);
    while b.0<b.1 && threshold.is_none_or(|t| b.0<t && b.1>=t){
        if now()>=s.deadline {break}
        let beta=threshold.unwrap_or(guess.clamp(b.0+1,b.1));
        let Ok(v)=s.run(p,(p.empty().count_ones()/2) as u8,beta-1,beta) else {break};
        if v>=beta {b.0=b.0.max(v)}else{b.1=b.1.min(v)}guess=v;progress(*b);
    }
}
/// One shared TT and worker pool per batch. Flush each completed parent so a
/// later timeout/crash loses at most the current parent's unfinished search.
pub fn batch(raw:&str,o:&Options)->Result<(),String>{
    if o.threads==0||o.threads>256||o.selectivity!=0||o.selective_exact{return Err("tree jobs require nonselective search".into())}
    let input=json::parse(raw)?;let rows=input.arr()?;if rows.len()>256{return Err("batch limit 256".into())}
    crate::search::distributed_execute(o,now()+o.time_ms as f64,|s| batch_rows(s,rows,0)).0
}
/// Suite transport correlates out-of-order records by batchIndex. The tree
/// transport above retains its original ordered streaming contract.
pub fn suite_batch(raw:&str,o:&Options)->Result<(),String>{
    if o.threads==0||o.threads>256||o.selectivity!=0||o.selective_exact{return Err("tree jobs require nonselective search".into())}
    let input=json::parse(raw)?;let rows=input.arr()?;if rows.len()>256{return Err("batch limit 256".into())}
    let deadline=now()+o.time_ms as f64;
    if rows.len() <= 1 {
        return crate::search::distributed_execute(o,deadline,|s| batch_rows(s,rows,0)).0;
    }
    // Independent roots/leaves use exactly the slot budget, one atomic TT.
    let table=std::sync::Arc::new(SharedTable::new(o.tt_entries));
    let next=std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let mut workers=vec![];
        for _ in 0..o.threads.min(rows.len()) {
            let table=table.clone();let next=&next;
            workers.push(scope.spawn(move || -> Result<(),String> {
                let mut s=Search::new(4,deadline);s.shared=Some(table);
                loop {
                    if now()>=deadline {break}
                    let i=next.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
                    if i>=rows.len(){break}
                    batch_rows(&mut s,&rows[i..i+1],i)?;
                }
                Ok(())
            }));
        }
        for worker in workers {worker.join().map_err(|_| "batch worker panic")??;}
        Ok(())
    })
}
fn batch_rows(s:&mut Search,rows:&[Json],offset:usize)->Result<(),String>{
        for (index,row) in rows.iter().enumerate(){
            let index=index+offset;
            let started=now();let k=row.get("key")?.str()?;let p=position(k)?;let mode=row.get("mode")?.str()?;
            if mode=="value"||mode=="bound"{
                let mut b=interval(row)?;
                let threshold=if mode=="bound"{let t=row.get("threshold")?.num()?;if !(-64..=65).contains(&t){return Err("threshold".into())}Some(t as i32)}else{None};
                narrow_observed(s,p,&mut b,threshold,|b|{
                    let complete=threshold.map_or(b.0==b.1,|t|b.0>=t||b.1<t);
                    println!("{{\"batchIndex\":{index},\"version\":1,\"progress\":true,\"key\":{},\"mode\":{},\"threshold\":{},\"lower\":{},\"upper\":{},\"complete\":{},\"children\":[],\"perspective\":\"side-to-move\"}}",json::quote(k),json::quote(mode),threshold.map_or("null".into(),|t|t.to_string()),b.0,b.1,complete);
                    let _=std::io::stdout().flush();
                });
                let complete=threshold.map_or(b.0==b.1,|t|b.0>=t||b.1<t);
                println!("{{\"batchIndex\":{index},\"version\":1,\"key\":{},\"mode\":{},\"threshold\":{},\"lower\":{},\"upper\":{},\"complete\":{},\"children\":[],\"elapsedMs\":{:.3},\"perspective\":\"side-to-move\"}}",json::quote(k),json::quote(mode),threshold.map_or("null".into(),|t|t.to_string()),b.0,b.1,complete,now()-started);
                std::io::stdout().flush().map_err(|e|e.to_string())?;
                if !complete{break}continue
            }
            if mode!="all"&&mode!="best"{return Err("tree mode".into())}
            let mut b=interval(row)?;let g=groups(p);let mut cs=vec![];
            for (c,m) in g {let ck=key(c);let saved=row.get("children")?.arr()?.iter().find(|x|x.get("key").ok().and_then(|x|x.str().ok())==Some(ck.as_str()));cs.push((c,m,saved.map(interval).transpose()?.unwrap_or((-64,64))));}
            if cs.is_empty(){let v=p.diff()*p.side as i32;if v<b.0||v>b.1{return Err("terminal contradiction".into())}b=(v,v)}
            if mode=="all"{
                for (c,_,cb) in &mut cs{narrow(s,*c,cb,None);}
                if !cs.is_empty(){b.0=b.0.max(cs.iter().map(|x|-x.2.1).max().unwrap());b.1=b.1.min(cs.iter().map(|x|-x.2.0).max().unwrap());}
            }else{
                // Use child intervals before spending a root search.
                if !cs.is_empty(){b.0=b.0.max(cs.iter().map(|x|-x.2.1).max().unwrap());b.1=b.1.min(cs.iter().map(|x|-x.2.0).max().unwrap());}
                if b.0>b.1{return Err("tree contradiction".into())}
                narrow(s,p,&mut b,None);
                if b.0==b.1{for (c,_,cb) in &mut cs{
                    // Exact parent v implies every child >= -v. Distinguish
                    // ties from strictly worse moves, not merely <= v.
                    cb.0=cb.0.max(-b.0);if cb.0>cb.1{return Err("child contradiction".into())}
                    narrow(s,*c,cb,Some(1-b.0));
                }}
            }
            if b.0>b.1{return Err("tree contradiction".into())}
            let complete=b.0==b.1&&cs.iter().all(|x|if mode=="all"{x.2.0==x.2.1}else{x.2.0> -b.0||x.2==(-b.0,-b.0)});
            let children=cs.iter().map(|(c,m,cb)|format!("{{\"key\":{},\"lower\":{},\"upper\":{},\"moves\":[{}]}}",json::quote(&key(*c)),cb.0,cb.1,m.iter().map(|m|json::quote(m)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
            println!("{{\"batchIndex\":{index},\"version\":1,\"key\":{},\"mode\":{},\"lower\":{},\"upper\":{},\"complete\":{},\"children\":[{}],\"elapsedMs\":{:.3},\"perspective\":\"side-to-move\"}}",json::quote(k),json::quote(mode),b.0,b.1,complete,children,now()-started);
            std::io::stdout().flush().map_err(|e|e.to_string())?;
            if !complete {break}
        }Ok(())
}
/// Streaming, read-only seed conversion. Ignore only an incomplete final line.
pub fn seed(path:&std::path::Path,proof:bool)->Result<(),String>{
    let emit=|v:&Json|->Result<(),String>{let (lo,hi)=interval(v)?;if lo> -64||hi<64{let p=book::canonical(json::position(v.get("position")?)?);println!("{{\"key\":{},\"lower\":{lo},\"upper\":{hi}}}",json::quote(&key(p)));}Ok(())};
    if proof {let v=json::parse(&std::fs::read_to_string(path).map_err(|e|e.to_string())?)?;let Json::Obj(nodes)=v.get("nodes")? else{return Err("proof nodes".into())};for n in nodes.values(){emit(n)?}}
    else {let f=std::fs::File::open(path).map_err(|e|e.to_string())?;let size=f.metadata().map_err(|e|e.to_string())?.len();let mut r=std::io::BufReader::new(std::io::Read::take(f,size));let mut l=String::new();loop{l.clear();if r.read_line(&mut l).map_err(|e|e.to_string())?==0||!l.ends_with('\n'){break}emit(&json::parse(&l)?)?}}
    Ok(())
}

/// Independent native single-thread reference check for local fixtures only.
pub fn verify(path:&std::path::Path)->Result<(),String>{
    let mut s=Search::new(1<<20,f64::INFINITY);let mut count=0;
    for line in std::io::BufReader::new(std::fs::File::open(path).map_err(|e|e.to_string())?).lines(){
        let v=json::parse(&line.map_err(|e|e.to_string())?)?;let p=position(v.get("key")?.str()?)?;
        if p.empty().count_ones()>30{return Err("verification limited to <=30 empties".into())}
        let exact=s.exact(p,false).map_err(|_|"reference search aborted")?;
        if interval(&v)?!=(exact,exact){return Err("incorrect parent value".into())}
        let mode=v.get("mode")?.str()?;let mut ties=vec![];
        let cs=v.get("children")?.arr()?;let g=groups(p);
        if cs.len()!=g.len(){return Err("missing children".into())}
        for (c,(p,moves)) in cs.iter().zip(g){
            if c.get("key")?.str()?!=key(p){return Err("child transition".into())}
            let value=s.exact(p,false).map_err(|_|"reference child aborted")?;let (lo,hi)=interval(c)?;
            if !(lo<=value&&value<=hi){return Err("unsound child bound".into())}
            if mode=="all"&&lo!=hi{return Err("opponent child not exact".into())}
            if -value==exact{if lo!=hi{return Err("tie not exact".into())}ties.extend(moves)}else if mode=="best"&&lo<=-exact{return Err("worse move not strictly bounded".into())}
        }
        ties.sort();if ties.first().map(String::as_str)!=v.get("bestMove")?.str().ok(){return Err("chosen tie mismatch".into())}count+=1;
    }
    println!("{{\"verifiedParents\":{count},\"maxEmpties\":30,\"threads\":1}}");Ok(())
}
