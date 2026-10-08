#[cfg(not(target_arch = "wasm32"))]
pub mod exact_cache;
#[cfg(not(target_arch = "wasm32"))]
pub mod book;
pub mod board;
pub mod cli;
pub mod json;
pub mod search;
pub mod pattern;
pub mod probcut;
#[cfg(not(target_arch = "wasm32"))]
pub mod legacy_search;
#[cfg(not(target_arch = "wasm32"))]
pub mod dataset;
use std::cell::RefCell;
thread_local! {static RESULT:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};}
pub fn api(input: &str) -> Result<String, String> {
    let v = json::parse(input)?;
    let p = json::position(v.get("position")?)?;
    let opts = v.get("options").ok();
    let time = opts
        .and_then(|x| x.get("timeMs").ok())
        .map_or(Ok(1000), json::Json::num)?;
    if !(0..=7_200_000).contains(&time) {
        return Err("Invalid timeMs".into());
    }
    let tt_mb=opts.and_then(|x|x.get("ttMb").ok()).map_or(Ok(32),json::Json::num)?;
    if !(1..=256).contains(&tt_mb) {return Err("ttMb must be 1..256 MiB".into())}
    let exact = opts
        .and_then(|x| x.get("exactIfPossible").ok())
        .map_or(Ok(true), json::Json::boolean)?;
    let best_only=opts.and_then(|x|x.get("bestOnly").ok()).map_or(Ok(false),json::Json::boolean)?;
    let review_solve=opts.and_then(|x|x.get("reviewSolve").ok()).map_or(Ok(false),json::Json::boolean)?;
    if review_solve {
        let o=search::Options {time_ms:time as u64,tt_entries:search::entries_for_mb(tt_mb as usize)?,..Default::default()};
        let value_only=opts.and_then(|x|x.get("valueOnly").ok()).map_or(Ok(false),json::Json::boolean)?;
        return Ok(if value_only {search::exact_value_json(p,&o)} else {search::prove_json(p,&o)});
    }
    Ok((if best_only{search::choose}else{search::analyze})(
        p,
        &search::Options {
            time_ms: time as u64,
            exact,
            tt_entries: search::entries_for_mb(tt_mb as usize)?,
            ..Default::default()
        },
    )
    .json())
}
/// Allocate exactly len bytes; caller must return the same pointer and length.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut bytes = vec![0u8; len].into_boxed_slice();
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    ptr
}
/// # Safety
/// ptr/len must be a live allocation returned by alloc, freed exactly once.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub unsafe extern "C" fn free(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)))
}
/// # Safety
/// ptr/len must refer to initialized readable bytes in this module's memory.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub unsafe extern "C" fn analyze_json(ptr: *const u8, len: usize) {
    let result = std::str::from_utf8(std::slice::from_raw_parts(ptr, len))
        .map_err(|e| e.to_string())
        .and_then(api)
        .unwrap_or_else(|e| format!("{{\"error\":{}}}", json::quote(&e)));
    RESULT.with(|r| *r.borrow_mut() = result.into_bytes());
}
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn result_len() -> usize {
    RESULT.with(|r| r.borrow().len())
}

/// Smoke-test ABI: verifies the exact model embedded in the distributed WASM.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn eval_model_checksum()->u32 {
    if pattern::validate_model(pattern::MODEL).is_err(){return 0}
    pattern::MODEL.iter().fold(2166136261u32,|h,b|(h^*b as u32).wrapping_mul(16777619))
}

#[cfg(not(target_arch = "wasm32"))]
pub mod dist;

#[cfg(not(target_arch = "wasm32"))]
pub mod shards;

#[cfg(not(target_arch = "wasm32"))]
pub mod tree;

#[cfg(not(target_arch = "wasm32"))]
pub mod tree_export;

pub mod eval2;
#[cfg(not(target_arch="wasm32"))]
pub mod eval_lab;

pub mod eval3;

// Optional runtime weight installation. The existing API stays on the old model
// until the caller explicitly loads eval3; no smoke weights are embedded.
#[cfg(target_arch = "wasm32")]
thread_local! {static EVAL3_MODEL:RefCell<Option<std::sync::Arc<eval3::Model>>>=const{RefCell::new(None)};}
#[cfg(target_arch = "wasm32")]
pub(crate) fn wasm_eval3()->Option<std::sync::Arc<eval3::Model>> {
    EVAL3_MODEL.with(|m|m.borrow().clone())
}
/// Load a DSEVAL04 model. Returns 0 on success, 1 on invalid data; len=0 resets.
/// # Safety
/// For len>0, ptr/len must designate readable bytes allocated in this module.
/// The caller retains ownership and must free its input buffer afterwards.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub unsafe extern "C" fn load_eval3(ptr:*const u8,len:usize)->i32 {
    if len==0 {PROBCUT3.with(|c|*c.borrow_mut()=Default::default());EVAL3_MODEL.with(|m|*m.borrow_mut()=None);return 0;}
    match eval3::Model::decode(std::slice::from_raw_parts(ptr,len)) {
        Ok(model)=>{PROBCUT3.with(|c|*c.borrow_mut()=Default::default());EVAL3_MODEL.with(|m|*m.borrow_mut()=Some(std::sync::Arc::new(model)));0},
        Err(_)=>1,
    }
}

#[cfg(not(target_arch="wasm32"))]
pub mod match_book;

#[cfg(target_arch="wasm32")]
thread_local! {static PROBCUT3:RefCell<probcut::Config>=RefCell::new(probcut::Config::default());}
#[cfg(target_arch="wasm32")]
pub(crate) fn wasm_probcut3() -> probcut::Config { PROBCUT3.with(|c|c.borrow().clone()) }
/// Load checksum-bound UTF-8 calibration CSV after load_eval3; len=0 disables.
/// Returns 1 on invalid CSV, confidence or model mismatch, leaving configuration intact.
/// # Safety
/// For len>0 ptr/len must designate readable module memory; caller retains ownership.
#[cfg(target_arch="wasm32")]
#[no_mangle]
pub unsafe extern "C" fn load_probcut3(ptr:*const u8,len:usize,confidence:f64)->i32 {
    if !confidence.is_finite() || confidence<=0. {return 1}
    let table = if len==0 {None} else {
        let Some(model)=wasm_eval3() else {return 1};
        let Ok(csv)=std::str::from_utf8(std::slice::from_raw_parts(ptr,len)) else {return 1};
        let Ok(table)=probcut::Table::decode(csv,&model) else {return 1};
        Some(std::sync::Arc::new(table))
    };
    PROBCUT3.with(|c| {let mut c=c.borrow_mut();c.table=table;c.confidence=confidence;});
    0
}
/// Set eval3 ordering for subsequent API searches (0 off, 1 on).
#[cfg(target_arch="wasm32")]
#[no_mangle]
pub extern "C" fn set_eval3_ordering(enabled:u32)->i32 {
    if enabled>1 {return 1}
    PROBCUT3.with(|c|c.borrow_mut().ordering=enabled!=0);0
}
