mod catalog;
mod cli;
mod conformance;
mod conformance_candidate;
mod conformance_candidate_exceptions;
mod conformance_candidate_gate;
mod conformance_candidate_native;
mod conformance_candidate_semantics;
#[cfg(test)]
mod conformance_pdf_requalification;
mod coverage;
mod demo;
mod pdf_harness;
mod pdf_viewers;
mod representative;
mod sfnt;
mod svg_font_subset_runner;
mod svg_font_subset_targets;
mod svg_font_subsets;
mod svg_viewers;

use std::env;
use std::process::{Command, ExitCode};

use clap::Parser;

use cli::{CatalogCommand, Cli, ConformanceCommand, Family};

fn main() -> ExitCode {
    let args = env::args().collect::<Vec<_>>();
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            // Help goes to stdout and succeeds; every malformed invocation
            // keeps xtask's single failure status instead of Clap's usage code.
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    match run(cli.family) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(family: Family) -> Result<(), String> {
    match family {
        Family::Catalog(command) => run_catalog(command),
        Family::Conformance(command) => run_conformance(command),
        Family::Representative(command) => representative::run(command),
        Family::PdfHarness(command) => pdf_harness::run(command),
        Family::Demo(command) => demo::run(command),
    }
}

fn run_catalog(command: CatalogCommand) -> Result<(), String> {
    match command {
        CatalogCommand::Verify => {
            catalog::verify()?;
            println!("verified compatibility revision 1 catalog");
        }
        CatalogCommand::Import { source_root } => {
            catalog::import(&source_root)?;
            println!("imported compatibility revision 1 catalog");
        }
        CatalogCommand::Materialize => {
            catalog::materialize()?;
            println!("materialized compatibility revision 1 catalog");
        }
        CatalogCommand::ImportLicenseSources { source_root } => {
            catalog::import_license_sources(&source_root)?;
            println!("imported compatibility revision 1 license sources");
        }
    }
    Ok(())
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn run_conformance(command: ConformanceCommand) -> Result<(), String> {
    use ConformanceCommand as C;
    match command {
        C::Inventory => println!("{}", coverage::inventory_json()?),
        C::OwnershipCatalog => println!("{}", coverage::ownership_catalog()?),
        C::SeedOwnership => println!("{}", coverage::seed_ownership()?),
        C::VerifyOwnership => coverage::verify_ownership()?,
        C::ExtendOwnership {
            previous,
            issue,
            reason,
        } => coverage::extend_ownership(&previous, issue, &reason)?,
        C::RecordTestOwner {
            package,
            binary,
            module,
            function,
            rationale,
            obligations,
        } => coverage::record_test_owner(
            &package,
            &binary,
            &module,
            &function,
            &rationale,
            &obligations,
        )?,
        C::RecordCaseOwner {
            case,
            rationale,
            obligations,
        } => coverage::record_case_owner(&case, &rationale, &obligations)?,
        C::VerifyRepresentativeOwner { case } => {
            coverage::verify_representative_owner(&case)?;
            println!("verified representative owner {case}");
        }
        C::RecordRepresentativeOwner { case, rationale } => {
            coverage::record_representative_owner(&case, &rationale)?
        }
        C::RecordExceptionTestOwner {
            rationale,
            obligations,
        } => coverage::record_exception_test_owner(&rationale, &obligations)?,
        C::RefreshExceptionTestOwnerSource => coverage::refresh_exception_test_owner_source()?,
        C::RecordLibraryTestOwner {
            module,
            function,
            rationale,
            obligations,
        } => coverage::record_library_test_owner(&module, &function, &rationale, &obligations)?,
        C::RefreshLibraryTestOwnerSource { module } => {
            coverage::refresh_library_test_owner_source(&module)?
        }
        C::RefreshTestOwnerSource { module } => coverage::refresh_test_owner_source(&module)?,
        C::RecordClosureOwner {
            obligation,
            verifier,
            rationale,
        } => coverage::record_closure_owner(&obligation, &verifier, &rationale)?,
        C::InventoryDigest => println!("{}", coverage::inventory_digest()?),
        C::VerifyInventory => coverage::verify_inventory()?,
        C::VerifyResults { report } => coverage::verify_results(&report)?,
        C::VerifyNativeSnapshots { directory } => coverage::verify_snapshots(&directory)?,
        C::StageWasmSnapshots => coverage::stage_wasm_snapshots()?,
        C::TestGroupFilter { manifest } => {
            println!("{}", coverage::test_group_filter(&manifest)?)
        }
        C::BuildNativeCli => {
            let (path, _) = conformance_candidate_native::build()?;
            println!("prepared native release CLI: {}", path.display());
        }
        C::RefreshTestOwnerSources { modules } => coverage::refresh_test_owner_sources(&modules)?,
        C::StageTraces => coverage::stage_traces()?,
        C::AggregateTraces { directory } => coverage::aggregate_traces(&directory)?,
        C::JoinTraceResults { directory } => coverage::join_trace_results(&directory)?,
        C::VerifyJoinedResults { directory } => coverage::verify_joined_results(&directory)?,
        C::VerifyWasmSnapshots { directory } => coverage::verify_wasm_snapshots(&directory)?,
        C::ReportWasmSnapshots { directory } => coverage::report_wasm_snapshots(&directory)?,
        C::StageNativeBaseline {
            snapshots,
            baseline,
        } => coverage::stage_baseline(&snapshots, &baseline)?,
        C::VerifyNativeBaseline => coverage::verify_baseline()?,
        C::ReplayNativeSnapshots { snapshots } => coverage::replay_snapshots(&snapshots)?,
        C::PdfJudgmentsGenerate => coverage::generate_pdf_judgments()?,
        C::VerifyPdfJudgments => coverage::verify_pdf_judgments()?,
        C::LayersReport | C::VerifyLayers { strict: false } => {
            println!("{}", coverage::report_layers()?)
        }
        C::VerifyLayers { strict: true } => coverage::verify_layers()?,
        C::VerifyClosure => coverage::verify_closure()?,
        C::Verify { case_id } => {
            conformance::verify(&case_id)?;
            println!("verified {case_id}");
        }
        C::VerifyAll => {
            let case_ids = conformance::case_ids()?;
            for case_id in &case_ids {
                conformance::verify(case_id)?;
                println!("verified {case_id}");
            }
            println!("verified {} registered cases", case_ids.len());
            conformance_candidate_gate::verify_registry()?;
        }
        C::CandidatePrepareAssets { directory } => {
            conformance_candidate::prepare_assets(&directory)?
        }
        C::CandidateCompare { collection } => {
            print_json(&conformance_candidate::comparisons(&collection)?)?
        }
        C::CandidateSemantics { directory } => print_json(
            &conformance_candidate_semantics::verify(&conformance::repo_root().join(directory))?,
        )?,
        C::CandidateMaterialize { directory } => {
            print_json(&conformance_candidate_gate::materialize(&directory)?)?
        }
        C::CandidateVerify { directory } => {
            print_json(&conformance_candidate_gate::execute(&directory)?)?
        }
        C::CandidateNativeCapture { directory } => {
            print_json(&conformance_candidate_native::capture(&directory)?)?
        }
        C::CandidateNativeCheck { directory } => {
            print_json(&conformance_candidate_native::replay(&directory)?)?
        }
        C::QualifyExcalifontCanary { directory } => print_json(
            &conformance_candidate_native::qualify_text_canary(&directory)?,
        )?,
        C::CandidateInspect { directory } => {
            print_json(&conformance_candidate_gate::inspect(&directory)?)?
        }
        C::CandidateProposeExceptions { directory } => {
            print_json(&conformance_candidate_exceptions::prepare(&directory)?)?
        }
        C::CandidateRevise { directory } => conformance_candidate_gate::revise(&directory)?,
        C::CandidateExecutions => print_json(&conformance_candidate_gate::execute_registry()?)?,
        C::VerifySvgViewers => {
            svg_viewers::verify()?;
            println!("verified canonical SVG viewer qualification");
        }
        C::VerifySvgFontSubsets => {
            svg_font_subsets::verify()?;
            println!("verified canonical SVG font subset viewer qualification");
        }
        C::VerifySvgFontSubsetRelease => {
            let state = svg_font_subset_targets::verify_release()?;
            println!("verified SVG font subset release evidence ({state})");
        }
        C::MaterializeSvgFontSubsetTarget {
            target,
            test_log,
            output,
        } => {
            svg_font_subset_targets::materialize(&target, &test_log, &output)?;
            println!("materialized SVG font subset target evidence for {target}");
        }
        C::MaterializeSvgFontSubsetCrossTarget { test_log } => {
            svg_font_subset_targets::materialize_cross_target(&test_log)?;
            println!("materialized cross-target SVG font subset measurement");
        }
        C::AggregateSvgFontSubsetTargets { input, output } => {
            svg_font_subset_targets::aggregate(&input, &output)?;
            println!("aggregated SVG font subset target evidence");
        }
        C::RecordSvgFontSubsetTargetAggregate { aggregate, run_url } => {
            svg_font_subset_targets::record_aggregate(&aggregate, &run_url)?;
            println!("recorded SVG font subset target aggregate into the release evidence");
        }
        C::GenerateSvgFontSubsetRustReport {
            inputs,
            viewer,
            output,
        } => {
            svg_font_subset_runner::generate(&inputs, &viewer, &output)?;
            println!("generated pure-Rust SVG font subset viewer report");
        }
        C::MaterializeSvgFontSubsetEvidence {
            inputs,
            chromium,
            pure_rust,
        } => {
            svg_font_subsets::materialize(&inputs, &chromium, &pure_rust)?;
            println!("materialized SVG font subset viewer evidence");
        }
        C::VerifyPdfViewers => {
            pdf_viewers::verify()?;
            println!("verified canonical PDF viewer and structural qualification");
        }
        C::RefreshPdfiumRuntime => {
            pdf_viewers::refresh_pdfium_runtime()?;
            println!("refreshed Rust PDFium runtime record against retained calibration rasters");
        }
        C::Generate { case_id } => {
            run_reference_oracle("generate", &case_id)?;
            conformance::verify(&case_id)?;
        }
        C::Reproduce { case_id } => {
            run_reference_oracle("reproduce", &case_id)?;
            conformance::verify(&case_id)?;
        }
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
