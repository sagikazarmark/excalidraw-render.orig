//! Command grammar for every xtask family. Parsing, validation, and help come
//! from these definitions; command modules receive typed values only.
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    bin_name = "cargo run --locked -p xtask --",
    about = "Repository evidence, catalog, and conformance maintenance",
    after_help = "See docs/development.md for tests and expectation updates.",
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub family: Family,
}

#[derive(Debug, Subcommand)]
pub enum Family {
    /// Compatibility revision 1 asset catalog.
    #[command(subcommand)]
    Catalog(CatalogCommand),
    /// Conformance cases, ownership ledgers, and qualification evidence.
    #[command(subcommand)]
    Conformance(ConformanceCommand),
    /// Representative scene intake and native observations.
    #[command(subcommand)]
    Representative(RepresentativeCommand),
    /// Independent qpdf structural inspection and calibration fixtures.
    #[command(subcommand)]
    PdfHarness(PdfHarnessCommand),
    /// Side-by-side demo against official Excalidraw exports.
    #[command(subcommand)]
    Demo(DemoCommand),
}

/// Rejects retired invocations with their migration guidance before parsing,
/// so arguments that no longer match any grammar still get a useful answer.
pub fn retired(args: &[String]) -> Option<&'static str> {
    let [family, command, ..] = args else {
        return None;
    };
    match (family.as_str(), command.as_str()) {
        ("catalog", command) if command.contains("approval") => {
            Some("approval commands are retired; review expectation changes in Git and run catalog verify")
        }
        ("conformance", command) if command.contains("approval") => {
            Some("approval commands are retired; review expectation changes in Git and run the corresponding verifier")
        }
        ("conformance", "layers-generate") => {
            Some("five-layer maintenance is retired; use conformance layers-report (read-only, advisory)")
        }
        ("conformance", "candidate-pdf-requalify") => {
            Some("completed PDF backend requalification workflow retired; use current PDF harness and native snapshot checks")
        }
        ("conformance", "pdf-requalification-subject") => {
            Some("historical PDF requalification subject command retired; retained evidence remains available")
        }
        ("conformance", "refresh-pdf-viewer-dependencies") => {
            Some("historical PDF viewer dependency refresh retired; use conformance verify-pdf-viewers to check retained qualification evidence")
        }
        _ => None,
    }
}

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Verify the committed catalog.
    Verify,
    /// Import the catalog from a local Excalidraw build.
    Import {
        /// Local Excalidraw dist/prod path.
        source_root: String,
    },
    /// Regenerate derived catalog files.
    Materialize,
    /// Import license sources for catalog assets.
    ImportLicenseSources {
        /// Local license source path.
        source_root: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConformanceCommand {
    /// Print the obligation inventory as JSON.
    Inventory,
    /// Print the ownership catalog.
    OwnershipCatalog,
    /// Print an ownership authority migration record.
    OwnershipAuthorityMigration {
        /// Previous spec commit hash.
        base: String,
        /// Reviewed new spec SHA-256.
        hash: String,
        /// Review note repository path.
        note: String,
    },
    /// Print a seeded ownership catalog.
    SeedOwnership,
    /// Verify the ownership ledger.
    VerifyOwnership,
    /// Extend the ownership ledger with new obligations.
    ExtendOwnership {
        /// Previous catalog JSON.
        previous: String,
        /// Tracking issue.
        #[arg(value_parser = parsed::<u64>("invalid extension issue"))]
        issue: u64,
        /// New-obligation gap reason.
        #[arg(allow_hyphen_values = true)]
        reason: String,
    },
    /// Record a test as owner of obligations.
    RecordTestOwner {
        /// Test package.
        #[arg(long, default_value = "excalidraw-render")]
        package: String,
        /// Test binary.
        #[arg(long, default_value = "fast")]
        binary: String,
        /// Test module: the full path from the target's crate root; an empty
        /// string selects the root of a test binary.
        module: String,
        /// Test function.
        function: String,
        /// Reviewed rationale.
        #[arg(allow_hyphen_values = true)]
        rationale: String,
        /// Owned obligation IDs.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        obligations: Vec<String>,
    },
    /// Record a registered case as owner of obligations.
    RecordCaseOwner {
        /// Registered case.
        case: String,
        /// Reviewed rationale.
        #[arg(allow_hyphen_values = true)]
        rationale: String,
        /// Owned obligation IDs.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        obligations: Vec<String>,
    },
    /// Print the reconciled representative PNG binding record.
    ReconcileStage4Bindings {
        /// Qualified native canary record.
        proof: String,
    },
    /// Verify a representative case owner.
    VerifyRepresentativeOwner {
        /// Representative case.
        case: String,
    },
    /// Record a representative case owner.
    RecordRepresentativeOwner {
        /// Representative case.
        case: String,
        /// Reviewed rationale.
        #[arg(allow_hyphen_values = true)]
        rationale: String,
    },
    /// Record the exception test as owner of obligations.
    RecordExceptionTestOwner {
        /// Reviewed rationale.
        #[arg(allow_hyphen_values = true)]
        rationale: String,
        /// Owned obligation IDs.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        obligations: Vec<String>,
    },
    /// Refresh the exception test owner source binding.
    RefreshExceptionTestOwnerSource,
    /// Record a library test as owner of obligations.
    RecordLibraryTestOwner {
        /// Library test module.
        module: String,
        /// Test function.
        function: String,
        /// Reviewed rationale.
        #[arg(allow_hyphen_values = true)]
        rationale: String,
        /// Owned obligation IDs.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        obligations: Vec<String>,
    },
    /// Refresh a library test owner source binding.
    RefreshLibraryTestOwnerSource {
        /// Library test module.
        module: String,
    },
    /// Refresh a test owner source binding.
    RefreshTestOwnerSource {
        /// Test module.
        module: String,
    },
    /// Record a closure obligation owner.
    RecordClosureOwner {
        /// Closure obligation.
        obligation: String,
        /// Record verifier.
        verifier: String,
        /// Reviewed rationale.
        #[arg(allow_hyphen_values = true)]
        rationale: String,
    },
    /// Print the inventory digest.
    InventoryDigest,
    /// Verify the obligation inventory.
    VerifyInventory,
    /// Rebind font packaging evidence.
    RebindFontPackaging {
        /// Previous commit.
        base: String,
        /// Current snapshot directory.
        candidate: String,
        /// Receipt.
        receipt: Option<String>,
    },
    /// Rebind font packaging ownership.
    RebindFontPackagingOwnership {
        /// Previous commit.
        base: String,
        /// Final spec SHA-256.
        spec: String,
        /// Receipt.
        receipt: Option<String>,
    },
    /// Verify a result bundle.
    VerifyResults {
        /// Result bundle path.
        report: String,
    },
    /// Verify native snapshots.
    VerifyNativeSnapshots {
        /// Snapshot directory.
        directory: String,
    },
    /// Stage WASM snapshots.
    StageWasmSnapshots,
    /// Print the test filter for a test group.
    TestGroupFilter {
        /// Test group manifest.
        manifest: String,
    },
    /// Build the native release CLI.
    BuildNativeCli,
    /// Refresh test owner source bindings.
    RefreshTestOwnerSources {
        /// Test modules.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        modules: Vec<String>,
    },
    /// Stage trace logs.
    StageTraces,
    /// Aggregate trace logs.
    AggregateTraces {
        /// Trace log directory.
        directory: String,
    },
    /// Join trace and public results.
    JoinTraceResults {
        /// Trace/public result directory.
        directory: String,
    },
    /// Verify joined trace and public results.
    VerifyJoinedResults {
        /// Trace/public result directory.
        directory: String,
    },
    /// Verify WASM snapshots.
    VerifyWasmSnapshots {
        /// WASM log directory.
        directory: String,
    },
    /// Report WASM snapshots.
    ReportWasmSnapshots {
        /// WASM log directory.
        directory: String,
    },
    /// Stage a new native baseline.
    StageNativeBaseline {
        /// Snapshot directory.
        snapshots: String,
        /// New baseline directory.
        baseline: String,
    },
    /// Verify the native baseline.
    VerifyNativeBaseline,
    /// Replay native snapshots.
    ReplayNativeSnapshots {
        /// Snapshot directory.
        snapshots: String,
    },
    /// Generate PDF judgments.
    PdfJudgmentsGenerate,
    /// Verify PDF judgments.
    VerifyPdfJudgments,
    /// Print the read-only, advisory five-layer report.
    LayersReport,
    /// Print the five-layer report, or audit it with --strict.
    VerifyLayers {
        /// Audit the layer records instead of reporting.
        #[arg(long)]
        strict: bool,
    },
    /// Verify the closure record.
    VerifyClosure,
    /// Verify a registered case.
    Verify {
        /// Case ID.
        case_id: String,
    },
    /// Verify every registered case.
    VerifyAll,
    /// Prepare candidate assets.
    CandidatePrepareAssets {
        /// Candidate directory.
        directory: String,
    },
    /// Print candidate comparisons.
    CandidateCompare {
        /// Viewer collection.
        collection: String,
    },
    /// Print candidate semantics.
    CandidateSemantics {
        /// Candidate directory.
        directory: String,
    },
    /// Materialize a candidate.
    CandidateMaterialize {
        /// Candidate directory.
        directory: String,
    },
    /// Execute a candidate.
    CandidateVerify {
        /// Candidate directory.
        directory: String,
    },
    /// Capture native candidate artifacts.
    CandidateNativeCapture {
        /// Candidate directory.
        directory: String,
    },
    /// Replay native candidate artifacts.
    CandidateNativeCheck {
        /// Candidate directory.
        directory: String,
    },
    /// Qualify the Excalifont text canary.
    QualifyExcalifontCanary {
        /// Independent canary evidence directory.
        directory: String,
    },
    /// Inspect a candidate.
    CandidateInspect {
        /// Candidate directory.
        directory: String,
    },
    /// Propose candidate exceptions.
    CandidateProposeExceptions {
        /// Candidate directory.
        directory: String,
    },
    /// Revise a candidate.
    CandidateRevise {
        /// New candidate directory.
        directory: String,
    },
    /// Execute every registered candidate.
    CandidateExecutions,
    /// Verify canonical SVG viewer qualification.
    VerifySvgViewers,
    /// Verify canonical SVG font subset viewer qualification.
    VerifySvgFontSubsets,
    /// Verify SVG font subset release evidence.
    VerifySvgFontSubsetRelease,
    /// Materialize SVG font subset target evidence.
    MaterializeSvgFontSubsetTarget {
        /// Target.
        target: String,
        /// Qualification test log.
        test_log: String,
        /// Target report path.
        output: String,
    },
    /// Materialize the cross-target SVG font subset measurement.
    MaterializeSvgFontSubsetCrossTarget {
        /// Qualification test log.
        test_log: String,
    },
    /// Aggregate SVG font subset target evidence.
    AggregateSvgFontSubsetTargets {
        /// Target report directory.
        input: String,
        /// Aggregate report path.
        output: String,
    },
    /// Record the SVG font subset target aggregate into the release evidence.
    RecordSvgFontSubsetTargetAggregate {
        /// Downloaded aggregate report.
        aggregate: String,
        /// Workflow run URL.
        run_url: String,
    },
    /// Generate the pure-Rust SVG font subset viewer report.
    GenerateSvgFontSubsetRustReport {
        /// Viewer input manifest.
        inputs: String,
        /// Canonical pure-Rust viewer executable.
        viewer: String,
        /// Viewer report path.
        output: String,
    },
    /// Materialize SVG font subset viewer evidence.
    MaterializeSvgFontSubsetEvidence {
        /// Viewer input manifest.
        inputs: String,
        /// Chromium viewer report.
        chromium: String,
        /// Pure-Rust viewer report.
        pure_rust: String,
    },
    /// Verify canonical PDF viewer and structural qualification.
    VerifyPdfViewers,
    /// Refresh the Rust PDFium runtime record.
    RefreshPdfiumRuntime,
    /// Generate a case with the reference oracle, then verify it.
    Generate {
        /// Case ID.
        case_id: String,
    },
    /// Reproduce a case with the reference oracle, then verify it.
    Reproduce {
        /// Case ID.
        case_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum RepresentativeCommand {
    /// Prepare an intake bundle.
    Prepare {
        /// Source scene.
        source: PathBuf,
        /// Plan.
        plan: PathBuf,
        /// Output bundle.
        output: PathBuf,
    },
    /// Verify an intake bundle.
    Verify {
        /// Bundle.
        bundle: PathBuf,
    },
    /// Capture native observations for a bundle.
    Capture {
        /// Bundle.
        bundle: PathBuf,
        /// Renderer CLI.
        renderer: PathBuf,
        /// Output.
        output: PathBuf,
        /// Process timeout in seconds.
        #[arg(
            long,
            default_value_t = 180.0,
            allow_negative_numbers = true,
            value_parser = parsed::<f64>("invalid process timeout")
        )]
        timeout: f64,
    },
    /// Verify a native capture.
    VerifyCapture {
        /// Bundle.
        bundle: PathBuf,
        /// Output.
        output: PathBuf,
    },
    /// Registered case verification and judgment.
    #[command(subcommand)]
    Case(CaseCommand),
    /// Identical-input viewer controls.
    Controls(ControlsArgs),
    /// Evidence collection and verification.
    #[command(subcommand)]
    Evidence(EvidenceCommand),
    /// Viewer asset collection and verification.
    #[command(subcommand)]
    ViewerAssets(ViewerAssetsCommand),
    /// JSON-only evidence retention.
    #[command(subcommand)]
    Compact(CompactCommand),
}

#[derive(Debug, Subcommand)]
pub enum CaseCommand {
    /// Verify a registered case.
    Verify {
        /// Case directory.
        directory: PathBuf,
        #[command(flatten)]
        replay: Replay,
    },
    /// Verify every registered case.
    VerifyRegistry {
        #[command(flatten)]
        replay: Replay,
    },
    /// Judge retained rasters for a case.
    Judge {
        /// Case directory.
        directory: PathBuf,
        /// Viewer collection.
        collection: PathBuf,
    },
    /// Print a case subject digest.
    Subject {
        /// Case directory.
        directory: PathBuf,
    },
    /// Create an unapproved case from explicit expectations.
    Materialize(MaterializeArgs),
}

#[derive(Debug, Args)]
pub struct Replay {
    /// Renderer CLI to replay native captures with.
    #[arg(long)]
    pub renderer: Option<PathBuf>,
    /// Also replay viewers.
    #[arg(long, requires = "renderer")]
    pub viewers: bool,
}

#[derive(Debug, Args)]
pub struct MaterializeArgs {
    /// Case output directory.
    pub output: PathBuf,
    /// Intake bundle.
    #[arg(long)]
    pub intake: PathBuf,
    /// Reference.
    #[arg(long)]
    pub reference: PathBuf,
    /// Evidence record.
    #[arg(long)]
    pub evidence_record: PathBuf,
    /// Expectation spec.
    #[arg(long)]
    pub spec: PathBuf,
}

#[derive(Debug, Args)]
pub struct ControlsArgs {
    /// Viewer variant directory.
    pub directory: String,
    /// Comparison to classify; defaults to every comparison.
    #[arg(long = "comparison")]
    pub comparisons: Vec<String>,
    /// Tile size.
    #[arg(long, value_parser = parsed::<u32>("invalid tile size"))]
    pub tiles: Option<u32>,
    /// Create-only report path.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum EvidenceCommand {
    /// Collect evidence.
    Collect(EvidenceArgs),
    /// Verify evidence.
    Verify(EvidenceArgs),
}

#[derive(Debug, Args)]
pub struct EvidenceArgs {
    /// Bundle.
    pub bundle: PathBuf,
    /// Reference.
    pub reference: PathBuf,
    /// Native capture.
    pub native: PathBuf,
    /// Output.
    pub output: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum ViewerAssetsCommand {
    /// Collect viewer assets.
    Collect {
        /// Directory.
        directory: PathBuf,
    },
    /// Verify viewer assets.
    Verify {
        /// Directory.
        directory: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CompactCommand {
    /// Retain compact evidence in a new output.
    Retain {
        /// Bundle.
        bundle: PathBuf,
        /// Reference.
        reference: PathBuf,
        /// Native.
        native: PathBuf,
        /// Viewers.
        viewers: PathBuf,
        /// New output.
        output: PathBuf,
    },
    /// Verify retained integrity offline.
    Verify {
        /// Output.
        output: PathBuf,
    },
    /// Validate a full collection at the recorded roots.
    Check {
        /// Output.
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum DemoCommand {
    /// Render every example in tools/demo/examples.json and write a static page.
    Build {
        /// Output directory, replaced on each build [default: target/demo].
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum PdfHarnessCommand {
    /// Generate handwritten calibration fixtures.
    GenerateFixtures {
        /// Output directory.
        output: PathBuf,
    },
    /// Inspect a PDF against a structural expectation.
    Inspect {
        /// Input PDF.
        input: PathBuf,
        /// Expectation JSON.
        expectation: PathBuf,
        /// Expected outcome.
        expected: Outcome,
        /// Output report JSON.
        output: PathBuf,
    },
}

/// Parses a value with the command's established diagnostic; range checks
/// stay with the command.
fn parsed<T: std::str::FromStr>(
    message: &'static str,
) -> impl Fn(&str) -> Result<T, &'static str> + Clone {
    move |value| value.parse().map_err(|_| message)
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Outcome {
    Pass,
    Fail,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Family {
        Cli::try_parse_from(std::iter::once("xtask").chain(args.iter().copied()))
            .unwrap()
            .family
    }

    #[test]
    fn grammar_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn subcommand_names_match_established_invocations() {
        let command = Cli::command();
        let names = |family: &str| {
            command
                .find_subcommand(family)
                .unwrap()
                .get_subcommands()
                .map(|c| c.get_name().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names("catalog"),
            ["verify", "import", "materialize", "import-license-sources"]
        );
        assert_eq!(
            names("conformance"),
            [
                "inventory",
                "ownership-catalog",
                "ownership-authority-migration",
                "seed-ownership",
                "verify-ownership",
                "extend-ownership",
                "record-test-owner",
                "record-case-owner",
                "reconcile-stage4-bindings",
                "verify-representative-owner",
                "record-representative-owner",
                "record-exception-test-owner",
                "refresh-exception-test-owner-source",
                "record-library-test-owner",
                "refresh-library-test-owner-source",
                "refresh-test-owner-source",
                "record-closure-owner",
                "inventory-digest",
                "verify-inventory",
                "rebind-font-packaging",
                "rebind-font-packaging-ownership",
                "verify-results",
                "verify-native-snapshots",
                "stage-wasm-snapshots",
                "test-group-filter",
                "build-native-cli",
                "refresh-test-owner-sources",
                "stage-traces",
                "aggregate-traces",
                "join-trace-results",
                "verify-joined-results",
                "verify-wasm-snapshots",
                "report-wasm-snapshots",
                "stage-native-baseline",
                "verify-native-baseline",
                "replay-native-snapshots",
                "pdf-judgments-generate",
                "verify-pdf-judgments",
                "layers-report",
                "verify-layers",
                "verify-closure",
                "verify",
                "verify-all",
                "candidate-prepare-assets",
                "candidate-compare",
                "candidate-semantics",
                "candidate-materialize",
                "candidate-verify",
                "candidate-native-capture",
                "candidate-native-check",
                "qualify-excalifont-canary",
                "candidate-inspect",
                "candidate-propose-exceptions",
                "candidate-revise",
                "candidate-executions",
                "verify-svg-viewers",
                "verify-svg-font-subsets",
                "verify-svg-font-subset-release",
                "materialize-svg-font-subset-target",
                "materialize-svg-font-subset-cross-target",
                "aggregate-svg-font-subset-targets",
                "record-svg-font-subset-target-aggregate",
                "generate-svg-font-subset-rust-report",
                "materialize-svg-font-subset-evidence",
                "verify-pdf-viewers",
                "refresh-pdfium-runtime",
                "generate",
                "reproduce",
            ]
        );
        assert_eq!(
            names("representative"),
            [
                "prepare",
                "verify",
                "capture",
                "verify-capture",
                "case",
                "controls",
                "evidence",
                "viewer-assets",
                "compact"
            ]
        );
        assert_eq!(names("pdf-harness"), ["generate-fixtures", "inspect"]);
    }

    #[test]
    fn test_owner_keeps_options_empty_module_and_trailing_identifiers() {
        let Family::Conformance(ConformanceCommand::RecordTestOwner {
            package,
            binary,
            module,
            function,
            rationale,
            obligations,
        }) = parse(&[
            "conformance",
            "record-test-owner",
            "--package",
            "p",
            "--binary",
            "b",
            "",
            "f",
            "-reviewed",
            "a",
            "--b",
            "",
            "a",
        ])
        else {
            panic!()
        };
        assert_eq!((package.as_str(), binary.as_str()), ("p", "b"));
        assert_eq!((module.as_str(), function.as_str()), ("", "f"));
        assert_eq!(rationale, "-reviewed");
        assert_eq!(obligations, ["a", "--b", "", "a"]);

        let Family::Conformance(ConformanceCommand::RecordTestOwner {
            package, binary, ..
        }) = parse(&["conformance", "record-test-owner", "m", "f", "r"])
        else {
            panic!()
        };
        assert_eq!(
            (package.as_str(), binary.as_str()),
            ("excalidraw-render", "fast")
        );
    }

    #[test]
    fn optional_and_repeated_values_are_preserved() {
        let Family::Conformance(ConformanceCommand::RebindFontPackaging { receipt, .. }) =
            parse(&["conformance", "rebind-font-packaging", "a", "b"])
        else {
            panic!()
        };
        assert_eq!(receipt, None);
        let Family::Representative(RepresentativeCommand::Controls(controls)) = parse(&[
            "representative",
            "controls",
            "d",
            "--comparison",
            "x",
            "--comparison",
            "x",
        ]) else {
            panic!()
        };
        // Duplicate comparisons are a domain error reported by the command.
        assert_eq!(controls.comparisons, ["x", "x"]);
        let Family::Conformance(ConformanceCommand::RefreshTestOwnerSources { modules }) =
            parse(&["conformance", "refresh-test-owner-sources"])
        else {
            panic!()
        };
        assert!(modules.is_empty());
    }

    #[test]
    fn dependent_options_accept_either_order() {
        for args in [
            ["--renderer", "cli", "--viewers"],
            ["--viewers", "--renderer", "cli"],
        ] {
            let Family::Representative(RepresentativeCommand::Case(CaseCommand::VerifyRegistry {
                replay,
            })) = parse(&[&["representative", "case", "verify-registry"][..], &args].concat())
            else {
                panic!()
            };
            assert!(replay.viewers);
            assert_eq!(replay.renderer, Some(PathBuf::from("cli")));
        }
    }

    #[test]
    fn retired_guidance_precedes_parsing() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(retired(&args(&["conformance", "record-layers-approval"]))
            .unwrap()
            .starts_with("approval commands are retired"));
        assert!(retired(&args(&["representative", "approval"])).is_none());
        assert!(retired(&args(&["conformance"])).is_none());
    }
}
