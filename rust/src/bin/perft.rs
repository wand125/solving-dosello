use dosello_ai::{board::*, cli};
fn main() {
    cli::main_result(|| {
        let d = std::env::args()
            .nth(1)
            .unwrap_or("4".into())
            .parse()
            .map_err(|_| "depth")?;
        let start = std::time::Instant::now();
        let n = perft(Position::initial(), d);
        println!(
            "{{\"depth\":{d},\"leaves\":{n},\"elapsedMs\":{}}}",
            start.elapsed().as_millis()
        );
        Ok(())
    })
}
