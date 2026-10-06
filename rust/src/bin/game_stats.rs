//! Reproducible uniform-random playout statistics; no search evaluation.
use dosello_ai::board::Position;
use std::collections::HashMap;
type Key=(u64,u64,u64,u64,i8);
fn key(p:Position)->Key {(p.black,p.white,p.hleft,p.vtop,p.side)}
fn main(){
 let games:usize=std::env::args().nth(1).unwrap_or("100000".into()).parse().unwrap();
 let depth:usize=std::env::args().nth(2).unwrap_or("5".into()).parse().unwrap();
 assert!(games>0 && (1..=6).contains(&depth));
 let mut seed=0x73ac4291u64;let mut counts=[0u64;29];let mut sums=[0u64;29];let mut lengths=[0u64;29];
 for _ in 0..games {let mut p=Position::initial();let mut ply=0;
 loop {let ms=p.moves();if ms.is_empty(){p=p.pass();if p.moves().is_empty(){lengths[ply]+=1;break}continue}
 counts[ply]+=1;sums[ply]+=ms.len() as u64;
 seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;p=p.play(ms[(seed % ms.len() as u64) as usize]);ply+=1;
 }}
 let mean=lengths.iter().enumerate().map(|(i,n)|i as f64 * *n as f64).sum::<f64>()/games as f64;
 let branch:Vec<f64>=(0..28).map(|i|if counts[i]>0{sums[i] as f64/counts[i] as f64}else{0.}).collect();
 let mut frontier=HashMap::from([(key(Position::initial()),Position::initial())]);let mut unique=vec![1usize];let mut records=vec![1u64];
 let mut paths=HashMap::from([(key(Position::initial()),1u64)]);
 for _ in 0..depth {let mut next=HashMap::new();let mut npaths=HashMap::new();
 for (k,p0) in &frontier {let mut p=*p0;if p.moves().is_empty(){p=p.pass();}for m in p.moves(){let q=p.play(m);let kq=key(q);next.insert(kq,q);*npaths.entry(kq).or_insert(0u64)+=paths[k];}}
 unique.push(next.len());records.push(npaths.values().sum());frontier=next;paths=npaths;}
 let full_log=branch.iter().filter(|b|**b>0.).map(|b|b.log10()).sum::<f64>();
 let n=mean.floor() as usize;let typical_log=branch[..n].iter().map(|b|b.log10()).sum::<f64>()+(mean-n as f64)*branch[n].log10();
 // Illustrative extrapolation only: freeze the last measured unique-state growth ratio.
 let growth=unique[depth] as f64/unique[depth-1] as f64;
 let position_log=(unique[depth] as f64).log10()+(mean-depth as f64)*growth.log10();
 let collision_log=typical_log + mean/depth as f64*(unique[depth] as f64/records[depth] as f64).log10();
 println!("{{\"games\":{games},\"seed\":1940669073,\"meanPlacements\":{mean},\"branchingByPlacement\":{branch:?},\"lengthHistogram\":{lengths:?},\"exhaustiveDepth\":{depth},\"distinctByDepth\":{unique:?},\"recordsByDepth\":{records:?},\"full28BranchProductLog10\":{full_log},\"meanLengthBranchProductLog10\":{typical_log},\"naivePositionExtrapolationLog10\":{position_log},\"collisionAdjustedPositionProxyLog10\":{collision_log}}}");
}
