mod catalog;
mod conformance;
mod conformance_candidate;
mod conformance_candidate_exceptions;
mod conformance_candidate_gate;
mod conformance_candidate_native;
mod conformance_candidate_semantics;
mod conformance_pdf_bridge;
#[cfg(test)]
mod conformance_pdf_requalification;
mod coverage;
mod pdf_harness;
mod pdf_viewers;
mod representative;
mod svg_font_subset_runner;
mod svg_font_subset_targets;
mod svg_font_subsets;
mod svg_viewers;

use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("catalog") => run_catalog(args),
        Some("conformance") => run_conformance(args),
        Some("representative") => representative::run(args),
        Some("pdf-harness") => pdf_harness::run(args),
        _ => Err(usage()),
    }
}

fn run_catalog(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let args = args.by_ref().collect::<Vec<_>>();
    if args.first().is_some_and(|s| s.contains("approval")) {
        return Err("approval commands are retired; review expectation changes in Git and run catalog verify".into());
    }
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("verify") => {
            no_more_args(args)?;
            catalog::verify()?;
            println!("verified compatibility revision 1 catalog");
        }
        Some("import") => {
            let source_root = required_arg(&mut args, "local Excalidraw dist/prod path")?;
            no_more_args(args)?;
            catalog::import(&source_root)?;
            println!("imported compatibility revision 1 catalog");
        }
        Some("materialize") => {
            no_more_args(args)?;
            catalog::materialize()?;
            println!("materialized compatibility revision 1 catalog");
        }
        Some("import-license-sources") => {
            let source_root = required_arg(&mut args, "local license source path")?;
            no_more_args(args)?;
            catalog::import_license_sources(&source_root)?;
            println!("imported compatibility revision 1 license sources");
        }
        _ => return Err(usage()),
    }
    Ok(())
}

