//! The repository README's Rust examples are not doctested: the published
//! crate packages only its own README, so including the workspace README from
//! `src/` would break doctests run from the crates.io source. Each repository
//! example must instead appear verbatim in the crate README, which `lib.rs`
//! doctests.

const REPOSITORY_README: &str = include_str!("../../../../README.md");
const CRATE_README: &str = include_str!("../../README.md");

/// Returns each fenced block whose info string starts with `rust`, fences
/// included.
fn rust_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        match current.as_mut() {
            Some(block) => {
                block.push_str(line);
                block.push('\n');
                if line == "```" {
                    blocks.extend(current.take());
                }
            }
            None if line.starts_with("```rust") => current = Some(format!("{line}\n")),
            None => {}
        }
    }
    assert!(current.is_none(), "unterminated rust block");
    blocks
}

#[test]
fn repository_readme_examples_are_doctested_through_the_crate_readme() {
    let examples = rust_blocks(REPOSITORY_README);
    assert!(
        !examples.is_empty(),
        "the repository README has no Rust example"
    );
    for example in examples {
        assert!(
            CRATE_README.contains(&example),
            "repository README example is not in the crate README verbatim:\n{example}"
        );
    }
}
