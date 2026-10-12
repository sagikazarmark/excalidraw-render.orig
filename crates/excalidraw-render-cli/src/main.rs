use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::num::IntErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use excalidraw_render::{
    license_bundle, ColorMode, CompatibilityDiagnostic, CompatibilityRevision, ErrorDescriptor,
    FrameOptions, MetadataOptions, OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest,
    Renderer, ResourceLimits, SceneDocument, Sizing, SupplementalAssets,
};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use tempfile::TempPath;

mod frames;
mod human;

use frames::{run_frames, FramesCli};
use human::HumanReport;

const EXIT_OPERATIONAL: u8 = 1;
const EXIT_USAGE: u8 = 2;
const MAX_SCENE_BYTES: u64 = 67_108_864;

const LONG_ABOUT: &str = "\
Render an Excalidraw scene document (.excalidraw JSON) to SVG, PNG, or PDF.

Rendering needs no browser, Node.js, network access, or installed fonts. The
output extension selects the format; use --format for standard output or other
names. Warnings describing substitutions, omissions, and fallbacks are printed
to stderr; use --report-format json for a machine-readable report.";

const EXAMPLES: &str = "\
Examples:
  excalidraw-render drawing.excalidraw -o drawing.svg
  excalidraw-render drawing.excalidraw -o drawing.png --scale 2
  excalidraw-render drawing.excalidraw -o drawing.pdf --max-dimension 4096
  excalidraw-render - -o - --format svg < drawing.excalidraw > drawing.svg
  excalidraw-render frames drawing.excalidraw --list
  excalidraw-render frames drawing.excalidraw -o frames --format png";

const EXIT_STATUS: &str = "\
Exit status:
  0  the artifact was delivered (warnings may still have been reported)
  1  reading, parsing, rendering, or writing failed, or an output already exists
  2  the command line or a request value is invalid";

/// The whole command line: the default scene render, or a subcommand.
#[derive(Parser)]
#[command(
    name = "excalidraw-render",
    about = "Render Excalidraw scene documents to SVG, PNG, or PDF",
    long_about = LONG_ABOUT,
    after_help = EXAMPLES,
    after_long_help = format!("{EXAMPLES}\n\n{EXIT_STATUS}"),
    disable_version_flag = true,
    args_conflicts_with_subcommands = true,
    arg_required_else_help = true,
    subcommand_value_name = "COMMAND",
    subcommand_help_heading = "Commands"
)]
struct Invocation {
    #[command(subcommand)]
    command: Option<CliCommand>,
    #[command(flatten)]
    render: Cli,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Render each selected frame to its own file in a directory
    #[command(
        long_about = frames::LONG_ABOUT,
        after_help = frames::EXAMPLES,
        disable_version_flag = true
    )]
    Frames(FramesCli),
}

#[derive(Clone, Copy, ValueEnum)]
enum FormatArg {
    Svg,
    Png,
    Pdf,
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Svg => Self::Svg,
            FormatArg::Png => Self::Png,
            FormatArg::Pdf => Self::Pdf,
        }
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
enum ReportFormat {
    /// Readable warnings and errors
    #[default]
    Human,
    /// One JSON report document
    Json,
    /// Nothing on success; one error line on failure
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum PdfFallbackArg {
    /// Rasterize content PDF cannot express as an isolated image
    AllowIsolated,
    /// Fail instead of rasterizing
    Forbid,
}

const OUTPUT: &str = "Output";
const RENDERING: &str = "Rendering";
const FRAMES: &str = "Frames";
const METADATA: &str = "Embedded metadata";
const ASSETS: &str = "Image assets";
const REPORTING: &str = "Reporting";
const INFORMATION: &str = "Information";

/// The default scene render and the standalone information commands.
#[derive(Args)]
struct Cli {
    /// Excalidraw scene file (.excalidraw JSON), or - to read standard input
    input: Option<String>,

    /// Output file, or - for standard output; a .svg, .png, or .pdf extension
    /// selects the format
    #[arg(
        short,
        long,
        value_name = "FILE",
        allow_hyphen_values = true,
        help_heading = OUTPUT
    )]
    output: Option<String>,
    /// Output format; required for standard output or an unrecognized
    /// extension
    #[arg(short, long, value_enum, help_heading = OUTPUT)]
    format: Option<FormatArg>,
    /// Replace existing output and report files
    #[arg(long, help_heading = OUTPUT)]
    force: bool,

    /// Space around the drawing, in scene pixels [default: 10]
    #[arg(
        long,
        value_name = "PX",
        allow_negative_numbers = true,
        help_heading = RENDERING
    )]
    padding: Option<f64>,
    /// Multiply the output size by FACTOR [default: 1]
    #[arg(
        long,
        value_name = "FACTOR",
        allow_negative_numbers = true,
        conflicts_with = "max_dimension",
        help_heading = RENDERING
    )]
    scale: Option<f64>,
    /// Shrink PNG and PDF output so its longer side is at most PX pixels
    /// (never enlarges; SVG keeps its size; replaces --scale)
    #[arg(
        long,
        value_name = "PX",
        allow_negative_numbers = true,
        help_heading = RENDERING
    )]
    max_dimension: Option<f64>,
    /// Use a transparent background instead of the scene's background color
    #[arg(long, help_heading = RENDERING)]
    no_background: bool,
    /// Render with Excalidraw's dark theme
    #[arg(long, help_heading = RENDERING)]
    dark_mode: bool,
    /// Whether PDF output may rasterize isolated content that vector PDF cannot
    /// express [default: allow-isolated]
    #[arg(long, value_enum, value_name = "MODE", help_heading = RENDERING)]
    pdf_raster_fallback: Option<PdfFallbackArg>,
    // Hidden, like --list-revisions, while the renderer implements a single
    // revision: there is nothing to choose between yet.
    /// Compatibility revision to render with [default: 1; see
    /// --list-revisions]
    #[arg(long, value_name = "N", hide = true, help_heading = RENDERING)]
    compatibility_revision: Option<String>,

    /// Turn frame rendering off: no frame clipping, names, or outlines
    #[arg(long, help_heading = FRAMES)]
    no_frame_rendering: bool,
    /// Do not clip frame content to the frame
    #[arg(long, help_heading = FRAMES)]
    no_frame_clipping: bool,
    /// Do not draw frame names
    #[arg(long, help_heading = FRAMES)]
    no_frame_names: bool,
    /// Do not draw frame outlines
    #[arg(long, help_heading = FRAMES)]
    no_frame_outlines: bool,

    /// Embed the scene so Excalidraw can reopen the exported file for editing
    #[arg(long, help_heading = METADATA)]
    embed_scene: bool,
    /// Embed the renderer version and the Excalidraw release it matches
    #[arg(long, help_heading = METADATA)]
    embed_provenance: bool,
    /// Embed a summary of the rendering warnings
    #[arg(long, help_heading = METADATA)]
    embed_diagnostics: bool,

    /// Supply the bytes of image FILE_ID from PATH when the scene does not
    /// embed them (repeatable)
    #[arg(long, value_name = "FILE_ID=PATH", help_heading = ASSETS)]
    asset: Vec<String>,
    /// Declare the media type of a supplied --asset, e.g. logo=image/png
    /// (repeatable)
    #[arg(long, value_name = "FILE_ID=MEDIA_TYPE", help_heading = ASSETS)]
    asset_mime: Vec<String>,

    /// How to report warnings and errors on stderr
    #[arg(long, value_enum, default_value_t, help_heading = REPORTING)]
    report_format: ReportFormat,
    /// Write the report to PATH instead of stderr
    #[arg(long, value_name = "PATH", help_heading = REPORTING)]
    report_output: Option<String>,
    /// Also print informational messages in the human report
    #[arg(short, long, help_heading = REPORTING)]
    verbose: bool,

    /// List the supported compatibility revisions and exit
    #[arg(long, hide = true, help_heading = INFORMATION)]
    list_revisions: bool,
    /// Print the version and the Excalidraw release it matches, then exit
    /// (add --verbose for the exact build identity to quote in bug reports)
    #[arg(short = 'V', long, help_heading = INFORMATION)]
    version: bool,
    /// Print the bundled third-party license notices, then exit
    #[arg(long, help_heading = INFORMATION)]
    licenses: bool,
}

#[derive(Serialize)]
struct ArtifactDescriptor {
    media_type: &'static str,
    byte_length: usize,
    sha256: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ReportError {
    Library(ErrorDescriptor),
    Cli(CliError),
}

impl ReportError {
    fn code(&self) -> &str {
        match self {
            Self::Library(error) => error.code().as_str(),
            Self::Cli(error) => error.code,
        }
    }

    /// Advice the code alone does not imply: only the message tells a PNG or
    /// PDF the sizing shrank to nothing from other encoding failures.
    fn sizing_hint(&self) -> Option<&'static str> {
        match self {
            Self::Library(error) => human::sizing_hint(error.code().as_str(), error.message()),
            Self::Cli(_) => None,
        }
    }

    /// The message a person reads: the report message, with the element a
    /// library failure names.
    fn human_message(&self) -> String {
        match self {
            Self::Library(error) => match error.element_id() {
                Some(element) => format!("{} (element {element})", error.message()),
                None => error.message().to_owned(),
            },
            Self::Cli(error) => error.message.clone(),
        }
    }
}

#[derive(Serialize)]
struct CliError {
    family: &'static str,
    category: Option<&'static str>,
    code: &'static str,
    message: String,
    format: Option<OutputFormat>,
    element_id: Option<String>,
    json_pointer: Option<String>,
    details: Option<CliDetails>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CliDetails {
    ParseLimit {
        field: &'static str,
        configured: u64,
        value_kind: &'static str,
        measurement: CliMeasurement,
    },
    ResourceLimit {
        field: &'static str,
        configured: u64,
        value_kind: &'static str,
        measurement: CliMeasurement,
        recovery: Option<&'static str>,
    },
    CliArgument {
        argument: String,
        value: Option<String>,
    },
    CliPath {
        entries: Vec<CliPathEntry>,
    },
    CliIo {
        role: IoRole,
        asset_id: Option<String>,
        operation: CliOperation,
        path: String,
    },
    Delivery {
        role: DeliveryRole,
        operation: CliOperation,
        path: String,
        artifact_state: DeliveryState,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CliMeasurement {
    AtLeast { value: u64 },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum IoRole {
    Input,
    Asset,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DeliveryRole {
    Output,
    Report,
}

impl DeliveryRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Output => "output",
            Self::Report => "report",
        }
    }
}

impl IoRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Asset => "asset",
        }
    }
}

#[derive(Clone, Copy)]
enum CliOperation {
    Open,
    Read,
    Create,
    Write,
    Flush,
    Replace,
}

impl CliOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Read => "read",
            Self::Create => "create",
            Self::Write => "write",
            Self::Flush => "flush",
            Self::Replace => "replace",
        }
    }
}

#[derive(Clone, Copy)]
enum DeliveryState {
    None,
    Complete,
    PossiblyPartial,
}

