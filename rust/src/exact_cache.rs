//! Restartable exact-score cache. Full positions are keys; only terminal exact
//! results enter it. Fixed records carry a checksum; incomplete tails are repaired.
use crate::{board::Position,book::Key};
use std::{collections::BTreeMap,fs::OpenOptions,io::{Read,Write},path::{Path,PathBuf},sync::{Mutex,atomic::{AtomicU64,Ordering}}};
struct Data {values:BTreeMap<Key,i32>,pending:Vec<(Position,i32)>}
pub struct ExactCache {data:Mutex<Data>,path:PathBuf,pub hits:AtomicU64,pub probes:AtomicU64,pub min_empties:u32}
fn checksum(bytes:&[u8])->u64 {bytes.iter().fold(0xcbf29ce484222325,|h,b|(h^*b as u64).wrapping_mul(0x100000001b3))}
impl ExactCache {
    pub fn load(path:&Path,min_empties:u32)->Result<Self,String> {
        let mut bytes=vec![];
        if path.exists() {std::fs::File::open(path).and_then(|mut f|f.read_to_end(&mut bytes)).map_err(|e|e.to_string())?;}
        let mut values=BTreeMap::new();
        if !bytes.is_empty() {
            if bytes.len()<8||&bytes[..8]!=b"DSEXACT1" {return Err("Invalid exact cache header".into())}
            let end=8+(bytes.len()-8)/48*48;
            for row in bytes[8..end].chunks_exact(48) {
                let word=|i|u64::from_le_bytes(row[i..i+8].try_into().unwrap());
                if checksum(&row[..40])!=word(40) {return Err("Exact cache checksum mismatch".into())}
                let p=Position{black:word(0),white:word(8),hleft:word(16),vtop:word(24),side:row[32] as i8}.validate()?;
                let value=row[33] as i8 as i32;
                if !(-64..=64).contains(&value)||value&1!=0 {return Err("Invalid exact cache score".into())}
                if values.insert(crate::book::key(p),value).is_some_and(|old|old!=value) {return Err("Conflicting exact cache scores".into())}
            }
            if end!=bytes.len() {OpenOptions::new().write(true).open(path).and_then(|f|f.set_len(end as u64)).map_err(|e|e.to_string())?;}
        }
        Ok(Self{data:Mutex::new(Data{values,pending:vec![]}),path:path.into(),hits:0.into(),probes:0.into(),min_empties})
    }
    pub fn get(&self,p:Position)->Option<i32> {
        self.probes.fetch_add(1,Ordering::Relaxed);
        let value=self.data.lock().unwrap().values.get(&crate::book::key(p)).copied();
        if value.is_some() {self.hits.fetch_add(1,Ordering::Relaxed);}
        value
    }
    pub fn insert(&self,p:Position,value:i32) {
        if p.empty().count_ones()<self.min_empties {return}
        let mut data=self.data.lock().unwrap();
        if let Some(old)=data.values.get(&crate::book::key(p)) {assert_eq!(*old,value,"Conflicting proven scores");return}
        data.values.insert(crate::book::key(p),value);data.pending.push((p,value));
    }
    pub fn len(&self)->usize {self.data.lock().unwrap().values.len()}
    pub fn checkpoint(&self)->Result<usize,String> {
        let mut data=self.data.lock().unwrap();if data.pending.is_empty() {return Ok(0)}
        let mut f=OpenOptions::new().create(true).append(true).open(&self.path).map_err(|e|e.to_string())?;
        if f.metadata().map_err(|e|e.to_string())?.len()==0 {f.write_all(b"DSEXACT1").map_err(|e|e.to_string())?;}
        for (p,value) in &data.pending {
            let mut row=[0u8;48];
            for (i,x) in [p.black,p.white,p.hleft,p.vtop].iter().enumerate() {row[i*8..i*8+8].copy_from_slice(&x.to_le_bytes());}
            row[32]=p.side as u8;row[33]=*value as i8 as u8;
            let check=checksum(&row[..40]);row[40..].copy_from_slice(&check.to_le_bytes());
            f.write_all(&row).map_err(|e|e.to_string())?;
        }
        f.sync_all().map_err(|e|e.to_string())?;let n=data.pending.len();data.pending.clear();Ok(n)
    }
}
