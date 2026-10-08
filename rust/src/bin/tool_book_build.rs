//! Validate a complete proof-only DSBOOK interchange before compacting it.
fn main() {
    dosello_ai::cli::main_result(|| {
        let a = dosello_ai::eval_lab::Args::read()?;
        let bytes = std::fs::read(a.get("--input", "")).map_err(|e| e.to_string())?;
        let book = dosello_ai::match_book::MatchBook::decode(&bytes)?;
        let out = book.encode_tool()?;
        dosello_ai::eval_lab::atomic(&a.get("--output", "tool-book.bin"), &out)?;
        println!("proven={} bytes={}", (out.len() - 12) / 36, out.len());
        Ok(())
    })
}
