//! Breadth-first PV margin tree. Durable work events follow synced book records.
use dosello_ai::{board::*,book::*,shards,search::Search,json,cli};
use std::{collections::{BTreeSet,VecDeque},path::{Path,PathBuf},fs::{File,OpenOptions},io::{BufRead,BufReader,Write},time::Instant};
fn main(){cli::main_result(run)}
fn run()->Result<(),String>{
 let mut output=PathBuf::from("rust/book/grow.jsonl");let mut base="rust/book/book.jsonl".to_string();let mut proof="proof/initial-proof.json".to_string();let mut threads=14usize;let mut margin=4i32;let mut ms=100u64;let mut exact_empties=36u32;let mut limit=10000usize;let mut seconds=600u64;let mut partition=(0u64,1u64);
 let mut args=std::env::args().skip(1);while let Some(a)=args.next(){let v=args.next().ok_or("Missing value")?;match a.as_str(){"--output"=>output=v.into(),"--book"=>base=v,"--proof"=>proof=v,"--threads"=>threads=v.parse().map_err(|_|"threads")?,"--margin"=>margin=v.parse().map_err(|_|"margin")?,"--time-ms"=>ms=v.parse().map_err(|_|"ms")?,"--exact-empties"=>exact_empties=v.parse().map_err(|_|"empties")?,"--max-positions"=>limit=v.parse().map_err(|_|"limit")?,"--seconds"=>seconds=v.parse().map_err(|_|"seconds")?,"--partition"=>{let (i,n)=v.split_once('/').ok_or("partition i/n")?;partition=(i.parse().map_err(|_|"partition")?,n.parse().map_err(|_|"partition")?);},_=>return Err(format!("Unknown {a}"))}}
 if threads==0||threads>14||margin<0||exact_empties>56||partition.1==0||partition.0>=partition.1{return Err("Invalid options".into())}
 if output.to_str()==Some(&base){return Err("Grow must use a new journal".into())}
 std::fs::create_dir_all(output.parent().ok_or("output parent")?).map_err(|e|e.to_string())?;
 let lock=PathBuf::from(format!("{}.lock",output.display()));std::fs::create_dir(&lock).map_err(|e|format!("Writer lock {}: {e}",lock.display()))?;
 struct Lock(PathBuf);impl Drop for Lock{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}let _guard=Lock(lock.clone());std::fs::write(lock.join("pid"),std::process::id().to_string()).map_err(|e|e.to_string())?;
 // Repair only our own append tail under the writer lock.
 let _=Book::load(&output,true)?;
 let mut book=shards::merged(&[base,output.to_string_lossy().into()],Path::new(&proof))?;
 for n in book.nodes.values_mut(){n.dirty=false;}
 let initial_nodes=book.nodes.len();let start=Instant::now();
 let config=PathBuf::from(format!("{}.config",output.display()));let settings=format!("v1 margin={margin} partition={}/{}",partition.0,partition.1);
 if config.exists(){if std::fs::read_to_string(&config).map_err(|e|e.to_string())?!=settings{return Err("Resume requires the same margin and partition; use a new output".into())}}else{std::fs::write(config,settings).map_err(|e|e.to_string())?;}
 let work=PathBuf::from(format!("{}.work.jsonl",output.display()));let mut done=BTreeSet::new();let mut events=vec![];let mut valid=0u64;
 if work.exists(){let mut reader=BufReader::new(File::open(&work).map_err(|e|e.to_string())?);let mut line=String::new();loop{line.clear();let read=reader.read_line(&mut line).map_err(|e|e.to_string())?;if read==0||!line.ends_with('\n'){break}let v=json::parse(&line)?;
   let p=json::position(v.get("position")?)?;let mut next=vec![];for c in v.get("next")?.arr()?{next.push(json::position(c)?);}done.insert(key(p));events.push(next);valid+=read as u64;
 }OpenOptions::new().write(true).open(&work).and_then(|f|f.set_len(valid)).map_err(|e|e.to_string())?;}

 let root=canonical(Position::initial());let mut seen=BTreeSet::new();let mut queue=VecDeque::new();seen.insert(key(root));if !done.contains(&key(root)){queue.push_back(root);}
 for next in events {for p in next {if seen.insert(key(p))&&!done.contains(&key(p)){queue.push_back(p);}}}
 // Re-sort recovered frontier by occupancy (pass nodes keep their ply).
 let mut pending:Vec<_>=queue.into_iter().collect();pending.sort_by_key(|p|std::cmp::Reverse(p.empty().count_ones()));let mut queue:VecDeque<_>=pending.into();
 let mut workfile=OpenOptions::new().create(true).append(true).open(&work).map_err(|e|e.to_string())?;
 let stop=PathBuf::from(format!("{}.stop",output.display()));let mut processed=0;let mut searched=0;let mut exact_attempts=0;let mut exact_success=0;
 while !queue.is_empty()&&processed<limit&&(seconds==0||start.elapsed().as_secs()<seconds)&&!stop.exists(){
  // One BFS layer at a time; all outstanding work is drained before checkpoint.
  let empties=queue.front().unwrap().empty().count_ones();let mut parents=vec![];
  while parents.len()<threads&&queue.front().is_some_and(|p|p.empty().count_ones()==empties)&&processed+parents.len()<limit{parents.push(queue.pop_front().unwrap());}
  let mut jobs=BTreeSet::new();
  for p in &parents {let k=key(*p);book.expand(k);for (_,c) in &book.nodes[&k].edges{let n=&book.nodes[c];if !n.evaluated&&!n.exact(){jobs.insert(*c);}}}
  let jobs:Vec<_>=jobs.into_iter().collect();let threshold=exact_empties;
  // Workers own small private TTs. Batches cap CPU at --threads.
  let results=std::thread::scope(|scope|{
   let mut handles=vec![];for chunk in jobs.chunks(jobs.len().div_ceil(threads).max(1)){handles.push(scope.spawn(move||{let mut s=Search::new(18,0.0);chunk.iter().map(|k|{let p=Position{black:k.0,white:k.1,hleft:k.2,vtop:k.3,side:k.4};(*k,evaluate_leaf(&mut s,p,ms,p.empty().count_ones()<=threshold))}).collect::<Vec<_>>() }));}
   handles.into_iter().flat_map(|h|h.join().expect("grow worker")).collect::<Vec<_>>()
  });
  for (k,r) in results {searched+=1;if r.attempted_exact {exact_attempts+=1;if r.lo==r.hi{exact_success+=1;}}book.apply(k,r);}
  // Adaptive terminal threshold stays in [34, requested threshold] after slow solves.
  if exact_attempts>=64 {if exact_success*4<exact_attempts&&exact_empties>34{exact_empties-=2;}exact_attempts=0;exact_success=0;}
  // Local minimax only; global propagation per batch would be quadratic.
  for p in &parents {let k=key(*p);let edges=book.nodes[&k].edges.clone();if !edges.is_empty(){let lo=edges.iter().map(|(_,c)|-book.nodes[c].hi).max().unwrap();let hi=edges.iter().map(|(_,c)|-book.nodes[c].lo).max().unwrap();let value=edges.iter().map(|(_,c)|-book.nodes[c].value).max().unwrap();let n=book.nodes.get_mut(&k).unwrap();n.lo=n.lo.max(lo);n.hi=n.hi.min(hi);if n.lo>n.hi{return Err("Conflicting parent bounds".into())}n.value=value.clamp(n.lo,n.hi);n.dirty=true;}}
  let mut batch=String::new();
  for p in parents {let k=key(p);let node=&book.nodes[&k];let mut next=vec![];
   for (_,c) in &node.edges {let child=&book.nodes[c];let value=if child.exact(){-child.lo}else{-child.value};if value<node.value-margin{continue}
    // Distribute canonical first-placement subtrees; later transpositions may overlap.
    if k==key(root)&&child.p.hash()%partition.1!=partition.0{continue}
    next.push(child.p);if seen.insert(*c){if queue.back().is_none_or(|p|p.empty().count_ones()>=child.p.empty().count_ones()){queue.push_back(child.p);}else{let at=queue.iter().position(|p|p.empty().count_ones()<child.p.empty().count_ones()).unwrap_or(queue.len());queue.insert(at,child.p);}}
   }
   batch.push_str(&format!("{{\"position\":{},\"next\":[{}]}}\n",json::position_json(p),next.iter().map(|p|json::position_json(*p)).collect::<Vec<_>>().join(",")));processed+=1;
  }
  book.checkpoint(&output)?;workfile.write_all(batch.as_bytes()).and_then(|_|workfile.sync_all()).map_err(|e|e.to_string())?;
 }
 let elapsed=start.elapsed().as_secs_f64();let report=format!("{{\"processed\":{processed},\"searched\":{searched},\"newNodes\":{},\"nodes\":{},\"pending\":{},\"elapsed\":{elapsed},\"positionsPerSecond\":{},\"exactEmpties\":{exact_empties}}}\n",book.nodes.len()-initial_nodes,book.nodes.len(),queue.len(),processed as f64/elapsed.max(0.001));
 atomic_write(&PathBuf::from(format!("{}.stats.json",output.display())),&report)?;print!("{report}");Ok(())
}
