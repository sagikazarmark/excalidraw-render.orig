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
fn external_readme(commit: &str, crate_dir: &str) -> Result<Option<(String, String)>, String> {
    let manifest = git_bytes(commit, &format!("{crate_dir}/Cargo.toml"))?;
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
    let crate_parts: Vec<&str> = crate_dir.split('/').collect();
    let mut parts = crate_parts.clone();
    for component in readme.split('/') {
        match component {
            ".." => {
                parts.pop().ok_or("readme escapes the repository")?;
            }
            "." | "" => {}
            name => parts.push(name),
        }
    }
    if parts.len() > crate_parts.len() && parts.starts_with(&crate_parts) {
        return Ok(None);
    }
    let name = parts.last().ok_or("empty readme path")?.to_string();
    Ok(Some((name, parts.join("/"))))
}

/// The crate directory the archive was packaged from, as Cargo recorded it,
/// checked against the commit: the manifest there must name this package.
/// Package names and directories differ (`excalidraw-render-fonts` lives in
/// `crates/excalidraw-fonts`), so the archive's own record is the source.
fn archived_crate_dir(
    archive: &Path,
    prefix: &str,
    commit: &str,
    crate_name: &str,
) -> Result<String, String> {
    let output = Command::new("tar")
        .arg("-xOzf")
        .arg(archive)
        .arg(format!("{prefix}/.cargo_vcs_info.json"))
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("snapshot archive lacks manifest/VCS provenance".into());
    }
    let value: Value = parse(&output.stdout)?;
    let dir = value["path_in_vcs"]
        .as_str()
        .ok_or("archived VCS provenance lacks path_in_vcs")?;
    if value["git"]["sha1"] != commit
        || value["git"]["dirty"] == true
        || dir.is_empty()
        || Path::new(dir)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("archived VCS provenance disagrees with commit".into());
    }
    let manifest = git_bytes(commit, &format!("{dir}/Cargo.toml"))?;
    let manifest: toml::Value =
        toml::from_str(std::str::from_utf8(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        != Some(crate_name)
    {
        return Err(format!(
            "archived VCS provenance names {dir}, which does not hold {crate_name} in the commit"
        ));
    }
    Ok(dir.to_owned())
}

pub(super) fn verify(candidate: &Path, bundle: &Snapshots) -> Result<(), String> {
    for source in [
        Some(&bundle.inventory),
        bundle.tool_manifest.as_ref(),
        bundle.dagger_lock.as_ref(),
        Some(&bundle.consumer_lock),
        Some(&bundle.consumer_manifest),
        Some(&bundle.consumer_config),
        Some(&bundle.collector),
    ]
    .into_iter()
    .flatten()
    {
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
        let archive = candidate.join("packages").join(name);
        let crate_dir = archived_crate_dir(&archive, prefix, &bundle.commit, crate_name)?;
        let readme = external_readme(&bundle.commit, &crate_dir)?;
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
            if matches!(
                relative,
                ".cargo_vcs_info.json" | "Cargo.toml" | "Cargo.lock"
            ) {
                continue;
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
            let authored = if relative == "Cargo.toml.orig" {
                "Cargo.toml"
            } else {
                relative
            };
            // Cargo archives a readme from outside the crate under its
            // file name at the package root.
            let path = match &readme {
                Some((name, path)) if name == authored => path.clone(),
                _ => format!("{crate_dir}/{authored}"),
            };
            if output.stdout != git_bytes(&bundle.commit, &path)? {
                return Err(format!("archived source disagrees with commit: {entry}"));
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
