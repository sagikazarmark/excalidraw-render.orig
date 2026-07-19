mod conformance;

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
    if args.next().as_deref() != Some("conformance") {
        return Err(usage());
    }

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
    let status = Command::new("bash")
        .arg(conformance::repo_root().join("tools/reference-oracle/run-conformance.sh"))
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
    "usage: cargo run --locked -p xtask -- conformance <generate|reproduce|verify|approval-subject|record-approval> <case-id> [approval fields]".to_owned()
}
