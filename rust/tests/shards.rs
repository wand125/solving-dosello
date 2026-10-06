use dosello_ai::{book::*,board::Position,shards};
use std::{path::PathBuf,process::Command};
fn temp(name:&str)->PathBuf{let p=std::env::temp_dir().join(format!("dosello-shards-{name}-{}",std::process::id()));let _=std::fs::remove_dir_all(&p);std::fs::create_dir_all(&p).unwrap();p}
#[test]
fn deterministic_export_and_partial_tail(){
 let dir=temp("roundtrip");let journal=dir.join("book.jsonl");let mut b=Book::new(Position::initial());b.expand_coverage(2,2,3);
 let mut samples=vec![];let mut seed=19427;for _ in 0..80{let p=dosello_ai::board::random_position(&mut seed,30);let mut random=Book::new(p);random.expand(random.root);b.nodes.extend(random.nodes);samples.push(p);}
 b.checkpoint(&journal).unwrap();
 use std::io::Write;std::fs::OpenOptions::new().append(true).open(&journal).unwrap().write_all(b"{unfinished").unwrap();
 let loaded=Book::load(&journal,false).unwrap();assert_eq!(loaded.nodes.len(),b.nodes.len());assert!(std::fs::read(&journal).unwrap().ends_with(b"{unfinished"));
 let a=shards::export(&loaded,&dir.join("a"),0,0).unwrap();let c=shards::export(&loaded,&dir.join("b"),0,0).unwrap();assert_eq!(a,c);
 std::fs::write(dir.join("a/samples.json"),format!("[{}]",samples.iter().map(|p|format!("{{\"position\":{},\"analysis\":{}}}",dosello_ai::json::position_json(*p),loaded.analysis_compact(*p))).collect::<Vec<_>>().join(","))).unwrap();
 let status=Command::new("node").arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/validate-shards.mjs")).arg(dir.join("a")).arg("--samples-only").status().unwrap();assert!(status.success());
 std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn resume_and_partition_subtrees(){
 let dir=temp("resume");let proof=dir.join("proof.json");std::fs::write(&proof,"{\"nodes\":{}}").unwrap();let missing=dir.join("missing.jsonl");
 let run=|name:&str,partition:&str|{let output=dir.join(format!("{name}.jsonl"));assert!(Command::new(env!("CARGO_BIN_EXE_book_grow")).args(["--output",output.to_str().unwrap(),"--book",missing.to_str().unwrap(),"--proof",proof.to_str().unwrap(),"--threads","1","--time-ms","0","--margin","128","--max-positions","1","--partition",partition]).status().unwrap().success());output};
 let p=run("all","0/1");let first=std::fs::read_to_string(format!("{}.work.jsonl",p.display())).unwrap();run("all","0/1");let resumed=std::fs::read_to_string(format!("{}.work.jsonl",p.display())).unwrap();assert_eq!(resumed.lines().count(),2);assert!(resumed.starts_with(&first));
 let children=|s:&str|{let v=dosello_ai::json::parse(s.lines().next().unwrap()).unwrap();v.get("next").unwrap().arr().unwrap().iter().map(|x|key(dosello_ai::json::position(x).unwrap())).collect::<std::collections::BTreeSet<_>>()};
 let mut union=std::collections::BTreeSet::new();for i in 0..2{let p=run(&format!("part{i}"),&format!("{i}/2"));union.extend(children(&std::fs::read_to_string(format!("{}.work.jsonl",p.display())).unwrap()));}
 assert_eq!(union,children(&first));
 let merged=shards::merged(&[dir.join("part0.jsonl").to_string_lossy().into(),dir.join("part1.jsonl").to_string_lossy().into()],&proof).unwrap();let all=Book::load(&dir.join("all.jsonl"),false).unwrap();for k in union{assert_eq!(merged.nodes[&k].lo,all.nodes[&k].lo);}
 std::fs::remove_dir_all(dir).unwrap();
}
