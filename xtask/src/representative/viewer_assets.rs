//! SVG viewer resource derivation. Font codec operations use the existing Rust
//! helper; browser font resolution remains in the pinned JavaScript harness.
use super::*;
use crate::cli::ViewerAssetsCommand;
use base64::Engine;
use regex::Regex;
use std::path::PathBuf;

const RESOLUTION: &str = "official-font-resolution.json";
const SUBSTITUTED: &str = "official-substituted.svg";

fn regex(pattern: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|e| e.to_string())
}
fn unquote(s: &str) -> &str {
    s.trim().trim_matches(['\'', '"'])
}
fn field(css: &str, name: &str) -> Result<Option<String>> {
    Ok(
        regex(&format!(r"(?i){}\s*:\s*([^;]+)", regex::escape(name)))?
            .captures(css)
            .map(|c| c[1].trim().to_owned()),
    )
}
fn texts(svg: &str) -> Result<Vec<(String, String)>> {
    let family = regex(r#"\bfont-family="([^"]*)""#)?;
    let tags = regex(r"<[^>]*>")?;
    regex(r"(?s)<text\b([^>]*)>(.*?)</text>")?
        .captures_iter(svg)
        .map(|c| {
            let f = family
                .captures(&c[1])
                .ok_or("text without explicit font-family")?[1]
                .to_owned();
            Ok((f, tags.replace_all(&c[2], "").into_owned()))
        })
        .collect()
}
fn primary(s: &str) -> &str {
    unquote(s.split(',').next().unwrap_or(s))
}
fn validate_substitute_family(family: &str) -> Result<()> {
    // Keep the legacy unquoted spelling for supported names. CSS escapes and
    // XML entities are deliberately not admitted at this injection boundary.
    require(
        !family.is_empty()
            && family.split(' ').all(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
            })
            && !matches!(
                family.to_ascii_lowercase().as_str(),
                "inherit" | "initial" | "unset" | "revert" | "revert-layer" | "default"
            ),
        "unsupported substitute font family: expected a simple CSS family name",
    )
}
fn missing(svg: &str) -> Result<BTreeSet<String>> {
    let embedded = regex(r"@font-face\s*\{([^}]*)\}")?
        .captures_iter(svg)
        .map(|c| {
            field(&c[1], "font-family")?
                .map(|s| unquote(&s).to_owned())
                .ok_or("font face without family".into())
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let missing: BTreeSet<_> = texts(svg)?
        .iter()
        .map(|(f, _)| primary(f).to_owned())
        .filter(|f| !embedded.contains(f))
        .collect();
    for family in &missing {
        validate_substitute_family(family)?;
    }
    Ok(missing)
}

fn substitutions(resolution: &Value, svg: &str) -> Result<Vec<Value>> {
    require(
        resolution["schema"] == "excalidraw-render/representative-font-resolution/v1",
        "invalid font resolution record",
    )?;
    require(
        resolution["input"]["sha256"] == sha256(svg.as_bytes()),
        "font resolution detached from official SVG",
    )?;
    let missing = missing(svg)?;
    let mut entries = array(resolution, "undeclared")?
        .iter()
        .filter(|e| {
            e["requested_family"]
                .as_str()
                .is_some_and(|f| missing.contains(f))
        })
        .cloned()
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        a["requested_family"]
            .as_str()
            .cmp(&b["requested_family"].as_str())
    });
    require(
        entries
            .iter()
            .map(|e| text(e, "requested_family"))
            .collect::<Result<Vec<_>>>()?
            == missing.iter().map(String::as_str).collect::<Vec<_>>(),
        "every undeclared family needs one observed substitute",
    )?;
    let pinned = decode(&read(
        &crate::conformance::repo_root(),
        "tests/oracle/reference-profiles/revision-1-assets.json",
    )?)?;
    let assets = array(&pinned["systemFonts"], "assets")?;
    for e in &entries {
        require(
            e["agrees"] == true
                && e["chromium"]["postScriptName"].is_string()
                && e["chromium"]["postScriptName"] == e["fontconfig"]["postscript_name"],
            "Chromium and fontconfig disagree",
        )?;
        require(
            e["system_font"]["path"] == e["fontconfig"]["file"]
                && assets.contains(&e["system_font"]),
            "substitute is not a pinned fontconfig match",
        )?;
    }
    Ok(entries)
}