impl DeliveryState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Complete => "complete",
            Self::PossiblyPartial => "possibly_partial",
        }
    }
}

#[derive(Serialize)]
struct CliPathEntry {
    role: &'static str,
    asset_id: Option<String>,
    path: String,
}

#[derive(Serialize)]
struct RenderReport<'a> {
    schema_version: u32,
    status: &'static str,
    renderer_version: &'static str,
    compatibility_revision: Option<CompatibilityRevision>,
    excalidraw_release: Option<&'static str>,
    reference_profile_digest: Option<&'static str>,
    format: Option<OutputFormat>,
    artifact: Option<ArtifactDescriptor>,
    error: Option<Box<ReportError>>,
    diagnostics: &'a [CompatibilityDiagnostic],
}

#[derive(Serialize)]
struct RevisionReport {
    schema_version: u32,
    renderer_version: &'static str,
    revisions: Vec<RevisionEntry>,
}

#[derive(Serialize)]
struct RevisionEntry {
    compatibility_revision: CompatibilityRevision,
    excalidraw_release: &'static str,
    reference_profile_digest: &'static str,
    default: bool,
}

/// Serializes each unit enum as its `as_str` spelling, so reports and human
/// messages share one spelling.
macro_rules! serialize_as_str {
    ($($name:ty),*) => {$(
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }
    )*};
}

serialize_as_str!(IoRole, DeliveryRole, CliOperation, DeliveryState);

struct SelectedIdentity {
    revision: CompatibilityRevision,
    release: &'static str,
    digest: &'static str,
}

struct Failure {
    error: Box<ReportError>,
    exit: u8,
    diagnostics: Vec<CompatibilityDiagnostic>,
    /// Human-report wording that replaces the report message, when the CLI
    /// knows more context (the input path, a JSON line and column).
    human_message: Option<String>,
    /// Human-report advice beyond what the error code implies.
    hints: Vec<String>,
}

impl Failure {
    fn cli(
        code: &'static str,
        message: impl Into<String>,
        details: Option<CliDetails>,
        exit: u8,
    ) -> Self {
        let message = message.into();
        Self {
            error: Box::new(ReportError::Cli(CliError {
                family: "cli",
                category: None,
                code,
                message: message.clone(),
                format: None,
                element_id: None,
                json_pointer: None,
                details: Some(details.unwrap_or_else(|| CliDetails::CliArgument {
                    argument: "command_line".to_owned(),
                    value: None,
                })),
            })),
            exit,
            diagnostics: Vec::new(),
            human_message: None,
            hints: Vec::new(),
        }
    }

    fn library(descriptor: ErrorDescriptor, exit: u8) -> Self {
        Self {
            error: Box::new(ReportError::Library(descriptor)),
            exit,
            diagnostics: Vec::new(),
            human_message: None,
            hints: Vec::new(),
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(hint.into());
        self
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().collect();
    let preparse_json = preparse_json_mode(&args);
    match Invocation::try_parse_from(&args) {
        Ok(Invocation {
            command: Some(CliCommand::Frames(frames)),
            ..
        }) => ExitCode::from(run_frames(frames)),
        Ok(Invocation { render, .. }) => ExitCode::from(run(render)),
        Err(error) => command_line_error(&error, preparse_json),
    }
}

/// Prints requested help, or reports a command-line grammar error the way the
/// preparsed report mode asks for.
fn command_line_error(error: &clap::Error, preparse_json: bool) -> ExitCode {
    use clap::error::ErrorKind;
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            if error.print().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_OPERATIONAL)
            }
        }
        // A bare invocation shows the help on stderr as a usage error.
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = error.print();
            ExitCode::from(EXIT_USAGE)
        }
        _ => {
            let rendered = error.to_string();
            let bytes = if preparse_json {
                let failure = Failure::cli(
                    "cli.invalid-arguments",
                    rendered.trim().to_owned(),
                    None,
                    EXIT_USAGE,
                );
                failure_report_bytes(ReportFormat::Json, None, None, failure, HumanReport::PLAIN)
            } else {
                human::clap_error(&rendered, "cli.invalid-arguments").into_bytes()
            };
            let _ = write_stream(&mut io::stderr().lock(), &bytes);
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn validate_report_options(cli: &Cli) -> Result<(), Failure> {
    if cli.report_output.as_deref() == Some("-") {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "--report-output must be stderr or a filesystem path other than -",
            Some(CliDetails::CliArgument {
                argument: "--report-output".to_owned(),
                value: Some("-".to_owned()),
            }),
            EXIT_USAGE,
        ));
    }
    if cli.report_format == ReportFormat::None && cli.report_output.is_some() {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "--report-output cannot be used with --report-format none",
            None,
            EXIT_USAGE,
        ));
    }
    Ok(())
}

fn run(cli: Cli) -> u8 {
    if cli.version || cli.licenses || cli.list_revisions {
        return run_standalone(&cli);
    }

    let mut identity = None;
    let mut selected_format = None;
    let result = run_render(&cli, &mut identity, &mut selected_format);
    match result {
        Ok((artifact, diagnostics)) => deliver_success(
            &cli,
            identity.as_ref().expect("render identity selected"),
            selected_format.expect("render format selected"),
            artifact,
            diagnostics,
        ),
        Err(failure) => emit_failure(&cli, identity.as_ref(), selected_format, failure),
    }
}

fn run_standalone(cli: &Cli) -> u8 {
    let count =
        usize::from(cli.version) + usize::from(cli.licenses) + usize::from(cli.list_revisions);
    let render_controls = cli.input.is_some()
        || cli.output.is_some()
        || cli.format.is_some()
        || cli.compatibility_revision.is_some()
        || cli.no_background
        || cli.padding.is_some()
        || cli.scale.is_some()
        || cli.max_dimension.is_some()
        || cli.dark_mode
        || cli.embed_scene
        || cli.embed_provenance
        || cli.embed_diagnostics
        || cli.no_frame_rendering
        || cli.no_frame_clipping
        || cli.no_frame_names
        || cli.no_frame_outlines
        || !cli.asset.is_empty()
        || !cli.asset_mime.is_empty()
        || cli.pdf_raster_fallback.is_some()
        || cli.force;
    let invalid = count != 1
        || render_controls
        || cli.report_output.as_deref() == Some("-")
        || ((cli.version || cli.licenses)
            && (cli.report_format != ReportFormat::Human || cli.report_output.is_some()));
    if invalid {
        return emit_failure(
            cli,
            None,
            None,
            Failure::cli(
                "cli.invalid-arguments",
                "standalone commands cannot be combined with render options",
                None,
                EXIT_USAGE,
            ),
        );
    }

    let profile = Renderer::new().profile_identity();
    if cli.version {
        let release = profile.excalidraw_release();
        let mut text = format!(
            "excalidraw-render {} (Excalidraw {})\n",
            env!("CARGO_PKG_VERSION"),
            release.strip_prefix('v').unwrap_or(release)
        );
        // The exact identity a bug report or reproduction pins; JSON reports
        // carry it too.
        if cli.verbose {
            text.push_str(&format!(
                "compatibility revision {}\nreference profile {}\n",
                profile.compatibility_revision(),
                profile.profile_digest()
            ));
        }
        return write_stdout(text.as_bytes());
    }
    if cli.licenses {
        return write_stdout(license_bundle().as_bytes());
    }

    // The renderer implements exactly one revision, which is also the default.
    let revisions = vec![RevisionEntry {
        compatibility_revision: profile.compatibility_revision(),
        excalidraw_release: profile.excalidraw_release(),
        reference_profile_digest: profile.profile_digest(),
        default: true,
    }];
    let bytes = match cli.report_format {
        ReportFormat::Human => revisions
            .iter()
            .map(|entry| {
                format!(
                    "{}\t{}\t{}{}\n",
                    entry.compatibility_revision,
                    entry.excalidraw_release,
                    entry.reference_profile_digest,
                    if entry.default { "\tdefault" } else { "" }
                )
            })
            .collect::<String>()
            .into_bytes(),
        ReportFormat::Json => json_line(&RevisionReport {
            schema_version: 1,
            renderer_version: env!("CARGO_PKG_VERSION"),
            revisions,
        }),
        ReportFormat::None => {
            return emit_failure(
                cli,
                None,
                None,
                Failure::cli(
                    "cli.invalid-arguments",
                    "--list-revisions does not accept --report-format none",
                    None,
                    EXIT_USAGE,
                ),
            )
        }
    };
    if report_output(cli) != "stderr" && destination_is_taken(Path::new(report_output(cli))) {
        return emit_failure(
            cli,
            None,
            None,
            Failure::cli(
                "cli.destination-exists",
                "report destination already exists",
                Some(CliDetails::Delivery {
                    role: DeliveryRole::Report,
                    operation: CliOperation::Create,
                    path: report_output(cli).to_owned(),
                    artifact_state: DeliveryState::None,
                }),
                EXIT_OPERATIONAL,
            ),
        );
    }
    deliver_report_bytes(cli, &bytes, DeliveryState::None)
}

fn run_render(
    cli: &Cli,
    identity: &mut Option<SelectedIdentity>,
    selected_format: &mut Option<OutputFormat>,
) -> Result<(Vec<u8>, Vec<CompatibilityDiagnostic>), Failure> {
    if cli.report_output.as_deref() == Some("-") {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "--report-output must be stderr or a filesystem path other than -",
            Some(CliDetails::CliArgument {
                argument: "--report-output".to_owned(),
                value: Some("-".to_owned()),
            }),
            EXIT_USAGE,
        ));
    }
    if cli.report_format == ReportFormat::None && cli.report_output.is_some() {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "--report-output cannot be used with --report-format none",
            Some(CliDetails::CliArgument {
                argument: "--report-output".to_owned(),
                value: cli.report_output.clone(),
            }),
            EXIT_USAGE,
        ));
    }
    let input = cli.input.as_deref().ok_or_else(|| {
        Failure::cli(
            "cli.invalid-arguments",
            "INPUT is required",
            None,
            EXIT_USAGE,
        )
    })?;
    let output = cli.output.as_deref().ok_or_else(|| {
        Failure::cli(
            "cli.invalid-arguments",
            "--output is required",
            None,
            EXIT_USAGE,
        )
        .with_hint("pass -o FILE, or -o - with --format to write standard output")
    })?;

    let renderer = select_renderer(cli.compatibility_revision.as_deref())?;
    let profile = renderer.profile_identity();
    *identity = Some(SelectedIdentity {
        revision: profile.compatibility_revision(),
        release: profile.excalidraw_release(),
        digest: profile.profile_digest(),
    });

    let format = select_format(cli.format, output)?;
    *selected_format = Some(format);

    let (asset_paths, asset_mimes) = parse_asset_declarations(&cli.asset, &cli.asset_mime)?;
    let mut validation_assets = SupplementalAssets::builder();
    for asset_id in asset_paths.keys() {
        if let Some(media_type) = asset_mimes.get(asset_id) {
            validation_assets =
                validation_assets.asset_with_media_type(asset_id, Vec::new(), media_type);
        } else {
            validation_assets = validation_assets.asset(asset_id, Vec::new());
        }
    }
    let validation_assets = validation_assets
        .build()
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))?;
    build_request(cli, format, validation_assets)?;
    validate_paths(input, output, report_output(cli), &asset_paths)?;
    check_destinations(output, report_output(cli), cli.force)?;

    let scene = read_scene(input)?;

    let limits = ResourceLimits::default();
    let mut assets_builder = SupplementalAssets::builder();
    let mut total_asset_bytes = 0_u64;
    for (asset_id, path) in &asset_paths {
        let aggregate_remaining = limits
            .max_total_asset_bytes()
            .saturating_sub(total_asset_bytes);
        let bytes = read_bounded_asset(
            path,
            asset_id,
            limits.max_asset_bytes(),
            limits.max_total_asset_bytes(),
            aggregate_remaining,
        )?;
        total_asset_bytes =
            total_asset_bytes.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if let Some(media_type) = asset_mimes.get(asset_id) {
            assets_builder = assets_builder.asset_with_media_type(asset_id, bytes, media_type);
        } else {
            assets_builder = assets_builder.asset(asset_id, bytes);
        }
    }
    let assets = assets_builder
        .build()
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))?;

    let request = build_request(cli, format, assets)?;
    let outcome = renderer.render(&scene, &request).map_err(|error| {
        let diagnostics = error.report().diagnostics().to_vec();
        let mut failure = Failure::library(error.descriptor(), EXIT_OPERATIONAL);
        failure.diagnostics = diagnostics;
        failure
    })?;
    let (artifact, report) = outcome.into_parts();
    Ok((artifact.into_bytes(), report.diagnostics().to_vec()))
}

