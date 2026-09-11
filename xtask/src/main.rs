mod catalog;
mod conformance;
mod coverage;
mod pdf_viewers;
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
        _ => Err(usage()),
    }
}

fn run_catalog(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        Some("verify") => {
            no_more_args(args)?;
            catalog::verify(true)?;
            println!("verified compatibility revision 1 catalog");
        }
        Some("reviewed-decodes-approval-subject") => {
            no_more_args(args)?;
            println!("sha256:{}", catalog::reviewed_decodes_approval_subject()?);
        }
        Some("record-reviewed-decodes-approval") => {
            let approved_by = required_arg(&mut args, "GitHub identity")?;
            let approved_at = required_arg(&mut args, "approval date")?;
            let approval_url = required_arg(&mut args, "approval URL")?;
            let expected_subject = required_arg(&mut args, "approval subject")?;
            no_more_args(args)?;
            catalog::record_reviewed_decodes_approval(
                &approved_by,
                &approved_at,
                &approval_url,
                &expected_subject,
            )?;
            println!("recorded compatibility revision 1 reviewed-decodes approval");
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
    match args.next().as_deref() {
        Some("inventory") => {
            no_more_args(args)?;
            println!("{}", coverage::inventory_json()?);
        }
        Some("inventory-digest") => {
            no_more_args(args)?;
            println!("{}", coverage::inventory_digest()?);
        }
        Some("verify-inventory") => {
            no_more_args(args)?;
            coverage::verify_inventory()?;
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
        Some("stage-native-baseline") => {
            let candidate = required_arg(&mut args, "snapshot directory")?;
            let output = required_arg(&mut args, "new baseline directory")?;
            no_more_args(args)?;
            coverage::stage_baseline(&candidate, &output)?;
        }
        Some("native-baseline-approval-subject") => {
            no_more_args(args)?;
            println!("sha256:{}", coverage::baseline_subject()?);
        }
        Some("record-native-baseline-approval") => {
            let login = required_arg(&mut args, "GitHub identity")?;
            let date = required_arg(&mut args, "approval date")?;
            let url = required_arg(&mut args, "approval comment URL")?;
            let subject = required_arg(&mut args, "approval subject")?;
            no_more_args(args)?;
            coverage::record_baseline_approval(&login, &date, &url, &subject)?;
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
        Some("verify-closure") => {
            no_more_args(args)?;
            coverage::verify_closure()?;
        }
        Some("verify") => {
            let case_id = required_arg(&mut args, "case ID")?;
            no_more_args(args)?;
            conformance::verify(&case_id, true)?;
            println!("verified {case_id}");
        }
        Some("verify-all") => {
            no_more_args(args)?;
            let case_ids = conformance::case_ids()?;
            for case_id in &case_ids {
                conformance::verify(case_id, true)?;
                println!("verified {case_id}");
            }
            println!("verified {} registered cases", case_ids.len());
        }
        Some("approval-subject") => {
            let case_id = required_arg(&mut args, "case ID")?;
            no_more_args(args)?;
            println!("sha256:{}", conformance::approval_subject(&case_id)?);
        }
        Some("record-approval") => {
            let case_id = required_arg(&mut args, "case ID")?;
            let approved_by = required_arg(&mut args, "GitHub identity")?;
            let approved_at = required_arg(&mut args, "approval date")?;
            let approval_url = required_arg(&mut args, "approval URL")?;
            let expected_subject = required_arg(&mut args, "approval subject")?;
            no_more_args(args)?;
            conformance::record_approval(
                &case_id,
                &approved_by,
                &approved_at,
                &approval_url,
                &expected_subject,
            )?;
            println!("recorded approval for {case_id}");
        }
        Some("verify-svg-viewers") => {
            no_more_args(args)?;
            svg_viewers::verify(true)?;
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
        Some("svg-font-subset-approval-subject") => {
            no_more_args(args)?;
            println!("sha256:{}", svg_font_subsets::approval_subject()?);
        }
        Some("record-svg-font-subset-approval") => {
            let approved_by = required_arg(&mut args, "GitHub identity")?;
            let approved_at = required_arg(&mut args, "approval date")?;
            let approval_url = required_arg(&mut args, "approval URL")?;
            let expected_subject = required_arg(&mut args, "approval subject")?;
            no_more_args(args)?;
            svg_font_subsets::record_approval(
                &approved_by,
                &approved_at,
                &approval_url,
                &expected_subject,
            )?;
            println!("recorded SVG font subset viewer evidence approval");
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
        Some("svg-viewer-approval-subject") => {
            no_more_args(args)?;
            println!("sha256:{}", svg_viewers::approval_subject()?);
        }
        Some("record-svg-viewer-approval") => {
            let approved_by = required_arg(&mut args, "GitHub identity")?;
            let approved_at = required_arg(&mut args, "approval date")?;
            let approval_url = required_arg(&mut args, "approval URL")?;
            let expected_subject = required_arg(&mut args, "approval subject")?;
            no_more_args(args)?;
            svg_viewers::record_approval(
                &approved_by,
                &approved_at,
                &approval_url,
                &expected_subject,
            )?;
            println!("recorded canonical SVG viewer qualification approval");
        }
        Some("verify-pdf-viewers") => {
            no_more_args(args)?;
            pdf_viewers::verify(true)?;
            println!("verified canonical PDF viewer and structural qualification");
        }
        Some("refresh-pdf-viewer-dependencies") => {
            no_more_args(args)?;
            pdf_viewers::refresh_dependencies()?;
            println!("refreshed PDF viewer qualification dependencies");
        }
        Some("pdf-viewer-approval-subject") => {
            no_more_args(args)?;
            println!("sha256:{}", pdf_viewers::approval_subject()?);
        }
        Some("record-pdf-viewer-approval") => {
            let approved_by = required_arg(&mut args, "GitHub identity")?;
            let approved_at = required_arg(&mut args, "approval date")?;
            let approval_url = required_arg(&mut args, "approval URL")?;
            let expected_subject = required_arg(&mut args, "approval subject")?;
            no_more_args(args)?;
            pdf_viewers::record_approval(
                &approved_by,
                &approved_at,
                &approval_url,
                &expected_subject,
            )?;
            println!("recorded canonical PDF harness qualification approval");
        }
        Some(mode @ ("generate" | "reproduce")) => {
            let case_id = required_arg(&mut args, "case ID")?;
            no_more_args(args)?;
            run_reference_oracle(mode, &case_id)?;
            conformance::verify(&case_id, false)?;
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
        "catalog <import <local-dist-prod>|import-license-sources <local-path>|materialize|verify|",
        "reviewed-decodes-approval-subject|record-reviewed-decodes-approval> | ",
        "conformance <inventory|inventory-digest|verify-inventory|verify-results|verify-native-snapshots|stage-native-baseline|native-baseline-approval-subject|record-native-baseline-approval|verify-native-baseline|replay-native-snapshots|verify-closure|generate|reproduce|verify|verify-all|approval-subject|record-approval|",
        "verify-svg-viewers|verify-svg-font-subsets|verify-svg-font-subset-release|materialize-svg-font-subset-target|materialize-svg-font-subset-cross-target|aggregate-svg-font-subset-targets|record-svg-font-subset-target-aggregate|svg-font-subset-approval-subject|record-svg-font-subset-approval|generate-svg-font-subset-rust-report|materialize-svg-font-subset-evidence|svg-viewer-approval-subject|record-svg-viewer-approval|",
        "verify-pdf-viewers|refresh-pdf-viewer-dependencies|pdf-viewer-approval-subject|record-pdf-viewer-approval> ",
        "[case-id] [approval fields]"
    )
    .to_owned()
}
