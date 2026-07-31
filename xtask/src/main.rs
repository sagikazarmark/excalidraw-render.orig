mod catalog;
mod conformance;
mod pdf_viewers;
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
    match args.next().as_deref() {
        Some("verify") => {
            let case_id = required_arg(&mut args, "case ID")?;
            no_more_args(args)?;
            conformance::verify(&case_id, true)?;
            println!("verified {case_id}");
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
    let runner = if case_id == "focused.excalifont-text.v1" {
        "tools/reference-oracle/run-typography-conformance.sh"
    } else if case_id == "focused.frame-export.v1" {
        "tools/reference-oracle/run-frame-conformance.sh"
    } else {
        "tools/reference-oracle/run-conformance.sh"
    };
    let status = Command::new("bash")
        .arg(conformance::repo_root().join(runner))
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
        "catalog <import <local-dist-prod>|import-license-sources <local-path>|materialize|verify> | ",
        "conformance <generate|reproduce|verify|approval-subject|record-approval|",
        "verify-svg-viewers|svg-viewer-approval-subject|record-svg-viewer-approval|",
        "verify-pdf-viewers|pdf-viewer-approval-subject|record-pdf-viewer-approval> ",
        "[case-id] [approval fields]"
    )
    .to_owned()
}
