use dosello_ai::{board::*, cli, dist, json};
fn main() {
    cli::main_result(|| {
        let a=cli::args()?;
        let arg=|name: &str, default: &str| a.rest.chunks(2).find(|p|p[0]==name).and_then(|p|p.get(1)).cloned().unwrap_or(default.into());
        let kind=arg("--type","exact");
        if kind=="tree-inspect" {
            let input=arg("--requests", "");
            if input.is_empty(){println!("{}",dosello_ai::tree::inspect(a.position));}
            else {for k in json::parse(&input)?.arr()?{println!("{}",dosello_ai::tree::inspect(dosello_ai::tree::position(k.str()?)?));}}
            return Ok(())
        }
        if kind=="tree-batch" {
            let path=arg("--request-file", "");
            let raw=if path.is_empty(){arg("--requests","[]")}else{std::fs::read_to_string(path).map_err(|e|e.to_string())?};
            return dosello_ai::tree::batch(&raw,&a.opts)
        }
        if kind=="tree-verify" {return dosello_ai::tree::verify(std::path::Path::new(&arg("--source","")))}
        if kind=="tree-seed" {return dosello_ai::tree::seed(std::path::Path::new(&arg("--source","")),arg("--format","book")=="proof")}

        if kind=="inspect" {println!("{}",dist::inspect(a.position));return Ok(())}
        if kind=="sample" {
            let mut seed=arg("--seed","12345").parse().map_err(|_|"seed")?;
            let empties=arg("--empties","28").parse().map_err(|_|"empties")?;
            if empties>34 {return Err("sample limited to <=34 empties".into())}
            println!("{}",json::position_json(random_position(&mut seed,empties)));return Ok(())
        }
        if kind=="perft" {println!("{{\"depth\":4,\"leaves\":{}}}",perft(Position::initial(),4));return Ok(())}
        let threshold=arg("--threshold","0").parse().map_err(|_|"threshold")?;
        let checkpoint=arg("--checkpoint", "job-checkpoint.json");
        let (tx,rx)=std::sync::mpsc::channel::<()>();
        let heartbeat=std::thread::spawn(move || {
            let start=std::time::Instant::now();
            while rx.recv_timeout(std::time::Duration::from_secs(10))==Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                eprintln!("progress elapsedSecs={} searching=true",start.elapsed().as_secs());
            }
        });
        let result=dist::solve(a.position,&a.opts,&kind,threshold,std::path::Path::new(&checkpoint));
        drop(tx);heartbeat.join().map_err(|_|"heartbeat panic")?;
        println!("{}",result?);Ok(())
    });
}