/// The renderer for `--compatibility-revision`, which accepts only the one
/// revision the library implements; absent, it selects that revision.
fn select_renderer(requested: Option<&str>) -> Result<Renderer, Failure> {
    let renderer = Renderer::new();
    let Some(value) = requested else {
        return Ok(renderer);
    };
    let argument = || {
        Some(CliDetails::CliArgument {
            argument: "--compatibility-revision".to_owned(),
            value: Some(value.to_owned()),
        })
    };
    let parsed = match value.parse::<u64>() {
        Ok(revision) => Some(revision),
        // Too large for u64 is still a well-formed revision, just not one
        // this renderer implements.
        Err(error) if *error.kind() == IntErrorKind::PosOverflow => None,
        Err(_) => {
            return Err(Failure::cli(
                "cli.invalid-arguments",
                "compatibility revision must be a nonnegative integer",
                argument(),
                EXIT_USAGE,
            ))
        }
    };
    let supported = renderer.profile_identity().compatibility_revision();
    if parsed != Some(supported.get()) {
        let revision = parsed.map_or_else(|| value.to_owned(), |revision| revision.to_string());
        return Err(Failure::cli(
            "revision.unsupported",
            format!(
                "compatibility revision {revision} is not supported; this renderer implements revision {supported}"
            ),
            argument(),
            EXIT_USAGE,
        ));
    }
    Ok(renderer)
}

fn build_request(
    cli: &Cli,
    format: OutputFormat,
    assets: SupplementalAssets,
) -> Result<RenderRequest, Failure> {
    let sizing = cli
        .max_dimension
        .map(Sizing::MaxDimension)
        .unwrap_or_else(|| Sizing::Scale(cli.scale.unwrap_or(1.0)));
    let mut builder = RenderRequest::builder(format)
        .include_background(!cli.no_background)
        .padding(cli.padding.unwrap_or(10.0))
        .sizing(sizing)
        .color_mode(if cli.dark_mode {
            ColorMode::Dark
        } else {
            ColorMode::Light
        })
        .frame_options(
            FrameOptions::default()
                .with_enabled(!cli.no_frame_rendering)
                .with_clip(!cli.no_frame_clipping)
                .with_show_name(!cli.no_frame_names)
                .with_outline(!cli.no_frame_outlines),
        )
        .metadata_options(
            MetadataOptions::default()
                .with_embedded_scene(cli.embed_scene)
                .with_provenance(cli.embed_provenance)
                .with_diagnostic_summary(cli.embed_diagnostics),
        )
        .supplemental_assets(assets)
        .resource_limits(ResourceLimits::default());
    if let Some(fallback) = cli.pdf_raster_fallback {
        builder = builder.pdf_options(PdfOptions::default().with_raster_fallback(match fallback {
            PdfFallbackArg::AllowIsolated => PdfRasterFallback::AllowIsolated,
            PdfFallbackArg::Forbid => PdfRasterFallback::Forbid,
        }));
    }
    builder
        .build()
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))
}

fn select_format(explicit: Option<FormatArg>, output: &str) -> Result<OutputFormat, Failure> {
    let inferred = if output == "-" {
        None
    } else {
        Path::new(output)
            .extension()
            .and_then(|value| value.to_str())
            .and_then(|value| match value.to_ascii_lowercase().as_str() {
                "svg" => Some(OutputFormat::Svg),
                "png" => Some(OutputFormat::Png),
                "pdf" => Some(OutputFormat::Pdf),
                _ => None,
            })
    };
    match (explicit.map(OutputFormat::from), inferred) {
        (Some(format), Some(inferred)) if format != inferred => Err(Failure::cli(
            "cli.format-mismatch",
            format!(
                "--format {} does not match the extension of output '{output}'",
                format.as_str()
            ),
            Some(CliDetails::CliArgument {
                argument: "--format".to_owned(),
                value: Some(format.as_str().to_owned()),
            }),
            EXIT_USAGE,
        )),
        (Some(format), _) => Ok(format),
        (None, Some(format)) => Ok(format),
        (None, None) => Err(Failure::cli(
            "cli.format-required",
            "--format is required for stdout or an unrecognized output extension",
            None,
            EXIT_USAGE,
        )),
    }
}

type AssetDeclarations = (BTreeMap<String, String>, BTreeMap<String, String>);

fn parse_asset_declarations(
    assets: &[String],
    mimes: &[String],
) -> Result<AssetDeclarations, Failure> {
    let mut paths = BTreeMap::new();
    for value in assets {
        let (id, path) = split_asset_option(value, "--asset")?;
        if path == "-" {
            return Err(invalid_asset(
                "--asset",
                "supplemental assets must be filesystem paths and cannot use stdin",
                value,
            ));
        }
        if paths.insert(id.to_owned(), path.to_owned()).is_some() {
            return Err(invalid_asset("--asset", "duplicate --asset file ID", value));
        }
    }
    let mut media_types = BTreeMap::new();
    for value in mimes {
        let (id, media_type) = split_asset_option(value, "--asset-mime")?;
        if media_types
            .insert(id.to_owned(), media_type.to_owned())
            .is_some()
        {
            return Err(invalid_asset(
                "--asset-mime",
                "duplicate --asset-mime file ID",
                value,
            ));
        }
    }
    if let Some((id, _)) = media_types.iter().find(|(id, _)| !paths.contains_key(*id)) {
        return Err(Failure::cli(
            "cli.asset-mime-without-asset",
            "every --asset-mime requires one matching --asset",
            Some(CliDetails::CliArgument {
                argument: "--asset-mime".to_owned(),
                value: Some(id.clone()),
            }),
            EXIT_USAGE,
        ));
    }
    Ok((paths, media_types))
}

fn split_asset_option<'a>(value: &'a str, argument: &str) -> Result<(&'a str, &'a str), Failure> {
    let Some((id, item)) = value.split_once('=') else {
        return Err(invalid_asset(
            argument,
            "asset options require FILE_ID=VALUE",
            value,
        ));
    };
    if id.is_empty() || item.is_empty() {
        return Err(invalid_asset(
            argument,
            "asset option values must not be empty",
            value,
        ));
    }
    Ok((id, item))
}

fn invalid_asset(argument: &str, message: &str, value: &str) -> Failure {
    Failure::cli(
        "cli.invalid-asset-option",
        message,
        Some(CliDetails::CliArgument {
            argument: argument.to_owned(),
            value: Some(value.to_owned()),
        }),
        EXIT_USAGE,
    )
}

fn validate_paths(
    input: &str,
    output: &str,
    report: &str,
    assets: &BTreeMap<String, String>,
) -> Result<(), Failure> {
    let mut sources = Vec::new();
    if input != "-" {
        sources.push(("input", None, input));
    }
    for (id, path) in assets {
        sources.push(("asset", Some(id.as_str()), path.as_str()));
    }
    let mut destinations = Vec::new();
    if output != "-" {
        destinations.push(("output", None, output));
    }
    if report != "stderr" {
        destinations.push(("report", None, report));
    }

    for source in &sources {
        for destination in &destinations {
            if same_path(source.2, destination.2) {
                return Err(path_conflict([*source, *destination]));
            }
        }
    }
    if destinations.len() == 2 && same_path(destinations[0].2, destinations[1].2) {
        return Err(path_conflict([destinations[0], destinations[1]]));
    }
    Ok(())
}

fn path_conflict<const N: usize>(entries: [(&'static str, Option<&str>, &str); N]) -> Failure {
    let named = entries
        .iter()
        .map(|(role, asset_id, path)| match asset_id {
            Some(asset_id) => format!("{role} {asset_id} '{path}'"),
            None => format!("{role} '{path}'"),
        })
        .collect::<Vec<_>>();
    Failure::cli(
        "cli.path-conflict",
        format!(
            "{} are the same file; input, asset, output, and report paths must differ",
            named.join(" and ")
        ),
        Some(CliDetails::CliPath {
            entries: entries
                .into_iter()
                .map(|(role, asset_id, path)| CliPathEntry {
                    role,
                    asset_id: asset_id.map(str::to_owned),
                    path: path.to_owned(),
                })
                .collect(),
        }),
        EXIT_USAGE,
    )
}

fn same_path(left: &str, right: &str) -> bool {
    let left_path = Path::new(left);
    let right_path = Path::new(right);
    if let (Ok(left_meta), Ok(right_meta)) = (fs::metadata(left_path), fs::metadata(right_path)) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            return left_meta.dev() == right_meta.dev() && left_meta.ino() == right_meta.ino();
        }
        #[cfg(not(unix))]
        {
            if let (Ok(left), Ok(right)) =
                (fs::canonicalize(left_path), fs::canonicalize(right_path))
            {
                return left == right;
            }
        }
    }
    lexical_absolute(left_path) == lexical_absolute(right_path)
}

