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