fn run_conformance(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let args = args.by_ref().collect::<Vec<_>>();
    if args.first().is_some_and(|s| s.contains("approval")) {
        return Err("approval commands are retired; review expectation changes in Git and run the corresponding verifier".into());
    }
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("inventory") => {
            no_more_args(args)?;
            println!("{}", coverage::inventory_json()?);
        }
        Some("ownership-catalog") => {
            no_more_args(args)?;
            println!("{}", coverage::ownership_catalog()?);
        }
        Some("ownership-authority-migration") => {
            let base = required_arg(&mut args, "previous spec commit hash")?;
            let hash = required_arg(&mut args, "reviewed new spec SHA-256")?;
            let note = required_arg(&mut args, "review note repository path")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&coverage::ownership_authority_migration(
                    &base, &hash, &note
                )?)
                .map_err(|e| e.to_string())?
            );
        }
        Some("seed-ownership") => {
            no_more_args(args)?;
            println!("{}", coverage::seed_ownership()?);
        }
        Some("verify-ownership") => {
            no_more_args(args)?;
            coverage::verify_ownership()?;
        }
        Some("extend-ownership") => {
            let previous = required_arg(&mut args, "previous catalog JSON")?;
            let issue = required_arg(&mut args, "tracking issue")?
                .parse::<u64>()
                .map_err(|_| "invalid extension issue")?;
            let reason = required_arg(&mut args, "new-obligation gap reason")?;
            no_more_args(args)?;
            coverage::extend_ownership(&previous, issue, &reason)?;
        }
        Some("record-test-owner") => {
            // `[--package <package>] [--binary <binary>] <module> <function>
            // <rationale> <ids...>`: the renderer's `fast` binary by default;
            // `module` is the full path from the target's crate root, and an
            // empty string selects the root of a test binary.
            let mut package = "excalidraw-render".to_string();
            let mut binary = "fast".to_string();
            let mut module = required_arg(&mut args, "test module")?;
            loop {
                match module.as_str() {
                    "--package" => package = required_arg(&mut args, "test package")?,
                    "--binary" => binary = required_arg(&mut args, "test binary")?,
                    _ => break,
                }
                module = required_arg(&mut args, "test module")?;
            }
            let function = required_arg(&mut args, "test function")?;
            let rationale = required_arg(&mut args, "reviewed rationale")?;
            coverage::record_test_owner(
                &package,
                &binary,
                &module,
                &function,
                &rationale,
                &args.collect::<Vec<_>>(),
            )?;
        }
        Some("record-case-owner") => {
            let case = required_arg(&mut args, "registered case")?;
            let rationale = required_arg(&mut args, "reviewed rationale")?;
            coverage::record_case_owner(&case, &rationale, &args.collect::<Vec<_>>())?;
        }
        Some("reconcile-stage4-bindings") => {
            let proof = required_arg(&mut args, "qualified native canary record")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&coverage::reconcile_stage4_bindings(&proof)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("verify-representative-owner") => {
            let case = required_arg(&mut args, "representative case")?;
            no_more_args(args)?;
            coverage::verify_representative_owner(&case)?;
            println!("verified representative owner {case}");
        }
        Some("record-representative-owner") => {
            let case = required_arg(&mut args, "representative case")?;
            let rationale = required_arg(&mut args, "reviewed rationale")?;
            no_more_args(args)?;
            coverage::record_representative_owner(&case, &rationale)?;
        }
        Some("record-exception-test-owner") => {
            let rationale = required_arg(&mut args, "reviewed rationale")?;
            coverage::record_exception_test_owner(&rationale, &args.collect::<Vec<_>>())?;
        }
        Some("refresh-exception-test-owner-source") => {
            no_more_args(args)?;
            coverage::refresh_exception_test_owner_source()?;
        }
        Some("record-library-test-owner") => {
            let module = required_arg(&mut args, "library test module")?;
            let function = required_arg(&mut args, "test function")?;
            let rationale = required_arg(&mut args, "reviewed rationale")?;
            coverage::record_library_test_owner(
                &module,
                &function,
                &rationale,
                &args.collect::<Vec<_>>(),
            )?;
        }
        Some("refresh-library-test-owner-source") => {
            let module = required_arg(&mut args, "library test module")?;
            no_more_args(args)?;
            coverage::refresh_library_test_owner_source(&module)?;
        }
        Some("refresh-test-owner-source") => {
            let module = required_arg(&mut args, "test module")?;
            no_more_args(args)?;
            coverage::refresh_test_owner_source(&module)?;
        }
        Some("record-closure-owner") => {
            let obligation = required_arg(&mut args, "closure obligation")?;
            let verifier = required_arg(&mut args, "record verifier")?;
            let rationale = required_arg(&mut args, "reviewed rationale")?;
            no_more_args(args)?;
            coverage::record_closure_owner(&obligation, &verifier, &rationale)?;
        }
        Some("inventory-digest") => {
            no_more_args(args)?;
            println!("{}", coverage::inventory_digest()?);
        }
        Some("verify-inventory") => {
            no_more_args(args)?;
            coverage::verify_inventory()?;
        }
        Some("rebind-font-packaging") => {
            let base = required_arg(&mut args, "previous commit")?;
            let candidate = required_arg(&mut args, "current snapshot directory")?;
            no_more_args(args)?;
            coverage::rebind_font_packaging(&base, &candidate)?;
        }
        Some("rebind-font-packaging-ownership") => {
            let base = required_arg(&mut args, "previous commit")?;
            let spec = required_arg(&mut args, "final spec SHA-256")?;
            no_more_args(args)?;
            coverage::rebind_font_packaging_ownership(&base, &spec)?;
        }
        Some("verify-results") => {
            let report = required_arg(&mut args, "result bundle path")?;
            no_more_args(args)?;
            coverage::verify_results(&report)?;
        }
        Some("verify-native-snapshots") => {
            let directory = required_arg(&mut args, "snapshot directory")?;
            no_more_args(args)?;
            coverage::verify_snapshots(&directory)?;
        }
        Some("stage-wasm-snapshots") => {
            no_more_args(args)?;
            coverage::stage_wasm_snapshots()?;
        }
        Some("test-group-filter") => {
            let path = required_arg(&mut args, "test group manifest")?;
            no_more_args(args)?;
            println!("{}", coverage::test_group_filter(&path)?);
        }
        Some("build-native-cli") => {
            no_more_args(args)?;
            let (path, _) = conformance_candidate_native::build()?;
            println!("prepared native release CLI: {}", path.display());
        }
        Some("refresh-test-owner-sources") => {
            coverage::refresh_test_owner_sources(&args.collect::<Vec<_>>())?;
        }
        Some("stage-traces") => {
            no_more_args(args)?;
            coverage::stage_traces()?;
        }
        Some("aggregate-traces") => {
            let directory = required_arg(&mut args, "trace log directory")?;
            no_more_args(args)?;
            coverage::aggregate_traces(&directory)?;
        }
        Some("join-trace-results") => {
            let directory = required_arg(&mut args, "trace/public result directory")?;
            no_more_args(args)?;
            coverage::join_trace_results(&directory)?;
        }
        Some("verify-joined-results") => {
            let directory = required_arg(&mut args, "trace/public result directory")?;
            no_more_args(args)?;
            coverage::verify_joined_results(&directory)?;
        }
        Some("verify-wasm-snapshots") => {
            let directory = required_arg(&mut args, "WASM log directory")?;
            no_more_args(args)?;
            coverage::verify_wasm_snapshots(&directory)?;
        }
        Some("report-wasm-snapshots") => {
            let directory = required_arg(&mut args, "WASM log directory")?;
            no_more_args(args)?;
            coverage::report_wasm_snapshots(&directory)?;
        }
        Some("stage-native-baseline") => {
            let candidate = required_arg(&mut args, "snapshot directory")?;
            let output = required_arg(&mut args, "new baseline directory")?;
            no_more_args(args)?;
            coverage::stage_baseline(&candidate, &output)?;
        }
        Some("verify-native-baseline") => {
            no_more_args(args)?;
            coverage::verify_baseline()?;
        }
        Some("replay-native-snapshots") => {
            let candidate = required_arg(&mut args, "snapshot directory")?;
            no_more_args(args)?;
            coverage::replay_snapshots(&candidate)?;
        }
        Some("pdf-judgments-generate") => {
            no_more_args(args)?;
            coverage::generate_pdf_judgments()?;
        }
        Some("verify-pdf-judgments") => {
            no_more_args(args)?;
            coverage::verify_pdf_judgments()?;
        }
        Some("layers-report") => {
            no_more_args(args)?;
            println!("{}", coverage::report_layers()?);
        }
        Some("layers-generate" | "layers-approval-subject" | "record-layers-approval") => {
            return Err("five-layer maintenance is retired; use conformance layers-report (read-only, advisory)".into());
        }
        Some("verify-layers") => {
            let strict = match args.next().as_deref() {
                None => false,
                Some("--strict") => true,
                Some(other) => return Err(format!("unexpected argument {other:?}")),
            };
            no_more_args(args)?;
            if strict {
                coverage::verify_layers()?;
            } else {
                println!("{}", coverage::report_layers()?);
            }
        }
        Some("verify-closure") => {
            no_more_args(args)?;
            coverage::verify_closure()?;
        }
        Some("verify") => {
            let case_id = required_arg(&mut args, "case ID")?;
            no_more_args(args)?;
            conformance::verify(&case_id)?;
            println!("verified {case_id}");
        }
        Some("verify-all") => {
            no_more_args(args)?;
            let case_ids = conformance::case_ids()?;
            for case_id in &case_ids {
                conformance::verify(case_id)?;
                println!("verified {case_id}");
            }
            println!("verified {} registered cases", case_ids.len());
            conformance_candidate_gate::verify_registry()?;
        }
        Some("candidate-prepare-assets") => {
            let directory = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            conformance_candidate::prepare_assets(&directory)?;
        }
        Some("candidate-compare") => {
            let path = required_arg(&mut args, "viewer collection")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate::comparisons(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-semantics") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_semantics::verify(
                    &conformance::repo_root().join(path)
                )?)
                .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-materialize") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_gate::materialize(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-verify") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_gate::execute(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-native-capture") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_native::capture(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-native-check") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_native::replay(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("qualify-excalifont-canary") => {
            let path = required_arg(&mut args, "independent canary evidence directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_native::qualify_text_canary(
                    &path
                )?)
                .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-inspect") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_gate::inspect(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-propose-exceptions") => {
            let path = required_arg(&mut args, "candidate directory")?;
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_exceptions::prepare(&path)?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-revise") => {
            let path = required_arg(&mut args, "new candidate directory")?;
            no_more_args(args)?;
            conformance_candidate_gate::revise(&path)?;
        }
        Some("candidate-executions") => {
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_candidate_gate::execute_registry()?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("candidate-pdf-requalify") => {
            return Err("completed PDF backend requalification workflow retired; use current PDF harness and native snapshot checks".into());
        }
        Some("pdf-requalification-subject") => {
            return Err("historical PDF requalification subject command retired; retained evidence remains available".into());
        }
        Some("collect-pdf-colorspace-bridge") => {
            no_more_args(args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&conformance_pdf_bridge::collect()?)
                    .map_err(|e| e.to_string())?
            );
        }
        Some("verify-svg-viewers") => {
            no_more_args(args)?;
            svg_viewers::verify()?;
            println!("verified canonical SVG viewer qualification");
        }
        Some("verify-svg-font-subsets") => {
            no_more_args(args)?;
            svg_font_subsets::verify()?;
            println!("verified canonical SVG font subset viewer qualification");
        }
        Some("verify-svg-font-subset-release") => {
            no_more_args(args)?;
            let state = svg_font_subset_targets::verify_release()?;
            println!("verified SVG font subset release evidence ({state})");
        }
        Some("materialize-svg-font-subset-target") => {
            let target = required_arg(&mut args, "target")?;
            let test_log = required_arg(&mut args, "qualification test log")?;
            let output = required_arg(&mut args, "target report path")?;
            no_more_args(args)?;
            svg_font_subset_targets::materialize(&target, &test_log, &output)?;
            println!("materialized SVG font subset target evidence for {target}");
        }
        Some("materialize-svg-font-subset-cross-target") => {
            let test_log = required_arg(&mut args, "qualification test log")?;
            no_more_args(args)?;
            svg_font_subset_targets::materialize_cross_target(&test_log)?;
            println!("materialized cross-target SVG font subset measurement");
        }
        Some("aggregate-svg-font-subset-targets") => {
            let input = required_arg(&mut args, "target report directory")?;
            let output = required_arg(&mut args, "aggregate report path")?;
            no_more_args(args)?;
            svg_font_subset_targets::aggregate(&input, &output)?;
            println!("aggregated SVG font subset target evidence");
        }
        Some("record-svg-font-subset-target-aggregate") => {
            let aggregate = required_arg(&mut args, "downloaded aggregate report")?;
            let run_url = required_arg(&mut args, "workflow run URL")?;
            no_more_args(args)?;
            svg_font_subset_targets::record_aggregate(&aggregate, &run_url)?;
            println!("recorded SVG font subset target aggregate into the release evidence");
        }
        Some("generate-svg-font-subset-rust-report") => {
            let inputs = required_arg(&mut args, "viewer input manifest")?;
            let viewer = required_arg(&mut args, "canonical pure-Rust viewer executable")?;
            let output = required_arg(&mut args, "viewer report path")?;
            no_more_args(args)?;
            svg_font_subset_runner::generate(&inputs, &viewer, &output)?;
            println!("generated pure-Rust SVG font subset viewer report");
        }
        Some("materialize-svg-font-subset-evidence") => {
            let inputs = required_arg(&mut args, "viewer input manifest")?;
            let chromium = required_arg(&mut args, "Chromium viewer report")?;
            let pure_rust = required_arg(&mut args, "pure-Rust viewer report")?;
            no_more_args(args)?;
            svg_font_subsets::materialize(&inputs, &chromium, &pure_rust)?;
            println!("materialized SVG font subset viewer evidence");
        }
        Some("verify-pdf-viewers") => {
            no_more_args(args)?;
            pdf_viewers::verify()?;
            println!("verified canonical PDF viewer and structural qualification");
        }
        Some("refresh-pdf-viewer-dependencies") => {
            return Err("historical PDF viewer dependency refresh retired; use conformance verify-pdf-viewers to check retained qualification evidence".into());
        }
        Some("refresh-pdfium-runtime") => {
            no_more_args(args)?;
            pdf_viewers::refresh_pdfium_runtime()?;
            println!("refreshed Rust PDFium runtime record against retained calibration rasters");
        }
        Some(mode @ ("generate" | "reproduce")) => {
            let case_id = required_arg(&mut args, "case ID")?;
            no_more_args(args)?;
            run_reference_oracle(mode, &case_id)?;
            conformance::verify(&case_id)?;
        }
        _ => return Err(usage()),
    }
    Ok(())
}

