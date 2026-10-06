use crate::board::*;
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn now_ms() -> f64;
}
#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    unsafe { now_ms() }
}
#[derive(Clone, Copy)]
pub struct Weights {
    pub discs: i32,
    pub corners: i32,
    pub danger: i32,
    pub edges: i32,
    pub mobility: i32,
    pub potential: i32,
    pub frontier: i32,
    pub parity: i32,
}
impl Default for Weights {
    fn default() -> Self {
        Self {
            discs: 2,
            corners: 30,
            danger: 12,
            edges: 3,
            mobility: 9,
            potential: 2,
            frontier: 3,
            parity: 2,
        }
    }
}
pub fn evaluate(p: Position, w: Weights) -> i32 {
    let own = p.own();
    let opp = p.opp();
    let e = p.empty();
    let n = e.count_ones() as i32;
    let difference =
        |mask: u64| (own & mask).count_ones() as i32 - (opp & mask).count_ones() as i32;
    let mut danger = 0;
    for (corner, near) in [
        (0, 0x302u64),
        (7, 0xc040),
        (56, 0x0203000000000000),
        (63, 0x40c0000000000000),
    ] {
        if e & (1 << corner) != 0 {
            danger += difference(near)
        }
    }
    let front = adjacent(e);
    let pot_own = e & adjacent(opp);
    let pot_opp = e & adjacent(own);
    let potential = |touch: u64| placements(e) - placements(e & !touch);
    let isolated = e & !((shift(e, 1)) | shift(e, 3) | shift(e, 4) | shift(e, 6));
    let playable = n - isolated.count_ones() as i32;
    let raw = difference(!0) * w.discs * (64 - n) / 32 + difference(0x8100000000000081) * w.corners
        - danger * w.danger
        + difference(0x7e8181818181817e) * w.edges
        + (p.mobility() - p.pass().mobility()) * w.mobility
        + (potential(pot_own) - potential(pot_opp)) * w.potential
        - difference(front) * w.frontier
        + if (playable / 2) % 2 == 1 {
            w.parity
        } else {
            -w.parity
        };
    (raw / 8).clamp(-63, 63)
}
#[derive(Clone, Copy, Default)]
struct Entry {
    position: Position,
    value: i16,
    depth: u8,
    bound: u8,
    best: u16,
}
// Full position comparison prevents collision-induced incorrect exact results.
pub struct Search {
    table: Vec<Entry>,
    #[cfg(not(target_arch="wasm32"))]
    pub shared:Option<std::sync::Arc<SharedTable>>,
    pub nodes: u64,
    pub deadline: f64,
    pub weights: Weights,
    pub aborted: bool,
    pub progress: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}
