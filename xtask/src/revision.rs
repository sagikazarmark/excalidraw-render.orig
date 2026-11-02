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

    /// The path of the identity's digest sidecar.
    pub(crate) fn sidecar_path(self) -> String {
        let path = self.path();
        format!("{}.sha256", path.strip_suffix(".json").unwrap_or(&path))
    }

    /// The approved digest, lowercase hex: the sidecar's, once the file's
    /// bytes are checked to have it.
    pub(crate) fn digest(self) -> Result<String, String> {
        let sidecar = self.sidecar_path();
        let digest = fs::read_to_string(repo_root().join(&sidecar))
            .map_err(|error| format!("failed to read {sidecar}: {error}"))?
            .trim()
            .to_owned();
        if !crate::viewer_identity::is_lower_hex(&digest, 64) {
            return Err(format!("{sidecar} holds no SHA-256 digest"));
        }
        let path = self.path();
        let bytes = fs::read(repo_root().join(&path))
            .map_err(|error| format!("failed to read {path}: {error}"))?;
        if sha256(&bytes) != digest {
            return Err(format!("{path} does not match {sidecar}"));
        }
        Ok(digest)
    }

    /// The digest as the records spell it, `sha256:<hex>`.
    pub(crate) fn prefixed_digest(self) -> Result<String, String> {
        Ok(format!("sha256:{}", self.digest()?))
    }
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
            let digest = reference.digest().unwrap();
            assert_eq!(
                reference.prefixed_digest().unwrap(),
                format!("sha256:{digest}")
            );
            assert!(reference.sidecar_path().ends_with(".sha256"));
        }
        assert_eq!(
            Reference::FrameProfile.sidecar_path(),
            "tests/oracle/reference-profiles/revision-1-frame.sha256"
        );
    }
}
