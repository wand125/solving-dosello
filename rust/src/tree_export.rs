//! Bounded-memory tree publication: partition/spill, merge, external priority
//! selection, then one hash-prefix partition at a time. No whole-tree Book.
use crate::{board::*,book::{self,Book},json,shards,tree};
use std::{collections::{BTreeMap,BinaryHeap},fs::{self,File},io::{BufRead,BufReader,BufWriter,Read,Write},path::Path};
type Result<T> = std::result::Result<T,String>;
fn io<T>(r:std::io::Result<T>)->Result<T>{r.map_err(|e|e.to_string())}
fn key(r:&[u8])->Vec<u8>{r[8..41].to_vec()}
fn hash(r:&[u8])->u64{u64::from_le_bytes(r[..8].try_into().unwrap())}
fn pos(r:&[u8])->Position{let b=|i|u64::from_le_bytes(r[i..i+8].try_into().unwrap());Position{black:b(8),white:b(16),hleft:b(24),vtop:b(32),side:r[40] as i8}}
fn rank(r:&[u8])->(bool,std::cmp::Reverse<u32>,book::Key){(r[42]==192&&r[43]==64,std::cmp::Reverse(pos(r).empty().count_ones()),book::key(pos(r)))}
fn write_record(w:&mut impl Write,r:&[u8])->Result<()>{io(w.write_all(&(r.len() as u32).to_le_bytes()))?;io(w.write_all(r))}
fn read_record(r:&mut impl Read)->Result<Option<Vec<u8>>>{let mut l=[0;4];let n=io(r.read(&mut l[..1]))?;if n==0{return Ok(None)}io(r.read_exact(&mut l[1..]))?;let n=u32::from_le_bytes(l) as usize;if !(47..=559).contains(&n){return Err("spill record length".into())}let mut data=vec![0;n];io(r.read_exact(&mut data))?;Ok(Some(data))}
fn interval(m:&[u8])->(i32,i32,i32,u8){let x=m[1] as i8 as i32;let y=m[2] as i8 as i32;match m[3]&7{0=>(x,x,x,m[3]>>3),1=>(x,64,x,m[3]>>3),2=>(-64,x,x,m[3]>>3),3=>(x,y,x,m[3]>>3),_=>(-64,64,x,m[3]>>3)}}
fn encode(m:&mut[u8],lo:i32,hi:i32,v:i32,d:u8){let (kind,x,y)=if lo==hi{(0,lo,hi)}else if lo> -64&&hi<64{(3,lo,hi)}else if lo> -64{(1,lo,64)}else if hi<64{(2,hi,hi)}else{(4,v,v)};m[1]=x as i8 as u8;m[2]=y as i8 as u8;m[3]=(d<<3)|kind;}
fn merge(a:&mut Vec<u8>,b:Vec<u8>)->Result<()>{
    if key(a)!=key(&b)||a.len()!=b.len(){return Err("record identity/legality".into())}
    let lo=(a[42] as i8).max(b[42] as i8);let hi=(a[43] as i8).min(b[43] as i8);if lo>hi{return Err("record node contradiction".into())}
    a[42]=lo as u8;a[43]=hi as u8;a[44]=book::reported(a[44] as i8 as i32,lo as i32,hi as i32) as i8 as u8;
    for (m,n) in a[47..].chunks_exact_mut(4).zip(b[47..].chunks_exact(4)){
        if m[0]!=n[0]{return Err("record moves mismatch".into())}
        let (l,h,v,d)=interval(m);let (ll,hh,vv,dd)=interval(n);let l=l.max(ll);let h=h.min(hh);
        if l>h{return Err("record move contradiction".into())}encode(m,l,h,if dd>d{vv}else{v},d.max(dd));
    }
    // A certified tree tie has precedence over the older book's choice.
    let best=u16::from_le_bytes([b[45],b[46]]);
    if lo==hi && best!=65535 {
        let anchor=(best/64) as u8;let vertical=best%64-best/64==8;
        if b[47..].chunks_exact(4).any(|m|m[0]==(anchor|if vertical{64}else{0}) && m[3]&7==0 && m[1] as i8==lo){a[45..47].copy_from_slice(&b[45..47]);}
    }Ok(())
}
fn tree_record(v:&json::Json)->Result<Vec<u8>>{
    if v.get("treeVersion")?.num()?!=1{return Err("tree version".into())}
    let p=tree::position(v.get("key")?.str()?)?;let mut b=Book::new(p);
    let apply=|b:&mut Book,v:&json::Json|->Result<()> {let p=tree::position(v.get("key")?.str()?)?;let lo=v.get("lower")?.num()?;let hi=v.get("upper")?.num()?;if lo< -64||hi>64||lo>hi{return Err("tree interval".into())}let k=book::key(p);if !b.nodes.contains_key(&k){b.nodes.extend(Book::new(p).nodes)}let n=b.nodes.get_mut(&k).unwrap();n.lo=lo as i32;n.hi=hi as i32;n.value=book::reported(0,n.lo,n.hi);if n.exact(){n.depth=(p.empty().count_ones()/2) as u8;}Ok(())};
    apply(&mut b,v)?;
    let expected=tree::groups(p);let cs=v.get("children")?.arr()?;
    if cs.len()!=expected.len(){return Err("tree children length".into())}
    for (c,(q,_)) in cs.iter().zip(&expected){if c.get("key")?.str()?!=tree::key(*q){return Err("tree child key".into())}apply(&mut b,c)?;}
    if let Ok(name)=v.get("bestMove")?.str(){let n=b.nodes.get_mut(&book::key(p)).unwrap();n.best=Some(name.into());}
    shards::record(&b,p,0).ok_or("missing tree record".into())
}
fn sample(r:&[u8])->String{
    let p=pos(r);let ms=p.moves();let mut moves=vec![];
    for (m,x) in ms.iter().zip(r[47..].chunks_exact(4)){let(lo,hi,v,d)=interval(x);moves.push(format!("{{\"move\":{},\"cells\":[{},{}],\"value\":{},\"lower\":{lo},\"upper\":{hi},\"exact\":{},\"depth\":{d},\"bound\":{}}}",json::quote(&m.to_string()),m.a,m.b,v,lo==hi,json::quote(if lo==hi{"exact"}else if lo> -64{"lower"}else if hi<64{"upper"}else{"heuristic"})));}
    format!("{{\"position\":{},\"analysis\":{{\"moves\":[{}]}}}}",json::position_json(p),moves.join(","))
}
pub fn export(input:&Path,base:Option<&Path>,out:&Path,max_bytes:usize)->Result<String>{
    io(fs::create_dir_all(out))?;let scratch=out.join(".spill");io(fs::create_dir(&scratch))?;
    let mut writers=Vec::new();for i in 0..256{writers.push(BufWriter::new(io(File::create(scratch.join(format!("in{i}"))))?));}
    let mut tree_sample_candidates=BTreeMap::new();
    let mut reader=BufReader::new(io(File::open(input))?);let mut l=String::new();
    loop{l.clear();if io(reader.read_line(&mut l))?==0{break}if !l.ends_with('\n'){return Err("incomplete tree snapshot".into())}let r=tree_record(&json::parse(&l)?)?;
        tree_sample_candidates.insert(rank(&r),key(&r));if tree_sample_candidates.len()>32{tree_sample_candidates.pop_last();}write_record(&mut writers[(hash(&r)>>56) as usize],&r)?;}
    for w in &mut writers{io(w.flush())?}drop(writers);
    let tree_keys:std::collections::BTreeSet<_>=tree_sample_candidates.into_values().collect();
    // Merge only one 1/256 hash partition at a time; base is an immutable build.
    for i in 0..256{
        let mut records:BTreeMap<Vec<u8>,Vec<u8>>=BTreeMap::new();
        if let Some(base)=base{for j in 0..256{let file=base.join(format!("{i:02x}/{j:02x}.bin"));if !file.exists(){continue}let bytes=io(fs::read(file))?;
            if bytes.len()<16||&bytes[..8]!=b"DSSHRD01"{return Err("base shard magic".into())}
            let count=u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
            if 16+count*4>bytes.len(){return Err("base offsets".into())}
            for n in 0..count {let a=u32::from_le_bytes(bytes[12+n*4..16+n*4].try_into().unwrap()) as usize;let z=u32::from_le_bytes(bytes[16+n*4..20+n*4].try_into().unwrap()) as usize;
                let r=bytes.get(a..z).ok_or("base record bounds")?.to_vec();if r.len()<47||r.len()!=47+4*r[41] as usize{return Err("base record size".into())}records.insert(key(&r),r);
            }
        }}
        let mut r=BufReader::new(io(File::open(scratch.join(format!("in{i}"))))?);
        while let Some(b)=read_record(&mut r)?{let k=key(&b);if let Some(a)=records.get_mut(&k){merge(a,b)?}else{records.insert(k,b);}}
        io(fs::remove_file(scratch.join(format!("in{i}"))))?;
        let mut rows:Vec<_>=records.into_values().collect();rows.sort_by_cached_key(|r|rank(r));
        let mut w=BufWriter::new(io(File::create(scratch.join(format!("sorted{i}"))))?);for r in rows{write_record(&mut w,&r)?;}io(w.flush())?;
    }
    let mut readers=Vec::new();let mut writers=Vec::new();let mut heap=BinaryHeap::new();
    for i in 0..256{let mut r=BufReader::new(io(File::open(scratch.join(format!("sorted{i}"))))?);if let Some(row)=read_record(&mut r)?{heap.push(std::cmp::Reverse((rank(&row),i,row)));}readers.push(r);writers.push(BufWriter::new(io(File::create(scratch.join(format!("selected{i}"))))?));}
    let mut seen=vec![false;65536];let mut size=0usize;let mut count=0usize;let mut kinds=[0usize;3];let mut samples=vec![];let mut tree_samples=0;
    while let Some(std::cmp::Reverse((_,i,r)))=heap.pop(){let shard=(hash(&r)>>48) as usize;let cost=r.len()+4+if seen[shard]{0}else{16};
        if max_bytes==0||size+cost<=max_bytes{
            size+=cost;count+=1;seen[shard]=true;for m in r[47..].chunks_exact(4){kinds[if m[3]&7==0{0}else if m[3]&7<4{1}else{2}]+=1;}
            let tree_sample=tree_keys.contains(&key(&r));if tree_sample{tree_samples+=1}
            if samples.len()<16||count%10007==0||tree_sample{samples.push(sample(&r));}
            write_record(&mut writers[i],&r)?;
        }
        if let Some(r)=read_record(&mut readers[i])?{heap.push(std::cmp::Reverse((rank(&r),i,r)));}
    }
    for w in &mut writers{io(w.flush())?}drop(writers);drop(readers);
    let mut distribution=vec![];let mut digest=0xcbf29ce484222325u64;
    for i in 0..256{let mut r=BufReader::new(io(File::open(scratch.join(format!("selected{i}"))))?);let mut rows=vec![];while let Some(r)=read_record(&mut r)?{rows.push(r)}rows.sort_by_key(|r|(hash(r),book::key(pos(r))));
        let mut at=0;while at<rows.len(){let shard=(hash(&rows[at])>>48) as u16;let mut end=at+1;while end<rows.len()&&(hash(&rows[end])>>48) as u16==shard{end+=1}
            let subset=&rows[at..end];let mut bytes=b"DSSHRD01".to_vec();bytes.extend_from_slice(&(subset.len() as u32).to_le_bytes());let mut offset=16+subset.len()*4;for r in subset{bytes.extend_from_slice(&(offset as u32).to_le_bytes());offset+=r.len();}bytes.extend_from_slice(&(offset as u32).to_le_bytes());for r in subset{bytes.extend_from_slice(r)}
            for b in &bytes{digest=(digest^*b as u64).wrapping_mul(0x100000001b3)}let dir=out.join(format!("{:02x}",shard>>8));io(fs::create_dir_all(&dir))?;io(fs::write(dir.join(format!("{:02x}.bin",shard&255)),&bytes))?;distribution.push(bytes.len());at=end;
        }
    }
    io(fs::remove_dir_all(scratch))?;distribution.sort();let pct=|p:usize|distribution.get(distribution.len().saturating_sub(1)*p/100).copied().unwrap_or(0);let build=format!("v{digest:016x}");
    let index=format!("{{\"version\":1,\"build\":\"{build}\",\"path\":\"{build}/\",\"shardCount\":65536,\"hash\":\"dosello-position-xor-rotate64-v1\",\"records\":{count},\"bytes\":{size},\"bytesPerRecord\":{},\"exact\":{},\"bound\":{},\"estimate\":{},\"minDepth\":0,\"treeSamples\":{tree_samples},\"shards\":{{\"nonempty\":{},\"min\":{},\"p50\":{},\"p95\":{},\"max\":{}}},\"buildInfo\":\"bounded-memory tree merge\"}}\n",size as f64/count.max(1) as f64,kinds[0],kinds[1],kinds[2],distribution.len(),pct(0),pct(50),pct(95),pct(100));
    io(fs::write(out.join("samples.json"),format!("[{}]",samples.join(","))))?;io(fs::write(out.join("index.json"),&index))?;Ok(index)
}
