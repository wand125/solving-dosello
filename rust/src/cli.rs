use crate::{board::Position, json, search::Options};
pub struct Args {
    pub opts: Options,
    pub position: Position,
    pub rest: Vec<String>,
    pub wld: bool,
    pub prove_best: bool,
    pub root_score: bool,
}
pub fn args() -> Result<Args, String> {
    let mut opts = Options::default();
    let mut eval_args = std::collections::BTreeMap::new();
    let mut words = vec![];
    let mut rest = vec![];
    let mut wld = false;
    let mut prove_best = false;
    let mut root_score = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--eval" | "--weights" | "--probcut" | "--probcut-t" | "--eval-ordering" => {
                eval_args.insert(a, it.next().ok_or("missing evaluator option value")?);
            }
            "--time" => opts.time_ms = it.next().ok_or("time")?.parse().map_err(|_| "time")?,
            "--threads" => {
                opts.threads = it.next().ok_or("threads")?.parse().map_err(|_| "threads")?
            }
            "--tt-mb" => opts.tt_entries=crate::search::entries_for_mb(it.next().ok_or("tt-mb")?.parse().map_err(|_|"tt-mb")?)?,
            "--tt" => {
                let n: u32 = it.next().ok_or("tt")?.parse().map_err(|_| "tt")?;
                if !(10..=26).contains(&n) {
                    return Err("--tt must be 10..26 (log2 entries total)".into());
                }
                opts.tt_entries = 1 << n
            }
            "--selectivity" => {opts.selectivity=it.next().ok_or("selectivity")?.parse().map_err(|_|"selectivity")?;if opts.selectivity>3{return Err("selectivity must be 0..3".into())}},
            "--exact" => opts.exact = true,
            "--no-exact" => opts.exact = false,
            "--wld" => wld = true,
            "--prove-best" => prove_best = true,
            "--root-score" => root_score = true,
            "--selective-exact" => opts.selective_exact = true,
            "--json" => words.push(
                std::fs::read_to_string(it.next().ok_or("JSON path")?)
                    .map_err(|e| e.to_string())?,
            ),
            _ if a.starts_with("--") => {
                rest.push(a);
                if let Some(x) = it.next() {
                    rest.push(x)
                }
            }
            _ => words.push(a),
        }
    }
    if !eval_args.is_empty() {
        #[cfg(not(target_arch="wasm32"))] {
            let args = crate::eval_lab::Args(eval_args);
            let model = args.model()?;
            opts.probcut3 = args.search_config(&model)?;
            match model.as_deref() {
                Some(crate::eval_lab::Evaluator::Eval3(m)) => opts.eval3 = Some(m.clone()),
                Some(_) => return Err("analyze/solve support --eval old|eval3".into()),
                None => {}
            }
        }
        #[cfg(target_arch="wasm32")] {return Err("use wasm model loading API".into());}
    }
    if root_score && prove_best {return Err("Choose --root-score or --prove-best".into())}
    let s = words.join(" ");
    let position = if s.trim().starts_with('{') {
        json::position(&json::parse(&s)?)?
    } else {
        Position::sequence(&s)?
    };
    Ok(Args {
        opts,
        position,
        rest,
        wld,
        prove_best,
        root_score,
    })
}
pub fn main_result(f: impl FnOnce() -> Result<(), String>) {
    if let Err(e) = f() {
        eprintln!("Error: {e}");
        std::process::exit(1)
    }
}
