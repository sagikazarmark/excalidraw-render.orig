//! Check staged archive payloads against the claimed immutable Git tree.
use super::*;
use std::process::Command;

fn git_bytes(commit: &str, path: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(["show", &format!("{commit}:{path}")])
        .current_dir(repo_root())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("snapshot source absent from commit: {path}"));
    }
    Ok(output.stdout)
}

pub(super) fn verify(candidate: &Path, bundle: &Snapshots) -> Result<(), String> {
    for source in [
        &bundle.inventory,
        &bundle.tool_manifest,
        &bundle.dagger_lock,
        &bundle.consumer_lock,
        &bundle.consumer_manifest,
        &bundle.consumer_config,
        &bundle.collector,
    ] {
        let bytes = git_bytes(&bundle.commit, &source.path)?;
        if sha256(&bytes) != source.sha256 || bytes.len() as u64 != source.byte_length {
            return Err(format!(
                "snapshot source disagrees with commit: {}",
                source.path
            ));
        }
    }
    // Every archived authored file must match the commit; Cargo's normalized
    // manifest and generated package lock/VCS metadata are handled separately.
    for package in &bundle.packages {
        let name = Path::new(&package.path)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("invalid archive name")?;
        let prefix = name
            .strip_suffix(".crate")
            .ok_or("invalid archive suffix")?;
        let crate_name = prefix
            .strip_suffix("-0.1.0")
            .ok_or("unexpected package version")?;
        let archive = candidate.join("packages").join(name);
        let listing = Command::new("tar")
            .arg("-tzf")
            .arg(&archive)
            .output()
            .map_err(|e| e.to_string())?;
        if !listing.status.success() {
            return Err("cannot list snapshot archive".into());
        }
        let listing = String::from_utf8(listing.stdout).map_err(|e| e.to_string())?;
        let mut seen = BTreeSet::new();
        for entry in listing.lines().filter(|s| !s.ends_with('/')) {
            let relative = entry
                .strip_prefix(&format!("{prefix}/"))
                .ok_or("unexpected archive root")?;
            if !seen.insert(relative)
                || Path::new(relative)
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err("duplicate or unsafe snapshot archive entry".into());
            }
            let output = Command::new("tar")
                .arg("-xOzf")
                .arg(&archive)
                .arg(entry)
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(format!("cannot read archived {entry}"));
            }
            if relative == ".cargo_vcs_info.json" {
                let value: Value = parse(&output.stdout)?;
                if value["git"]["sha1"] != bundle.commit
                    || value["git"]["dirty"] == true
                    || value["path_in_vcs"] != format!("crates/{crate_name}")
                {
                    return Err("archived VCS provenance disagrees with commit".into());
                }
            } else if !matches!(relative, "Cargo.toml" | "Cargo.lock") {
                let authored = if relative == "Cargo.toml.orig" {
                    "Cargo.toml"
                } else {
                    relative
                };
                if output.stdout
                    != git_bytes(&bundle.commit, &format!("crates/{crate_name}/{authored}"))?
                {
                    return Err(format!("archived source disagrees with commit: {entry}"));
                }
            }
        }
        if !["Cargo.toml", "Cargo.toml.orig", ".cargo_vcs_info.json"]
            .iter()
            .all(|s| seen.contains(s))
        {
            return Err("snapshot archive lacks manifest/VCS provenance".into());
        }
    }
    Ok(())
}
