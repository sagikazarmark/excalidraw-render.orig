//! A per-test scratch directory for the harness tiers, which hand candidates to
//! external viewers and read their rasters back. It lives under Cargo's scratch
//! space for integration tests (`CARGO_TARGET_TMPDIR`, `target/tmp` by
//! default) rather than at a literal `target/` path, is removed when the guard
//! drops normally, and is kept — with its path printed — when the guard drops
//! while the test is panicking, so a failed gate leaves its evidence behind.
//!
//! The harness scripts only accept paths inside the checkout: the PDF adapters
//! `realpath` and refuse anything outside the repository, and the Chromium
//! viewer bind-mounts the repository root into its container and takes
//! repository-relative paths. [`ScratchDir::relative`] gives a file that form,
//! which holds whenever the target directory is inside the checkout.

use std::fs;
use std::path::{Path, PathBuf};

use super::repository_root;

pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Creates a fresh `<CARGO_TARGET_TMPDIR>/<name>-<pid>` directory.
    pub fn new(name: &str) -> Self {
        let path =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).expect("stale scratch directory is removable");
        }
        fs::create_dir_all(&path).expect("scratch directory is creatable");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The absolute path of `file` inside the directory.
    pub fn join(&self, file: &str) -> PathBuf {
        self.path.join(file)
    }

    /// Writes `bytes` to `file` inside the directory and returns the path the
    /// harness scripts accept for it, see [`Self::relative`].
    pub fn write(&self, file: &str, bytes: &[u8]) -> String {
        fs::write(self.join(file), bytes)
            .unwrap_or_else(|error| panic!("{file} is written to scratch: {error}"));
        self.relative(file)
    }

    /// The path of `file` relative to the repository root, the form the harness
    /// scripts accept.
    pub fn relative(&self, file: &str) -> String {
        let root = repository_root();
        let relative = self.path.strip_prefix(&root).unwrap_or_else(|_| {
            panic!(
                "the harness scripts only accept paths inside the checkout, but the scratch \
                 directory {} is outside {}; keep CARGO_TARGET_DIR inside the repository",
                self.path.display(),
                root.display()
            )
        });
        let relative = relative.join(file);
        relative.to_str().expect("scratch path is UTF-8").to_owned()
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "keeping scratch directory {} for inspection",
                self.path.display()
            );
            return;
        }
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "scratch directory {} was not removed: {error}",
                self.path.display()
            );
        }
    }
}
