use std::process::Command;

#[test]
fn verifies_the_sealed_revision_catalog_offline() {
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["catalog", "verify"])
            .output()
            .expect("run catalog verifier")
    };
    let output = run();
    let repeated = run();

    assert!(
        output.status.success(),
        "verifier failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::str::from_utf8(&output.stdout).expect("verifier output is UTF-8"),
        "verified compatibility revision 1 catalog\n"
    );
    assert_eq!(output.status, repeated.status);
    assert_eq!(output.stdout, repeated.stdout);
    assert_eq!(output.stderr, repeated.stderr);
}
