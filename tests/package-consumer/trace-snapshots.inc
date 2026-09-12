// Generated inside render::test_support: the approved request adapter is reused
// with its public facade imports resolved to this test build of the same crate.
fn collect_traces(order: &str) {
    let input: Value = serde_json::from_slice(include_bytes!("snapshot-inputs.json")).unwrap();
    let mut executions=input["executions"].as_array().unwrap().clone();
    if order=="reverse" { executions.reverse(); }
    for execution in executions {
        let key=&execution["key"];
        let scene=SceneDocument::parse(input["scenes"][text(key,"case").unwrap()].as_str().unwrap().as_bytes()).unwrap();
        let request=map_request(key,&execution["options"]).unwrap();
        let mut samples=Vec::new();
        for _ in 0..2 {
            let (result,trace)=match &request {
                Request::Scene(request)=>super::render_with_trace(crate::catalog::REVISION_1.profile(),&scene,request),
                Request::Frame(ordinal,request)=> {
                    let frame=scene.frames().find(|f|u64::from(f.ordinal())==*ordinal).unwrap();
                    super::render_frame_with_trace(&crate::catalog::REVISION_1,frame,request,None)
                }
            };
            let describe=|bytes:&[u8]|json!({"sha256":hash(bytes),"byte_length":bytes.len()});
            let (outcome,report)=match result {
                Ok(result)=>(json!({"kind":"artifact","bytes":describe(result.artifact().bytes())}),serde_json::to_vec(result.report()).unwrap()),
                Err(error)=>(json!({"kind":"failure","code":error.code().as_str()}),serde_json::to_vec(error.report()).unwrap()),
            };
            samples.push(json!({"outcome":outcome,"report":describe(&report),"trace":describe(&trace)}));
        }
        let line=format!("TRACE_SNAPSHOT_RESULT={}",json!({"key":key,"samples":samples}));
        #[cfg(target_arch="wasm32")]
        wasm_bindgen_test::console_log!("{}",line);
        #[cfg(not(target_arch="wasm32"))]
        println!("{}",line);
    }
    let target=if cfg!(target_arch="wasm32") {"wasm32-unknown-unknown"}else{"x86_64-unknown-linux-gnu"};
    let line=format!("TRACE_SNAPSHOT_COMPLETE={}",json!({"order":order,"target":target,"input_sha256":hash(include_bytes!("snapshot-inputs.json"))}));
    #[cfg(target_arch="wasm32")]
    wasm_bindgen_test::console_log!("{}",line);
    #[cfg(not(target_arch="wasm32"))]
    println!("{}",line);
}

#[cfg_attr(target_arch="wasm32",wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch="wasm32"),test)]
fn trace_snapshot_forward(){collect_traces("forward");}

#[cfg_attr(target_arch="wasm32",wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch="wasm32"),test)]
fn trace_snapshot_reverse(){collect_traces("reverse");}
