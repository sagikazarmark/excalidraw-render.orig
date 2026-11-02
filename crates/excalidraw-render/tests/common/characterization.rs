//! Characterizations of known defects: the renderer's current, wrong output for
//! an open issue, held exactly so that any movement is seen.
//!
//! A characterization is not parity evidence. It records what the renderer
//! does today where it is known to be wrong, so that a change cannot move the
//! defect unnoticed, in either direction. When the pinned value moves because
//! the output improved, the right response is to close the issue and drop the
//! pin, not to transcribe the new value; [`known_defect`] says so in its
//! failure. The parity harnesses' `PinnedDivergence` and `PinnedGeometryDefect`
//! are the same idea for measurements against official evidence.

use std::fmt::Debug;

/// Holds the observed output of known defect `issue` (`"#190"`) at its pinned
/// value. `context` names what was observed (a fixture row and a format).
/// Passes while the two are equal; otherwise fails naming the issue, both
/// values and what to do about it.
#[track_caller]
pub fn known_defect<T: PartialEq + Debug>(issue: &str, context: &str, observed: T, pinned: T) {
    if let Some(message) = moved(issue, context, &observed, &pinned) {
        panic!("{message}");
    }
}

/// The failure [`known_defect`] reports, if the observation moved.
pub fn moved<T: PartialEq + Debug>(
    issue: &str,
    context: &str,
    observed: &T,
    pinned: &T,
) -> Option<String> {
    (observed != pinned).then(|| {
        format!(
            "known defect {issue} moved ({context}): observed {observed:?}, pinned {pinned:?}. \
             If the output improved, close the issue and drop the pin; otherwise the defect \
             changed and the characterization must be revisited"
        )
    })
}
