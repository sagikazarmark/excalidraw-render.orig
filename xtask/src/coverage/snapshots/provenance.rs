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

/// The repository path of a manifest-declared `readme` that lies outside the
/// crate directory, keyed by the file name Cargo archives it under.
fn external_readme(commit: &str, crate_name: &str) -> Result<Option<(String, String)>, String> {
    let manifest = git_bytes(commit, &format!("crates/{crate_name}/Cargo.toml"))?;
    let manifest: toml::Value =
        toml::from_str(std::str::from_utf8(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let Some(readme) = manifest
        .get("package")
        .and_then(|p| p.get("readme"))
        .and_then(|r| r.as_str())
    else {
        return Ok(None);
    };
    let mut parts = vec!["crates", crate_name];
    for component in readme.split('/') {
        match component {
            ".." => {
                parts.pop().ok_or("readme escapes the repository")?;
            }
            "." | "" => {}
            name => parts.push(name),
        }
    }
    if parts.first() == Some(&"crates") && parts.get(1) == Some(&crate_name) {
        return Ok(None);
    }
    let name = parts.last().ok_or("empty readme path")?.to_string();
    Ok(Some((name, parts.join("/"))))
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
        let (crate_name, _) = SNAPSHOT_PACKAGES
            .iter()
            .find(|(name, version)| prefix == format!("{name}-{version}"))
            .ok_or("unexpected package version")?;
        let readme = external_readme(&bundle.commit, crate_name)?;
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
                // Cargo archives a readme from outside the crate under its
                // file name at the package root.
                let path = match &readme {
                    Some((name, path)) if name == authored => path.clone(),
                    _ => format!("crates/{crate_name}/{authored}"),
                };
                if output.stdout != git_bytes(&bundle.commit, &path)? {
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