fn required_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing {name}; {}", usage()))
}

fn no_more_args(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if let Some(argument) = args.next() {
        return Err(format!("unexpected argument {argument:?}; {}", usage()));
    }
    Ok(())
}

fn run_reference_oracle(mode: &str, case_id: &str) -> Result<(), String> {
    let runner = conformance::case_runner(case_id)?;
    let status = Command::new("bash")
        .arg(conformance::repo_root().join(&runner))
        .arg(mode)
        .arg(case_id)
        .status()
        .map_err(|error| format!("failed to start reference oracle: {error}"))?;
    if !status.success() {
        return Err(format!("reference oracle exited with {status}"));
    }
    Ok(())
}

fn usage() -> String {
    concat!(
        "usage: cargo run --locked -p xtask -- ",
        "representative <prepare|verify|capture|verify-capture|case|controls|viewer-assets|evidence|compact> | ",
        "pdf-harness <generate-fixtures|inspect> | ",
        "catalog <import <local-dist-prod>|import-license-sources <local-path>|materialize|verify> | ",
        "conformance layers-report | conformance test-group-filter <manifest> | conformance refresh-test-owner-sources <module>... | conformance build-native-cli | ",
        "conformance <inventory|ownership-catalog|seed-ownership|extend-ownership|verify-ownership|record-test-owner|record-library-test-owner|record-case-owner|record-exception-test-owner|refresh-exception-test-owner-source|refresh-test-owner-source|refresh-library-test-owner-source|record-closure-owner|inventory-digest|verify-inventory|verify-results|stage-traces|aggregate-traces|join-trace-results|verify-joined-results|verify-native-snapshots|stage-wasm-snapshots|verify-wasm-snapshots|report-wasm-snapshots|stage-native-baseline|verify-native-baseline|replay-native-snapshots|pdf-judgments-generate|verify-pdf-judgments|verify-layers|verify-closure|generate|reproduce|verify|verify-all|",
        "verify-svg-viewers|verify-svg-font-subsets|verify-svg-font-subset-release|materialize-svg-font-subset-target|materialize-svg-font-subset-cross-target|aggregate-svg-font-subset-targets|record-svg-font-subset-target-aggregate|generate-svg-font-subset-rust-report|materialize-svg-font-subset-evidence|",
        "verify-pdf-viewers|refresh-pdfium-runtime> ",
        "[arguments]\nSee CONTRIBUTING.md for tests and expectation updates."
    )
    .to_owned()
}
