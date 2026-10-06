//! Import an independently checked coordinator certificate; never run a solver.
use dosello_ai::{book::*, cli, json};
use std::{path::PathBuf, process::Command};
fn main() { cli::main_result(|| {
    let mut proof=PathBuf::from("proof/initial-proof.json");
    let mut path=PathBuf::from("rust/book/book.jsonl");
    let mut args=std::env::args().skip(1);
    while let Some(a)=args.next() {
        let v=args.next().ok_or("Missing argument")?;
        match a.as_str() {"--proof"=>proof=v.into(),"--book"=>path=v.into(),_=>return Err(format!("Unknown {a}"))}
    }
    let lock=PathBuf::from(format!("{}.lock",path.display()));
    std::fs::create_dir(&lock).map_err(|e|format!("Stop the builder first: {}: {e}",lock.display()))?;
    struct Lock(PathBuf);impl Drop for Lock {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let _lock=Lock(lock.clone());
    std::fs::write(lock.join("pid"),std::process::id().to_string()).map_err(|e|e.to_string())?;
    let binary=std::env::current_exe().map_err(|e|e.to_string())?.with_file_name("job");
    let verifier=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/verify-initial-proof.py");
    let result=Command::new("python3").arg(verifier).arg("--proof").arg(&proof).arg("--binary").arg(binary).status().map_err(|e|e.to_string())?;
    if !result.success(){return Err("Proof verification failed; journal unchanged".into())}
    let v=json::parse(&std::fs::read_to_string(&proof).map_err(|e|e.to_string())?)?;
    let json::Json::Obj(nodes)=v.get("nodes")? else{return Err("Expected proof nodes".into())};
    let mut rows=vec![];
    for n in nodes.values() {rows.push((json::position(n.get("position")?)?,i32::try_from(n.get("lower")?.num()?).map_err(|_|"lower")?,i32::try_from(n.get("upper")?.num()?).map_err(|_|"upper")?));}
    let mut book=Book::load(&path,false)?;
    book.import_bounds(&rows)?;
    let saved=book.checkpoint(&path)?;
    println!("Imported {} proof nodes; saved {saved} records; root [{},{}]",rows.len(),book.nodes[&book.root].lo,book.nodes[&book.root].hi);
    Ok(())
})}
