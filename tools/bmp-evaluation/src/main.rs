fn main() {
    let report = bmp_evaluation::report();
    let serialized = serde_json::to_string_pretty(&report).unwrap() + "\n";
    match std::env::args().nth(1).as_deref() {
        Some("--record") => {
            std::fs::write(
                concat!(env!("CARGO_MANIFEST_DIR"), "/expected.json"),
                serialized,
            )
            .unwrap();
            std::fs::write(
                concat!(env!("CARGO_MANIFEST_DIR"), "/matrix.md"),
                bmp_evaluation::matrix(&report),
            )
            .unwrap();
        }
        None => print!("{serialized}"),
        _ => panic!("usage: bmp-evaluation [--record]"),
    }
}