fn font_tool(args: &[String]) -> Result<Value> {
    let path = crate::conformance::repo_root()
        .join("target/representative-viewer-fonts/release/representative-intake-viewer-fonts");
    decode(&evidence::command(
        path.to_str().ok_or("font helper path")?,
        args,
        120,
    )?)
}
fn build_font_tool() -> Result<()> {
    evidence::command(
        "cargo",
        &[
            "build",
            "--locked",
            "--release",
            "--quiet",
            "--manifest-path",
            "tools/representative-intake/viewer-fonts/Cargo.toml",
            "--target-dir",
            "target/representative-viewer-fonts",
        ]
        .map(str::to_owned),
        600,
    )?;
    Ok(())
}

fn observe(directory: &Path) -> Result<()> {
    let svg = String::from_utf8(read(directory, "official.svg")?).map_err(|e| e.to_string())?;
    if missing(&svg)?.is_empty() {
        return Ok(());
    }
    let resolution = directory.join(RESOLUTION);
    let mut args = evidence::docker_args()?;
    args.extend([
        "node".into(),
        "../representative-intake/font-resolution.mjs".into(),
        evidence::relative(&directory.join("official.svg"))?,
        format!("{}/{}", evidence::relative(directory)?, RESOLUTION),
    ]);
    evidence::command("docker", &args, 600)?;
    let value = decode(&fs::read(resolution).map_err(|e| e.to_string())?)?;
    for (i, entry) in substitutions(&value, &svg)?.iter().enumerate() {
        let path = text(&entry["system_font"], "path")?;
        let extension = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .ok_or("font extension")?;
        let bytes = evidence::command(
            "docker",
            &[
                "run",
                "--rm",
                "--network",
                "none",
                "--platform",
                "linux/amd64",
                evidence::IMAGE,
                "cat",
                path,
            ]
            .map(str::to_owned),
            300,
        )?;
        require(
            descriptor(&bytes)
                == json!({"bytes":entry["system_font"]["bytes"],"sha256":entry["system_font"]["sha256"]}),
            "system font differs from profile",
        )?;
        evidence::write_new(
            &directory.join(format!("assets/official-substitute-{i}.{extension}")),
            &bytes,
        )?;
    }
    Ok(())
}

fn text_decode(text: &str) -> String {
    // Only scan source ampersands: decoded text must never be decoded again.
    let mut parts = text.split('&');
    let mut decoded = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        if let Some((replacement, rest)) = [
            ("amp;", '&'),
            ("lt;", '<'),
            ("gt;", '>'),
            ("quot;", '"'),
            ("apos;", '\''),
        ]
        .iter()
        .find_map(|(entity, ch)| part.strip_prefix(entity).map(|rest| (*ch, rest)))
        {
            decoded.push(replacement);
            decoded.push_str(rest);
        } else {
            decoded.push('&');
            decoded.push_str(part);
        }
    }
    decoded
}

fn percent_decode(s: &str) -> Vec<u8> {
    let hex = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    };
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                result.push(high * 16 + low);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    result
}

