//! Comparing a render's compatibility report with the expected report a
//! conformance case checks in. The checked file is canonical JSON and the
//! renderer serializes its report in specification order, so the two agree as
//! JSON values and not as bytes; the one byte-form contract is
//! `outcome_contract`'s standalone empty report.

use std::path::Path;

use excalidraw_render::CompatibilityReport;
use serde_json::Value;

/// The checked expected report at `path` as a JSON value.
pub fn checked_report(path: &Path) -> Value {
    super::read_json(path)
}

/// The renderer's `report` as the JSON value its schema serializes to.
pub fn report_value(report: &CompatibilityReport) -> Value {
    serde_json::to_value(report).expect("report serializes")
}

/// Asserts that `report` is the checked expected report at `path`, naming
/// `context` when it is not.
pub fn assert_report_matches(report: &CompatibilityReport, path: &Path, context: &str) {
    assert_eq!(
        report_value(report),
        checked_report(path),
        "{context}: report differs from its checked expectation"
    );
}
