//! Authored PNG/APNG axes expanded into concrete ownership obligations. These
//! are qualification requirements, not observations of runtime acceptance.
use super::*;

pub(super) const PATH: &str = "tests/conformance/coverage/png-profiles.json";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: String,
    sample_scope: String,
    sample_layouts: Vec<Layout>,
    interlace: Vec<String>,
    containers: Vec<String>,
    row_filters: Vec<u8>,
    cicp: Cicp,
    orientation: Orientation,
    qualification_rows: Vec<Qualification>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    id: String,
    criterion: String,
    spec_anchor: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    name: String,
    color_type: u8,
    depth: u8,
    transparency: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cicp {
    primaries: Vec<u8>,
    transfers: Vec<u8>,
    ranges: Vec<u8>,
    depths: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Orientation {
    values: Vec<u8>,
    byte_orders: Vec<String>,
    positions: Vec<String>,
}

pub(super) fn rows(bytes: &[u8]) -> Result<Vec<PolicyRow>, String> {
    let inventory: Inventory = parse(bytes)?;
    if inventory.schema != "excalidraw-render/png-profile-inventory/v1"
        || inventory.sample_scope != "untagged-8-bit-lattice"
        || inventory.interlace != ["none", "adam7"]
        || inventory.containers != ["static", "apng-included-default", "apng-separate-default"]
        || inventory.row_filters != [0, 1, 2, 3, 4]
        || inventory.cicp.primaries != [1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 22]
        || inventory.cicp.transfers != [1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18]
        || inventory.cicp.ranges != [0, 1]
        || inventory.cicp.depths != [8, 16]
        || inventory.orientation.values != [1, 2, 3, 4, 5, 6, 7, 8]
        || inventory.orientation.byte_orders != ["little", "big"]
        || inventory.orientation.positions != ["before-idat", "after-idat"]
    {
        return Err("invalid or incomplete revision-1 PNG inventory axes".into());
    }
    let expected: BTreeSet<_> = [
        (0, vec![1, 2, 4, 8, 16]),
        (2, vec![8, 16]),
        (3, vec![1, 2, 4, 8]),
        (4, vec![8, 16]),
        (6, vec![8, 16]),
    ]
    .into_iter()
    .flat_map(|(c, depths)| depths.into_iter().map(move |d| (c, d)))
    .collect();
    let actual: BTreeSet<_> = inventory
        .sample_layouts
        .iter()
        .map(|l| (l.color_type, l.depth))
        .collect();
    if inventory.sample_layouts.len() != expected.len() || actual != expected {
        return Err("PNG sample layout matrix is incomplete or duplicated".into());
    }
    let mut rows = Vec::new();
    let mut push = |id: String, criterion: String, anchor: &str| {
        rows.push(PolicyRow {
            id,
            criterion,
            category: "codec-profile".into(),
            spec_anchor: anchor.into(),
        })
    };
    for layout in &inventory.sample_layouts {
        let (name, transparency): (&str, &[&str]) = match layout.color_type {
            0 => ("gray", &["none", "key"]),
            2 => ("rgb", &["none", "key"]),
            3 => ("indexed", &["none", "palette-prefix", "palette-full"]),
            4 => ("gray-alpha", &["intrinsic"]),
            6 => ("rgba", &["intrinsic"]),
            _ => unreachable!(),
        };
        if layout.name != format!("{name}{}", layout.depth) || layout.transparency != transparency {
            return Err("invalid PNG layout name or transparency domain".into());
        }
        for container in &inventory.containers {
            for alpha in &layout.transparency {
                for interlace in &inventory.interlace {
                    push(format!("codec:png:sample:{container}:{}:{alpha}:{interlace}",layout.name),
                format!("PNG/APNG {container}, color type {}, depth {}, {alpha}, {interlace}: untagged 8-bit-lattice sample expansion across all five row filters and tiny/odd pass dimensions; byte-preserving SVG, exact independent PNG/PDF pixels and alpha, default-frame selection, complete diagnostics and fresh-session repeatability. Non-lattice 16-bit precision is a separate obligation.",layout.color_type,layout.depth),"### 8.2 Raster and Nested-SVG Assets");
                }
            }
        }
    }
    for depth in &inventory.cicp.depths {
        for primary in &inventory.cicp.primaries {
            for transfer in &inventory.cicp.transfers {
                for range in &inventory.cicp.ranges {
                    push(format!("codec:png:cicp:rgba{depth}:p{primary}:t{transfer}:r{range}"),format!("PNG RGBA{depth} cICP [{primary},{transfer},0,{range}]: characterize actual admission, canonical-reference color/tone interpretation, alpha and transparent-zero behavior, SVG byte preservation and exact PNG/PDF recovery if unsupported. Recognized header values alone do not establish qualification."),"profile/chunk/CICP");
                }
            }
        }
    }
    for value in &inventory.orientation.values {
        for order in &inventory.orientation.byte_orders {
            for position in &inventory.orientation.positions {
                push(format!("codec:png:exif:o{value}:{order}:{position}"),format!("PNG eXIf orientation {value}, {order}-endian TIFF, {position}: independent non-square pixel/extent vectors for PNG/PDF, untouched source octets/dimensions in SVG, repeatability and silence."),"applies the format-specific orientation");
            }
        }
    }
    if inventory.qualification_rows.is_empty() {
        return Err("PNG inventory needs explicit qualification work".into());
    }
    for row in inventory.qualification_rows {
        if !row.id.starts_with("codec:png:") {
            return Err("invalid PNG qualification identity".into());
        }
        rows.push(PolicyRow {
            id: row.id,
            criterion: row.criterion,
            spec_anchor: row.spec_anchor,
            category: "codec-profile".into(),
        });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expanded_png_inventory_has_closed_axes_and_unique_ids() {
        let source = read(PATH).unwrap();
        let rows = rows(&source).unwrap();
        assert_eq!(rows.len(), 948);
        assert_eq!(
            rows.iter()
                .filter(|r| r.id.starts_with("codec:png:sample:"))
                .count(),
            180
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.id.starts_with("codec:png:cicp:rgba"))
                .count(),
            704
        );
        assert_eq!(
            rows.iter().map(|r| &r.id).collect::<BTreeSet<_>>().len(),
            rows.len()
        );
        for field in ["sample_layouts", "interlace", "containers", "row_filters"] {
            let mut bad: Value = parse(&source).unwrap();
            bad[field].as_array_mut().unwrap().pop();
            assert!(
                super::rows(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{field}"
            );
        }
        let mut bad: Value = parse(&source).unwrap();
        bad["sample_layouts"][0]["transparency"] = json!(["none"]);
        assert!(super::rows(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}