fn lexical_absolute(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn path_entry_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Whether writing to `path` would destroy something only `--force` may
/// replace: any existing entry, a dangling symlink included, except a device
/// or pipe, which is written in place.
fn destination_is_taken(path: &Path) -> bool {
    path_entry_exists(path) && !is_stream_destination(path)
}

/// Whether `path` is, or links to, a character device or a pipe (such as
/// `/dev/null`): a stream the artifact is written into, never a file to
/// stage beside and replace.
fn is_stream_destination(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        fs::metadata(path).is_ok_and(|metadata| {
            let kind = metadata.file_type();
            kind.is_char_device() || kind.is_fifo()
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn check_destinations(output: &str, report: &str, force: bool) -> Result<(), Failure> {
    if output != "-" {
        check_output_file_location(output)?;
    }
    for (role, path) in [
        (DeliveryRole::Output, output),
        (DeliveryRole::Report, report),
    ] {
        if (role == DeliveryRole::Output && path == "-")
            || (role == DeliveryRole::Report && path == "stderr")
        {
            continue;
        }
        if destination_is_taken(Path::new(path)) && !force {
            return Err(destination_exists(role, path));
        }
    }
    Ok(())
}

/// Rejects, before anything is rendered, an output file that could never be
/// delivered: a directory (which even `--force` cannot replace) or a path
/// whose directory is missing.
fn check_output_file_location(output: &str) -> Result<(), Failure> {
    let path = Path::new(output);
    let directory = if path.is_dir() {
        Some("is a directory")
    } else if output.ends_with(std::path::is_separator) {
        Some("names a directory")
    } else {
        None
    };
    if let Some(directory) = directory {
        return Err(Failure::cli(
            "cli.invalid-output-directory",
            format!("output '{output}' {directory}"),
            Some(CliDetails::CliArgument {
                argument: "--output".to_owned(),
                value: Some(output.to_owned()),
            }),
            EXIT_USAGE,
        )
        .with_hint(
            "pass a file path to -o, or use `excalidraw-render frames INPUT -o DIRECTORY \
             --format FORMAT` to write one file per frame",
        ));
    }
    let Some(error) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .and_then(not_a_directory)
    else {
        return Ok(());
    };
    Err(delivery_failure(
        DeliveryRole::Output,
        &DeliveryError::new(CliOperation::Create, &error),
        output,
        DeliveryState::None,
        Vec::new(),
    ))
}

/// Why `path` cannot hold new files, as the operating system words it (a
/// missing entry, or a file where a directory belongs); `None` for a
/// directory.
fn not_a_directory(path: &Path) -> Option<io::Error> {
    if path.is_dir() {
        return None;
    }
    Some(
        fs::read_dir(path)
            .err()
            .unwrap_or_else(|| io::ErrorKind::NotADirectory.into()),
    )
}

/// `cli.destination-exists` for an existing `role` destination at `path`.
fn destination_exists(role: DeliveryRole, path: &str) -> Failure {
    Failure::cli(
        "cli.destination-exists",
        format!("{} '{path}' already exists", role.as_str()),
        Some(CliDetails::Delivery {
            role,
            operation: CliOperation::Create,
            path: path.to_owned(),
            artifact_state: DeliveryState::None,
        }),
        EXIT_OPERATIONAL,
    )
}

/// Reads and parses the scene at `path` (`-` for stdin).
fn read_scene(path: &str) -> Result<SceneDocument, Failure> {
    let bytes = read_scene_bounded(path, MAX_SCENE_BYTES)?;
    SceneDocument::parse(&bytes).map_err(|error| {
        let descriptor = error.descriptor();
        let (message, hints) = human::scene_parse_context(path, &bytes, &descriptor);
        let mut failure = Failure::library(descriptor, EXIT_OPERATIONAL);
        failure.human_message = Some(message);
        failure.hints = hints;
        failure
    })
}

fn read_scene_bounded(path: &str, limit: u64) -> Result<Vec<u8>, Failure> {
    let mut reader: Box<dyn Read> =
        if path == "-" {
            Box::new(io::stdin().lock())
        } else {
            Box::new(File::open(path).map_err(|error| {
                io_failure(IoRole::Input, None, CliOperation::Open, path, &error)
            })?)
        };
    let capacity = usize::try_from(limit.min(1024 * 1024)).unwrap_or(1024 * 1024);
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_failure(IoRole::Input, None, CliOperation::Read, path, &error))?;
    let measured = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if measured > limit {
        return Err(Failure::cli(
            "scene.input-too-large",
            format!(
                "scene input {} exceeds the {} MiB limit",
                human::input_name(path),
                limit / (1024 * 1024)
            ),
            Some(CliDetails::ParseLimit {
                field: "max_input_bytes",
                configured: limit,
                value_kind: "bytes",
                measurement: CliMeasurement::AtLeast { value: measured },
            }),
            EXIT_OPERATIONAL,
        ));
    }
    Ok(bytes)
}

fn read_bounded_asset(
    path: &str,
    asset_id: &str,
    per_asset_limit: u64,
    aggregate_limit: u64,
    aggregate_remaining: u64,
) -> Result<Vec<u8>, Failure> {
    let file = File::open(path).map_err(|error| {
        io_failure(
            IoRole::Asset,
            Some(asset_id),
            CliOperation::Open,
            path,
            &error,
        )
    })?;
    let stream_limit = per_asset_limit.min(aggregate_remaining);
    let capacity = usize::try_from(stream_limit.min(1024 * 1024)).unwrap_or(1024 * 1024);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(stream_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            io_failure(
                IoRole::Asset,
                Some(asset_id),
                CliOperation::Read,
                path,
                &error,
            )
        })?;
    let measured = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if measured > aggregate_remaining {
        return Err(transport_limit_failure(
            "resource.total-asset-bytes-exceeded",
            "supplemental assets exceed the aggregate transport ceiling",
            "max_total_asset_bytes",
            aggregate_limit,
            aggregate_limit.saturating_add(1),
        ));
    }
    if measured > per_asset_limit {
        return Err(transport_limit_failure(
            "resource.asset-bytes-exceeded",
            "supplemental asset exceeds the per-file transport ceiling",
            "max_asset_bytes",
            per_asset_limit,
            measured,
        ));
    }
    Ok(bytes)
}

fn transport_limit_failure(
    code: &'static str,
    message: &'static str,
    field: &'static str,
    configured: u64,
    measured: u64,
) -> Failure {
    Failure::cli(
        code,
        message,
        Some(CliDetails::ResourceLimit {
            field,
            configured,
            value_kind: "bytes",
            measurement: CliMeasurement::AtLeast { value: measured },
            recovery: None,
        }),
        EXIT_OPERATIONAL,
    )
}

fn io_failure(
    role: IoRole,
    asset_id: Option<&str>,
    operation: CliOperation,
    path: &str,
    error: &io::Error,
) -> Failure {
    let (code, subject) = match (role, asset_id) {
        (IoRole::Input, _) if path == "-" => ("cli.input-read-failed", "standard input".to_owned()),
        (IoRole::Input, _) => ("cli.input-read-failed", format!("input '{path}'")),
        (IoRole::Asset, Some(asset_id)) => (
            "cli.asset-read-failed",
            format!("asset {asset_id} '{path}'"),
        ),
        (IoRole::Asset, None) => ("cli.asset-read-failed", format!("asset '{path}'")),
    };
    Failure::cli(
        code,
        format!(
            "cannot {} {subject}: {}",
            operation.as_str(),
            human::os_reason(error)
        ),
        Some(CliDetails::CliIo {
            role,
            asset_id: asset_id.map(str::to_owned),
            operation,
            path: path.to_owned(),
        }),
        EXIT_OPERATIONAL,
    )
}

fn deliver_success(
    cli: &Cli,
    identity: &SelectedIdentity,
    format: OutputFormat,
    artifact: Vec<u8>,
    diagnostics: Vec<CompatibilityDiagnostic>,
) -> u8 {
    let descriptor = ArtifactDescriptor {
        media_type: format.media_type(),
        byte_length: artifact.len(),
        sha256: sha256(&artifact),
    };
    let report = RenderReport {
        schema_version: 1,
        status: "success",
        renderer_version: env!("CARGO_PKG_VERSION"),
        compatibility_revision: Some(identity.revision),
        excalidraw_release: Some(identity.release),
        reference_profile_digest: Some(identity.digest),
        format: Some(format),
        artifact: Some(descriptor),
        error: None,
        diagnostics: &diagnostics,
    };
    let report_bytes = success_report_bytes(
        cli.report_format,
        &report,
        &diagnostics,
        HumanReport::new(cli.verbose, cli.max_dimension, Some(format)),
    );

    let output = cli.output.as_deref().expect("output validated");
    let mut staged_output = if output == "-" {
        None
    } else {
        match StagedFile::new(Path::new(output), &artifact, cli.force) {
            Ok(staged) => Some(staged),
            Err(error) => {
                return emit_failure(
                    cli,
                    Some(identity),
                    Some(format),
                    delivery_failure(
                        DeliveryRole::Output,
                        &error,
                        output,
                        DeliveryState::None,
                        diagnostics.clone(),
                    ),
                )
            }
        }
    };
    let report_output = report_output(cli);
    let mut staged_report = if report_output == "stderr" {
        None
    } else {
        match StagedFile::new(Path::new(report_output), &report_bytes, cli.force) {
            Ok(staged) => Some(staged),
            Err(error) => {
                return emit_failure(
                    cli,
                    Some(identity),
                    Some(format),
                    delivery_failure(
                        DeliveryRole::Report,
                        &error,
                        report_output,
                        DeliveryState::None,
                        diagnostics.clone(),
                    ),
                )
            }
        }
    };

    if let Some(staged) = staged_output.take() {
        if let Err(error) = staged.deliver() {
            return emit_failure(
                cli,
                Some(identity),
                Some(format),
                delivery_failure(
                    DeliveryRole::Output,
                    &error,
                    output,
                    DeliveryState::None,
                    diagnostics.clone(),
                ),
            );
        }
    } else if let Err((error, artifact_state)) =
        write_stream_tracked(&mut io::stdout().lock(), &artifact)
    {
        // A reader that stopped early (`| head`) is not news to the person at
        // the terminal: fail without a message, as pipeline tools do. A JSON
        // or file report is still written, since a program asked for it.
        if error.kind == io::ErrorKind::BrokenPipe
            && cli.report_format != ReportFormat::Json
            && report_output == "stderr"
        {
            return EXIT_OPERATIONAL;
        }
        return emit_failure(
            cli,
            Some(identity),
            Some(format),
            delivery_failure(
                DeliveryRole::Output,
                &error,
                output,
                artifact_state,
                diagnostics.clone(),
            ),
        );
    }

    if let Some(staged) = staged_report.take() {
        if let Err(error) = staged.deliver() {
            return emergency_report_failure(
                cli,
                identity,
                format,
                &error,
                DeliveryState::Complete,
                diagnostics,
            );
        }
    } else if !report_bytes.is_empty()
        && write_stream(&mut io::stderr().lock(), &report_bytes).is_err()
    {
        return EXIT_OPERATIONAL;
    }
    0
}

fn stage_report_file(cli: &Cli, bytes: &[u8]) -> Result<Option<StagedFile>, DeliveryError> {
    let report = report_output(cli);
    if report == "stderr" {
        Ok(None)
    } else {
        StagedFile::new(Path::new(report), bytes, cli.force).map(Some)
    }
}

fn success_report_bytes(
    mode: ReportFormat,
    report: &RenderReport<'_>,
    diagnostics: &[CompatibilityDiagnostic],
    human: HumanReport,
) -> Vec<u8> {
    match mode {
        ReportFormat::Json => json_line(report),
        ReportFormat::Human => format!(
            "{}{}",
            human.notes(),
            human::diagnostics(diagnostics, human.verbose)
        )
        .into_bytes(),
        ReportFormat::None => Vec::new(),
    }
}

fn emit_failure(
    cli: &Cli,
    identity: Option<&SelectedIdentity>,
    format: Option<OutputFormat>,
    failure: Failure,
) -> u8 {
    let exit = failure.exit;
    let diagnostics = failure.diagnostics.clone();
    let human = HumanReport::new(cli.verbose, None, format);
    let bytes = failure_report_bytes(cli.report_format, identity, format, failure, human);
    let report_output = report_output(cli);
    if report_output == "stderr"
        || cli.report_format == ReportFormat::None
        || !report_destination_is_safe(cli)
    {
        let _ = write_stream(&mut io::stderr().lock(), &bytes);
    } else if let Err(error) =
        StagedFile::new(Path::new(report_output), &bytes, cli.force).and_then(StagedFile::deliver)
    {
        let fallback = delivery_failure(
            DeliveryRole::Report,
            &error,
            report_output,
            DeliveryState::None,
            diagnostics,
        );
        let emergency = failure_report_bytes(
            emergency_report_format(cli.report_format),
            identity,
            format,
            fallback,
            human,
        );
        let _ = write_stream(&mut io::stderr().lock(), &emergency);
        return EXIT_OPERATIONAL;
    }
    exit
}

fn report_output(cli: &Cli) -> &str {
    cli.report_output.as_deref().unwrap_or("stderr")
}

fn emergency_report_format(format: ReportFormat) -> ReportFormat {
    if format == ReportFormat::Json {
        ReportFormat::Json
    } else {
        ReportFormat::None
    }
}

fn report_destination_is_safe(cli: &Cli) -> bool {
    let report = report_output(cli);
    if report == "-" {
        return false;
    }
    if report == "stderr" || (destination_is_taken(Path::new(report)) && !cli.force) {
        return report == "stderr";
    }
    if cli
        .input
        .as_deref()
        .is_some_and(|input| input != "-" && same_path(input, report))
        || cli
            .output
            .as_deref()
            .is_some_and(|output| output != "-" && same_path(output, report))
    {
        return false;
    }
    !cli.asset.iter().any(|value| {
        value
            .split_once('=')
            .is_some_and(|(_, path)| !path.is_empty() && same_path(path, report))
    })
}

/// The failure report: one JSON document, or for people the grouped
/// diagnostics, an error line, and its hints (`none` keeps only the line).
fn failure_report_bytes(
    mode: ReportFormat,
    identity: Option<&SelectedIdentity>,
    format: Option<OutputFormat>,
    failure: Failure,
    human: HumanReport,
) -> Vec<u8> {
    if mode != ReportFormat::Json {
        let code = failure.error.code();
        let message = failure
            .human_message
            .clone()
            .unwrap_or_else(|| failure.error.human_message());
        let mut text = String::new();
        if mode == ReportFormat::Human {
            text.push_str(&human::diagnostics(&failure.diagnostics, human.verbose));
        }
        text.push_str(&human::error_line(&message, code));
        if mode == ReportFormat::Human {
            text.push_str(&human::hint_lines(
                code,
                failure
                    .hints
                    .iter()
                    .map(String::as_str)
                    .chain(failure.error.sizing_hint()),
            ));
        }
        return text.into_bytes();
    }
    json_line(&RenderReport {
        schema_version: 1,
        status: "failure",
        renderer_version: env!("CARGO_PKG_VERSION"),
        compatibility_revision: identity.map(|value| value.revision),
        excalidraw_release: identity.map(|value| value.release),
        reference_profile_digest: identity.map(|value| value.digest),
        format,
        artifact: None,
        error: Some(failure.error),
        diagnostics: &failure.diagnostics,
    })
}

/// Why writing a destination failed: the step and the operating system's
/// reason.
struct DeliveryError {
    operation: CliOperation,
    kind: io::ErrorKind,
    reason: String,
}

impl DeliveryError {
    fn new(operation: CliOperation, error: &io::Error) -> Self {
        Self {
            operation,
            kind: error.kind(),
            reason: human::os_reason(error),
        }
    }

    /// The message for failing to deliver `subject` (for example
    /// `output 'a.svg'`).
    fn message(&self, subject: &str) -> String {
        format!(
            "cannot {} {subject}: {}",
            self.operation.as_str(),
            self.reason
        )
    }

    /// Advice for delivering to `path`: name a missing parent directory, or
    /// the flag that replaces a destination created meanwhile.
    fn hint(&self, path: &Path) -> Option<String> {
        match self.kind {
            io::ErrorKind::AlreadyExists => Some("use --force to overwrite it".to_owned()),
            io::ErrorKind::NotFound => path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty() && !parent.is_dir())
                .map(|parent| {
                    format!(
                        "directory '{}' does not exist; create it first",
                        parent.display()
                    )
                }),
            _ => None,
        }
    }
}

fn delivery_failure(
    role: DeliveryRole,
    error: &DeliveryError,
    path: &str,
    artifact_state: DeliveryState,
    diagnostics: Vec<CompatibilityDiagnostic>,
) -> Failure {
    let subject = if role == DeliveryRole::Output && path == "-" {
        "output to standard output".to_owned()
    } else {
        format!("{} '{path}'", role.as_str())
    };
    let mut failure = Failure::cli(
        if role == DeliveryRole::Output {
            "cli.output-write-failed"
        } else {
            "cli.report-write-failed"
        },
        error.message(&subject),
        Some(CliDetails::Delivery {
            role,
            operation: error.operation,
            path: path.to_owned(),
            artifact_state,
        }),
        EXIT_OPERATIONAL,
    );
    failure.diagnostics = diagnostics;
    if path != "-" {
        failure.hints.extend(error.hint(Path::new(path)));
    }
    failure
}

fn emergency_report_failure(
    cli: &Cli,
    identity: &SelectedIdentity,
    format: OutputFormat,
    error: &DeliveryError,
    artifact_state: DeliveryState,
    diagnostics: Vec<CompatibilityDiagnostic>,
) -> u8 {
    let failure = delivery_failure(
        DeliveryRole::Report,
        error,
        report_output(cli),
        artifact_state,
        diagnostics,
    );
    let bytes = failure_report_bytes(
        emergency_report_format(cli.report_format),
        Some(identity),
        Some(format),
        failure,
        HumanReport::PLAIN,
    );
    let _ = write_stream(&mut io::stderr().lock(), &bytes);
    EXIT_OPERATIONAL
}

fn deliver_report_bytes(cli: &Cli, bytes: &[u8], artifact_state: DeliveryState) -> u8 {
    let report_output = report_output(cli);
    if report_output == "stderr" {
        return if write_stream(&mut io::stderr().lock(), bytes).is_ok() {
            0
        } else {
            EXIT_OPERATIONAL
        };
    }
    match StagedFile::new(Path::new(report_output), bytes, false).and_then(StagedFile::deliver) {
        Ok(()) => 0,
        Err(error) => {
            let failure = delivery_failure(
                DeliveryRole::Report,
                &error,
                report_output,
                artifact_state,
                Vec::new(),
            );
            let bytes =
                failure_report_bytes(cli.report_format, None, None, failure, HumanReport::PLAIN);
            let _ = write_stream(&mut io::stderr().lock(), &bytes);
            EXIT_OPERATIONAL
        }
    }
}

fn write_stdout(bytes: &[u8]) -> u8 {
    if write_stream(&mut io::stdout().lock(), bytes).is_ok() {
        0
    } else {
        EXIT_OPERATIONAL
    }
}

fn write_stream(writer: &mut impl Write, bytes: &[u8]) -> Result<(), DeliveryError> {
    write_stream_tracked(writer, bytes).map_err(|(error, _)| error)
}

fn write_stream_tracked(
    writer: &mut impl Write,
    mut bytes: &[u8],
) -> Result<(), (DeliveryError, DeliveryState)> {
    let mut wrote_bytes = false;
    let state = |wrote_bytes| {
        if wrote_bytes {
            DeliveryState::PossiblyPartial
        } else {
            DeliveryState::None
        }
    };
    while !bytes.is_empty() {
        match writer.write(bytes) {
            Ok(0) => {
                let error = io::Error::from(io::ErrorKind::WriteZero);
                return Err((
                    DeliveryError::new(CliOperation::Write, &error),
                    state(wrote_bytes),
                ));
            }
            Ok(written) => {
                wrote_bytes = true;
                bytes = &bytes[written..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                // Linux reports a write that moved some bytes before failing as
                // a short count, so a failed call there moved none. macOS can
                // fail a pipe write with EPIPE after its reader consumed part
                // of it, so elsewhere a failed call may have moved bytes.
                return Err((
                    DeliveryError::new(CliOperation::Write, &error),
                    state(wrote_bytes || !cfg!(target_os = "linux")),
                ));
            }
        }
    }
    loop {
        match writer.flush() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Err((
                    DeliveryError::new(CliOperation::Flush, &error),
                    state(wrote_bytes),
                ))
            }
        }
    }
}

