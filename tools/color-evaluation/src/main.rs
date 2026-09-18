fn main() {
    let report = color_evaluation::report();
    if std::env::args().any(|x| x == "--record") {
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/expected-native.json"),
            serde_json::to_string_pretty(&report).unwrap() + "\n",
        )
        .unwrap();
    } else {
        color_evaluation::verify(&report, include_str!("../expected-native.json"));
    }
}
