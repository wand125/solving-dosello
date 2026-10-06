use dosello_ai::{book::*,board::Position,cli,json};
use std::{path::PathBuf,collections::{BTreeMap,BTreeSet}};
fn main(){cli::main_result(||{
    let mut path=PathBuf::from("rust/book/book.jsonl");let mut output=PathBuf::from("exports/opening-book.json");let mut copy=PathBuf::from("docs/play/wasm/opening-book.json");let mut max_ply=12u32;let mut limit=40000usize;
    let mut args=std::env::args().skip(1);while let Some(a)=args.next(){let v=args.next().ok_or("Missing argument")?;match a.as_str(){"--book"=>path=v.into(),"--output"=>output=v.into(),"--copy"=>copy=v.into(),"--max-ply"=>max_ply=v.parse().map_err(|_|"ply")?,"--max-entries"=>limit=v.parse().map_err(|_|"entries")?,_=>return Err(format!("Unknown {a}"))}}
    if limit==0{return Err("--max-entries must be positive".into())}
    let mut book=Book::load(&path,false)?;
    book.expand_coverage(3,2,3);
    let proof=json::parse(&std::fs::read_to_string("proof/initial-proof.json").map_err(|e|e.to_string())?)?;
    let json::Json::Obj(proof_nodes)=proof.get("nodes")? else{return Err("proof nodes".into())};
    let mut proof_bounds=Vec::new();
    for n in proof_nodes.values(){proof_bounds.push((json::position(n.get("position")?)?,n.get("lower")?.num()? as i32,n.get("upper")?.num()? as i32));}
    book.import_bounds(&proof_bounds)?;
    let mut required=BTreeSet::new();
    for n in proof_nodes.values(){let p=json::position(n.get("position")?)?;for t in 0..8{required.insert(key(transform(p,t)));}}
    if limit<required.len(){return Err(format!("--max-entries must cover {} proof positions",required.len()))}
    let mut entries=BTreeMap::new();
    // Retain independently solved historical lines, then overlay the live DAG.
    for legacy in ["rust/book/principal-line.json","rust/book/principal-line-v2.json"] {
        if let Ok(s)=std::fs::read_to_string(legacy){let v=json::parse(&s)?;for e in v.get("entries")?.arr()?{let p=json::position(e.get("position")?)?;let analysis=e.get("analysis")?;
            // The std JSON AST has no serializer; extract the analysis through a
            // small generic serializer rather than copying source substrings.
            entries.insert(key(p),(json::position_json(p),serialize(analysis)));}}
    }
    let mut nodes:Vec<_>=book.nodes.values().filter(|n|n.expanded&&(required.contains(&key(n.p))||(56-n.p.empty().count_ones())/2<=max_ply)).collect();
    nodes.sort_by_key(|n|(!required.contains(&key(n.p)),(56-n.p.empty().count_ones())/2,std::cmp::Reverse(n.visits),key(n.p)));
    let mut emitted=0;
    for n in nodes {for t in 0..8{let p=transform(n.p,t);if emitted>=limit{break}let a=book.analysis_compact(p);
        // Do not replace a fully exact historical entry with incomplete values.
        let old_exact=entries.get(&key(p)).is_some_and(|(_,s)|json::parse(s).ok().and_then(|v|v.get("complete").ok().and_then(|b|b.boolean().ok())).unwrap_or(false));
        if !old_exact || required.contains(&key(p)) {entries.insert(key(p),(json::position_json(p),a));}emitted+=1;}if emitted>=limit{break}}
    let root=key(Position::initial());let mut entries:Vec<_>=entries.into_iter().collect();entries.sort_by_cached_key(|(k,(_,a))|{let exact=json::parse(a).ok().and_then(|v|v.get("complete").ok().and_then(|x|x.boolean().ok())).unwrap_or(false);(*k!=root,!required.contains(k),!exact,std::cmp::Reverse((k.0|k.1).count_zeros()),*k)});entries.truncate(limit);
    let rows=entries.iter().take(1024).map(|(k,(p,a))|format!("{{\"position\":{p},\"analysis\":{a}{}}}",if *k==root{",\"sequence\":\"\""}else{""})).collect::<Vec<_>>();
    let text=format!("{{\"version\":1,\"perspective\":\"side-to-move\",\"generatedAt\":{},\"entries\":[{}]}}\n",stamp(),rows.join(","));
    // DSBOOK03: count u32 LE; board 4*u64, side i8, count u8, position lower/upper/value i8;
    // each move a,b,value(i8),lower(i8),upper(i8),depth,exact flag (7 bytes).
    let mut binary=b"DSBOOK03".to_vec();binary.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (k,(_,a)) in &entries {
        for bits in [k.0,k.1,k.2,k.3] {binary.extend_from_slice(&bits.to_le_bytes());}
        binary.push(k.4 as u8);let a=json::parse(a)?;let moves=a.get("moves")?.arr()?;binary.push(moves.len() as u8);
        let lo=a.get("lower").ok().and_then(|x|x.num().ok()).unwrap_or_else(||moves.iter().map(|m|m.get("lower").ok().and_then(|x|x.num().ok()).unwrap_or(-64)).max().unwrap_or(-64));
        let hi=a.get("upper").ok().and_then(|x|x.num().ok()).unwrap_or_else(||moves.iter().map(|m|m.get("upper").ok().and_then(|x|x.num().ok()).unwrap_or(64)).max().unwrap_or(64));
        let value=a.get("value").ok().and_then(|x|x.num().ok()).unwrap_or(lo).clamp(lo,hi);
        binary.extend_from_slice(&[lo as i8 as u8,hi as i8 as u8,value as i8 as u8]);
        for m in moves {let name=m.get("move")?.str()?.as_bytes();let cell=|i:usize|(name[i+1]-b'1')*8+name[i]-b'a';
            let v=m.get("value")?.num()? as i8;let exact=m.get("exact")?.boolean()?;
            let lo=m.get("lower").ok().and_then(|v|v.num().ok()).unwrap_or(if exact{v as i64}else{-64});
            let hi=m.get("upper").ok().and_then(|v|v.num().ok()).unwrap_or(if exact{v as i64}else{64});
            binary.extend_from_slice(&[cell(0),cell(3),v as u8,lo as i8 as u8,hi as i8 as u8,m.get("depth")?.num()? as u8,u8::from(exact)]);
        }
    }
    if binary.len()>10_000_000{return Err("Binary book exceeds 10 MB; lower --max-entries".into())}
    for path in [&output,&copy] {if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;}}
    for path in [output.with_extension("bin"),copy.with_extension("bin")] {let tmp=path.with_extension("bin.tmp");std::fs::write(&tmp,&binary).map_err(|e|e.to_string())?;std::fs::rename(tmp,path).map_err(|e|e.to_string())?;}
    println!("Binary book: {} positions, {} bytes",entries.len(),binary.len());
    atomic_write(&output,&text)?;atomic_write(&copy,&text)?;println!("Exported {} positions, {} bytes to {} and {}",rows.len(),text.len(),output.display(),copy.display());Ok(())
})}
fn serialize(v:&json::Json)->String{use json::Json::*;match v{Null=>"null".into(),Bool(b)=>b.to_string(),Num(n)=>n.to_string(),Real(n)=>n.to_string(),Str(s)=>json::quote(s),Arr(a)=>format!("[{}]",a.iter().map(serialize).collect::<Vec<_>>().join(",")),Obj(o)=>format!("{{{}}}",o.iter().map(|(k,v)|format!("{}:{}",json::quote(k),serialize(v))).collect::<Vec<_>>().join(","))}}
