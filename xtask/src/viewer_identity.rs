//! Where the viewer verifiers read the identities they check. A viewer's
//! version, container and runtime live in one place each: the reference
//! profile for the pinned container and its Chromium, Node, npm and
//! Playwright; a viewer profile for its own runtime records; and the live tool
//! (a lockfile, a toolchain file, the PDFium runtime record) for what that
//! profile must agree with. No verifier repeats a version as a literal, so a
//! viewer bump edits the profile, its sidecar and the evidence, and not xtask.

use std::fs;

use serde_json::Value;

use crate::conformance::{repo_root, sha256};

pub(crate) const PDFIUM_PROFILE_PATH: &str =
    "tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json";

/// A profile JSON (`*.json`), read only when its bytes match the digest in
/// its `*.sha256` sidecar.
pub(crate) fn sealed_profile(path: &str) -> Result<Value, String> {
    let bytes = fs::read(repo_root().join(path))
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    let sidecar = path
        .strip_suffix(".json")
        .map(|stem| format!("{stem}.sha256"))
        .ok_or_else(|| format!("{path} is not a JSON profile"))?;
    if sha256(&bytes) != read_text(&sidecar)?.trim() {
        return Err(format!("{path} does not match {sidecar}"));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid {path}: {error}"))
}

/// The scene reference profile, which `conformance verify-all` binds to every
/// case.
pub(crate) fn reference_profile() -> Result<Value, String> {
    sealed_profile(&crate::revision::Reference::Profile.path())
}

/// The pinned reference container's runtime, as the reference profile
/// records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceRuntime {
    /// `image@sha256:digest`.
    pub container: String,
    /// `linux/amd64`.
    pub architecture: String,
    pub ubuntu: String,
    pub chromium: String,
    /// Without a `v` prefix.
    pub node: String,
    pub npm: String,
    pub playwright: String,
    pub browser_arguments: Vec<String>,
}

impl ReferenceRuntime {
    pub(crate) fn from_profile(profile: &Value) -> Result<Self, String> {
        let platform = &profile["canonicalPlatform"];
        let field = |value: &Value, name: &str| -> Result<String, String> {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("the reference profile records no {name}"))
        };
        Ok(Self {
            container: format!(
                "{}@{}",
                field(&platform["container"]["image"], "container image")?,
                field(
                    &platform["container"]["perPlatformDigest"],
                    "container digest"
                )?
            ),
            architecture: field(&platform["architecture"], "architecture")?,
            ubuntu: field(&platform["container"]["ubuntu"], "Ubuntu release")?,
            chromium: field(&platform["runtime"]["chromium"], "Chromium version")?,
            node: field(&platform["runtime"]["node"], "Node version")?,
            npm: field(&platform["runtime"]["npm"], "npm version")?,
            playwright: field(&platform["runtime"]["playwright"], "Playwright version")?,
            browser_arguments: profile["browser"]["launch"]["explicitArguments"]
                .as_array()
                .ok_or("the reference profile records no browser arguments")?
                .iter()
                .map(|argument| field(argument, "browser argument"))
                .collect::<Result<_, _>>()?,
        })
    }

    pub(crate) fn load() -> Result<Self, String> {
        Self::from_profile(&reference_profile()?)
    }
}

/// A canonical SVG viewer profile (`chromium` or `pure-rust`).
pub(crate) fn svg_viewer_profile(viewer: &str) -> Result<Value, String> {
    sealed_profile(&format!(
        "tests/conformance/viewer-profiles/canonical-svg-{viewer}-v1.json"
    ))
}

/// The value of a viewer profile's `runtime` record named `name`.
pub(crate) fn runtime_record<'a>(profile: &'a Value, name: &str) -> Result<&'a str, String> {
    let records = profile["runtime"]
        .as_array()
        .ok_or("viewer profile runtime records must be an array")?;
    let mut matching = records.iter().filter(|record| record["name"] == name);
    let value = matching
        .next()
        .and_then(|record| record["value"].as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("viewer profile records no {name} runtime"))?;
    if matching.next().is_some() {
        return Err(format!("viewer profile records {name} twice"));
    }
    Ok(value)
}

/// The one version of `name` a Cargo lockfile resolves.
pub(crate) fn cargo_locked_version(lockfile: &str, name: &str) -> Result<String, String> {
    let versions: Vec<&str> = lockfile
        .split("[[package]]")
        .filter_map(|record| {
            let mut lines = record
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty());
            (lines.next() == Some(&format!("name = \"{name}\"")))
                .then(|| lines.next())
                .flatten()
                .and_then(|line| line.strip_prefix("version = \""))
                .and_then(|version| version.strip_suffix('"'))
        })
        .collect();
    match versions.as_slice() {
        [version] => Ok((*version).to_owned()),
        [] => Err(format!("the lockfile resolves no {name}")),
        _ => Err(format!("the lockfile resolves {name} at {versions:?}")),
    }
}

/// The `channel` a `rust-toolchain.toml` pins.
pub(crate) fn toolchain_channel(toolchain: &str) -> Result<String, String> {
    toolchain
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "channel")
        .map(|(_, value)| value.trim().trim_matches('"').to_owned())
        .filter(|channel| !channel.is_empty())
        .ok_or_else(|| "rust-toolchain.toml pins no channel".to_owned())
}

/// Whether `value` is `length` lowercase hexadecimal digits.
pub(crate) fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `value` is a full lowercase Git commit hash.
pub(crate) fn is_commit(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|commit| is_lower_hex(commit, 40))
}

/// Reads a repository file as UTF-8.
pub(crate) fn read_text(relative: &str) -> Result<String, String> {
    fs::read_to_string(repo_root().join(relative))
        .map_err(|error| format!("failed to read {relative}: {error}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reference_runtime_reads_the_profile() {
        let runtime = ReferenceRuntime::load().expect("the reference profile reads");
        assert!(runtime.container.contains("@sha256:"));
        assert!(!runtime.node.starts_with('v'));
        assert!(!runtime.browser_arguments.is_empty());
    }

    #[test]
    fn cargo_locked_version_requires_exactly_one_resolution() {
        let lock = "[[package]]\nname = \"resvg\"\nversion = \"0.48.1\"\n\n\
                    [[package]]\nname = \"usvg\"\nversion = \"0.48.1\"\n\n\
                    [[package]]\nname = \"usvg\"\nversion = \"0.49.0\"\n";
        assert_eq!(cargo_locked_version(lock, "resvg").unwrap(), "0.48.1");
        assert!(cargo_locked_version(lock, "fontdb")
            .unwrap_err()
            .contains("no fontdb"));
        assert!(cargo_locked_version(lock, "usvg").is_err());
    }

    #[test]
    fn runtime_records_are_unique_and_non_empty() {
        let profile = json!({"runtime": [
            {"name": "node", "value": "24.16.0"},
            {"name": "npm", "value": ""},
            {"name": "twice", "value": "1"},
            {"name": "twice", "value": "2"},
        ]});
        assert_eq!(runtime_record(&profile, "node").unwrap(), "24.16.0");
        assert!(runtime_record(&profile, "npm").is_err());
        assert!(runtime_record(&profile, "twice").is_err());
        assert!(runtime_record(&profile, "absent").is_err());
    }

    #[test]
    fn toolchain_channel_reads_the_pinned_channel() {
        assert_eq!(
            toolchain_channel("[toolchain]\nchannel = \"1.97.1\"\n").unwrap(),
            "1.97.1"
        );
        assert!(toolchain_channel("[toolchain]\n").is_err());
    }
}