/// A fully written, synced temporary sibling of one destination. Only its path
/// is held, so staging many frames never holds one descriptor per frame. A
/// device or pipe destination has no sibling to stage: its bytes are held and
/// written in place on delivery.
struct StagedFile {
    staged: Staged,
    destination: PathBuf,
    replace: bool,
}

enum Staged {
    Temporary(TempPath),
    Stream(Vec<u8>),
}

impl StagedFile {
    fn new(destination: &Path, bytes: &[u8], replace: bool) -> Result<Self, DeliveryError> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Self::new_at(parent, destination, bytes, replace)
    }

    fn new_at(
        staging_parent: &Path,
        destination: &Path,
        bytes: &[u8],
        replace: bool,
    ) -> Result<Self, DeliveryError> {
        if is_stream_destination(destination) {
            return Ok(Self {
                staged: Staged::Stream(bytes.to_vec()),
                destination: destination.to_path_buf(),
                replace,
            });
        }
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("output");
        let prefix = format!(".{name}.");
        let mut builder = tempfile::Builder::new();
        builder.prefix(&prefix).suffix(".tmp");
        // Published files keep the mode an ordinary create gives: 0666 less the umask.
        #[cfg(unix)]
        builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(0o666));
        let mut file = builder
            .tempfile_in(staging_parent)
            .map_err(|error| DeliveryError::new(CliOperation::Create, &error))?;
        file.write_all(bytes)
            .map_err(|error| DeliveryError::new(CliOperation::Write, &error))?;
        file.flush()
            .and_then(|()| file.as_file().sync_all())
            .map_err(|error| DeliveryError::new(CliOperation::Flush, &error))?;
        Ok(Self {
            staged: Staged::Temporary(file.into_temp_path()),
            destination: destination.to_path_buf(),
            replace,
        })
    }

    /// Atomically publishes the staged file; without `replace` an existing
    /// destination, even one created after staging, is never overwritten. A
    /// device or pipe is opened and written instead.
    fn deliver(self) -> Result<(), DeliveryError> {
        match self.staged {
            Staged::Stream(bytes) => {
                let mut stream = fs::OpenOptions::new()
                    .write(true)
                    .open(&self.destination)
                    .map_err(|error| DeliveryError::new(CliOperation::Open, &error))?;
                write_stream(&mut stream, &bytes)
            }
            Staged::Temporary(temporary) if self.replace => temporary
                .persist(&self.destination)
                .map_err(|error| DeliveryError::new(CliOperation::Replace, &error.error)),
            Staged::Temporary(temporary) => temporary
                .persist_noclobber(&self.destination)
                .map_err(|error| DeliveryError::new(CliOperation::Create, &error.error)),
        }
    }
}

