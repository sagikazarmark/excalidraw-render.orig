//! The scratch directory the harness tiers write their candidates into: it
//! lives inside the checkout even with external Cargo build output, is removed when the test passes, and
//! is kept for inspection when the test panics.

use std::fs;

use crate::common::repository_root;
use crate::common::scratch::ScratchDir;

#[test]
fn a_scratch_dir_lives_under_the_target_scratch_space_and_goes_away_on_drop() {
    let path = {
        let scratch = ScratchDir::new("scratch-dir-lifetime");
        let path = scratch.path().to_owned();
        assert!(path.is_dir(), "scratch dir is created on construction");
        assert!(
            path.starts_with(repository_root().join("target/tmp")),
            "{} is not under the repository's harness scratch directory",
            path.display()
        );
        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("scratch dir has a UTF-8 name")
            .starts_with("scratch-dir-lifetime-"));
        fs::write(scratch.join("candidate.txt"), b"candidate").expect("scratch is writable");
        path
    };
    assert!(!path.exists(), "scratch dir is removed with its contents");
}

#[test]
fn a_scratch_file_has_a_repository_relative_path_for_the_harness_scripts() {
    let scratch = ScratchDir::new("scratch-dir-relative");
    let relative = scratch.relative("candidate.pdf");
    assert!(
        !relative.starts_with('/'),
        "{relative} is not repository-relative"
    );
    assert_eq!(
        repository_root().join(&relative),
        scratch.join("candidate.pdf")
    );
}

#[test]
fn a_panicking_test_keeps_its_scratch_dir_for_inspection() {
    let scratch = ScratchDir::new("scratch-dir-panic");
    let kept = scratch.path().to_owned();
    fs::write(scratch.join("evidence.txt"), b"evidence").expect("scratch is writable");

    let unwound = std::panic::catch_unwind(move || {
        let _guard = scratch;
        panic!("the test failed after writing its candidate");
    });

    assert!(unwound.is_err(), "the guarded closure panicked");
    assert!(
        kept.join("evidence.txt").is_file(),
        "a scratch dir dropped while panicking is kept"
    );
    fs::remove_dir_all(&kept).expect("the kept scratch dir is removable");
}
