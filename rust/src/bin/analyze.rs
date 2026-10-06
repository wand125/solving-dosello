use dosello_ai::{cli, search};
fn main() {
    cli::main_result(|| {
        let a = cli::args()?;
        if a.root_score {return Err("--root-score is available in solve".into())}
        if a.prove_best && a.wld {return Err("--prove-best requires disc scores, not --wld".into())}
        if !a.rest.is_empty() {
            return Err(format!("Unknown options: {:?}", a.rest));
        }
        println!("{}", if a.prove_best { search::prove_json(a.position, &a.opts) } else { search::analyze(a.position, &a.opts).json() });
        Ok(())
    })
}