fn unicode_ranges(css: &Value) -> Result<Vec<std::ops::RangeInclusive<u32>>> {
    let hex = |value: &str| -> Result<u32> {
        require(
            !value.is_empty() && value.len() <= 6 && value.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid font unicode-range",
        )?;
        u32::from_str_radix(value, 16).map_err(|e| e.to_string())
    };
    let mut ranges = Vec::new();
    for value in array(css, "unicode_range")? {
        let token = value
            .as_str()
            .ok_or("invalid font unicode-range")?
            .to_ascii_uppercase();
        let digits = token
            .strip_prefix("U+")
            .ok_or("invalid font unicode-range")?;
        let (start, end) = if let Some((start, end)) = digits.split_once('-') {
            (hex(start)?, hex(end)?)
        } else if let Some(question) = digits.find('?') {
            let (prefix, suffix) = digits.split_at(question);
            // Match the pinned viewer's admitted wildcard syntax.
            require(
                !prefix.is_empty() && suffix.len() <= 5 && suffix.bytes().all(|b| b == b'?'),
                "invalid font unicode-range",
            )?;
            (
                hex(&digits.replace('?', "0"))?,
                hex(&digits.replace('?', "F"))?,
            )
        } else {
            let point = hex(digits)?;
            (point, point)
        };
        require(
            start <= end && end <= 0x10ffff,
            "font unicode-range is outside Unicode",
        )?;
        ranges.push(start..=end);
    }
    ranges.sort_unstable_by_key(|range| *range.start());
    require(
        !ranges.is_empty()
            && ranges
                .windows(2)
                .all(|pair| pair[0].end() < pair[1].start()),
        "empty or overlapping font unicode-ranges",
    )?;
    Ok(ranges)
}

struct CoverageFace<'a> {
    face: rustybuzz::Face<'a>,
    ranges: Vec<std::ops::RangeInclusive<u32>>,
}

