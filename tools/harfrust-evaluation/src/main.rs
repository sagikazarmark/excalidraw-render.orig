use std::time::Instant;

fn main() {
    let started = Instant::now();
    let report = harfrust_evaluation::report();
    let elapsed = started.elapsed();
    match std::env::args().nth(1).as_deref() {
        Some("--record") => {
            let path = std::env::args().nth(2).expect("--record <path>");
            std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap() + "\n").unwrap();
            eprintln!("recorded {path} in {elapsed:.1?}");
        }
        _ => {
            let mut summary = report.clone();
            summary.as_object_mut().unwrap().remove("examples");
            summary.as_object_mut().unwrap().remove("parse");
            println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            eprintln!("elapsed {elapsed:.1?}");
        }
    }
}
