use super::*;

fn occurrences(bytes: &[u8], needle: &[u8]) -> usize {
    bytes.windows(needle.len()).filter(|w| *w == needle).count()
}

fn notices(artifact: &[u8]) -> Result<Value> {
    fn collect(bytes: &[u8], identities: &mut BTreeSet<String>) -> Result<()> {
        let marker = b"\"asset_identity\":\"";
        for (start, _) in bytes
            .windows(marker.len())
            .enumerate()
            .filter(|(_, w)| *w == marker)
        {
            let rest = &bytes[start + marker.len()..];
            if let Some(end) = rest.iter().position(|b| *b == b'"') {
                let id = &rest[..end];
                if id.starts_with(b"font.") || id.starts_with(b"license.") {
                    identities.insert(
                        std::str::from_utf8(id)
                            .map_err(|e| e.to_string())?
                            .to_owned(),
                    );
                }
            }
        }
        Ok(())
    }
    let mut identities = BTreeSet::new();
    collect(artifact, &mut identities)?;
    if artifact.starts_with(b"%PDF-") {
        for (offset, _) in artifact
            .windows(6)
            .enumerate()
            .filter(|(_, w)| *w == b"stream")
        {
            let rest = &artifact[offset + 6..];
            let rest = rest
                .strip_prefix(b"\r\n")
                .or_else(|| rest.strip_prefix(b"\n"));
            if let Some(rest) = rest {
                if let Some(end) = rest.windows(9).position(|w| w == b"endstream") {
                    // This inspects metadata in renderer-produced PDFs, not a
                    // general PDF parser. Bound inflation for malformed input.
                    if let Ok(bytes) = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                        &rest[..end],
                        64 * 1024 * 1024,
                    ) {
                        collect(&bytes, &mut identities)?;
                    }
                }
            }
        }
    }
    Ok(json!(identities))
}

pub(super) fn verify(artifact: &[u8], format: &str, expected: &Value) -> Result<()> {
    match format {
        "svg" => {
            let text = std::str::from_utf8(artifact).map_err(|e| e.to_string())?;
            let doc = roxmltree::Document::parse(text).map_err(|e| format!("invalid SVG: {e}"))?;
            let root = doc.root_element();
            require(root.tag_name().name() == "svg", "invalid SVG root")?;
            require(
                json!([root.attribute("width"), root.attribute("height")]) == expected["svg_size"],
                "SVG size differs",
            )?;
            require(
                json!(occurrences(artifact, b"<!-- payload-start -->") > 0)
                    == expected["embedded_scene"],
                "embedded scene differs",
            )?;
            let minimum = expected["min_text_elements"]
                .as_u64()
                .ok_or("invalid minimum text count")?;
            require(
                occurrences(artifact, b"<text") as u64 >= minimum,
                "selectable text differs",
            )?;
            require(
                json!(occurrences(artifact, b"<a ")) == expected["links"],
                "links differ",
            )?;
            require(
                json!(root.attribute("filter") == Some("invert(93%) hue-rotate(180deg)"))
                    == expected["root_theme_filter"],
                "root theme filter differs",
            )?;
            require(
                notices(artifact)? == expected["notices"],
                "mandatory notices differ",
            )?;
        }
        "png" => {
            require(
                artifact.starts_with(b"\x89PNG\r\n\x1a\n")
                    && artifact.len() >= 24
                    && &artifact[12..16] == b"IHDR",
                "invalid PNG header",
            )?;
            let dimension = |range: std::ops::Range<usize>| {
                u32::from_be_bytes(
                    artifact[range]
                        .try_into()
                        .expect("checked PNG header length"),
                )
            };
            require(
                json!([dimension(16..20), dimension(20..24)]) == expected["png_size"],
                "PNG size differs",
            )?;
        }
        "pdf" => {
            require(artifact.starts_with(b"%PDF-"), "invalid PDF header")?;
            require(
                notices(artifact)? == expected["notices"],
                "mandatory notices differ",
            )?;
            let marker = b"/MediaBox [0 0 ";
            let start = artifact
                .windows(marker.len())
                .position(|w| w == marker)
                .ok_or("missing PDF MediaBox")?
                + marker.len();
            let end = artifact[start..]
                .iter()
                .position(|b| *b == b']')
                .ok_or("invalid PDF MediaBox")?
                + start;
            let dimensions = std::str::from_utf8(&artifact[start..end])
                .map_err(|e| e.to_string())?
                .split_whitespace()
                .collect::<Vec<_>>();
            require(
                json!(dimensions) == expected["pdf_media_box"],
                "PDF MediaBox differs",
            )?;
        }
        _ => return Err("unknown semantic format".into()),
    }
    Ok(())
}
