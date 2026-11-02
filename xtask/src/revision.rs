//! The compatibility revision's reference identities: where each lives and
//! what its digest is. The digest is read from the file's `.sha256` sidecar,
//! after checking the file still has it, so the sidecar is the one place a
//! profile update changes; no verifier repeats it as a literal.

use std::fs;

use crate::conformance::{repo_root, sha256};

/// The compatibility revision these identities belong to.
pub(crate) const REVISION: u64 = 1;

/// One reference identity of the revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reference {
    /// The scene reference profile (the pinned oracle, browser and fonts).
    Profile,
    /// The frame-export reference profile built on it.
    FrameProfile,
    /// The manifest of the official package's assets.
    AssetManifest,
    /// The scene surface the revision admits.
    SceneSurface,
}

impl Reference {
    /// The repository-relative path of the identity's JSON.
    pub(crate) fn path(self) -> String {
        match self {
            Self::Profile => format!("tests/oracle/reference-profiles/revision-{REVISION}.json"),
            Self::FrameProfile => {
                format!("tests/oracle/reference-profiles/revision-{REVISION}-frame.json")
            }
            Self::AssetManifest => {
                format!("tests/oracle/reference-profiles/revision-{REVISION}-assets.json")
            }
            Self::SceneSurface => format!("tests/oracle/scene-surfaces/revision-{REVISION}.json"),
        }
    }

    /// The approved digest, lowercase hex: the sidecar's, once the file's
    /// bytes are checked to have it.
    pub(crate) fn digest(self) -> Result<String, String> {
        sealed_digest(&self.path())
    }

    /// The digest as the records spell it, `sha256:<hex>`.
    pub(crate) fn prefixed_digest(self) -> Result<String, String> {
        Ok(format!("sha256:{}", self.digest()?))
    }
}

/// The digest of a repository file that has a `.sha256` sidecar beside it
/// (`name.json` and `name.sha256`, `name.bin` and `name.sha256`): the
/// sidecar's, once the file's bytes are checked to have it. An artifact pinned
/// this way is updated by regenerating it and its sidecar, with no verifier
/// edit.
pub(crate) fn sealed_digest(path: &str) -> Result<String, String> {
    let sidecar = match path.rsplit_once('.') {
        Some((stem, extension)) if !extension.contains('/') => format!("{stem}.sha256"),
        _ => return Err(format!("{path} has no extension to replace")),
    };
    let digest = fs::read_to_string(repo_root().join(&sidecar))
        .map_err(|error| format!("failed to read {sidecar}: {error}"))?
        .trim()
        .to_owned();
    if !crate::viewer_identity::is_lower_hex(&digest, 64) {
        return Err(format!("{sidecar} holds no SHA-256 digest"));
    }
    let bytes = fs::read(repo_root().join(path))
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    if sha256(&bytes) != digest {
        return Err(format!("{path} does not match {sidecar}"));
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reference_identity_matches_its_sidecar() {
        for reference in [
            Reference::Profile,
            Reference::FrameProfile,
            Reference::AssetManifest,
            Reference::SceneSurface,
        ] {
            reference.digest().unwrap();
        }
        assert_eq!(
            Reference::FrameProfile.path(),
            "tests/oracle/reference-profiles/revision-1-frame.json"
        );
    }

    #[test]
    fn sealed_digests_check_the_file_against_its_sidecar() {
        let bin = "tests/oracle/compatibility-artifacts/revision-1-chromium-149-cjk-scalars.bin";
        assert_eq!(
            sealed_digest(bin).unwrap(),
            sha256(&fs::read(repo_root().join(bin)).unwrap())
        );
        // A file without a sidecar has no sealed digest.
        let schema = "tests/oracle/scene-surfaces/scene-surface-v1.schema.json";
        assert!(sealed_digest(schema)
            .unwrap_err()
            .contains("failed to read"));
    }
}
