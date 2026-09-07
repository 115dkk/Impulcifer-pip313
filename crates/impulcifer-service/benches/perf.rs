#![forbid(unsafe_code)]
#[path = "../examples/demo_brir.rs"]
mod demo;

#[path = "../tests/bench_support/service.rs"]
mod service;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--emit-diagnostic") {
        service::emit_diagnostic();
        return Ok(());
    }
    if !std::env::args().any(|arg| arg == "--pipeline") {
        service::run();
        return Ok(());
    }
    println!("| scenario | rust median ms | rust min ms |");
    for vbass in [false, true] {
        let mut times = Vec::new();
        for iteration in 0..6 {
            let dir = demo::DemoCopy::new(&demo::root().join("data/demo"), false)?;
            let result = demo::run(&dir.0, vbass)?;
            println!(
                "PA03_RAW {} {iteration} {result}",
                if vbass { "vbass" } else { "default" }
            );
            if iteration > 0 {
                times.push(result["seconds"].as_f64().unwrap() * 1000.0);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "| {} | {:.6} | {:.6} |",
            if vbass { "vbass" } else { "default" },
            times[2],
            times[0]
        );
    }
    Ok(())
}
