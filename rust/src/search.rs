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
pub fn hand_evaluate(p: Position, w: Weights) -> i32 {
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
pub fn evaluate(p:Position,_w:Weights)->i32 {crate::pattern::evaluate(p)}
#[derive(Clone, Copy, Default)]
struct Entry {
    position: Position,
    value: i16,
    depth: u8,
    bound: u8,
    best: u16,
}
// Cache-line buckets. The independent, nonlinear board signature checks all
// colours, pair directions and side; the incremental key is only an index.
#[derive(Clone, Copy, Default)]
struct PackedEntry { check: u64, data: u64 }
#[derive(Clone, Copy, Default)]
#[repr(align(64))]
struct Bucket([PackedEntry;4]);
fn signature(p:Position)->u64 {
    table_hash(p.black.wrapping_add(0x9e3779b97f4a7c15)) ^
    table_hash(p.white.wrapping_add(0x243f6a8885a308d3)) ^
    table_hash(p.hleft.wrapping_add(0x13198a2e03707344)) ^
    table_hash(p.vtop.wrapping_add(0xa4093822299f31d0)) ^
    if p.side==1 {0} else {0x082efa98ec4e6c89}
}
fn pack(e:Entry,policy:u8)->u64 {
    e.value as i8 as u8 as u64 | (e.best as u64)<<8 |
    (e.depth as u64)<<20 | (e.bound as u64)<<26 | (policy as u64)<<28
}
fn unpack(data:u64,p:Position)->Entry {
    Entry{position:p,value:data as u8 as i8 as i16,best:((data>>8)&4095) as u16,
          depth:((data>>20)&63) as u8,bound:((data>>26)&3) as u8}
}
fn replacement_rank(data:u64,age:u8)->i32 {
    ((data>>20)&63) as i32 - 4*((age.wrapping_sub((data>>30) as u8))&3) as i32
}
/// MiB is binary (2^20 bytes); round down to a power-of-two bucket count.
/// Bounds avoid overflow and accidental unbounded allocations from CLI input.
pub fn entries_for_mb(mb:usize)->Result<usize,String> {
    if !(1..=32768).contains(&mb) {return Err("--tt-mb must be 1..32768".into())}
    let buckets=mb*16384;
    Ok((1usize << (usize::BITS-1-buckets.leading_zeros()))*4)
}
#[cfg(not(target_arch="wasm32"))]
#[path = "parallel.rs"]
mod parallel;
#[cfg(not(target_arch="wasm32"))]
#[path = "proof_checkpoint.rs"]
mod proof_checkpoint;
#[cfg(not(target_arch="wasm32"))]
pub fn prove_checkpoint_json(p:Position,o:&Options,path:&std::path::Path)->Result<String,String> {
    proof_checkpoint::prove(p,o,path)
}

pub struct Search {
    #[cfg(not(target_arch="wasm32"))]
    parallel: Option<std::sync::Arc<parallel::Pool>>,
    #[cfg(not(target_arch="wasm32"))]
    root_null: bool,
    #[cfg(not(target_arch="wasm32"))]
    cancellation: Option<std::sync::Arc<parallel::Cancellation>>,
    age: u8,
    pub last_empties: u32,
    selectivity: u8,
    probing: bool,
    table: Vec<Bucket>,
    #[cfg(not(target_arch="wasm32"))]
    pub shared:Option<std::sync::Arc<SharedTable>>,
    #[cfg(not(target_arch="wasm32"))]
    pub exact_cache:Option<std::sync::Arc<crate::exact_cache::ExactCache>>,
    pub nodes: u64,
    pub deadline: f64,
    pub weights: Weights,
    pub aborted: bool,
    pub progress: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}
impl Search {
    pub fn new(entries: usize, deadline: f64) -> Self {
        Self {
            #[cfg(not(target_arch="wasm32"))]
            parallel: None,
            #[cfg(not(target_arch="wasm32"))]
            root_null: false,
            #[cfg(not(target_arch="wasm32"))]
            cancellation: None,
            age: 0,
            last_empties: 10,
            selectivity: 0,
            probing: false,
            table: vec![Bucket::default(); entries.max(4).next_power_of_two()/4],
            #[cfg(not(target_arch="wasm32"))]
            shared:None,
            #[cfg(not(target_arch="wasm32"))]
            exact_cache:None,
            nodes: 0,
            deadline,
            weights: Weights::default(),
            aborted: false,
            progress: None,
        }
    }
    /// Each statistical policy has a separate TT namespace. Terminal searches
    /// never use statistical cuts; depth guards separate heuristics from proofs.
    pub fn set_selectivity(&mut self,level:u8) {self.selectivity=level.min(3);}
    pub fn clear(&mut self) {self.table.fill(Bucket::default());}
    fn index(&self, key:u64)->usize {table_hash(key) as usize & (self.table.len()-1)}
    fn entry_key(&self,p:Position,key:u64)->Entry {
        #[cfg(not(target_arch="wasm32"))]
        if let Some(t)=&self.shared {return t.get(key,p,self.selectivity).unwrap_or_default()}
        let check=signature(p);
        self.table[self.index(key)].0.iter().filter(|e|e.data!=0&&e.check==check&&((e.data>>28)&3)==self.selectivity as u64)
            .max_by_key(|e|(e.data>>20)&63).map_or(Entry::default(),|e|unpack(e.data,p))
    }
    fn entry(&self,p:Position)->Entry {self.entry_key(p,p.hash())}
    fn save(&mut self,key:u64,e:Entry) {
        #[cfg(not(target_arch="wasm32"))]
        if e.bound==1 && e.depth as u32>=e.position.empty().count_ones()/2 {
            if let Some(cache)=&self.exact_cache {cache.insert(e.position,e.value as i32);}
        }
        #[cfg(not(target_arch="wasm32"))]
        if let Some(t)=&self.shared {t.save(key,e,self.selectivity,self.age);return}
        let check=signature(e.position);let i=self.index(key);
        let slots=&mut self.table[i].0;
        let j=slots.iter().position(|x|x.data==0||x.check==check).unwrap_or_else(||
            (0..4).min_by_key(|&j|replacement_rank(slots[j].data,self.age)).unwrap());
        if slots[j].check==check && ((slots[j].data>>28)&3)==self.selectivity as u64 && ((slots[j].data>>20)&63)>e.depth as u64 {return}
        slots[j]=PackedEntry{check,data:pack(e,self.selectivity)|((self.age as u64&3)<<30)};
    }
    pub fn run(&mut self,p:Position,depth:u8,alpha:i32,beta:i32)->Result<i32,()> {
        self.age=self.age.wrapping_add(1);
        #[cfg(not(target_arch="wasm32"))]
        if let Some(t)=&self.shared {self.age=t.generation.fetch_add(1,std::sync::atomic::Ordering::Relaxed) as u8;}
        self.run_key(p,p.hash(),depth,alpha,beta)
    }
    fn run_key(&mut self, p: Position,key:u64, depth: u8, mut alpha: i32, beta: i32) -> Result<i32, ()> {
        self.nodes += 1;
        #[cfg(not(target_arch="wasm32"))]
        if self.nodes & 255 == 0 && self.cancelled() { return Err(()); }
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
            #[cfg(not(target_arch="wasm32"))]
            if let Some(cache)=&self.exact_cache {
                if p.empty().count_ones()>=cache.min_empties {if let Some(value)=cache.get(p) {return Ok(value)}}
            }
            let e=p.empty();
            let live=e & (shift(e,1)|shift(e,3)|shift(e,4)|shift(e,6));
            // Isolated empty cells can never acquire a partner. Final occupancy
            // is even, and all remaining discs can change colour.
            let max=((!e).count_ones()+live.count_ones()) as i32 & !1;
            if max<=alpha {return Ok(max)}
            if -max>=beta {return Ok(-max)}
            if live.count_ones()<=self.last_empties {self.nodes-=1;return self.last(p,alpha,beta,false);}
            if depth>=6 && (alpha>=40 || beta<=-40) {
                let max=p.occupancy_bound();
                let upper=max-2*p.stable_edges(p.opp()).count_ones() as i32;
                if upper<=alpha {return Ok(upper)}
                let lower=2*p.stable_edges(p.own()).count_ones() as i32-max;
                if lower>=beta {return Ok(lower)}
            }
        }
        let original_alpha = alpha;
        let old = self.entry_key(p,key);
        let hit = old.bound != 0 && old.position == p;
        if hit && old.depth >= depth {
            let v = old.value as i32;
            if old.bound == 1 || old.bound == 2 && v >= beta || old.bound == 3 && v <= alpha {
                return Ok(v);
            }
        }
        let lazy=depth<=2;
        let mut ms = if lazy {p.lazy_moves()} else {p.move_list()};
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
        if !exact_depth && self.selectivity>0 && !self.probing {
            let phase=crate::pattern::phase(p);
            for c in crate::probcut::models().iter().filter(|c|c.phase==phase&&c.deep==depth&&c.n>=100&&c.slope>0.25) {
                // Conservative confidence multipliers, never advertised as a
                // Gaussian probability guarantee (residual tails are empirical).
                let z=match self.selectivity {1=>5.0,2=>4.0,_=>3.0};
                let margin=z*c.sigma.max(2.0)+2.0;
                let high=((beta as f64+margin-c.intercept)/c.slope).ceil() as i32;
                let low=((alpha as f64-margin-c.intercept)/c.slope).floor() as i32;
                self.probing=true;
                let trial=if high<64&&beta<64 {self.run_key(p,key,c.shallow,high-1,high).map(|v|if v>=high {Some(beta)}else{None})}else{Ok(None)};
                self.probing=false;
                if let Some(v)=trial? {return Ok(v)}
                self.probing=true;
                let trial=if low> -64&&alpha> -64 {self.run_key(p,key,c.shallow,low,low+1).map(|v|if v<=low {Some(alpha)}else{None})}else{Ok(None)};
                self.probing=false;
                if let Some(v)=trial? {return Ok(v)}
            }
        }
        let mut ranks=[0i32;112];
        if depth>=3 {
            for i in 0..ms.len() {
                let m=&ms[i];
                let child=p.play(*m);
                let ck=p.child_hash(key,*m);
                // Exact leaf nodes bypass the TT, so an ETC probe for them
                // cannot find a sufficient-depth entry. Avoid a random cache
                // miss for every candidate directly above the leaf solver.
                let empty=child.empty();
                let live=empty & (shift(empty,1)|shift(empty,3)|shift(empty,4)|shift(empty,6));
                let e=if exact_depth && live.count_ones()<=self.last_empties {Entry::default()}
                    else {self.entry_key(child,ck)};
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
                ranks[i]=if hit && m.id()==old.best {-100000} else {if depth>=16 {child.mobility()*4-shallow*8} else {child.mobility()*128-parity*16}};
            }
        } else if hit {
            if let Some(i)=ms.iter().position(|m|m.id()==old.best) {ms.swap(0,i);}
        }
        let mut best=-1000;
        let mut best_id=ms[0].id();
        for i in 0..ms.len() {
            if depth>=3 { let j=(i..ms.len()).min_by_key(|&j|ranks[j]).unwrap();ms.swap(i,j);ranks.swap(i,j); }
            let m=if depth<=2 {p.with_flips(ms[i])}else{ms[i]};
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
            // YBWC: only expose younger siblings after the eldest has failed
            // to cut. The same PVS windows/order are used by serial and workers.
            #[cfg(not(target_arch="wasm32"))]
            if i == 0 && exact_depth && depth >= 12 && ms.len() > 1 {
                if let Some(pool) = self.parallel.clone() {
                    if pool.has_idle() && depth >= pool.min_depth {
                        let mut remaining: Vec<_> = (1..ms.len()).collect();
                        remaining.sort_by_key(|&j| ranks[j]);
                        let moves = remaining.into_iter().map(|j| ms[j]).collect();
                        let result = self.split(pool, p, key, depth, alpha, beta, best, best_id, moves)?;
                        best = result.0;
                        best_id = result.1;
                        break;
                    }
                }
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
        #[cfg(not(target_arch="wasm32"))]
        if self.nodes & 255 == 0 && self.cancelled() { return Err(()); }
        if self.nodes&255==0 && now()>=self.deadline {self.aborted=true;return Err(())}
        let e=p.empty();
        let h=e&(e>>1)&0x7f7f7f7f7f7f7f7f;let v=e&(e>>8);
        if h|v==0 {return Ok(p.diff()*p.side as i32)}
        let mut starts=h|v;let mut best=-65;
        while starts!=0 {
            let a=starts.trailing_zeros() as u8;let bit=1u64<<a;starts&=starts-1;
            for (mask,step) in [(h,1),(v,8)] {
                if mask&bit==0 {continue}
                let m=p.with_flips(Move{a,b:a+step,flips:0});if m.flips==0 {continue}
                let value=-self.last(p.play(m),-beta,-alpha,false)?;
                best=best.max(value);alpha=alpha.max(value);if alpha>=beta {return Ok(best)}
            }
        }
        if best==-65 {
            if passed {return Ok(p.diff()*p.side as i32)}
            return self.last(p.pass(),-beta,-alpha,true).map(|v|-v);
        }
        Ok(best)
    }
    pub fn exact(&mut self, p: Position, wld: bool) -> Result<i32, ()> {
        let d = (p.empty().count_ones() / 2) as u8;
        #[cfg(not(target_arch="wasm32"))]
        if !wld && self.root_null {
            let (mut lower,mut upper)=(-64,64);
            let mut guess=evaluate(p,self.weights)/2*2;
            while lower<upper {
                let beta=if guess==lower {guess+2}else{guess};
                guess=self.run(p,d,beta-1,beta)?;
                if guess<beta {upper=guess}else{lower=guess}
            }
            return Ok(guess)
        }
        if wld {
            self.run(p, d, -1, 1).map(i32::signum)
        } else {
            let mut lo=-64;let mut hi=64;
            let mut guess=evaluate(p,self.weights);let mut width=8;
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
    /// A selective endgame prediction is a seed only. Always run a complete,
    /// non-statistical terminal search before returning any exact result.
    pub fn selective_exact(&mut self,p:Position)->Result<i32,()> {
        let d=(p.empty().count_ones()/2) as u8;
        let deadline=self.deadline;let start=now();self.set_selectivity(1);
        self.deadline=start+(deadline-start).max(0.0)*0.1;
        let mut guess=evaluate(p,self.weights);
        for depth in 1..d {match self.run(p,depth,-65,65){Ok(v)=>guess=v,Err(_)=>break}}
        self.deadline=deadline;self.aborted=false;
        // Existing heuristic TT entries have depth strictly below terminal
        // depth. They may order moves but cannot provide terminal cutoffs.
        let trial=self.run(p,d,(guess-4).max(-65),(guess+4).min(65))?;
        if trial>guess-4&&trial<guess+4 {Ok(trial)}else{self.exact(p,false)}
    }
    /// Prove the root score, then prove one child attains it. Inferior moves
    /// only need upper bounds; their exact values are intentionally omitted.
    pub fn prove_best(&mut self,p:Position)->Result<(i32,Option<Move>),()> {
        let value=self.exact(p,false)?;
        let mut ms=p.moves();
        let preferred=self.entry(p).best;
        ms.sort_by_key(|m|m.id()!=preferred);
        let d=(p.empty().count_ones()/2) as u8;
        for m in ms {
            // Root proof says every child >= -value. A child upper bound <=
            // -value therefore proves equality, even when the TT PV was lost.
            if self.run(p.play(m),d.saturating_sub(1),-value,-value+1)?<=-value {
                return Ok((value,Some(m)))
            }
        }
        if p.mobility()==0 {Ok((value,None))} else {Err(())}
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
    pub selectivity: u8,
    pub selective_exact: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            time_ms: 1000,
            selectivity: 1,
            selective_exact: false,
            threads: 1,
            exact: true,
            tt_entries: if cfg!(target_arch="wasm32") {1<<21} else {1<<24},
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
    s.set_selectivity(o.selectivity);
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
        if let Ok(v) = if o.selective_exact {s.selective_exact(child)} else {s.exact(child, false)} {
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
    let entries = if workers>1 {4}else{o.tt_entries};
    #[cfg(not(target_arch="wasm32"))]
    let shared=if workers>1 {Some(std::sync::Arc::new(SharedTable::new(o.tt_entries)))} else {None};
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
                    search.set_selectivity(o.selectivity);
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

// Nonblocking two-word seqlock. SeqCst orders every payload read/write.
// High 32 bits of data are a monotonically increasing version; odd = busy.
// Exhausted versions are never reused, avoiding ABA even for stalled readers.
#[cfg(not(target_arch="wasm32"))]
#[derive(Default)]
struct AtomicEntry {check:std::sync::atomic::AtomicU64,data:std::sync::atomic::AtomicU64}
#[cfg(not(target_arch="wasm32"))]
impl AtomicEntry {
    fn read(&self)->Option<PackedEntry> {
        use std::sync::atomic::Ordering::SeqCst;
        let data=self.data.load(SeqCst);
        if data&(1<<32)!=0 || data&0xffffffff==0 {return None}
        let check=self.check.load(SeqCst);
        if self.data.load(SeqCst)!=data {return None}
        Some(PackedEntry{check,data})
    }
    fn write(&self,e:PackedEntry) {
        use std::sync::atomic::Ordering::SeqCst;
        let old=self.data.load(SeqCst);
        let version=old>>32;
        if version&1!=0 || version>=0xfffffffe {return}
        if self.data.compare_exchange(old,((version+1)<<32)|(old&0xffffffff),SeqCst,SeqCst).is_err() {return}
        self.check.store(e.check,SeqCst);
        self.data.store(((version+2)<<32)|e.data,SeqCst);
    }
}
#[cfg(not(target_arch="wasm32"))]
#[derive(Default)]
#[repr(align(64))]
struct AtomicBucket([AtomicEntry;4]);
#[cfg(not(target_arch="wasm32"))]
pub struct SharedTable {slots:Vec<AtomicBucket>,generation:std::sync::atomic::AtomicUsize}
#[cfg(not(target_arch="wasm32"))]
impl SharedTable {
    pub fn new(entries:usize)->Self {Self{slots:(0..entries.max(4).next_power_of_two()/4).map(|_|AtomicBucket::default()).collect(),generation:0.into()}}
    fn index(&self,key:u64)->usize {table_hash(key) as usize & (self.slots.len()-1)}
    fn get(&self,key:u64,p:Position,policy:u8)->Option<Entry> {
        let check=signature(p);
        self.slots[self.index(key)].0.iter().filter_map(AtomicEntry::read)
            .filter(|e|e.check==check&&((e.data>>28)&3)==policy as u64)
            .max_by_key(|e|(e.data>>20)&63).map(|e|unpack(e.data,p))
    }
    fn save(&self,key:u64,e:Entry,policy:u8,age:u8) {
        let check=signature(e.position);let slots=&self.slots[self.index(key)].0;
        let mut victim=0;let mut shallow=64i32;
        for (j,slot) in slots.iter().enumerate() {
            match slot.read() {
                None=>{victim=j;break},
                Some(old) if old.check==check=>{
                    if ((old.data>>28)&3)==policy as u64 && ((old.data>>20)&63)>e.depth as u64 {return}
                    victim=j;break
                },
                Some(old)=>{let depth=replacement_rank(old.data,age);if depth<shallow {shallow=depth;victim=j}}
            }
        }
        slots[victim].write(PackedEntry{check,data:pack(e,policy)|((age as u64&3)<<30)});
    }
}
#[inline]
fn table_hash(mut key:u64)->u64 {
    key=(key^(key>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    key=(key^(key>>27)).wrapping_mul(0x94d049bb133111eb);
    key^(key>>31)
}

/// Separate schema: complete remains false because inferior moves are not valued.
pub fn prove_json(p:Position,o:&Options)->String {
 let start=now();
 #[cfg(not(target_arch="wasm32"))]
 let (result,nodes)=parallel_prove_best(p,o.tt_entries,o.threads,start+o.time_ms as f64);
 #[cfg(target_arch="wasm32")]
 let (result,nodes)={let mut s=Search::new(o.tt_entries,start+o.time_ms as f64);let r=s.prove_best(p).ok();(r,s.nodes)};
 let value=result.map_or("null".into(),|(v,_)|v.to_string());
 let mv=result.and_then(|(_,m)|m);
 format!("{{\"mode\":\"prove-best\",\"proofComplete\":{},\"complete\":false,\"exact\":{},\"value\":{},\"bestMove\":{},\"nodes\":{},\"elapsedMs\":{:.3},\"perspective\":\"side-to-move\"}}",result.is_some(),result.is_some(),value,mv.map_or("null".into(),|m|crate::json::quote(&m.to_string())),nodes,now()-start)
}

/// Playing mode: iterative PVS at the root spends the whole clock finding the
/// best move. Analysis mode separately supplies a value for every legal move.
pub fn choose(p:Position,o:&Options)->Analysis {
 let ms=p.moves();if ms.is_empty(){return analyze(p,o)}
 let start=now();let mut s=Search::new(o.tt_entries,start+o.time_ms as f64);s.set_selectivity(o.selectivity);
 let fallback=*ms.iter().max_by_key(|m|-evaluate(p.play(**m),Weights::default())).unwrap();
 let mut best=MoveValue{mv:fallback,value:-evaluate(p.play(fallback),Weights::default()),exact:false,depth:0,pv:vec![fallback.to_string()],nodes:0};
 let max=(p.empty().count_ones()/2) as u8;
 for d in 1..=max {if now()>=s.deadline{break}
     let Ok(value)=s.run(p,d,-65,65) else{break};
     let mv=if d==max {match s.prove_best(p){Ok((_,Some(m)))=>m,_=>break}}else{let e=s.entry(p);match ms.iter().find(|m|m.id()==e.best){Some(m)=>*m,None=>break}};
     best=MoveValue{mv,value,exact:d==max,depth:d,pv:s.pv(p,64),nodes:s.nodes};
 }
 best.nodes=s.nodes;
 Analysis{complete:false,moves:vec![best],nodes:s.nodes,elapsed_ms:now()-start,terminal:false,pass:false,value:None}
}

/// Fixed worker pool with interior YBWC splits and a helpful master.
#[cfg(not(target_arch="wasm32"))]
pub fn parallel_exact(p:Position,entries:usize,threads:usize,deadline:f64)->(Option<i32>,u64) {
    let (result,nodes)=parallel::solve(p,entries,threads,deadline,false);
    (result.map(|(v,_)|v),nodes)
}
#[cfg(not(target_arch="wasm32"))]
pub fn parallel_prove_best(p:Position,entries:usize,threads:usize,deadline:f64)->(Option<(i32,Option<Move>)>,u64) {
    parallel::solve(p,entries,threads,deadline,true)
}

#[cfg(test)]
mod packed_tests {
    use super::*;
    #[cfg(not(target_arch="wasm32"))]
    #[test]
    fn atomic_payloads_never_tear() {
        use std::sync::atomic::Ordering::SeqCst;
        let entry=AtomicEntry::default();
        std::thread::scope(|scope| {
            for worker in 0..if std::env::var_os("DOSELLO_TEST_SMALL").is_some() { 2 } else { 4 } {let entry=&entry;scope.spawn(move|| {
                for n in 1..=50000u64 {
                    let data=n*4+worker;
                    entry.write(PackedEntry{check:table_hash(data),data});
                }
            });}
            for _ in 0..2 {let entry=&entry;scope.spawn(move|| {
                for _ in 0..300000 {
                    if let Some(e)=entry.read() {assert_eq!(e.check,table_hash(e.data&0xffffffff));}
                }
            });}
        });
        entry.data.store((0xfffffffe<<32)|7,SeqCst);
        entry.check.store(table_hash(7),SeqCst);
        entry.write(PackedEntry{check:table_hash(9),data:9});
        assert_eq!(entry.read().unwrap().data&0xffffffff,7);
        entry.data.store((3<<32)|7,SeqCst);
        assert!(entry.read().is_none());
    }
    #[test]
    fn layout_and_roundtrip() {
        assert_eq!(std::mem::size_of::<PackedEntry>(),16);
        assert_eq!(std::mem::size_of::<Bucket>(),64);
        #[cfg(not(target_arch="wasm32"))] {
            assert_eq!(std::mem::size_of::<AtomicEntry>(),16);
            assert_eq!(std::mem::size_of::<AtomicBucket>(),64);
        }
        let p=Position::initial();
        for value in -64..=64 {for depth in 0..=28 {for bound in 1..=3 {for policy in 0..=3 {
            let e=Entry{position:p,value,depth,bound,best:4095};
            let packed=pack(e,policy);let read=unpack(packed,p);
            assert_eq!((read.value,read.depth,read.bound,read.best),(value,depth,bound,4095));
            assert_eq!((packed>>28)&3,policy as u64);
        }}}}
        assert_eq!(entries_for_mb(256).unwrap()*16,256*1048576);
        assert_eq!(entries_for_mb(16384).unwrap()*16,16384*1048576);
    }
}

#[cfg(not(target_arch="wasm32"))]
pub(crate) fn distributed_execute<R: Send>(o: &Options, deadline: f64, f: impl FnOnce(&mut Search) -> R) -> (R, u64) {
    parallel::execute(o.tt_entries, o.threads, deadline, f)
}
