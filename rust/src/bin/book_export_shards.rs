use dosello_ai::{cli,shards};
use std::path::PathBuf;
fn main(){cli::main_result(||{
 let mut records=None;let mut base=None;let mut paths=vec![];let mut trees=vec![];let mut proof=PathBuf::from("proof/initial-proof.json");let mut out=PathBuf::from("exports/book/staging");let mut depth=0u8;let mut bytes=0usize;
 let mut args=std::env::args().skip(1);while let Some(a)=args.next(){let v=args.next().ok_or("Missing value")?;match a.as_str(){"--book"=>paths.push(v),"--tree"=>trees.push(v),"--tree-records"=>records=Some(v),"--base-shards"=>base=Some(v),"--proof"=>proof=v.into(),"--output"=>out=v.into(),"--min-depth"=>depth=v.parse().map_err(|_|"depth")?,"--max-bytes"=>bytes=v.parse().map_err(|_|"bytes")?,_=>return Err(format!("Unknown {a}"))}}
 if paths.is_empty(){paths.push("rust/book/book.jsonl".into());if std::path::Path::new("rust/book/grow.jsonl").exists(){paths.push("rust/book/grow.jsonl".into());}}
 if out.exists(){return Err("Output must be a new directory (immutable builds)".into())}
 if let Some(records)=records {print!("{}",dosello_ai::tree_export::export(std::path::Path::new(&records),base.as_deref().map(std::path::Path::new),&out,bytes)?);return Ok(())}
 let mut book=shards::merged(&paths,&proof)?;for path in trees {book.import_tree(std::path::Path::new(&path))?;}print!("{}",shards::export(&book,&out,depth,bytes)?);Ok(())
})}