fn preparse_json_mode(args: &[OsString]) -> bool {
    let mut report_options = 0_u32;
    let mut valid_json = 0_u32;
    let mut index = 1;
    while index < args.len() {
        let value = args[index].to_string_lossy();
        if value == "--report-format" {
            report_options += 1;
            if args.get(index + 1).is_some_and(|next| next == "json") {
                valid_json += 1;
                index += 1;
            }
        } else if let Some(choice) = value.strip_prefix("--report-format=") {
            report_options += 1;
            if choice == "json" {
                valid_json += 1;
            }
        }
        index += 1;
    }
    report_options == 1 && valid_json == 1
}

/// `report` as one line of JSON.
fn json_line(report: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(report).expect("reports serialize");
    bytes.push(b'\n');
    bytes
}

/// The `sha256:`-prefixed lowercase hex digest artifact descriptors carry.
fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", lower_hex(&Sha256::digest(bytes)))
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use excalidraw_render::{
        CompatibilityDiagnostic, CompatibilityRevision, ErrorDescriptor, OutputFormat,
        RenderRequest, Renderer, SceneDocument,
    };

    use super::{
        json_line, lexical_absolute, parse_asset_declarations, preparse_json_mode, same_path,
        select_format, sha256, split_asset_option, ArtifactDescriptor, CliDetails, CliError,
        CliMeasurement, CliOperation, CliPathEntry, DeliveryError, DeliveryRole, DeliveryState,
        Failure, FormatArg, IoRole, RenderReport, ReportError, RevisionEntry, RevisionReport,
        StagedFile, EXIT_USAGE,
    };

    /// `--help` describes every argument of every command.
    #[test]
    fn every_command_and_argument_is_described() {
        fn check(command: &clap::Command) {
            for argument in command.get_arguments() {
                assert!(
                    argument.get_help().is_some(),
                    "{} {} has no help",
                    command.get_name(),
                    argument.get_id()
                );
            }
            for subcommand in command.get_subcommands() {
                assert!(
                    subcommand.get_about().is_some(),
                    "{}",
                    subcommand.get_name()
                );
                check(subcommand);
            }
        }
        let mut command = <super::Invocation as clap::CommandFactory>::command();
        command.build();
        check(&command);
        command.debug_assert();
    }

    /// Artifact digests are the `sha256:`-prefixed lowercase hex of the FIPS
    /// 180-4 SHA-256 digest, checked against its published test vectors.
    #[test]
    fn artifact_digests_are_prefixed_sha256_hex() {
        assert_eq!(
            sha256(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// The CLI error code and exit status a usage failure carries.
    fn cli_failure(failure: &Failure) -> (&str, u8) {
        match failure.error.as_ref() {
            ReportError::Cli(error) => (error.code, failure.exit),
            ReportError::Library(_) => panic!("a CLI failure, not a library one"),
        }
    }

    /// The `--argument`'s offending value the failure's details name.
    fn argument_details(failure: &Failure) -> (String, Option<String>) {
        match failure.error.as_ref() {
            ReportError::Cli(CliError {
                details: Some(CliDetails::CliArgument { argument, value }),
                ..
            }) => (argument.clone(), value.clone()),
            _ => panic!("a CLI argument failure"),
        }
    }

    #[test]
    fn the_output_extension_selects_the_format_when_none_is_given() {
        assert_eq!(select_format(None, "out.svg").ok(), Some(OutputFormat::Svg));
        assert_eq!(select_format(None, "OUT.PNG").ok(), Some(OutputFormat::Png));
        assert_eq!(
            select_format(None, "nested/dir.d/out.Pdf").ok(),
            Some(OutputFormat::Pdf)
        );
        assert_eq!(
            select_format(Some(FormatArg::Png), "out.png").ok(),
            Some(OutputFormat::Png)
        );
    }

    #[test]
    fn an_explicit_format_wins_where_the_output_has_no_recognized_extension() {
        assert_eq!(
            select_format(Some(FormatArg::Pdf), "-").ok(),
            Some(OutputFormat::Pdf)
        );
        assert_eq!(
            select_format(Some(FormatArg::Svg), "out.txt").ok(),
            Some(OutputFormat::Svg)
        );
        assert_eq!(
            select_format(Some(FormatArg::Png), "out").ok(),
            Some(OutputFormat::Png)
        );
    }

    #[test]
    fn a_format_that_contradicts_the_extension_is_a_usage_error() {
        let failure = select_format(Some(FormatArg::Png), "out.svg").expect_err("mismatch");
        assert_eq!(cli_failure(&failure), ("cli.format-mismatch", EXIT_USAGE));
        assert_eq!(
            argument_details(&failure),
            ("--format".to_owned(), Some("png".to_owned()))
        );
    }

    #[test]
    fn stdout_and_unrecognized_extensions_require_an_explicit_format() {
        for output in ["-", "out.txt", "out", "out.svg.bak"] {
            let failure = select_format(None, output).expect_err(output);
            assert_eq!(
                cli_failure(&failure),
                ("cli.format-required", EXIT_USAGE),
                "{output}"
            );
        }
    }

    #[test]
    fn asset_declarations_pair_each_file_id_with_its_path_and_optional_media_type() {
        let (paths, media_types) = parse_asset_declarations(
            &[
                "logo=assets/logo.png".to_owned(),
                "photo=/abs/photo=with=equals.jpg".to_owned(),
            ],
            &["photo=image/jpeg".to_owned()],
        )
        .ok()
        .expect("declarations parse");
        assert_eq!(
            paths.into_iter().collect::<Vec<_>>(),
            [
                ("logo".to_owned(), "assets/logo.png".to_owned()),
                ("photo".to_owned(), "/abs/photo=with=equals.jpg".to_owned()),
            ]
        );
        assert_eq!(
            media_types.into_iter().collect::<Vec<_>>(),
            [("photo".to_owned(), "image/jpeg".to_owned())]
        );

        let (paths, media_types) = parse_asset_declarations(&[], &[])
            .ok()
            .expect("no declarations parse");
        assert!(paths.is_empty());
        assert!(media_types.is_empty());
    }

    #[test]
    fn malformed_asset_declarations_are_usage_errors_naming_the_offending_option() {
        for (assets, mimes, argument, value) in [
            (vec!["logo"], vec![], "--asset", "logo"),
            (vec!["=logo.png"], vec![], "--asset", "=logo.png"),
            (vec!["logo="], vec![], "--asset", "logo="),
            (vec!["logo=-"], vec![], "--asset", "logo=-"),
            (
                vec!["logo=a.png", "logo=b.png"],
                vec![],
                "--asset",
                "logo=b.png",
            ),
            (
                vec!["logo=a.png"],
                vec!["logo=image/png", "logo=image/webp"],
                "--asset-mime",
                "logo=image/webp",
            ),
            (vec!["logo=a.png"], vec!["logo"], "--asset-mime", "logo"),
        ] {
            let assets: Vec<String> = assets.into_iter().map(str::to_owned).collect();
            let mimes: Vec<String> = mimes.into_iter().map(str::to_owned).collect();
            let failure = parse_asset_declarations(&assets, &mimes)
                .err()
                .unwrap_or_else(|| panic!("{assets:?} {mimes:?} is refused"));
            assert_eq!(
                cli_failure(&failure),
                ("cli.invalid-asset-option", EXIT_USAGE),
                "{assets:?} {mimes:?}"
            );
            assert_eq!(
                argument_details(&failure),
                (argument.to_owned(), Some(value.to_owned())),
                "{assets:?} {mimes:?}"
            );
        }
    }

    #[test]
    fn a_media_type_needs_the_asset_it_describes() {
        let failure =
            parse_asset_declarations(&["logo=a.png".to_owned()], &["photo=image/jpeg".to_owned()])
                .expect_err("an orphan media type is refused");
        assert_eq!(
            cli_failure(&failure),
            ("cli.asset-mime-without-asset", EXIT_USAGE)
        );
        assert_eq!(
            argument_details(&failure),
            ("--asset-mime".to_owned(), Some("photo".to_owned()))
        );
    }

    #[test]
    fn an_asset_option_splits_at_its_first_equals_sign() {
        assert_eq!(
            split_asset_option("id=a=b", "--asset").ok(),
            Some(("id", "a=b"))
        );
        assert!(split_asset_option("id", "--asset").is_err());
        assert!(split_asset_option("=value", "--asset").is_err());
        assert!(split_asset_option("id=", "--asset").is_err());
    }

    #[test]
    fn lexical_absolute_resolves_dot_segments_without_touching_the_filesystem() {
        assert_eq!(
            lexical_absolute(Path::new("/a/./b/../c/d")),
            PathBuf::from("/a/c/d")
        );
        assert_eq!(
            lexical_absolute(Path::new("/a/b/../../../c")),
            PathBuf::from("/c")
        );
        let cwd = std::env::current_dir().expect("the current directory is readable");
        assert_eq!(
            lexical_absolute(Path::new("out/../scene.svg")),
            cwd.join("scene.svg")
        );
        assert_eq!(lexical_absolute(Path::new("./x")), cwd.join("x"));
    }

    #[test]
    fn same_path_compares_lexically_for_paths_that_do_not_exist_yet() {
        assert!(same_path("out/render.svg", "out/render.svg"));
        assert!(same_path("out/render.svg", "./out/../out/render.svg"));
        assert!(same_path(
            "/tmp/does-not-exist/a",
            "/tmp/does-not-exist/b/../a"
        ));
        assert!(!same_path("out/render.svg", "out/render.png"));
        assert!(!same_path("a", "b"));
    }

    #[test]
    fn same_path_follows_the_filesystem_identity_of_existing_paths() {
        let scratch = tempfile::tempdir().expect("the scratch directory is created");
        let dir = scratch.path();
        let file = dir.join("scene.excalidraw");
        fs::write(&file, b"{}").expect("the scene file is written");
        let other = dir.join("other.excalidraw");
        fs::write(&other, b"{}").expect("the other file is written");
        let file = file.to_str().expect("the scratch path is UTF-8");
        let other = other.to_str().expect("the scratch path is UTF-8");

        assert!(same_path(file, file));
        assert!(same_path(
            file,
            &format!(
                "{}/./scene.excalidraw",
                dir.to_str().expect("the scratch path is UTF-8")
            )
        ));
        assert!(!same_path(file, other));
        #[cfg(unix)]
        {
            let link = dir.join("link.excalidraw");
            std::os::unix::fs::symlink(file, &link).expect("the symlink is created");
            assert!(
                same_path(file, link.to_str().expect("the scratch path is UTF-8")),
                "a symlink aliases its target"
            );
        }
    }

    /// The entry names in `directory`, sorted.
    fn entries(directory: &Path) -> Vec<String> {
        let mut names = fs::read_dir(directory)
            .expect("the scratch directory is listed")
            .map(|entry| {
                entry
                    .expect("the entry is read")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    #[test]
    fn staging_writes_a_hidden_sibling_that_delivery_publishes_with_the_create_mode() {
        let scratch = tempfile::tempdir().expect("the scratch directory is created");
        let destination = scratch.path().join("result.svg");
        let staged = StagedFile::new(&destination, b"artifact", false)
            .unwrap_or_else(|error| panic!("{}", error.message("the stage")));
        let [temporary] = entries(scratch.path()).try_into().expect("one staged file");
        assert!(
            temporary.starts_with(".result.svg.") && temporary.ends_with(".tmp"),
            "{temporary} is a hidden staging name"
        );
        assert!(!destination.exists(), "staging never publishes");

        assert!(staged.deliver().is_ok());
        assert_eq!(entries(scratch.path()), ["result.svg"]);
        assert_eq!(fs::read(&destination).expect("published"), b"artifact");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let ordinary = scratch.path().join("ordinary");
            fs::File::create(&ordinary).expect("an ordinary file is created");
            let mode = |path: &Path| fs::metadata(path).expect("stat").permissions().mode();
            assert_eq!(mode(&destination), mode(&ordinary), "0666 less the umask");
        }
    }

    #[test]
    fn delivery_without_force_never_replaces_a_destination_created_after_staging() {
        let scratch = tempfile::tempdir().expect("the scratch directory is created");
        let destination = scratch.path().join("result.svg");
        let staged = StagedFile::new(&destination, b"artifact", false)
            .unwrap_or_else(|error| panic!("{}", error.message("the stage")));
        fs::write(&destination, b"late").expect("a racing writer creates the destination");

        assert!(matches!(
            staged.deliver(),
            Err(DeliveryError {
                operation: CliOperation::Create,
                ..
            })
        ));
        assert_eq!(fs::read(&destination).expect("kept"), b"late");
        assert_eq!(
            entries(scratch.path()),
            ["result.svg"],
            "no staging leftovers"
        );
    }

    #[test]
    fn forced_delivery_replaces_a_destination_created_after_staging() {
        let scratch = tempfile::tempdir().expect("the scratch directory is created");
        let destination = scratch.path().join("result.svg");
        let staged = StagedFile::new(&destination, b"artifact", true)
            .unwrap_or_else(|error| panic!("{}", error.message("the stage")));
        fs::write(&destination, b"late").expect("a racing writer creates the destination");

        assert!(staged.deliver().is_ok());
        assert_eq!(fs::read(&destination).expect("replaced"), b"artifact");
        assert_eq!(entries(scratch.path()), ["result.svg"]);
    }

    #[test]
    fn staging_failures_are_create_failures_and_undelivered_files_are_removed() {
        let scratch = tempfile::tempdir().expect("the scratch directory is created");
        let missing = scratch.path().join("missing").join("result.svg");
        assert!(matches!(
            StagedFile::new(&missing, b"artifact", false),
            Err(DeliveryError {
                operation: CliOperation::Create,
                ..
            })
        ));

        let staged = StagedFile::new(&scratch.path().join("result.svg"), b"artifact", false)
            .unwrap_or_else(|error| panic!("{}", error.message("the stage")));
        drop(staged);
        assert!(
            entries(scratch.path()).is_empty(),
            "an abandoned stage is removed"
        );
    }

    /// Frame batches stage every artifact before publishing any, so a staged
    /// file must not keep its descriptor open.
    #[cfg(target_os = "linux")]
    #[test]
    fn staged_files_hold_no_open_descriptors() {
        let open_descriptors = || {
            fs::read_dir("/proc/self/fd")
                .expect("descriptors are listed")
                .count()
        };
        let scratch = tempfile::tempdir().expect("the scratch directory is created");
        let before = open_descriptors();
        let staged = (0..256)
            .map(|index| {
                StagedFile::new(&scratch.path().join(format!("{index}.svg")), b"a", false)
                    .unwrap_or_else(|error| {
                        panic!(
                            "staging failed at {}: {}",
                            error.operation.as_str(),
                            error.reason
                        )
                    })
            })
            .collect::<Vec<_>>();
        // Other test threads may open a few descriptors meanwhile.
        assert!(open_descriptors() < before + 64);
        drop(staged);
    }

    /// The process arguments for `values`, with the program name in front as
    /// `env::args_os` reports it.
    fn args(values: &[&str]) -> Vec<OsString> {
        std::iter::once("excalidraw-render")
            .chain(values.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn json_mode_is_preparsed_from_exactly_one_json_report_format_option() {
        assert!(preparse_json_mode(&args(&["--report-format", "json"])));
        assert!(preparse_json_mode(&args(&["--report-format=json"])));
        assert!(preparse_json_mode(&args(&[
            "scene.excalidraw",
            "-o",
            "out.svg",
            "--report-format=json",
            "--force"
        ])));
        assert!(preparse_json_mode(&args(&[
            "frames",
            "scene.excalidraw",
            "--report-format",
            "json"
        ])));

        assert!(!preparse_json_mode(&args(&[])));
        assert!(!preparse_json_mode(&args(&["--report-format", "human"])));
        assert!(!preparse_json_mode(&args(&["--report-format=none"])));
        assert!(!preparse_json_mode(&args(&["--report-format"])));
        // Two report-format options are a clap error, reported the human way.
        assert!(!preparse_json_mode(&args(&[
            "--report-format=json",
            "--report-format",
            "json"
        ])));
        assert!(!preparse_json_mode(&args(&[
            "--report-format=json",
            "--report-format=human"
        ])));
        // The program name is never an option.
        assert!(!preparse_json_mode(&[OsString::from(
            "--report-format=json"
        )]));
    }

    /// A string exercising JSON escaping: quotes, backslashes, C0 controls,
    /// DEL, multi-byte text and a line separator.
    pub(crate) const TRICKY: &str = "q\"b\\ t\t n\n nul\u{0} del\u{7f} é 日本 \u{2028} 🎨";
    /// `TRICKY` as a JSON string literal.
    pub(crate) const TRICKY_JSON: &str = concat!(
        r#""q\"b\\ t\t n\n nul\u0000 "#,
        "del\u{7f} é 日本 \u{2028} 🎨",
        r#"""#
    );

    /// The diagnostics of rendering an image that references no file payload,
    /// under an element id holding a control character and non-ASCII text.
    pub(crate) fn missing_image_diagnostics() -> Vec<CompatibilityDiagnostic> {
        let scene = SceneDocument::parse(
            concat!(
                r#"{"type":"excalidraw","version":2,"elements":[{"id":"ié\n","type":"image","#,
                r#""x":0,"y":0,"width":10,"height":10,"fileId":"missing","updated":1}],"#,
                r#""appState":{},"files":{}}"#
            )
            .as_bytes(),
        )
        .expect("the image scene is admitted");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("the SVG request is valid");
        Renderer::new()
            .render(&scene, &request)
            .expect("the image scene renders")
            .report()
            .diagnostics()
            .to_vec()
    }

    /// `missing_image_diagnostics` as it appears in a report.
    pub(crate) const MISSING_IMAGE_DIAGNOSTIC_JSON: &str = concat!(
        r#"{"code":"image.missing-file","severity":"warning","format":"svg","#,
        r#""effects":["visual"],"element_id":"i"#,
        "é",
        r#"\n","json_pointer":"/elements/0/fileId","#,
        r#""message":"image reference had no authoritative file payload","#,
        r#""details":{"kind":"asset","asset_id":"missing","claimed_media_type":null,"#,
        r#""validated_media_type":null,"recovery":"omitted","recipe_id":null}}"#
    );

    /// The library error of parsing a truncated scene document.
    pub(crate) fn invalid_json_error() -> ErrorDescriptor {
        SceneDocument::parse(b"{")
            .expect_err("a truncated document is rejected")
            .descriptor()
    }

    /// `invalid_json_error` as it appears in a report.
    pub(crate) const INVALID_JSON_ERROR_JSON: &str = concat!(
        r#"{"family":"scene_parse","category":null,"code":"scene.invalid-json","#,
        r#""message":"scene document is not valid JSON","format":null,"element_id":null,"#,
        r#""json_pointer":"","details":null}"#
    );

    fn utf8(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("the report is UTF-8")
    }

    #[test]
    fn a_render_report_line_is_one_json_object_with_every_field_present() {
        let diagnostics = missing_image_diagnostics();
        let report = RenderReport {
            schema_version: 1,
            status: "success",
            renderer_version: "0.1.0",
            compatibility_revision: Some(CompatibilityRevision::REVISION_1),
            excalidraw_release: Some("v0.18.1"),
            reference_profile_digest: Some("sha256:abc"),
            format: Some(OutputFormat::Svg),
            artifact: Some(ArtifactDescriptor {
                media_type: "image/svg+xml",
                byte_length: 42,
                sha256: "sha256:def".to_owned(),
            }),
            error: None,
            diagnostics: &[&diagnostics[..], &diagnostics[..]].concat(),
        };
        assert_eq!(
            utf8(json_line(&report)),
            [
                r#"{"schema_version":1,"status":"success","renderer_version":"0.1.0","#,
                r#""compatibility_revision":1,"excalidraw_release":"v0.18.1","#,
                r#""reference_profile_digest":"sha256:abc","format":"svg","#,
                r#""artifact":{"media_type":"image/svg+xml","byte_length":42,"sha256":"sha256:def"},"#,
                r#""error":null,"diagnostics":["#,
                MISSING_IMAGE_DIAGNOSTIC_JSON,
                ",",
                MISSING_IMAGE_DIAGNOSTIC_JSON,
                "]}\n",
            ]
            .concat()
        );
    }

    #[test]
    fn a_failed_render_report_line_carries_the_library_error_and_nulls_the_rest() {
        let diagnostics = missing_image_diagnostics();
        let report = RenderReport {
            schema_version: 1,
            status: "error",
            renderer_version: "0.1.0",
            compatibility_revision: None,
            excalidraw_release: None,
            reference_profile_digest: None,
            format: None,
            artifact: None,
            error: Some(Box::new(ReportError::Library(invalid_json_error()))),
            diagnostics: &diagnostics,
        };
        assert_eq!(
            utf8(json_line(&report)),
            [
                r#"{"schema_version":1,"status":"error","renderer_version":"0.1.0","#,
                r#""compatibility_revision":null,"excalidraw_release":null,"#,
                r#""reference_profile_digest":null,"format":null,"artifact":null,"error":"#,
                INVALID_JSON_ERROR_JSON,
                r#","diagnostics":["#,
                MISSING_IMAGE_DIAGNOSTIC_JSON,
                "]}\n",
            ]
            .concat()
        );
    }

    /// The report line of a failed PNG render whose CLI error carries
    /// `details`, with every optional error field present.
    fn cli_error_line(details: Option<CliDetails>) -> String {
        let report = RenderReport {
            schema_version: 1,
            status: "error",
            renderer_version: "0.1.0",
            compatibility_revision: Some(CompatibilityRevision::REVISION_1),
            excalidraw_release: Some("v0.18.1"),
            reference_profile_digest: Some("sha256:abc"),
            format: Some(OutputFormat::Png),
            artifact: None,
            error: Some(Box::new(ReportError::Cli(CliError {
                family: "cli",
                category: Some("usage"),
                code: "cli.example",
                message: TRICKY.to_owned(),
                format: Some(OutputFormat::Pdf),
                element_id: Some(TRICKY.to_owned()),
                json_pointer: Some("/elements/0".to_owned()),
                details,
            }))),
            diagnostics: &[],
        };
        utf8(json_line(&report))
    }

    /// The report line `cli_error_line` renders around `details_json`.
    fn expected_cli_error_line(details_json: &str) -> String {
        [
            r#"{"schema_version":1,"status":"error","renderer_version":"0.1.0","#,
            r#""compatibility_revision":1,"excalidraw_release":"v0.18.1","#,
            r#""reference_profile_digest":"sha256:abc","format":"png","artifact":null,"#,
            r#""error":{"family":"cli","category":"usage","code":"cli.example","message":"#,
            TRICKY_JSON,
            r#","format":"pdf","element_id":"#,
            TRICKY_JSON,
            r#","json_pointer":"/elements/0","details":"#,
            details_json,
            r#"},"diagnostics":[]}"#,
            "\n",
        ]
        .concat()
    }

    #[test]
    fn cli_error_fields_serialize_as_explicit_nulls_when_absent() {
        let report = RenderReport {
            schema_version: 1,
            status: "error",
            renderer_version: "0.1.0",
            compatibility_revision: None,
            excalidraw_release: None,
            reference_profile_digest: None,
            format: None,
            artifact: None,
            error: Some(Box::new(ReportError::Cli(CliError {
                family: "cli",
                category: None,
                code: "cli.format-required",
                message: "m".to_owned(),
                format: None,
                element_id: None,
                json_pointer: None,
                details: None,
            }))),
            diagnostics: &[],
        };
        assert_eq!(
            utf8(json_line(&report)),
            concat!(
                r#"{"schema_version":1,"status":"error","renderer_version":"0.1.0","#,
                r#""compatibility_revision":null,"excalidraw_release":null,"#,
                r#""reference_profile_digest":null,"format":null,"artifact":null,"#,
                r#""error":{"family":"cli","category":null,"code":"cli.format-required","#,
                r#""message":"m","format":null,"element_id":null,"json_pointer":null,"#,
                r#""details":null},"diagnostics":[]}"#,
                "\n"
            )
        );
        assert_eq!(cli_error_line(None), expected_cli_error_line("null"));
    }

    #[test]
    fn every_cli_details_variant_serializes_byte_exactly() {
        let cases = [
            (
                CliDetails::ParseLimit {
                    field: "max_scene_bytes",
                    configured: 67_108_864,
                    value_kind: "bytes",
                    measurement: CliMeasurement::AtLeast { value: u64::MAX },
                },
                concat!(
                    r#"{"kind":"parse_limit","field":"max_scene_bytes","configured":67108864,"#,
                    r#""value_kind":"bytes","measurement":{"kind":"at_least","value":18446744073709551615}}"#
                )
                .to_owned(),
            ),
            (
                CliDetails::ResourceLimit {
                    field: "max_asset_bytes",
                    configured: 0,
                    value_kind: "bytes",
                    measurement: CliMeasurement::AtLeast { value: 1 },
                    recovery: Some("omitted"),
                },
                concat!(
                    r#"{"kind":"resource_limit","field":"max_asset_bytes","configured":0,"#,
                    r#""value_kind":"bytes","measurement":{"kind":"at_least","value":1},"#,
                    r#""recovery":"omitted"}"#
                )
                .to_owned(),
            ),
            (
                CliDetails::ResourceLimit {
                    field: "max_frames",
                    configured: 16_384,
                    value_kind: "count",
                    measurement: CliMeasurement::AtLeast { value: 16_385 },
                    recovery: None,
                },
                concat!(
                    r#"{"kind":"resource_limit","field":"max_frames","configured":16384,"#,
                    r#""value_kind":"count","measurement":{"kind":"at_least","value":16385},"#,
                    r#""recovery":null}"#
                )
                .to_owned(),
            ),
            (
                CliDetails::CliArgument {
                    argument: TRICKY.to_owned(),
                    value: Some(TRICKY.to_owned()),
                },
                [
                    r#"{"kind":"cli_argument","argument":"#,
                    TRICKY_JSON,
                    r#","value":"#,
                    TRICKY_JSON,
                    "}",
                ]
                .concat(),
            ),
            (
                CliDetails::CliArgument {
                    argument: "--format".to_owned(),
                    value: None,
                },
                r#"{"kind":"cli_argument","argument":"--format","value":null}"#.to_owned(),
            ),
            (
                CliDetails::CliPath {
                    entries: Vec::new(),
                },
                r#"{"kind":"cli_path","entries":[]}"#.to_owned(),
            ),
            (
                CliDetails::CliPath {
                    entries: vec![
                        CliPathEntry {
                            role: "input",
                            asset_id: None,
                            path: "scene.excalidraw".to_owned(),
                        },
                        CliPathEntry {
                            role: "asset",
                            asset_id: Some(TRICKY.to_owned()),
                            path: TRICKY.to_owned(),
                        },
                    ],
                },
                [
                    r#"{"kind":"cli_path","entries":[{"role":"input","asset_id":null,"#,
                    r#""path":"scene.excalidraw"},{"role":"asset","asset_id":"#,
                    TRICKY_JSON,
                    r#","path":"#,
                    TRICKY_JSON,
                    "}]}",
                ]
                .concat(),
            ),
            (
                CliDetails::CliIo {
                    role: IoRole::Input,
                    asset_id: None,
                    operation: CliOperation::Open,
                    path: TRICKY.to_owned(),
                },
                [
                    r#"{"kind":"cli_io","role":"input","asset_id":null,"operation":"open","path":"#,
                    TRICKY_JSON,
                    "}",
                ]
                .concat(),
            ),
            (
                CliDetails::CliIo {
                    role: IoRole::Asset,
                    asset_id: Some("a1".to_owned()),
                    operation: CliOperation::Read,
                    path: "a.png".to_owned(),
                },
                concat!(
                    r#"{"kind":"cli_io","role":"asset","asset_id":"a1","operation":"read","#,
                    r#""path":"a.png"}"#
                )
                .to_owned(),
            ),
            (
                CliDetails::Delivery {
                    role: DeliveryRole::Output,
                    operation: CliOperation::Replace,
                    path: TRICKY.to_owned(),
                    artifact_state: DeliveryState::PossiblyPartial,
                },
                [
                    r#"{"kind":"delivery","role":"output","operation":"replace","path":"#,
                    TRICKY_JSON,
                    r#","artifact_state":"possibly_partial"}"#,
                ]
                .concat(),
            ),
            (
                CliDetails::Delivery {
                    role: DeliveryRole::Report,
                    operation: CliOperation::Create,
                    path: "r.json".to_owned(),
                    artifact_state: DeliveryState::Complete,
                },
                concat!(
                    r#"{"kind":"delivery","role":"report","operation":"create","#,
                    r#""path":"r.json","artifact_state":"complete"}"#
                )
                .to_owned(),
            ),
        ];
        for (details, details_json) in cases {
            assert_eq!(
                cli_error_line(Some(details)),
                expected_cli_error_line(&details_json)
            );
        }
    }

    #[test]
    fn every_operation_and_delivery_state_keeps_its_spelling() {
        for (operation, spelling) in [
            (CliOperation::Open, "open"),
            (CliOperation::Read, "read"),
            (CliOperation::Create, "create"),
            (CliOperation::Write, "write"),
            (CliOperation::Flush, "flush"),
            (CliOperation::Replace, "replace"),
        ] {
            for (state, state_spelling) in [
                (DeliveryState::None, "none"),
                (DeliveryState::Complete, "complete"),
                (DeliveryState::PossiblyPartial, "possibly_partial"),
            ] {
                let details = CliDetails::Delivery {
                    role: DeliveryRole::Output,
                    operation,
                    path: "o".to_owned(),
                    artifact_state: state,
                };
                assert_eq!(
                    cli_error_line(Some(details)),
                    expected_cli_error_line(&format!(
                        r#"{{"kind":"delivery","role":"output","operation":"{spelling}","path":"o","artifact_state":"{state_spelling}"}}"#
                    ))
                );
            }
        }
    }

    #[test]
    fn a_revision_report_lists_every_revision_in_order() {
        let report = RevisionReport {
            schema_version: 1,
            renderer_version: "0.1.0",
            revisions: vec![
                RevisionEntry {
                    compatibility_revision: CompatibilityRevision::REVISION_1,
                    excalidraw_release: "v0.18.1",
                    reference_profile_digest: "sha256:abc",
                    default: true,
                },
                RevisionEntry {
                    compatibility_revision: CompatibilityRevision::REVISION_1,
                    excalidraw_release: TRICKY,
                    reference_profile_digest: "sha256:def",
                    default: false,
                },
            ],
        };
        assert_eq!(
            utf8(json_line(&report)),
            [
                r#"{"schema_version":1,"renderer_version":"0.1.0","revisions":["#,
                r#"{"compatibility_revision":1,"excalidraw_release":"v0.18.1","#,
                r#""reference_profile_digest":"sha256:abc","default":true},"#,
                r#"{"compatibility_revision":1,"excalidraw_release":"#,
                TRICKY_JSON,
                r#","reference_profile_digest":"sha256:def","default":false}]}"#,
                "\n",
            ]
            .concat()
        );
        let empty = RevisionReport {
            schema_version: 1,
            renderer_version: "0.1.0",
            revisions: Vec::new(),
        };
        assert_eq!(
            utf8(json_line(&empty)),
            "{\"schema_version\":1,\"renderer_version\":\"0.1.0\",\"revisions\":[]}\n"
        );
    }
}
