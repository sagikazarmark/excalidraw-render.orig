//! JSON-only retention. Offline verification checks retained integrity, not
//! absent pixels; `check` validates a full v2 collection at the recorded roots.
use super::*;
use crate::cli::CompactCommand;
use std::path::{Component, PathBuf};

const SCHEMA: &str = "excalidraw-render/representative-compact-evidence/v2";
const LEGACY: &str = "excalidraw-render/representative-compact-evidence/v1";
const ROOTS: [(&str, &str); 3] = [
    ("reference", "reference.json"),
    ("native", "capture.json"),
    ("viewers", "viewers.json"),
];

pub(super) fn run(command: CompactCommand) -> Result<()> {
    let result = match command {
        CompactCommand::Retain {
            bundle,
            reference,
            native,
            viewers,
            output,
        } => retain(&bundle, [&reference, &native, &viewers], &output)?,
        CompactCommand::Verify { output } => {
            verify(&output)?;
            json!({"status":"verified-compact-observations","approval":null})
        }
        CompactCommand::Check { output } => check(&output)?,
    };
    println!(
        "{}",
        String::from_utf8(json::compact(&result)?).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn safe(name: &str) -> Result<()> {
    require(
        !name.is_empty()
            && !name.contains('\\')
            && Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "unsafe compact file path",
    )
}
fn local(name: &str) -> Result<PathBuf> {
    safe(name)?;
    let root = crate::conformance::repo_root()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let path = root.join(name).canonicalize().map_err(|e| e.to_string())?;
    require(path.starts_with(&root), "compact path outside repository")?;
    Ok(path)
}

fn files(root: &Path) -> Result<BTreeMap<String, Value>> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<String, Value>) -> Result<()> {
        require(
            !fs::symlink_metadata(path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "symlink in compact collection",
        )?;
        for e in fs::read_dir(path).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let kind = e.file_type().map_err(|e| e.to_string())?;
            require(!kind.is_symlink(), "symlink in compact collection")?;
            if kind.is_dir() {
                walk(root, &e.path(), out)?;
            } else {
                require(kind.is_file(), "non-file in compact collection")?;
                let path = e.path();
                let name = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("non-UTF8 compact path")?;
                #[cfg(unix)]
                require(!name.contains('\\'), "backslash in compact filename")?;
                let name = name.replace('\\', "/");
                safe(&name)?;
                out.insert(
                    name,
                    descriptor(&fs::read(path).map_err(|e| e.to_string())?),
                );
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn signature(kind: &str, record: &Value, v2: bool) -> Result<Value> {
    let mut value = record.clone();
    let object = value.as_object_mut().ok_or("invalid receipt object")?;
    if kind == "native" {
        require(
            object.remove("renderer_binary").is_some() && object.remove("tool").is_some(),
            "missing native producer identity",
        )?;
    } else if kind == "viewers" {
        require(
            object.remove("sources").is_some() && object.remove("native").is_some(),
            "missing viewer producer identity",
        )?;
        if v2 {
            require(
                object.remove("producer").is_some(),
                "missing v2 viewer producer",
            )?;
        }
        for input in value["inputs"]
            .as_array_mut()
            .ok_or("invalid viewer inputs")?
        {
            let path = text(input, "path")?.to_owned();
            if path.ends_with("/capture.json") {
                *input = json!({"path":path});
            }
        }
        for capture in value["captures"]
            .as_array_mut()
            .ok_or("invalid viewer captures")?
        {
            for run in capture["runs"]
                .as_array_mut()
                .ok_or("invalid viewer repeats")?
            {
                let object = run.as_object_mut().ok_or("invalid viewer run")?;
                for field in [
                    "browser_process_id",
                    "container_namespace",
                    "virtual_time_ticks_base",
                ] {
                    object.remove(field);
                }
            }
        }
    }
    Ok(value)
}

fn inventory(record: &Value, kind: &str) -> Result<BTreeMap<String, Value>> {
    let object = record["inventory"][kind]
        .as_object()
        .ok_or("missing compact inventory")?;
    let mut result = BTreeMap::new();
    for (name, desc) in object {
        safe(name)?;
        fields(desc, &["bytes", "sha256"], "invalid compact descriptor")?;
        require(
            desc["bytes"].as_u64().is_some() && desc["sha256"].as_str().is_some_and(digest),
            "invalid compact descriptor",
        )?;
        result.insert(name.clone(), desc.clone());
    }
    Ok(result)
}

fn descriptor_at(record: &Value, kind: &str, name: &str) -> Result<Value> {
    inventory(record, kind)?
        .get(name)
        .cloned()
        .ok_or_else(|| format!("missing {kind}/{name} inventory"))
}

fn binding(record: &Value, kind: &str, name: &str) -> Result<Value> {
    let mut desc = descriptor_at(record, kind, name)?;
    desc["path"] = json!(format!("{}/{name}", text(&record["roots"], kind)?));
    Ok(desc)
}

fn verify(output: &Path) -> Result<Value> {
    let record = decode(&read(output, "compact.json")?)?;
    fields(
        &record,
        &[
            "schema",
            "status",
            "approval",
            "eligible_executions",
            "case_id",
            "bundle",
            "roots",
            "inventory",
            "retained",
            "signatures",
        ],
        "invalid compact envelope",
    )?;
    let v2 = record["schema"] == SCHEMA;
    require(
        v2 || record["schema"] == LEGACY,
        "unsupported compact schema",
    )?;
    require(
        record["status"] == "pending-review" && record["eligible_executions"] == json!([]),
        "compact evidence cannot grant approval",
    )?;
    let bundle = local(text(&record, "bundle")?)?;
    let manifest = super::verify(&bundle)?;
    require(
        manifest["case_id"] == record["case_id"],
        "compact case differs from intake",
    )?;
    for key in ["roots", "inventory", "retained", "signatures"] {
        fields(
            &record[key],
            &["reference", "native", "viewers"],
            "invalid compact root inventory",
        )?;
    }
    let mut all = BTreeSet::from(["compact.json".to_owned()]);
    let mut receipts = BTreeMap::new();
    for (kind, receipt) in ROOTS {
        safe(text(&record["roots"], kind)?)?;
        let expected = inventory(&record, kind)?;
        let retained = expected
            .keys()
            .filter(|n| n.ends_with(".json"))
            .cloned()
            .collect::<Vec<_>>();
        require(
            record["retained"][kind] == json!(retained),
            "retained list must include exactly all JSON records",
        )?;
        require(
            retained.iter().any(|n| n == receipt),
            "missing retained receipt",
        )?;
        let actual = files(&output.join(kind))?;
        require(
            actual.keys().cloned().collect::<Vec<_>>() == retained,
            &format!("{kind} retained inventory changed"),
        )?;
        for (name, desc) in actual {
            require(
                expected.get(&name) == Some(&desc),
                &format!("retained {kind}/{name} changed"),
            )?;
            all.insert(format!("{kind}/{name}"));
        }
        let value = decode(&read(&output.join(kind), receipt)?)?;
        let schema = text(&value, "schema")?;
        require(
            match kind {
                "reference" => schema == "excalidraw-render/representative-intake-reference/v1",
                "native" => matches!(
                    schema,
                    "excalidraw-render/representative-intake-capture/v1"
                        | "excalidraw-render/representative-intake-capture/v2"
                ),
                "viewers" => {
                    schema
                        == if v2 {
                            "excalidraw-render/representative-intake-viewers/v2"
                        } else {
                            "excalidraw-render/representative-intake-viewers/v1"
                        }
                }
                _ => false,
            },
            "unsupported retained receipt schema",
        )?;
        require(
            value["eligible_executions"] == json!([]),
            "retained receipts cannot grant approval",
        )?;
        require(
            signature(kind, &value, v2)? == record["signatures"][kind],
            &format!("{kind} receipt signature changed"),
        )?;
        receipts.insert(kind, value);
    }
    if v2 {
        require(
            files(output)?.keys().cloned().collect::<BTreeSet<_>>() == all,
            "compact file inventory changed",
        )?;
    }
    let native = &receipts["native"];
    let viewers = &receipts["viewers"];
    let reference = &receipts["reference"];
    let mut reference_files = BTreeMap::from([(
        "reference.json".to_owned(),
        descriptor_at(&record, "reference", "reference.json")?,
    )]);
    let mut variants = BTreeSet::new();
    for row in array(reference, "rows")? {
        let variant = text(row, "variant")?;
        require(
            valid_id(variant) && variants.insert(variant),
            "invalid or duplicate reference variant",
        )?;
        fields(
            &row["files"],
            &["official.svg", "official.png", "official-png.rgba.zlib"],
            "invalid reference artifact inventory",
        )?;
        for (name, binding) in row["files"].as_object().ok_or("reference file inventory")? {
            reference_files.insert(format!("{variant}/{name}"), binding.clone());
        }
    }
    require(
        json!(reference_files) == record["inventory"]["reference"],
        "reference inventory detached from receipt",
    )?;
    require(
        native["case_id"] == record["case_id"]
            && native["intake_manifest"] == descriptor(&read(&bundle, "manifest.json")?),
        "native receipt detached from intake",
    )?;
    let mut native_files = inventory(&record, "native")?;
    native_files.remove("capture.json");
    require(
        native["files"] == json!(native_files),
        "native inventory detached from receipt",
    )?;
    let mut viewer_files = inventory(&record, "viewers")?;
    viewer_files.remove("viewers.json");
    require(
        viewers["files"] == json!(viewer_files),
        "viewer inventory detached from its record",
    )?;
    let mut inputs = ["manifest.json", "plan.json", "scene.excalidraw"]
        .iter()
        .map(|n| evidence::bind(&bundle.join(n)))
        .collect::<Result<Vec<_>>>()?;
    inputs.push(binding(&record, "reference", "reference.json")?);
    inputs.push(binding(&record, "native", "capture.json")?);
    require(
        viewers["inputs"] == json!(inputs)
            && viewers["reference"] == inputs[3]
            && viewers["native"] == inputs[4],
        "evidence detached from input receipts",
    )?;
    require(
        reference["case_id"] == record["case_id"]
            && reference["scene_sha256"] == sha256(&read(&bundle, "scene.excalidraw")?)
            && reference["plan_sha256"] == sha256(&read(&bundle, "plan.json")?),
        "reference detached from intake",
    )?;
    for judgment in array(viewers, "judgments")? {
        let path = text(&judgment["report"], "path")?;
        let name = Path::new(path)
            .strip_prefix(text(&record["roots"], "viewers")?)
            .map_err(|_| "judgment outside viewer root")?
            .to_str()
            .ok_or("judgment path")?;
        safe(name)?;
        require(
            judgment["report"] == binding(&record, "viewers", name)?
                && decode(&read(&output.join("viewers"), name)?)? == judgment["result"],
            "judgment detached from retained report",
        )?;
    }
    Ok(record)
}

fn retain(bundle: &Path, roots: [&Path; 3], output: &Path) -> Result<Value> {
    evidence::verify_evidence(bundle, roots[0], roots[1], roots[2])?;
    let mut inventories = BTreeMap::new();
    let mut retained = BTreeMap::new();
    let mut signatures = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut contents = Files::new();
    for ((kind, receipt), directory) in ROOTS.into_iter().zip(roots) {
        let inventory = files(directory)?;
        let json_files = inventory
            .keys()
            .filter(|n| n.ends_with(".json"))
            .cloned()
            .collect::<Vec<_>>();
        for name in &json_files {
            let bytes = read(directory, name)?;
            require(
                inventory[name] == descriptor(&bytes),
                "collection changed during compact retention",
            )?;
            contents.insert(format!("{kind}/{name}"), bytes);
        }
        let value = decode(
            contents
                .get(&format!("{kind}/{receipt}"))
                .ok_or("missing receipt")?,
        )?;
        signatures.insert(kind, signature(kind, &value, true)?);
        retained.insert(kind, json_files);
        inventories.insert(kind, inventory);
        names.insert(kind, evidence::relative(directory)?);
    }
    let record = json!({"schema":SCHEMA,"status":"pending-review","approval":null,"eligible_executions":[],"case_id":super::verify(bundle)?["case_id"],
        "bundle":evidence::relative(bundle)?,"roots":names,"inventory":inventories,"retained":retained,"signatures":signatures});
    fs::create_dir(output).map_err(|e| format!("cannot create compact output: {e}"))?;
    let result: Result<()> = (|| {
        for (name, bytes) in contents {
            let path = output.join(name);
            fs::create_dir_all(path.parent().ok_or("file parent")?).map_err(|e| e.to_string())?;
            evidence::write_new(&path, &bytes)?;
        }
        evidence::write_new(&output.join("compact.json"), &encode(&record)?)?;
        verify(output)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result?;
    Ok(
        json!({"status":"retained-compact-observations","case_id":record["case_id"],"approval":null}),
    )
}

fn check(output: &Path) -> Result<Value> {
    let record = verify(output)?;
    require(
        record["schema"] == SCHEMA,
        "legacy v1 compact records support offline verify; fresh checks require a v2 collection",
    )?;
    let bundle = local(text(&record, "bundle")?)?;
    let roots = ROOTS
        .iter()
        .map(|(kind, _)| local(text(&record["roots"], kind)?))
        .collect::<Result<Vec<_>>>()?;
    evidence::verify_evidence(&bundle, &roots[0], &roots[1], &roots[2])?;
    for ((kind, receipt), directory) in ROOTS.into_iter().zip(&roots) {
        let mut fresh = files(directory)?;
        fresh.remove(receipt);
        let mut expected = inventory(&record, kind)?;
        expected.remove(receipt);
        require(
            fresh == expected,
            &format!("fresh {kind} collection differs"),
        )?;
        require(
            signature(kind, &decode(&read(directory, receipt)?)?, true)?
                == record["signatures"][kind],
            &format!("fresh {kind} receipt differs"),
        )?;
    }
    Ok(json!({"status":"fresh-collection-repeats-compact-record","approval":null}))
}
