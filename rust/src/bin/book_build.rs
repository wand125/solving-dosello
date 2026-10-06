use dosello_ai::{board::Position,book::*,search::{Search,SharedTable,entries_for_mb},cli};
use std::{collections::BTreeSet,fs::OpenOptions,io::Write,path::PathBuf,sync::{mpsc,Arc,Mutex},time::{Duration,Instant}};
fn main(){cli::main_result(run)}
fn run()->Result<(),String>{
    let mut threads=14usize;let mut coverage_depth=6u32;let mut coverage_all=2u32;let mut coverage_top_k=3usize;let mut leaf_ms=3000u64;let mut exact_ms=10000u64;let mut exact_empties=38u32;let mut tt=28u32;let mut tt_mb=Some(4096usize);let mut cache_min=30u32;let mut margin=4i32;let mut seconds=0u64;let mut checkpoint=60u64;
    let mut path=PathBuf::from("rust/book/book.jsonl");let mut results=PathBuf::from("rust/results");
    let mut args=std::env::args().skip(1);
    while let Some(a)=args.next(){if a=="--help"{println!("book_build [--threads 14] [--coverage-depth 6] [--coverage-all 2] [--coverage-top-k 3] [--leaf-ms 3000] [--exact-ms 10000] [--exact-empties 38] [--tt-mb 4096 (shared total MiB)] [--cache-min-empties 30] [--margin 4] [--checkpoint-secs 60] [--seconds 0] [--book rust/book/book.jsonl] [--results rust/results]\nStop cleanly: create <book>.stop; remove it to resume. SIGTERM/kill -9: resume last synced journal records.");return Ok(())}
        let v=args.next().ok_or("Missing argument")?;macro_rules! number{()=>{v.parse().map_err(|_|format!("Invalid {a}"))?}}
        match a.as_str(){"--coverage-depth"=>coverage_depth=number!(),"--coverage-all"=>coverage_all=number!(),"--coverage-top-k"=>coverage_top_k=number!(),"--threads"=>threads=number!(),"--leaf-ms"=>leaf_ms=number!(),"--exact-ms"=>exact_ms=number!(),"--exact-empties"=>exact_empties=number!(),"--tt"=>{tt=number!();tt_mb=None},"--tt-mb"=>tt_mb=Some(number!()),"--cache-min-empties"=>cache_min=number!(),"--margin"=>margin=number!(),"--seconds"=>seconds=number!(),"--checkpoint-secs"=>checkpoint=number!(),"--book"=>path=v.into(),"--results"=>results=v.into(),_=>return Err(format!("Unknown {a}"))}
    }
    if !(1..=64).contains(&threads)||!(10..=30).contains(&tt)||leaf_ms==0||exact_ms==0||checkpoint==0||exact_empties>56||margin<0||coverage_depth>28||coverage_all>28||coverage_top_k>112||cache_min>56{return Err("Invalid option range".into())}
    let entries=if let Some(mb)=tt_mb {entries_for_mb(mb)?}else{1usize<<tt};
    std::fs::create_dir_all(path.parent().ok_or("Book directory")?).map_err(|e|e.to_string())?;
    std::fs::create_dir_all(&results).map_err(|e|e.to_string())?;
    // mkdir is atomic: refuse concurrent writers. A crash leaves this lock for
    // the launcher/operator to verify and remove; never silently steal a lock.
    let lock=PathBuf::from(format!("{}.lock",path.display()));std::fs::create_dir(&lock).map_err(|e|format!("{}: {e}; verify old PID before removing stale lock",lock.display()))?;
    struct Lock(PathBuf);impl Drop for Lock{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let _lock=Lock(lock.clone());std::fs::write(lock.join("pid"),std::process::id().to_string()).map_err(|e|e.to_string())?;
    let mut book=Book::load(&path,true)?;book.expand(book.root);book.expand_coverage(coverage_depth,coverage_all,coverage_top_k);book.propagate();book.checkpoint(&path)?;
    let state_path=PathBuf::from(format!("{}.state.json",path.display()));
    let previous_elapsed=std::fs::read_to_string(&state_path).ok().and_then(|s|dosello_ai::json::parse(&s).ok()).and_then(|v|v.get("elapsedSeconds").ok().and_then(|x|x.num().ok())).unwrap_or(0) as u64;
    let mut log=OpenOptions::new().create(true).append(true).open(results.join("book-progress.log")).map_err(|e|e.to_string())?;
    let (jobs_tx,jobs_rx)=mpsc::channel::<(Key,Position,bool)>();let jobs_rx=Arc::new(Mutex::new(jobs_rx));let (tx,rx)=mpsc::channel();let mut workers=vec![];
    let shared=Arc::new(SharedTable::new(entries));
    let cache=Arc::new(dosello_ai::exact_cache::ExactCache::load(&PathBuf::from(format!("{}.exact.bin",path.display())),cache_min)?);
    for node in book.nodes.values().filter(|n|n.exact()) {cache.insert(node.p,node.lo);}
    cache.checkpoint()?;
    writeln!(log,"engine ttMiB={} shared=true workers={threads} cacheLoaded={}",entries*16/1048576,cache.len()).map_err(|e|e.to_string())?;
    for _ in 0..threads{let shared=shared.clone();let cache=cache.clone();let jobs=jobs_rx.clone();let tx=tx.clone();workers.push(std::thread::spawn(move||{let mut search=Search::new(4,0.0);search.shared=Some(shared);search.exact_cache=Some(cache);loop{let job=jobs.lock().unwrap().recv();let Ok((k,p,exact))=job else{break};let r=evaluate_leaf(&mut search,p,if exact{exact_ms}else{leaf_ms},exact);if tx.send((k,r)).is_err(){break}}}));}drop(tx);
    let start=Instant::now();let mut last_save=Instant::now();let mut last_log=Instant::now()-Duration::from_secs(10);let mut busy=BTreeSet::new();let mut dispatch=book.completed;let initial_completed=book.completed;let mut boosted=false;let mut stopping=false;
    loop{
        if seconds>0&&start.elapsed().as_secs()>=seconds || PathBuf::from(format!("{}.stop",path.display())).exists(){stopping=true;}
        let mut queue=book.frontier(&busy,margin);
        while !stopping&&busy.len()<threads {
            let chosen=if dispatch%32==31{queue.iter().filter(|(_,_,k)|!busy.contains(&k.0)).min_by_key(|(_,_,k)|{let n=&book.nodes[&k.0];(n.updated,k.0)}).copied()}else{queue.pop()};
            let Some((_,_,std::cmp::Reverse(k)))=chosen else{break};
            if busy.contains(&k){continue}
            if book.nodes[&k].evaluated {book.expand(k);book.propagate();queue=book.frontier(&busy,margin);continue}
            let p=book.nodes[&k].p;let e=p.empty().count_ones();let exact=e<=exact_empties || (e<=exact_empties+2&&(boosted||dispatch%8==0));
            jobs_tx.send((k,p,exact)).map_err(|e|e.to_string())?;busy.insert(k);dispatch+=1;
        }
        if busy.is_empty(){stopping=true}
        if !busy.is_empty(){match rx.recv_timeout(Duration::from_millis(500)){Ok(first)=>{for (k,r) in std::iter::once(first).chain(rx.try_iter()){if r.attempted_exact&&book.nodes[&k].p.empty().count_ones()>exact_empties{boosted=r.lo==r.hi}busy.remove(&k);book.apply(k,r);}book.propagate();},Err(mpsc::RecvTimeoutError::Timeout)=>{},Err(e)=>return Err(format!("Worker failure: {e}"))}}
        if last_save.elapsed().as_secs()>=checkpoint || stopping&&busy.is_empty(){
            cache.checkpoint()?;
            let count=book.checkpoint(&path)?;let elapsed=previous_elapsed+start.elapsed().as_secs();
            atomic_write(&state_path,&format!("{{\"version\":1,\"pid\":{},\"elapsedSeconds\":{elapsed},\"completed\":{},\"nodes\":{},\"updatedAt\":{},\"adaptiveEmpties\":{}}}\n",std::process::id(),book.completed,book.nodes.len(),stamp(),exact_empties+if boosted{2}else{0}))?;
            writeln!(log,"checkpoint updatedAt={} records={count} bytes={}",stamp(),std::fs::metadata(&path).map_err(|e|e.to_string())?.len()).map_err(|e|e.to_string())?;log.flush().map_err(|e|e.to_string())?;last_save=Instant::now();
        }
        if last_log.elapsed().as_secs()>=10 || stopping&&busy.is_empty(){
            writeln!(log,"cache entries={} probes={} hits={}",cache.len(),cache.probes.load(std::sync::atomic::Ordering::Relaxed),cache.hits.load(std::sync::atomic::Ordering::Relaxed)).map_err(|e|e.to_string())?;
            let exact=book.nodes.values().filter(|n|n.exact()).count();let elapsed=previous_elapsed+start.elapsed().as_secs();let root=&book.nodes[&book.root];
            let moves=Position::initial().moves().iter().map(|m|{let n=&book.nodes[&key(canonical(Position::initial().play(*m)))];let bound=if n.exact(){"exact"}else if n.hi<64{"lower"}else if n.lo> -64{"upper"}else{"heuristic"};format!("{m}:{:+}/{bound}[{},{}]",reported(-n.value,-n.hi,-n.lo),-n.hi,-n.lo)}).collect::<Vec<_>>().join(" ");
            writeln!(log,"progress updatedAt={} pid={} elapsed={}s session={}s nodes={} exact={} leaves={} rate={:.2}/s busy={} root={:+}/{} rootMoves={} PV={}",stamp(),std::process::id(),elapsed,start.elapsed().as_secs(),book.nodes.len(),exact,book.completed,(book.completed-initial_completed) as f64/start.elapsed().as_secs_f64().max(0.001),busy.len(),root.value,root.bound(),moves,book.pv(Position::initial(),28).join(" ")).map_err(|e|e.to_string())?;log.flush().map_err(|e|e.to_string())?;last_log=Instant::now();
        }
        if stopping&&busy.is_empty(){break}
    }
    drop(jobs_tx);for worker in workers{worker.join().map_err(|_|"Worker panic")?;}Ok(())
}