impl Search {
    pub fn new(entries: usize, deadline: f64) -> Self {
        Self {
            table: vec![Entry::default(); entries.max(2).next_power_of_two()],
            #[cfg(not(target_arch="wasm32"))]
            shared:None,
            nodes: 0,
            deadline,
            weights: Weights::default(),
            aborted: false,
            progress: None,
        }
    }
    fn index(&self, key:u64)->usize {
        (table_hash(key) as usize & (self.table.len()-1)) & !1
    }
    fn entry_key(&self,p:Position,key:u64)->Entry {
        let i=self.index(key);
        let a=self.table[i]; let b=self.table[i+1];
        if a.bound!=0 && a.position==p && (b.bound==0 || b.position!=p || a.depth>=b.depth) {a} else {b}
    }
    fn entry(&self,p:Position)->Entry {self.entry_key(p,p.hash())}
    fn save(&mut self,key:u64,e:Entry) {
        let i=self.index(key);
        if self.table[i].bound==0 || self.table[i].depth<=e.depth {
            self.table[i]=e;
        } else {self.table[i+1]=e;}
        #[cfg(not(target_arch="wasm32"))]
        if e.depth>=5 { if let Some(t)=&self.shared {t.save(key,e);} }
    }
    pub fn run(&mut self,p:Position,depth:u8,alpha:i32,beta:i32)->Result<i32,()> {
        self.run_key(p,p.hash(),depth,alpha,beta)
    }
    fn run_key(&mut self, p: Position,key:u64, depth: u8, mut alpha: i32, beta: i32) -> Result<i32, ()> {
        self.nodes += 1;
        if self.nodes & 4095 == 0 {
            if let Some(counter) = &self.progress {
                counter.store(self.nodes as usize, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if self.nodes & 255 == 0 && now() >= self.deadline {
            self.aborted = true;
            return Err(());
        }
        let exact_depth=depth as u32>=p.empty().count_ones()/2;
        if exact_depth {
            let e=p.empty();
            let live=e & (shift(e,1)|shift(e,3)|shift(e,4)|shift(e,6));
            // Isolated empty cells can never acquire a partner. Final occupancy
            // is even, and all remaining discs can change colour.
            let max=((!e).count_ones()+live.count_ones()) as i32 & !1;
            if max<=alpha {return Ok(max)}
            if -max>=beta {return Ok(-max)}
            if live.count_ones()<=4 {self.nodes-=1;return self.last(p,alpha,beta,false);}
            if depth>=6 && (alpha>=40 || beta<=-40) {
                let max=p.occupancy_bound();
                let upper=max-2*p.stable_edges(p.opp()).count_ones() as i32;
                if upper<=alpha {return Ok(upper)}
                let lower=2*p.stable_edges(p.own()).count_ones() as i32-max;
                if lower>=beta {return Ok(lower)}
            }
        }
        let original_alpha = alpha;
        let mut old = self.entry_key(p,key);
        #[cfg(not(target_arch="wasm32"))]
        if depth>=5 && (old.bound==0 || old.position!=p || old.depth<depth) {
            if let Some(t)=&self.shared {if let Some(e)=t.get(key,p) {old=e;}}
        }
        let hit = old.bound != 0 && old.position == p;
        if hit && old.depth >= depth {
            let v = old.value as i32;
            if old.bound == 1 || old.bound == 2 && v >= beta || old.bound == 3 && v <= alpha {
                return Ok(v);
            }
        }
        let mut ms = p.move_list();
        if ms.is_empty() {
            let other = p.pass();
            if other.mobility()==0 {
                return Ok(p.diff() * p.side as i32);
            }
            return self.run_key(other,p.pass_hash(key), depth, -beta, -alpha).map(|x| -x);
        }
        if depth == 0 {
            return Ok(evaluate(p, self.weights));
        }
        let mut ranks=[0i32;112];
        if depth>=3 {
            for (i,m) in ms.iter().enumerate() {
                let child=p.play(*m);
                let ck=p.child_hash(key,*m);
                let e=self.entry_key(child,ck);
                // Enhanced Transposition Cutoff: a child's upper bound is a
                // lower bound for this node. Only sufficient depths qualify.
                if e.bound!=0 && e.position==child && e.depth>=depth-1 &&
                    (e.bound==1 || e.bound==3) && -(e.value as i32)>=beta {
                    let value=-(e.value as i32);
                    self.save(key,Entry{position:p,value:value as i16,depth,bound:2,best:m.id()});
                    return Ok(value);
                }
                let shallow=if depth>=16 {-self.run_key(child,ck,1,-65,65)?} else {0};
                let parity=if exact_depth && depth<=7 {region_parity(p.empty(),m.a)} else {0};
                ranks[i]=if hit && m.id()==old.best {-100000} else {child.mobility()*128-shallow-parity*16};
            }
        } else if hit {
            if let Some(i)=ms.iter().position(|m|m.id()==old.best) {ms.swap(0,i);}
        }
        let mut best = -1000;
        let mut best_id = ms[0].id();
        for i in 0..ms.len() {
            if depth>=3 { let j=(i..ms.len()).min_by_key(|&j|ranks[j]).unwrap();ms.swap(i,j);ranks.swap(i,j); }
            let m=ms[i];
            let child = p.play(m);
            let ck=p.child_hash(key,m);
            let mut v;
            if i == 0 {
                v = -self.run_key(child,ck, depth - 1, -beta, -alpha)?
            } else {
                v = -self.run_key(child,ck, depth - 1, -alpha - 1, -alpha)?;
                if v > alpha && v < beta {
                    v = -self.run_key(child,ck, depth - 1, -beta, -alpha)?
                }
            }
            if v > best {
                best = v;
                best_id = m.id()
            }
            alpha = alpha.max(v);
            if alpha >= beta {
                break;
            }
        }
        let bound = if best <= original_alpha {
            3
        } else if best >= beta {
            2
        } else {
            1
        };
        self.save(key,Entry {
            position:p,value:best as i16,depth,bound,best:best_id,
        });
        Ok(best)
    }
    fn last(&mut self,p:Position,mut alpha:i32,beta:i32,passed:bool)->Result<i32,()> {
        self.nodes+=1;
        if self.nodes&255==0 && now()>=self.deadline {self.aborted=true;return Err(())}
        let ms=p.move_list();
        if ms.is_empty() {
            if passed {return Ok(p.diff()*p.side as i32)}
            return self.last(p.pass(),-beta,-alpha,true).map(|v|-v);
        }
        let mut best=-65;
        for m in ms.iter() {
            let v=-self.last(p.play(*m),-beta,-alpha,false)?;
            best=best.max(v);alpha=alpha.max(v);if alpha>=beta {break}
        }
        Ok(best)
    }
    pub fn exact(&mut self, p: Position, wld: bool) -> Result<i32, ()> {
        let d = (p.empty().count_ones() / 2) as u8;
        if wld {
            self.run(p, d, -1, 1).map(i32::signum)
        } else {
            let w=self.run(p,d,-1,1)?;
            if w==0 {return Ok(0)}
            let mut lo=if w>0 {w.max(1)} else {-64};
            let mut hi=if w<0 {w.min(-1)} else {64};
            // Aspiration widens around a fail-soft WLD result, never treats a
            // bound as an exact disc difference.
            let mut guess=w; let mut width=8;
            loop {
                let a=(guess-width).max(lo-1); let b=(guess+width).min(hi+1);
                let v=self.run(p,d,a,b)?;
                if v>a && v<b {return Ok(v)}
                if v<=a {hi=hi.min(v)} else {lo=lo.max(v)}
                if lo==hi {return Ok(lo)}
                guess=v;width=(width*2).min(128);
            }
        }
    }
    pub fn pv(&self, mut p: Position, limit: usize) -> Vec<String> {
        let mut out = vec![];
        for _ in 0..limit {
            let ms = p.moves();
            if ms.is_empty() {
                if p.pass().moves().is_empty() {
                    break;
                }
                out.push("pass".into());
                p = p.pass();
                continue;
            }
            let e = self.entry(p);
            if e.bound == 0 || e.position != p {
                break;
            }
            if let Some(m) = ms.into_iter().find(|m| m.id() == e.best) {
                out.push(m.to_string());
                p = p.play(m)
            } else {
                break;
            }
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct MoveValue {
    pub mv: Move,
    pub value: i32,
    pub exact: bool,
    pub depth: u8,
    pub pv: Vec<String>,
    pub nodes: u64,
}
#[derive(Clone)]
pub struct Options {
    pub time_ms: u64,
    pub threads: usize,
    pub exact: bool,
    pub tt_entries: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            time_ms: 1000,
            threads: 1,
            exact: true,
            tt_entries: 1 << 20,
        }
    }
}
#[derive(Debug)]
pub struct Analysis {
    pub complete: bool,
    pub moves: Vec<MoveValue>,
    pub nodes: u64,
    pub elapsed_ms: f64,
    pub terminal: bool,
    pub pass: bool,
    pub value: Option<i32>,
}
fn analyze_move(p: Position, m: Move, o: &Options, deadline: f64, s: &mut Search) -> MoveValue {
    let child = p.play(m);
    let initial_nodes = s.nodes;
    s.deadline = deadline;
    s.aborted = false;
    let mut result = MoveValue {
        mv: m,
        value: -evaluate(child, s.weights),
        exact: false,
        depth: 0,
        pv: vec![m.to_string()],
        nodes: 0,
    };
    let max = (child.empty().count_ones() / 2) as u8;
    // Reserve the first half for completed iterative values, then try terminal search.
    let exact_start = now() + (deadline - now()) * 0.5;
    if o.exact && child.empty().count_ones() <= 34 {
        s.deadline = exact_start;
    }
    for depth in 0..=max {
        if now() >= deadline {
            break;
        }
        if depth > 0 && o.exact && child.empty().count_ones() <= 34 && now() >= exact_start {
            break;
        }
        let lo = if depth > 1 {
            (-result.value - 6).max(-65)
        } else {
            -65
        };
        let hi = if depth > 1 {
            (-result.value + 6).min(65)
        } else {
            65
        };
        let trial = s.run(child, depth, lo, hi);
        let v = match trial {
            Ok(v) if v <= lo || v >= hi => s.run(child, depth, -65, 65),
            x => x,
        };
        match v {
            Ok(v) => {
                result.value = -v;
                result.depth = depth + 1;
                result.exact = depth == max;
                result.pv = vec![m.to_string()];
                result.pv.extend(s.pv(child, depth as usize + 8));
            }
            Err(_) => break,
        }
        if result.exact {
            break;
        }
    }
    s.deadline = deadline;
    s.aborted = false;
    if o.exact && !result.exact && now() < deadline {
        if let Ok(v) = s.exact(child, false) {
            result.value = -v;
            result.exact = true;
            result.depth = max + 1;
            result.pv = vec![m.to_string()];
            result.pv.extend(s.pv(child, 64));
        }
    }
    result.nodes = s.nodes - initial_nodes;
    result
}
pub fn analyze(p: Position, o: &Options) -> Analysis {
    let start = now();
    let deadline = start + o.time_ms as f64;
    let ms = p.moves();
    if ms.is_empty() {
        let terminal = p.pass().moves().is_empty();
        if terminal {
            return Analysis {
                complete: true,
                moves: vec![],
                nodes: 0,
                elapsed_ms: now() - start,
                terminal: true,
                pass: false,
                value: Some(p.diff() * p.side as i32),
            };
        }
        let next = analyze(p.pass(), o);
        let value = next.moves.iter().map(|x| x.value).max().map(|x| -x);
        return Analysis {
            complete: next.complete,
            moves: vec![],
            nodes: next.nodes,
            elapsed_ms: now() - start,
            terminal: false,
            pass: true,
            value,
        };
    }
    let workers = o.threads.max(1).min(ms.len());
    let entries = (o.tt_entries / workers / if workers>1 {2} else {1}).max(1024);
    #[cfg(not(target_arch="wasm32"))]
    let shared=if workers>1 {Some(std::sync::Arc::new(SharedTable::new(o.tt_entries/2)))} else {None};
    #[cfg(not(target_arch = "wasm32"))]
    let mut values: Vec<MoveValue>;
    #[cfg(target_arch = "wasm32")]
    let mut values = vec![];
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let index = AtomicUsize::new(0);
        values = std::thread::scope(|scope| {
            let mut jobs = vec![];
            for _ in 0..workers {
                let ms = &ms;
                let index = &index;
                let shared=shared.clone();
                jobs.push(scope.spawn(move || {
                    let mut out = vec![];
                    let mut search = Search::new(entries, deadline);
                    search.shared=shared;
                    loop {
                        let i = index.fetch_add(1, Ordering::Relaxed);
                        if i >= ms.len() {
                            break;
                        }
                        let remaining = ms.len().saturating_sub(i);
                        let rounds = (remaining + workers - 1) / workers;
                        let local_deadline =
                            now() + (deadline - now()).max(0.0) / rounds.max(1) as f64;
                        out.push(analyze_move(p, ms[i], o, local_deadline, &mut search))
                    }
                    out
                }));
            }
            jobs.into_iter()
                .flat_map(|j| j.join().expect("search worker"))
                .collect()
        });
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut search = Search::new(entries, deadline);
        for (i, m) in ms.iter().enumerate() {
            let local_deadline = now() + (deadline - now()).max(0.0) / (ms.len() - i) as f64;
            values.push(analyze_move(p, *m, o, local_deadline, &mut search));
        }
    }
    values.sort_by_key(|v| v.mv.id());
    let nodes = values.iter().map(|x| x.nodes).sum();
    Analysis {
        complete: values.iter().all(|v| v.exact),
        moves: values,
        nodes,
        elapsed_ms: now() - start,
        terminal: false,
        pass: false,
        value: None,
    }
}
impl Analysis {
    pub fn best(&self) -> Option<&MoveValue> {
        self.moves.iter().max_by(|a, b| {
            a.value
                .cmp(&b.value)
                .then_with(|| b.mv.id().cmp(&a.mv.id()))
        })
    }
    pub fn json(&self) -> String {
        use crate::json::quote;
        let moves=self.moves.iter().map(|x|format!("{{\"move\":{},\"cells\":[{},{}],\"value\":{},\"exact\":{},\"depth\":{},\"bestReply\":{},\"pv\":[{}],\"nodes\":{}}}",quote(&x.mv.to_string()),x.mv.a,x.mv.b,x.value,x.exact,x.depth,x.pv.get(1).map_or("null".into(),|v|quote(v)),x.pv.iter().map(|v|quote(v)).collect::<Vec<_>>().join(","),x.nodes)).collect::<Vec<_>>().join(",");
        format!("{{\"complete\":{},\"moves\":[{moves}],\"bestMove\":{},\"nodes\":{},\"elapsedMs\":{:.3},\"nps\":{:.0},\"terminal\":{},\"pass\":{},\"value\":{},\"perspective\":\"side-to-move\"}}",self.complete,self.best().map_or("null".into(),|x|quote(&x.mv.to_string())),self.nodes,self.elapsed_ms,self.nodes as f64/(self.elapsed_ms/1000.0).max(0.000001),self.terminal,self.pass,self.value.map_or("null".into(),|v|v.to_string()))
    }
}

// Odd number of remaining domino placements in the connected empty region.
fn region_parity(e:u64,a:u8)->i32 {
    let mut region=1u64<<a;
    loop {let next=region|((shift(region,1)|shift(region,3)|shift(region,4)|shift(region,6))&e);
        if next==region {break} region=next;
    }
    ((region.count_ones()/2)&1) as i32
}

// Bounded, nonblocking seqlock slots: busy readers/writers skip the slot.
// All payloads are atomic and SeqCst, so no torn board can be accepted.
// Full board comparison makes fingerprint collisions harmless.
#[cfg(not(target_arch="wasm32"))]
struct AtomicEntry { version:std::sync::atomic::AtomicU64, data:[std::sync::atomic::AtomicU64;5] }
#[cfg(not(target_arch="wasm32"))]
impl AtomicEntry {
    fn new()->Self {Self{version:0.into(),data:std::array::from_fn(|_|0.into())}}
    fn read(&self)->Option<Entry> {
        use std::sync::atomic::Ordering::SeqCst;
        let v=self.version.load(SeqCst);if v&1!=0 {return None}
        let d=self.data.each_ref().map(|x|x.load(SeqCst));
        if self.version.load(SeqCst)!=v || d[4]==0 {return None}
        Some(Entry{position:Position{black:d[0],white:d[1],hleft:d[2],vtop:d[3],side:if d[4]>>48&1==0 {1} else {-1}},
            value:d[4] as i16,depth:(d[4]>>16) as u8,bound:(d[4]>>24) as u8,best:(d[4]>>32) as u16})
    }
    fn write(&self,e:Entry) {
        use std::sync::atomic::Ordering::SeqCst;
        let v=self.version.load(SeqCst);if v&1!=0 {return}
        if self.version.compare_exchange(v,v.wrapping_add(1),SeqCst,SeqCst).is_err() {return}
        let meta=e.value as u16 as u64 | (e.depth as u64)<<16 | (e.bound as u64)<<24 | (e.best as u64)<<32 | ((e.position.side==-1) as u64)<<48;
        for (a,b) in self.data.iter().zip([e.position.black,e.position.white,e.position.hleft,e.position.vtop,meta]) {a.store(b,SeqCst);}
        self.version.store(v.wrapping_add(2),SeqCst);
    }
}
#[cfg(not(target_arch="wasm32"))]
pub struct SharedTable {slots:Vec<AtomicEntry>}
#[cfg(not(target_arch="wasm32"))]
impl SharedTable {
    pub fn new(entries:usize)->Self {Self{slots:(0..entries.max(2).next_power_of_two()).map(|_|AtomicEntry::new()).collect()}}
    fn index(&self,key:u64)->usize {table_hash(key) as usize & (self.slots.len()-1) & !1}
    fn get(&self,key:u64,p:Position)->Option<Entry> {
        let i=self.index(key);
        [i,i+1].into_iter().filter_map(|j|self.slots[j].read()).filter(|e|e.position==p).max_by_key(|e|e.depth)
    }
    fn save(&self,key:u64,e:Entry) {
        let i=self.index(key);
        let replace=self.slots[i].read().map_or(true,|old|old.depth<=e.depth);
        self.slots[i+if replace {0}else{1}].write(e);
    }
}
#[inline]
fn table_hash(mut key:u64)->u64 {
    key=(key^(key>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    key=(key^(key>>27)).wrapping_mul(0x94d049bb133111eb);
    key^(key>>31)
}
