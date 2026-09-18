fn main() {
    let report = kurbo_evaluation::report();
    if std::env::args().nth(1).as_deref() == Some("--record") {
        std::fs::write(
            std::env::var("KURBO_EXPECTED").unwrap(),
            serde_json::to_string_pretty(&report).unwrap() + "\n",
        )
        .unwrap();
    } else {
        kurbo_evaluation::verify(&report);
        println!("Exact native observation matches");
    }
}
