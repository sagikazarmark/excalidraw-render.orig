// Appended inside the approved collector's module by xtask. This can call its
// private request adapter without changing that approval-pinned source file.
#[cfg(target_arch = "wasm32")]
fn wasm_collect(order: &str) {
    use wasm_bindgen_test::console_log;
    let input: Value = serde_json::from_slice(include_bytes!("snapshot-inputs.json")).unwrap();
    let mut executions = input["executions"].as_array().unwrap().clone();
    if order == "reverse" { executions.reverse(); }
    for execution in executions {
        let key = &execution["key"];
        let source = input["scenes"][text(key,"case").unwrap()].as_str().unwrap();
        let scene = SceneDocument::parse(source.as_bytes()).unwrap();
        let request = map_request(key,&execution["options"]).unwrap();
        let mut samples = Vec::new();
        for _ in 0..2 {
            let renderer = Renderer::new();
            let result = match &request {
                Request::Scene(request) => renderer.render(&scene,request),
                Request::Frame(ordinal,request) => {
                    let frame = scene.frames().find(|f|u64::from(f.ordinal())==*ordinal).expect("frame ordinal exists");
                    renderer.render_frame(frame,request)
                }
            };
            let describe = |bytes: &[u8]| json!({"sha256":hash(bytes),"byte_length":bytes.len()});
            let sample = match result {
                Ok(outcome) => json!({"outcome":{"kind":"artifact","bytes":describe(outcome.artifact().bytes())},
                    "report":describe(&serde_json::to_vec(outcome.report()).unwrap())}),
                Err(error) => json!({"outcome":{"kind":"failure","code":error.code().as_str(),
                    "descriptor":describe(&serde_json::to_vec(&error.descriptor()).unwrap())},
                    "report":describe(&serde_json::to_vec(error.report()).unwrap())}),
            };
            samples.push(sample);
        }
        // Preserve both measurements; the host validator rejects session drift.
        console_log!("WASM_SNAPSHOT_RESULT={}",json!({"key":key,"samples":samples}));
    }
    console_log!("WASM_SNAPSHOT_COMPLETE={}",json!({"order":order,"input_sha256":hash(include_bytes!("snapshot-inputs.json"))}));
}

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn snapshot_forward() { wasm_collect("forward"); }

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn snapshot_reverse() { wasm_collect("reverse"); }
