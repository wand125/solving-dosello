//! DSSHRD01: sorted hash + full canonical key, offset table, compact move intervals.
use crate::{board::*,book::*,json};
use std::{path::Path,collections::BTreeMap};
pub fn merged(paths:&[String],proof:&Path)->Result<Book,String>{
    let mut book=Book::new(Position::initial());
    for path in paths {
        eprintln!("Loading {path}");let other=Book::load(Path::new(path),false)?;eprintln!("Loaded {} canonical nodes",other.nodes.len());
        for (k,n) in other.nodes {if let Some(old)=book.nodes.get_mut(&k){
            let lo=old.lo.max(n.lo);let hi=old.hi.min(n.hi);if lo>hi{return Err("Conflicting journal bounds".into())}
            let expanded=old.expanded||n.expanded;let edges=if old.expanded{old.edges.clone()}else{n.edges.clone()};
            if n.depth>old.depth || n.exact(){*old=n;}
            old.lo=lo;old.hi=hi;old.value=old.value.clamp(lo,hi);old.expanded=expanded;old.edges=edges;
        }else{book.nodes.insert(k,n);}}
    }
    let v=json::parse(&std::fs::read_to_string(proof).map_err(|e|e.to_string())?)?;
    let json::Json::Obj(nodes)=v.get("nodes")? else{return Err("proof nodes".into())};
    let mut rows=vec![];for n in nodes.values(){rows.push((json::position(n.get("position")?)?,n.get("lower")?.num()? as i32,n.get("upper")?.num()? as i32));}
    book.import_bounds(&rows)?;Ok(book)
}
pub fn record(book:&Book,p:Position,min_depth:u8)->Option<Vec<u8>>{
    let n=&book.nodes[&key(p)];
    let ms=p.moves();let mut entries=vec![];let mut good=n.lo> -64||n.hi<64;let mut best=(i32::MIN,65535u16);
    for m in ms {
        let c=book.nodes.get(&key(canonical(p.play(m))));
        let (v,lo,hi,d)=c.map_or((-crate::search::evaluate(p.play(m),crate::search::Weights::default()),-64,64,0),|c|(-c.value,-c.hi,-c.lo,c.depth.saturating_add(1)));
        let keep=lo> -64||hi<64||d>=min_depth;
        good|=keep;
        // Preserve every legal move: filtered estimates become unknown depth zero.
        let (v,d)=if keep{(reported(v,lo,hi),d)}else{(0,0)};
        if v>best.0{best=(v,m.id());}
        let kind=if lo==hi{0}else if lo> -64&&hi<64{3}else if lo> -64{1}else if hi<64{2}else{4};
        let (a,b)=match kind{0|1|3=>(lo,hi),2=>(hi,hi),_=>(v,v)};
        entries.extend_from_slice(&[m.a|if m.b-m.a==8{64}else{0},a as i8 as u8,b as i8 as u8,(d.min(31)<<3)|kind]);
    }
    if !good{return None}
    // Follow the same certified tie as the solution tree, in canonical orientation.
    if let Some(name)=&n.best {if let Some(m)=p.moves().into_iter().find(|m|m.to_string()==*name){
        if book.nodes.get(&key(canonical(p.play(m)))).is_some_and(|c|n.exact()&&c.exact()&&-c.lo==n.lo){best.1=m.id();}
    }}
    let mut out=p.hash().to_le_bytes().to_vec();for b in [p.black,p.white,p.hleft,p.vtop]{out.extend_from_slice(&b.to_le_bytes());}
    out.extend_from_slice(&[p.side as u8,(entries.len()/4) as u8,n.lo as i8 as u8,n.hi as i8 as u8,reported(n.value,n.lo,n.hi) as i8 as u8]);
    out.extend_from_slice(&best.1.to_le_bytes());out.extend(entries);Some(out)
}
pub fn export(book:&Book,out:&Path,min_depth:u8,max_bytes:usize)->Result<String,String>{
    std::fs::create_dir_all(out).map_err(|e|e.to_string())?;
    let mut nodes:Vec<_>=book.nodes.values().collect();nodes.sort_by_key(|n|(n.lo== -64&&n.hi==64,n.p.empty().count_ones().wrapping_neg(),key(n.p)));
    let mut shards:BTreeMap<u16,Vec<(u64,Key,Vec<u8>)>>=BTreeMap::new();let mut size=0usize;let mut count=0;let mut kinds=[0usize;3];let mut samples=vec![];
    for n in nodes {if max_bytes>0&&size+51>max_bytes{break}if let Some(r)=record(book,n.p,min_depth){
        let shard=(n.p.hash()>>48) as u16;let cost=r.len()+4+if shards.contains_key(&shard){0}else{16};
        if max_bytes>0 && size+cost>max_bytes {continue}
        size+=cost;count+=1;
        for m in r[47..].chunks_exact(4){let kind=m[3]&7;kinds[if kind==0{0}else if kind<4{1}else{2}]+=1;}
        if samples.len()<16 || count%10007==0 {samples.push(format!("{{\"position\":{},\"analysis\":{}}}",json::position_json(n.p),book.analysis_compact(n.p)));}
        shards.entry(shard).or_default().push((n.p.hash(),key(n.p),r));
    }}
    let mut distribution=vec![];let mut digest=0xcbf29ce484222325u64;
    for (s,mut rows) in shards {rows.sort_by_key(|r|(r.0,r.1));let mut bytes=b"DSSHRD01".to_vec();bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
        let mut offset=16+rows.len()*4;for (_,_,r) in &rows {bytes.extend_from_slice(&(offset as u32).to_le_bytes());offset+=r.len();}bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        for (_,_,r) in rows {bytes.extend(r);}for b in &bytes{digest=(digest^*b as u64).wrapping_mul(0x100000001b3);}
        let dir=out.join(format!("{:02x}",s>>8));std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;std::fs::write(dir.join(format!("{:02x}.bin",s&255)),&bytes).map_err(|e|e.to_string())?;distribution.push(bytes.len());
    }
    distribution.sort();let pct=|p:usize|distribution.get(distribution.len().saturating_sub(1)*p/100).copied().unwrap_or(0);
    let version=format!("v{digest:016x}");
    let index=format!("{{\"version\":1,\"build\":\"{version}\",\"path\":\"{version}/\",\"shardCount\":65536,\"hash\":\"dosello-position-xor-rotate64-v1\",\"canonical\":\"lexicographic-u64-black-white-hleft-vtop-side-D4\",\"records\":{count},\"bytes\":{size},\"bytesPerRecord\":{},\"exact\":{},\"bound\":{},\"estimate\":{},\"minDepth\":{min_depth},\"shards\":{{\"nonempty\":{},\"min\":{},\"p50\":{},\"p95\":{},\"max\":{}}},\"buildInfo\":\"std-only exporter; counts are move entries; deterministic content version\"}}\n",size as f64/count.max(1) as f64,kinds[0],kinds[1],kinds[2],distribution.len(),pct(0),pct(50),pct(95),pct(100));
    std::fs::write(out.join("samples.json"),format!("[{}]",samples.join(","))).map_err(|e|e.to_string())?;
    std::fs::write(out.join("index.json"),&index).map_err(|e|e.to_string())?;Ok(index)
}
