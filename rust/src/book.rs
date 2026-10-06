//! Persistent symmetry-reduced AND/OR book. Only terminal-depth bounds are proofs.
use crate::{board::*, json, search::*};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
pub type Key = (u64,u64,u64,u64,i8);
pub fn key(p:Position)->Key {(p.black,p.white,p.hleft,p.vtop,p.side)}
pub fn cell(a:u8,t:u8)->u8 {
    let (mut r,mut c)=(a/8,a%8);
    if t>=4 {c=7-c;}
    for _ in 0..t%4 {(r,c)=(c,7-r);}
    r*8+c
}
fn bits(mut b:u64,t:u8)->u64 {let mut out=0;while b!=0 {let a=b.trailing_zeros() as u8;b&=b-1;out|=1<<cell(a,t);}out}
pub fn transform_move(m:Move,t:u8)->Move {let a=cell(m.a,t);let b=cell(m.b,t);Move{a:a.min(b),b:a.max(b),flips:bits(m.flips,t)}}
pub fn transform(p:Position,t:u8)->Position {
    let mut q=Position{black:bits(p.black,t),white:bits(p.white,t),side:p.side,..Default::default()};
    for (mut b,step) in [(p.hleft,1),(p.vtop,8)] {while b!=0 {let a=b.trailing_zeros() as u8;b&=b-1;let m=transform_move(Move{a,b:a+step,flips:0},t);if m.b-m.a==1 {q.hleft|=1<<m.a;}else{q.vtop|=1<<m.a;}}}
    q
}
pub fn canonical(p:Position)->Position {(0..8).map(|t|transform(p,t)).min_by_key(|p|key(*p)).unwrap()}
pub fn reported(value:i32,lo:i32,hi:i32)->i32 {if lo> -64 {lo}else if hi<64 {hi}else{value}}
pub fn stamp()->u64 {std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()}
#[derive(Clone,Debug)]
pub struct Node {
    pub p:Position,pub value:i32,pub lo:i32,pub hi:i32,pub depth:u8,
    pub eval_version:i64,pub evaluated:bool,pub expanded:bool,pub visits:u64,pub updated:u64,
    pub best:Option<String>,pub edges:Vec<(Option<Move>,Key)>,pub dirty:bool,
}
impl Node {
    fn new(p:Position)->Self {Self{p,value:evaluate(p,Weights::default()),lo:-64,hi:64,depth:0,eval_version:crate::pattern::revision(),evaluated:false,expanded:false,visits:0,updated:stamp(),best:None,edges:vec![],dirty:true}}
    pub fn exact(&self)->bool {self.lo==self.hi}
    pub fn bound(&self)->&'static str {if self.exact(){"exact"}else if self.lo > -64 {"lower"}else if self.hi<64 {"upper"}else{"heuristic"}}
}
pub struct Book {pub nodes:BTreeMap<Key,Node>,pub root:Key,pub completed:u64}
impl Book {
    pub fn new(p:Position)->Self {let p=canonical(p);let mut nodes=BTreeMap::new();nodes.insert(key(p),Node::new(p));Self{nodes,root:key(p),completed:0}}
    /// Preflight the entire batch before changing anything. Proofs only tighten.
    pub fn import_bounds(&mut self, rows:&[(Position,i32,i32)])->Result<(),String> {
        let mut merged=BTreeMap::new();
        for &(p,lo,hi) in rows {
            if !(-64..=64).contains(&lo)||!(-64..=64).contains(&hi)||lo>hi {return Err("Invalid proof interval".into())}
            let p=canonical(p);let k=key(p);
            let old=merged.entry(k).or_insert_with(||self.nodes.get(&k).map_or((-64,64),|n|(n.lo,n.hi)));
            let next=(old.0.max(lo),old.1.min(hi));
            if next.0>next.1 {return Err(format!("Proof contradicts book at {:?}: existing {:?}, proof [{lo},{hi}]",k,old))}
            *old=next;
        }
        for (k,(lo,hi)) in merged {
            let p=Position{black:k.0,white:k.1,hleft:k.2,vtop:k.3,side:k.4};
            let n=self.nodes.entry(k).or_insert_with(||Node::new(p));
            n.lo=lo;n.hi=hi;n.value=n.value.clamp(lo,hi);n.evaluated=true;n.updated=stamp();n.dirty=true;
            if lo==hi {n.depth=(p.empty().count_ones()/2) as u8;}
        }
        // Include all legal replies even at exact proof leaves, for human play.
        for &(p,_,_) in rows {self.expand(key(canonical(p)));}
        self.propagate();Ok(())
    }
    /// Compact tree snapshot merge, without materializing irrelevant children.
    pub fn import_tree(&mut self,path:&Path)->Result<(),String>{
        let f=File::open(path).map_err(|e|e.to_string())?;
        let size=f.metadata().map_err(|e|e.to_string())?.len();
        let mut r=BufReader::new(std::io::Read::take(f,size));let mut line=String::new();
        loop {line.clear();if r.read_line(&mut line).map_err(|e|e.to_string())?==0||!line.ends_with('\n'){break}
            let v=json::parse(&line)?;if v.get("treeVersion")?.num()?!=1{return Err("tree snapshot version".into())}
            let p=crate::tree::position(v.get("key")?.str()?)?;
            let lo=v.get("lower")?.num()?;let hi=v.get("upper")?.num()?;
            if lo< -64||hi>64||lo>hi{return Err("tree snapshot interval".into())}
            let n=self.nodes.entry(key(p)).or_insert_with(||Node::new(p));
            n.lo=n.lo.max(lo as i32);n.hi=n.hi.min(hi as i32);
            if n.lo>n.hi{return Err("tree snapshot contradicts book".into())}
            n.value=n.value.clamp(n.lo,n.hi);n.evaluated=true;
            if n.exact(){n.depth=(p.empty().count_ones()/2) as u8;}
            if let Ok(m)=v.get("bestMove")?.str(){
                if !p.moves().iter().any(|x|x.to_string()==m) && !(m=="pass"&&p.moves().is_empty()&&!p.pass().moves().is_empty()) {return Err("illegal tree move".into())}
                n.best=Some(m.into());
            }
        }Ok(())
    }
    pub fn expand(&mut self,k:Key) {
        if self.nodes[&k].expanded {return}
        let p=self.nodes[&k].p;
        let ms=p.moves();
        let edges:Vec<_>=if ms.is_empty(){if p.pass().moves().is_empty(){
            let n=self.nodes.get_mut(&k).unwrap();n.value=p.diff()*p.side as i32;n.lo=n.value;n.hi=n.value;n.evaluated=true;n.dirty=true;return
        }else{vec![(None,canonical(p.pass()))]}}else{ms.into_iter().map(|m|(Some(m),canonical(p.play(m)))).collect()};
        let mut links=vec![];
        for (m,p) in edges {let ck=key(p);self.nodes.entry(ck).or_insert_with(||Node::new(p));links.push((m,ck));}
        let n=self.nodes.get_mut(&k).unwrap();n.edges=links;n.expanded=true;n.dirty=true;n.updated=stamp();
    }
    /// Cover both players: every reply in the first all_plies placements,
    /// then the most plausible top_k replies at each node, with transpositions.
    pub fn expand_coverage(&mut self,depth:u32,all_plies:u32,top_k:usize) {
        let mut todo=vec![(self.root,0)];let mut seen=BTreeSet::new();
        while let Some((k,ply))=todo.pop(){if ply>=depth||!seen.insert(k){continue}
            self.expand(k);let mut edges=self.nodes[&k].edges.clone();
            edges.sort_by_key(|(_,c)|self.nodes[c].value);
            if ply>=all_plies {edges.truncate(top_k);}
            for (m,c) in edges {todo.push((c,ply+u32::from(m.is_some())));}
        }
        self.propagate();
    }
    pub fn propagate(&mut self) {
        // Occupancy increases on placement. A pass has the same occupancy but
        // its target has moves, so process moving nodes before pass nodes.
        let mut order:Vec<_>=self.nodes.iter().filter(|(_,n)|n.expanded).map(|(k,_)|*k).collect();
        order.sort_by_cached_key(|k|{let p=self.nodes[k].p;(p.empty().count_ones(),p.mobility()==0)});
        for k in order {
            let n=&self.nodes[&k];if n.edges.is_empty(){continue}
            let mut lo=-64;let mut hi=-64;let mut value=-65;let mut best=None;let mut depth=255;
            for (m,c) in &n.edges {let ch=&self.nodes[c];lo=lo.max(-ch.hi);hi=hi.max(-ch.lo);let v=-ch.value;
                depth=depth.min(ch.depth.saturating_add(u8::from(m.is_some())));
                if v>value {value=v;best=Some(m.map_or("pass".into(),|m|m.to_string()));}}
            lo=lo.max(n.lo);hi=hi.min(n.hi);assert!(lo<=hi,"contradictory proof");value=value.clamp(lo,hi);
            if lo==hi {best=n.edges.iter().max_by_key(|(_,c)|-self.nodes[c].hi).map(|(m,_)|m.map_or("pass".into(),|m|m.to_string()));}
            let n=self.nodes.get_mut(&k).unwrap();
            if (n.value,n.lo,n.hi,n.depth,&n.best)!=(value,lo,hi,depth,&best){n.value=value;n.lo=lo;n.hi=hi;n.depth=depth;n.best=best;n.updated=stamp();n.dirty=true;}
        }
    }
    /// Priority is cumulative minimax drop-out regret, then uncertainty and
    /// depth (deepen equally promising lines instead of breadth enumeration). A periodic oldest frontier job prevents starvation.
    pub fn frontier(&self,busy:&BTreeSet<Key>,margin:i32)->BinaryHeap<(i64,i32,std::cmp::Reverse<Key>)> {
        let mut paths=BTreeMap::new();let mut todo=BinaryHeap::new();let mut jobs=BinaryHeap::new();
        todo.push((0i64,std::cmp::Reverse(self.root)));
        while let Some((priority,std::cmp::Reverse(k)))=todo.pop(){
            if paths.get(&k).is_some_and(|v|*v>=priority){continue}paths.insert(k,priority);
            let n=&self.nodes[&k];
            if !n.expanded {
                if !busy.contains(&k)&&!n.exact(){jobs.push((priority,n.hi-n.lo+(56-n.p.empty().count_ones() as i32),std::cmp::Reverse(k)));}continue
            }
            for (_,c) in &n.edges {let ch=&self.nodes[c];if ch.exact()&&!ch.expanded {continue}
                let regret=(n.value+ch.value).max(0);
                let loss=regret.min(margin)+(regret-margin).max(0)*4;
                todo.push((priority-loss as i64*16,std::cmp::Reverse(*c)));
            }
        }
        jobs
    }
    pub fn apply(&mut self,k:Key,r:Evaluation) {
        let n=self.nodes.get_mut(&k).unwrap();n.lo=n.lo.max(r.lo);n.hi=n.hi.min(r.hi);assert!(n.lo<=n.hi);
        n.eval_version=crate::pattern::revision();n.value=r.value.clamp(n.lo,n.hi);n.depth=r.depth;n.best=r.best;n.evaluated=true;n.visits+=1;n.updated=stamp();n.dirty=true;self.completed+=1;
    }
    pub fn pv(&self,mut p:Position,limit:usize)->Vec<String>{
        let mut out=vec![];
        for _ in 0..limit {let n=match self.nodes.get(&key(canonical(p))){Some(n)=>n,None=>break};
            if !n.expanded {break}
            let ms=p.moves();if ms.is_empty(){if p.pass().moves().is_empty(){break}out.push("pass".into());p=p.pass();continue}
            let m=ms.into_iter().max_by_key(|m|self.nodes.get(&key(canonical(p.play(*m)))).map_or(-65,|c|if n.exact(){-c.hi}else{-c.value})).unwrap();out.push(m.to_string());p=p.play(m);
        }out
    }
    pub fn analysis(&self,p:Position)->String {self.analysis_with_pv(p,8)}
    pub fn analysis_compact(&self,p:Position)->String {self.analysis_with_pv(p,0)}
    fn analysis_with_pv(&self,p:Position,pv_limit:usize)->String {
        let n=&self.nodes[&key(canonical(p))];let mut rows=vec![];let mut complete=true;
        for m in p.moves(){let c=self.nodes.get(&key(canonical(p.play(m))));
            let (estimate,lo,hi,d,exact)=c.map_or((-evaluate(p.play(m),Weights::default()),-64,64,0,false),|c|(-c.value,-c.hi,-c.lo,c.depth.saturating_add(1),c.exact()));
            let v=reported(estimate,lo,hi);
            complete &= exact;
            let bound=if exact{"exact"}else if lo> -64{"lower"}else if hi<64{"upper"}else{"heuristic"};
            let mut pv=vec![m.to_string()];pv.extend(self.pv(p.play(m),pv_limit));
            rows.push(format!("{{\"move\":{},\"cells\":[{},{}],\"value\":{v},\"estimate\":{estimate},\"lower\":{lo},\"upper\":{hi},\"bound\":\"{bound}\",\"exact\":{exact},\"depth\":{d},\"pv\":[{}],\"nodes\":0}}",json::quote(&m.to_string()),m.a,m.b,pv.iter().map(|s|json::quote(s)).collect::<Vec<_>>().join(",")));
        }
        let pass=p.moves().is_empty()&&!p.pass().moves().is_empty();let terminal=p.moves().is_empty()&&!pass;
        if rows.is_empty(){complete=n.exact();}
        format!("{{\"moves\":[{}],\"complete\":{complete},\"exact\":{},\"value\":{},\"lower\":{},\"upper\":{},\"bound\":\"{}\",\"terminal\":{terminal},\"pass\":{pass},\"perspective\":\"side-to-move\"}}",rows.join(","),n.exact(),reported(n.value,n.lo,n.hi),n.lo,n.hi,n.bound())
    }
    fn record(&self,n:&Node)->String {
        format!("{{\"version\":1,\"evalVersion\":{},\"hash\":\"{:016x}\",\"position\":{},\"side\":{},\"value\":{},\"estimate\":{},\"lower\":{},\"upper\":{},\"bound\":\"{}\",\"depth\":{},\"evaluated\":{},\"expanded\":{},\"visits\":{},\"updatedAt\":{},\"bestMove\":{},\"analysis\":{}}}\n",n.eval_version,n.p.hash(),json::position_json(n.p),n.p.side,reported(n.value,n.lo,n.hi),n.value,n.lo,n.hi,n.bound(),n.depth,n.evaluated,n.expanded,n.visits,n.updated,n.best.as_ref().map_or("null".into(),|s|json::quote(s)),if n.expanded{self.analysis_compact(n.p)}else{"null".into()})
    }
    pub fn checkpoint(&mut self,path:&Path)->Result<usize,String> {
        if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
        let mut f=OpenOptions::new().create(true).append(true).open(path).map_err(|e|e.to_string())?;
        let dirty:Vec<_>=self.nodes.iter().filter(|(_,n)|n.dirty).map(|(k,_)|*k).collect();
        for k in &dirty {f.write_all(self.record(&self.nodes[k]).as_bytes()).map_err(|e|e.to_string())?;}
        f.sync_all().map_err(|e|e.to_string())?;
        for k in &dirty {self.nodes.get_mut(k).unwrap().dirty=false;}
        Ok(dirty.len())
    }
    pub fn load(path:&Path,repair:bool)->Result<Self,String> {
        if !path.exists(){return Ok(Self::new(Position::initial()))}
        // Snapshot length bounds a live append; only newline-terminated records count.
        let file=File::open(path).map_err(|e|e.to_string())?;
        let size=file.metadata().map_err(|e|e.to_string())?.len();
        let mut reader=BufReader::new(std::io::Read::take(file,size));
        let mut book=Self::new(Position::initial());
        let mut line=String::new();let mut end=0u64;
        loop {
            line.clear();let read=reader.read_line(&mut line).map_err(|e|e.to_string())?;
            if read==0 {break}
            if !line.ends_with('\n') {if repair {OpenOptions::new().write(true).open(path).and_then(|f|f.set_len(end)).map_err(|e|e.to_string())?;}break}
            end+=read as u64;
            let v=json::parse(&line)?;if v.get("version")?.num()?!=1{return Err("Unsupported journal version".into())}
            let p=json::position(v.get("position")?)?;if canonical(p)!=p{return Err("Noncanonical journal position".into())}
            let mut n=Node::new(p);n.value=v.get("estimate")?.num()? as i32;n.lo=v.get("lower")?.num()? as i32;n.hi=v.get("upper")?.num()? as i32;
            if !(-64..=64).contains(&n.lo)||!(-64..=64).contains(&n.hi)||n.lo>n.hi{return Err("Invalid proof interval".into())}
            n.depth=v.get("depth")?.num()? as u8;n.evaluated=v.get("evaluated")?.boolean()?;n.expanded=v.get("expanded")?.boolean()?;n.visits=v.get("visits")?.num()? as u64;n.updated=v.get("updatedAt")?.num()? as u64;n.best=v.get("bestMove")?.str().ok().map(str::to_owned);n.dirty=false;
            n.eval_version=v.get("evalVersion").ok().and_then(|v|v.num().ok()).unwrap_or(0);
            if !n.exact()&&n.eval_version!=crate::pattern::revision() {
                // Keep every terminal proof interval. Stale heuristic leaves
                // return to the queue; internal estimates are propagated below.
                n.value=evaluate(p,Weights::default()).clamp(n.lo,n.hi);n.depth=0;n.evaluated=false;n.best=None;
                n.eval_version=crate::pattern::revision();n.dirty=true;
            }
            book.nodes.insert(key(p),n);
        }
        // Edges are derived from rules, never trusted from disk. A killed append
        // may have saved a parent before its children: missing children are safe
        // unknowns; previously proved parent bounds remain valid.
        let expanded:Vec<_>=book.nodes.iter().filter(|(_,n)|n.expanded).map(|(k,_)|*k).collect();
        for k in expanded {let n=book.nodes.get_mut(&k).unwrap();n.expanded=false;let bounds=(n.lo,n.hi);n.lo=-64;n.hi=64;book.expand(k);let n=book.nodes.get_mut(&k).unwrap();n.lo=bounds.0;n.hi=bounds.1;}
        book.completed=book.nodes.values().map(|n|n.visits).sum();book.propagate();Ok(book)
    }
}
pub fn atomic_write(path:&Path,data:&str)->Result<(),String>{
    let tmp=path.with_extension("tmp");let mut f=File::create(&tmp).map_err(|e|e.to_string())?;f.write_all(data.as_bytes()).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;
    std::fs::rename(tmp,path).map_err(|e|e.to_string())?;
    if let Some(parent)=path.parent(){File::open(parent).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;}Ok(())
}
pub struct Evaluation {pub value:i32,pub lo:i32,pub hi:i32,pub depth:u8,pub best:Option<String>,pub attempted_exact:bool}
pub fn evaluate_leaf(s:&mut Search,p:Position,ms:u64,exact:bool)->Evaluation {
    s.set_selectivity(1);
    let start=now();let deadline=start+ms as f64;let max=(p.empty().count_ones()/2) as u8;
    let mut r=Evaluation{value:evaluate(p,s.weights),lo:-64,hi:64,depth:0,best:None,attempted_exact:exact};
    s.deadline=if exact{start+ms as f64*0.2}else{deadline};s.aborted=false;
    for d in 1..=max {if now()>=s.deadline{break}match s.run(p,d,-65,65){Ok(v)=>{r.value=v;r.depth=d;r.best=s.pv(p,1).first().cloned();if d==max{r.lo=v;r.hi=v;return r}},Err(_)=>break}}
    if exact&&now()<deadline {
        // A terminal null-window search preserves a useful proof even if the
        // subsequent exact-score search times out. Never promote heuristic TT bounds.
        s.deadline=start+ms as f64*0.5;s.aborted=false;
        let a=r.value.clamp(-63,63);
        if now()<s.deadline {if let Ok(v)=s.run(p,max,a,a+1){if v<=a{r.hi=v}else{r.lo=v}}}
        s.deadline=deadline;s.aborted=false;
        if let Ok(v)=s.exact(p,false){r.value=v;r.lo=v;r.hi=v;r.depth=max;r.best=s.pv(p,1).first().cloned();}
    }
    r.value=r.value.clamp(r.lo,r.hi);r
}
