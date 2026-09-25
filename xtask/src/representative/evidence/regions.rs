use super::*;

fn number(value: &Value, name: &str) -> Result<f64> {
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("invalid finite {name}"))
}

pub(super) fn validate_options(plan: &Value, reference: &Value) -> Result<()> {
    let id = text(reference, "variant")?;
    let variants = array(plan, "variants")?;
    let mut matching = variants.iter().filter(|v| v["id"] == id);
    let variant = matching.next().ok_or("unknown reference variant")?;
    require(matching.next().is_none(), "duplicate reference variant")?;
    let options = &reference["options"];
    fields(
        options,
        &[
            "exportBackground",
            "exportEmbedScene",
            "exportPadding",
            "exportScale",
            "exportWithDarkMode",
            "frameRendering",
            "maxWidthOrHeight",
            "viewBackgroundColor",
        ],
        "invalid reference options",
    )?;
    // JSON integer/decimal spellings have the same meaning to the JS producer.
    for (export, request) in [("exportPadding", "padding"), ("exportScale", "scale")] {
        require(
            number(&options[export], export)? == number(&variant[request], request)?,
            "reference numeric option differs from plan",
        )?;
    }
    for (export, request) in [
        ("exportBackground", "include_background"),
        ("exportEmbedScene", "metadata"),
    ] {
        require(
            variant[request].is_boolean() && options[export] == variant[request],
            "reference boolean option differs from plan",
        )?;
    }
    let mode = text(variant, "color_mode")?;
    require(
        matches!(mode, "light" | "dark") && options["exportWithDarkMode"] == (mode == "dark"),
        "reference color mode differs from plan",
    )?;
    require(
        options["frameRendering"] == json!({"enabled":true,"clip":true,"name":true,"outline":true})
            && options.get("maxWidthOrHeight") == Some(&Value::Null),
        "reference fixed export options differ",
    )
}

