//! Font container operations for representative viewer assets (#84).
//! Uses the renderer's pinned decoder, encoder and face parser so derived
//! viewer resources match the representative viewer adapter's
//! (`stage4-viewer-assets.mjs`) single-face output exactly.
use std::io::Read;

use serde_json::json;
use sha2::{Digest, Sha256};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["decode", woff2, output] => {
            let sfnt = wuff::decompress_woff2(&read(woff2)?).map_err(|e| format!("WOFF2 decode: {e:?}"))?;
            let face = face_info(&sfnt)?;
            write_new(output, &sfnt)?;
            face
        }
        ["encode", sfnt, output] => {
            let sfnt = read(sfnt)?;
            face_info(&sfnt)?;
            // Same options as the renderer's encode stage (spec 8.3.3).
            let woff2 = ttf2woff2::encode_with_options(
                &sfnt,
                ttf2woff2::EncodeOptions {
                    quality: ttf2woff2::BrotliQuality::from(11),
                    transform_glyf_loca: true,
                    threads: std::num::NonZeroUsize::new(1),
                },
            )
            .map_err(|e| format!("WOFF2 encode: {e:?}"))?;
            write_new(output, &woff2)?;
            json!({"bytes": woff2.len(), "sha256": sha256(&woff2)})
        }
        ["covers", sfnt] => {
            let sfnt = read(sfnt)?;
            let face = rustybuzz::Face::from_slice(&sfnt, 0).ok_or("invalid SFNT")?;
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
            let missing: Vec<String> = text
                .chars()
                .filter(|ch| face.glyph_index(*ch).is_none())
                .map(|ch| ch.to_string())
                .collect();
            json!({"missing": missing})
        }
        _ => return Err("usage: decode <woff2> <new.sfnt> | encode <sfnt> <new.woff2> | covers <sfnt> < text".into()),
    };
    println!("{value}");
    Ok(())
}

fn face_info(sfnt: &[u8]) -> Result<serde_json::Value, String> {
    let face = rustybuzz::Face::from_slice(sfnt, 0).ok_or("invalid decoded SFNT")?;
    let family = [16, 1]
        .into_iter()
        .find_map(|id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == id)
                .find_map(|n| n.to_string())
        })
        .ok_or("missing font family")?;
    let stretch = match face.width().to_number() {
        1 => "ultra-condensed",
        2 => "extra-condensed",
        3 => "condensed",
        4 => "semi-condensed",
        5 => "normal",
        6 => "semi-expanded",
        7 => "expanded",
        8 => "extra-expanded",
        9 => "ultra-expanded",
        _ => return Err("invalid font width".into()),
    };
    Ok(json!({
        "media_type": if sfnt.starts_with(b"OTTO") { "font/otf" } else { "font/ttf" },
        "sha256": sha256(sfnt),
        "face_index": 0,
        "family": family,
        "style": if face.is_italic() { "italic" } else { "normal" },
        "weight": face.weight().to_number(),
        "stretch": stretch,
    }))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

fn write_new(path: &str, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{path}: {e}"))?;
    file.write_all(bytes).map_err(|e| format!("{path}: {e}"))
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