fn derive(directory: &Path) -> Result<Files> {
    build_font_tool()?;
    let name = evidence::relative(directory)?;
    let mut outputs = Files::new();
    let mut plans = Vec::new();
    let scratch = capture::scratch()?;
    let face_pattern = regex(r"@font-face\s*\{([^}]*)\}")?;
    let url_pattern = regex(r"url\(([^)]+)\)")?;
    let hint = regex(
        r"(@font-face\s*\{[^}]*?src\s*:\s*url\(data:font/woff2;base64,[A-Za-z0-9+/=]+\))(\s*[;}])",
    )?;
    for kind in ["official", "native"] {
        let svg = String::from_utf8(read(directory, &format!("{kind}.svg"))?)
            .map_err(|e| e.to_string())?;
        let mut fonts = Vec::new();
        let mut images = Vec::new();
        let mut substituted = svg.clone();
        for (i, c) in face_pattern.captures_iter(&svg).enumerate() {
            let css = &c[1];
            let url = url_pattern.captures(css).ok_or("font face has no URL")?;
            let url = unquote(&url[1]);
            let data = base64::engine::general_purpose::STANDARD
                .decode(
                    url.strip_prefix("data:font/woff2;base64,")
                        .ok_or("undeclared font encoding")?,
                )
                .map_err(|e| e.to_string())?;
            let file = format!("assets/{kind}-font-{i}.woff2");
            let weight = field(css, "font-weight")?.unwrap_or("400".into());
            let weight: u32 = match weight.as_str() {
                "normal" => 400,
                "bold" => 700,
                _ => weight.parse().map_err(|_| "invalid font weight")?,
            };
            let family = field(css, "font-family")?.ok_or("font face without family")?;
            fonts.push(json!({"woff2":format!("{name}/{file}"),"source":{"media_type":"font/woff2","sha256":sha256(&data)},
                "css":{"family":unquote(&family),"style":field(css,"font-style")?.unwrap_or("normal".into()),"weight":weight,"stretch":field(css,"font-stretch")?.unwrap_or("normal".into()),
                "unicode_range":field(css,"unicode-range")?.unwrap_or("U+0-10FFFF".into()).split(',').map(str::trim).collect::<Vec<_>>()}}));
            outputs.insert(file, data);
        }
        if kind == "official" && !missing(&svg)?.is_empty() {
            let resolution_bytes = read(directory, RESOLUTION)?;
            let resolution = decode(&resolution_bytes)?;
            let mut rules = Vec::new();
            for (i, e) in substitutions(&resolution, &svg)?.iter().enumerate() {
                let extension = Path::new(text(&e["system_font"], "path")?)
                    .extension()
                    .and_then(|s| s.to_str())
                    .ok_or("font extension")?;
                let retained = format!("assets/official-substitute-{i}.{extension}");
                let bytes = read(directory, &retained)?;
                require(
                    descriptor(&bytes)
                        == json!({"bytes":e["system_font"]["bytes"],"sha256":e["system_font"]["sha256"]}),
                    "retained substitute changed",
                )?;
                let woff2 = scratch.path().join(format!("substitute-{i}.woff2"));
                font_tool(&[
                    "encode".into(),
                    directory.join(&retained).to_string_lossy().into_owned(),
                    woff2.to_string_lossy().into_owned(),
                ])?;
                let data = fs::read(&woff2).map_err(|e| e.to_string())?;
                let sfnt = scratch.path().join(format!("substitute-{i}.sfnt"));
                let face = font_tool(&[
                    "decode".into(),
                    woff2.to_string_lossy().into_owned(),
                    sfnt.to_string_lossy().into_owned(),
                ])?;
                let file = format!("assets/official-font-{}.woff2", fonts.len());
                let family = text(e, "requested_family")?;
                fonts.push(json!({"woff2":format!("{name}/{file}"),"source":{"media_type":"font/woff2","sha256":sha256(&data)},"css":{"family":family,"style":face["style"],"weight":face["weight"],"stretch":face["stretch"],"unicode_range":["U+0-10FFFF"]},
                    "substitution":{"requested_family":family,"system_font":e["system_font"],"chromium":e["chromium"],"retained":format!("{name}/{retained}"),"resolution":{"path":format!("{name}/{RESOLUTION}"),"bytes":resolution_bytes.len(),"sha256":sha256(&resolution_bytes)}}}));
                rules.push(format!("\n      @font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{}) format('woff2'); }}",base64::engine::general_purpose::STANDARD.encode(&data)));
                outputs.insert(file, data);
            }
            let closing = svg
                .find("</style>")
                .ok_or("official SVG has no font style element for substitute")?;
            substituted = format!("{}{}{}", &svg[..closing], rules.join(""), &svg[closing..]);
            outputs.insert(SUBSTITUTED.into(), substituted.as_bytes().to_vec());
        }
        let view = hint
            .replace_all(&substituted, "${1} format('woff2')${2}")
            .into_owned();
        for (i, c) in regex(r#"<image\b[^>]*\b(?:href|xlink:href)="([^"]+)"[^>]*>"#)?
            .captures_iter(&svg)
            .enumerate()
        {
            let (meta, payload) = c[1].split_once(',').ok_or("invalid image data URL")?;
            let media = meta
                .strip_prefix("data:")
                .ok_or("external image resource")?
                .split(';')
                .next()
                .ok_or("image media type")?;
            let data = if meta.ends_with(";base64") {
                base64::engine::general_purpose::STANDARD
                    .decode(payload)
                    .map_err(|e| e.to_string())?
            } else {
                percent_decode(payload)
            };
            let file = format!(
                "assets/{kind}-image-{i}.{}",
                if media == "image/svg+xml" {
                    "svg"
                } else {
                    "png"
                }
            );
            images.push(json!({"source":{"media_type":media,"sha256":sha256(&data)},"canonical":{"path":format!("{name}/{file}"),"media_type":media,"sha256":sha256(&data)}}));
            outputs.insert(file, data);
        }
        outputs.insert(format!("{kind}-view.svg"), view.as_bytes().to_vec());
        let mut declared = Vec::new();
        for (i, font) in fonts.iter().enumerate() {
            let file = format!("assets/{kind}-font-{i}.woff2");
            let input = scratch.path().join(format!("{kind}-{i}.woff2"));
            let output = scratch.path().join(format!("{kind}-{i}.sfnt"));
            evidence::write_new(&input, outputs.get(&file).ok_or("missing derived WOFF2")?)?;
            let mut face = font_tool(&[
                "decode".into(),
                input.to_string_lossy().into_owned(),
                output.to_string_lossy().into_owned(),
            ])?;
            let data = fs::read(output).map_err(|e| e.to_string())?;
            let sfnt_path = format!("assets/{kind}-font-{i}.sfnt");
            face["path"] = json!(format!("{name}/{sfnt_path}"));
            declared.push(json!({"source":font["source"],"css":font["css"],"sfnt":face}));
            outputs.insert(sfnt_path, data);
        }
        let mut decoded = BTreeMap::new();
        for (i, font) in fonts.iter().enumerate() {
            let css = &font["css"];
            let family = text(css, "family")?;
            // The pinned Rust viewer's fallback resolver does not enforce
            // family or Unicode-range selection across complementary faces.
            require(
                !decoded.contains_key(family),
                "unsupported same-family multiple faces: Rust viewer requires one face per family until its resolver is migrated",
            )?;
            let sfnt = outputs
                .get(&format!("assets/{kind}-font-{i}.sfnt"))
                .ok_or("missing decoded SFNT")?;
            decoded.insert(
                family,
                CoverageFace {
                    face: rustybuzz::Face::from_slice(sfnt, 0).ok_or("invalid SFNT")?,
                    ranges: unicode_ranges(css)?,
                },
            );
        }
        let mut rust = view.clone();
        for (full, text) in texts(&view)? {
            let family = primary(&full);
            let face = decoded
                .get(family)
                .ok_or_else(|| format!("text family {family:?} has no declared face"))?;
            require(
                text_decode(&text).chars().all(|ch| {
                    face.ranges.iter().any(|range| range.contains(&(ch as u32)))
                        && face.face.glyph_index(ch).is_some()
                }),
                "text face does not cover every character",
            )?;
            rust = rust.replace(
                &format!("font-family=\"{full}\""),
                &format!("font-family=\"{family}\""),
            );
        }
        drop(decoded);
        outputs.insert(format!("{kind}-rust.svg"), rust.into_bytes());
        outputs.insert(format!("{kind}-assets.json"),serde_json::to_vec_pretty(&json!({"schema":"excalidraw-render/svg-viewer-asset-declaration/v1","fonts":declared,"images":images})).map_err(|e|e.to_string())?);
        plans.push(json!({"kind":kind,"fonts":fonts,"images":images}));
    }
    // This plan is descriptive; v2 uses sorted tool JSON rather than JS key order.
    outputs.insert("viewer-assets-plan.json".into(), encode(&json!(plans))?);
    Ok(outputs)
}

pub(super) fn collect(directory: &Path) -> Result<()> {
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    evidence::relative(&directory)?;
    fs::create_dir(directory.join("assets")).map_err(|e| e.to_string())?;
    observe(&directory)?;
    for (name, bytes) in derive(&directory)? {
        evidence::write_new(&directory.join(name), &bytes)?;
    }
    Ok(())
}

pub(super) fn verify_assets(directory: &Path) -> Result<()> {
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    evidence::relative(&directory)?;
    for (name, bytes) in derive(&directory)? {
        require(
            read(&directory, &name)? == bytes,
            &format!("derived viewer resource differs: {name}"),
        )?;
    }
    Ok(())
}

pub(super) fn chromium_input(directory: &Path, kind: &str) -> PathBuf {
    if kind == "official" && directory.join(SUBSTITUTED).is_file() {
        directory.join(SUBSTITUTED)
    } else {
        directory.join(format!("{kind}.svg"))
    }
}

pub(super) fn run(command: ViewerAssetsCommand) -> Result<()> {
    match command {
        ViewerAssetsCommand::Collect { directory } => collect(&directory)?,
        ViewerAssetsCommand::Verify { directory } => verify_assets(&directory)?,
    }
    println!(
        "{}",
        json!({"status":"verified-viewer-assets","approval":null})
    );
    Ok(())
}
