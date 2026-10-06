use dosello_ai::{cli, json};
use std::io::{self, BufRead};
fn main() {
    cli::main_result(|| {
        let mut count = 0;
        for (line_no, line) in io::stdin().lock().lines().enumerate() {
            let line = line.map_err(|e| e.to_string())?;
            let v = json::parse(&line)?;
            let p = json::position(&v)?;
            let ms = p.moves();
            let expected = v.get("moves")?.arr()?;
            if ms.len() != expected.len() {
                return Err(format!(
                    "line {}: move count {} vs {}",
                    line_no + 1,
                    ms.len(),
                    expected.len()
                ));
            }
            for (m, e) in ms.iter().zip(expected) {
                let cells = e.get("cells")?.arr()?;
                if m.a as i64 != cells[0].num()? || m.b as i64 != cells[1].num()? {
                    return Err(format!("line {}: enumeration mismatch", line_no + 1));
                }
                let flips = e
                    .get("flips")?
                    .arr()?
                    .iter()
                    .try_fold(0u64, |a, x| x.num().map(|i| a | (1 << i)))?;
                if m.flips != flips {
                    return Err(format!("line {}: flips mismatch {m}", line_no + 1));
                }
                if p.play(*m) != json::position(e.get("next")?)? {
                    return Err(format!("line {}: resulting position mismatch", line_no + 1));
                }
            }
            count += 1;
        }
        println!("Verified {count} positions: moves, flips, resulting boards/pairings");
        Ok(())
    })
}