fn points(element: &Value) -> Result<Vec<(f64, f64)>> {
    let x = number(&element["x"], "element x")?;
    let y = number(&element["y"], "element y")?;
    let vertices = match element.get("points") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_array().ok_or("invalid crop points")?),
    };
    let points = if let Some(vertices) = vertices.filter(|p| !p.is_empty()) {
        vertices
            .iter()
            .map(|p| {
                let p = p
                    .as_array()
                    .filter(|p| p.len() == 2)
                    .ok_or("invalid crop point")?;
                Ok((x + number(&p[0], "point x")?, y + number(&p[1], "point y")?))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![
            (x, y),
            (
                x + number(&element["width"], "element width")?,
                y + number(&element["height"], "element height")?,
            ),
        ]
    };
    require(
        points.iter().all(|(x, y)| x.is_finite() && y.is_finite()),
        "invalid crop extent",
    )?;
    Ok(points)
}

pub(super) fn derive(scene: &Value, plan: &Value, reference: &Value) -> Result<Value> {
    validate_options(plan, reference)?;
    let background = scene["appState"]
        .get("viewBackgroundColor")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or(json!("#ffffff"));
    require(
        reference["options"]["viewBackgroundColor"] == background,
        "reference background differs from scene",
    )?;
    let live = array(scene, "elements")?
        .iter()
        .filter(|e| !truthy(&e["isDeleted"]))
        .collect::<Vec<_>>();
    require(
        !live.is_empty(),
        "empty scene has no anchored viewer extent",
    )?;
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for element in &live {
        require(
            element.get("angle").is_none_or(|v| v.as_f64() == Some(0.0)),
            "rotated source crops need a reviewed bounds adapter",
        )?;
        min_x = min_x.min(number(&element["x"], "element x")?);
        min_y = min_y.min(number(&element["y"], "element y")?);
        // Unrotated points can extend left/up from the authored element origin.
        // Use the same source extent for the export origin and crop anchors.
        for (x, y) in points(element)? {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
        }
    }
    let padding = number(&reference["options"]["exportPadding"], "reference padding")?;
    let scale = number(&reference["options"]["exportScale"], "reference scale")?;
    require(scale > 0.0, "non-positive reference scale")?;
    let (width, height) = receipts::dimensions(reference)?;
    let mut crops = Vec::new();
    let mut whole = None;
    for region in array(plan, "regions")? {
        if region["id"] == "whole" {
            whole = Some(region["class"].clone());
            continue;
        }
        let anchors = array(region, "element_ids")?;
        let mut points = Vec::new();
        for element in live.iter().filter(|e| anchors.contains(&e["id"])) {
            points.extend(self::points(element)?);
        }
        require(!points.is_empty(), "crop has no source anchors")?;
        let left = ((points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min) - min_x + padding
            - 4.0)
            * scale)
            .floor()
            .max(0.0);
        let top = ((points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min) - min_y + padding
            - 4.0)
            * scale)
            .floor()
            .max(0.0);
        let right = ((points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max) - min_x
            + padding
            + 4.0)
            * scale)
            .ceil()
            .min(width as f64);
        let bottom = ((points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max) - min_y
            + padding
            + 4.0)
            * scale)
            .ceil()
            .min(height as f64);
        require(
            [left, top, right, bottom].iter().all(|n| n.is_finite())
                && right > left
                && bottom > top,
            "empty or invalid anchored crop",
        )?;
        crops.push(json!({"id":region["id"],"class":region["class"],"x":left as u32,"y":top as u32,"width":(right-left) as u32,"height":(bottom-top) as u32}));
    }
    Ok(
        json!({"width":width,"height":height,"whole_class":whole.ok_or("missing whole region")?,"crops":crops}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Value, Value, Value) {
        let scene = json!({"elements":[
            {"id":"line","x":100,"y":100,"points":[[0,0],[-100,-80]]},
            {"id":"box","x":120,"y":120,"width":20,"height":20}
        ],"appState":{}});
        let plan = json!({"variants":[{"id":"light","padding":10,"scale":1,
            "include_background":true,"metadata":false,"color_mode":"light"}],
            "regions":[{"id":"whole","class":"mixed-representative","element_ids":[]},
                {"id":"line","class":"geometry/image","element_ids":["line"]},
                {"id":"box","class":"geometry/image","element_ids":["box"]}]});
        let reference = json!({"variant":"light","width":160,"height":140,"options":{
            "exportPadding":10,"exportScale":1,"exportBackground":true,"exportEmbedScene":false,
            "exportWithDarkMode":false,"frameRendering":{"enabled":true,"clip":true,"name":true,"outline":true},
            "maxWidthOrHeight":null,"viewBackgroundColor":"#ffffff"}});
        (scene, plan, reference)
    }

    #[test]
    fn reference_options_are_bound_to_the_variant_and_scene() {
        let (scene, plan, reference) = fixture();
        for (key, value) in [
            ("exportPadding", json!(11)),
            ("exportScale", json!(2)),
            ("exportBackground", json!(false)),
            ("exportEmbedScene", json!(true)),
            ("exportWithDarkMode", json!(true)),
            ("maxWidthOrHeight", json!(160)),
            (
                "frameRendering",
                json!({"enabled":true,"clip":true,"name":false,"outline":true}),
            ),
            ("viewBackgroundColor", json!("#000000")),
        ] {
            let mut changed = reference.clone();
            changed["options"][key] = value;
            assert!(derive(&scene, &plan, &changed).is_err(), "accepted {key}");
        }
        for key in reference["options"].as_object().unwrap().keys() {
            let mut changed = reference.clone();
            changed["options"].as_object_mut().unwrap().remove(key);
            assert!(derive(&scene, &plan, &changed).is_err(), "missing {key}");
        }
        let mut changed = reference.clone();
        changed["variant"] = json!("dark");
        assert!(validate_options(&plan, &changed).is_err());
        let mut changed = plan.clone();
        changed["variants"]
            .as_array_mut()
            .unwrap()
            .push(plan["variants"][0].clone());
        assert!(validate_options(&changed, &reference).is_err());
    }

    #[test]
    fn options_use_js_numeric_equality_and_dark_mapping() {
        let (scene, mut plan, mut reference) = fixture();
        reference["options"]["exportPadding"] = json!(10.0);
        reference["options"]["exportScale"] = json!(1.0);
        assert!(derive(&scene, &plan, &reference).is_ok());
        plan["variants"][0]["color_mode"] = json!("dark");
        reference["options"]["exportWithDarkMode"] = json!(true);
        assert!(derive(&scene, &plan, &reference).is_ok());
        for invalid in [json!("1"), json!(true), Value::Null] {
            reference["options"]["exportScale"] = invalid;
            assert!(validate_options(&plan, &reference).is_err());
        }
    }

    #[test]
    fn negative_vertices_shift_the_shared_export_origin() {
        let (scene, plan, reference) = fixture();
        let actual = derive(&scene, &plan, &reference).unwrap();
        assert_eq!(
            actual["crops"],
            json!([
                {"id":"line","class":"geometry/image","x":6,"y":6,"width":108,"height":88},
                {"id":"box","class":"geometry/image","x":126,"y":106,"width":28,"height":28}
            ])
        );
        // Check unselected elements too: they still determine the export origin.
        let mut malformed = scene.clone();
        malformed["elements"][0]["points"] = json!([[0, 0], [-100]]);
        let mut box_only = plan.clone();
        box_only["regions"].as_array_mut().unwrap().remove(1);
        assert!(derive(&malformed, &box_only, &reference).is_err());
        let mut rotated = scene;
        rotated["elements"][0]["angle"] = json!(0.1);
        assert!(derive(&rotated, &plan, &reference).is_err());
    }

    #[test]
    fn registered_cases_keep_their_retained_crop_plans() {
        let root = crate::conformance::repo_root();
        for (case, proposal) in [
            ("velox-design", "velox84-evidence-v2"),
            ("varmq-queue-diagram", "varmq84-evidence-v3"),
            ("excali-local-slides", "slides84-evidence-v2"),
        ] {
            let directory = root.join(format!("tests/conformance/cases/representative/{case}"));
            let case_record = decode(&read(&directory, "case.json").unwrap()).unwrap();
            let bundle = root.join(case_record["intake"]["path"].as_str().unwrap());
            let plan = decode(&read(&bundle, "plan.json").unwrap()).unwrap();
            let scene = decode(&read(&bundle, "scene.excalidraw").unwrap()).unwrap();
            let reference = decode(&read(&directory, "reference/reference.json").unwrap()).unwrap();
            for row in array(&reference, "rows").unwrap() {
                let variant = row["variant"].as_str().unwrap();
                let retained = decode(
                    &read(
                        &root,
                        &format!(
                            "tests/conformance/proposals/{proposal}/viewers/{variant}/regions.json"
                        ),
                    )
                    .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    derive(&scene, &plan, row).unwrap(),
                    retained,
                    "{case}/{variant}"
                );
            }
        }
    }
}
