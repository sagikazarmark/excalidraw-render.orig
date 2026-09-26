use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use excalidraw_render::{
    license_bundle, ColorMode, CompatibilityDiagnostic, CompatibilityRevision, ErrorDescriptor,
    FrameOptions, MetadataOptions, OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest,
    Renderer, ResourceLimits, SceneDocument, Sizing, SupplementalAssets,
    DEFAULT_COMPATIBILITY_REVISION,
};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use tempfile::TempPath;

mod frames;

use frames::{run_frames, FramesCli};

const EXIT_OPERATIONAL: u8 = 1;
const EXIT_USAGE: u8 = 2;
const MAX_SCENE_BYTES: u64 = 67_108_864;

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
    #[default]
    Human,
    Json,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum PdfFallbackArg {
    AllowIsolated,
    Forbid,
}

#[derive(Parser)]
#[command(
    name = "excalidraw-render",
    disable_version_flag = true,
    about = "Render Excalidraw scene documents to SVG, PNG, or PDF",
    after_help = "Use `excalidraw-render frames --help` to export each selected frame to a directory."
)]
struct Cli {
    /// Input Excalidraw scene document, or - for stdin.
    #[arg(allow_hyphen_values = true)]
    input: Option<String>,

    /// Output path, or - for stdout.
    #[arg(short, long, allow_hyphen_values = true)]
    output: Option<String>,

    #[arg(short, long, value_enum)]
    format: Option<FormatArg>,
    #[arg(long)]
    compatibility_revision: Option<String>,
    #[arg(long)]
    no_background: bool,
    #[arg(long)]
    padding: Option<f64>,
    #[arg(long, conflicts_with = "max_dimension")]
    scale: Option<f64>,
    #[arg(long)]
    max_dimension: Option<f64>,
    #[arg(long)]
    dark_mode: bool,
    #[arg(long)]
    embed_scene: bool,
    #[arg(long)]
    embed_provenance: bool,
    #[arg(long)]
    embed_diagnostics: bool,
    #[arg(long)]
    no_frame_rendering: bool,
    #[arg(long)]
    no_frame_clipping: bool,
    #[arg(long)]
    no_frame_names: bool,
    #[arg(long)]
    no_frame_outlines: bool,
    #[arg(long, value_name = "FILE_ID=PATH")]
    asset: Vec<String>,
    #[arg(long, value_name = "FILE_ID=MEDIA_TYPE")]
    asset_mime: Vec<String>,
    #[arg(long, value_enum)]
    pdf_raster_fallback: Option<PdfFallbackArg>,
    #[arg(long, value_enum, default_value_t)]
    report_format: ReportFormat,
    #[arg(long)]
    report_output: Option<String>,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    list_revisions: bool,
    #[arg(long)]
    version: bool,
    #[arg(long)]
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
    fn human_parts(&self) -> (&str, &str, &str) {
        match self {
            Self::Library(error) => (
                error.family().as_str(),
                error.code().as_str(),
                error.message(),
            ),
            Self::Cli(error) => (error.family, error.code, &error.message),
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
        }
    }

    fn library(descriptor: ErrorDescriptor, exit: u8) -> Self {
        Self {
            error: Box::new(ReportError::Library(descriptor)),
            exit,
            diagnostics: Vec::new(),
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().collect();
    let preparse_json = preparse_json_mode(&args);
    if args.get(1).is_some_and(|value| value == "frames") {
        let mut frame_args = args.clone();
        frame_args.remove(1);
        let frames = match FramesCli::try_parse_from(&frame_args) {
            Ok(frames) => frames,
            Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
                return print_help(error);
            }
            Err(error) => {
                let failure = Failure::cli(
                    "cli.invalid-arguments",
                    error.to_string().trim().to_owned(),
                    None,
                    EXIT_USAGE,
                );
                emit_early_failure(failure, preparse_json);
                return ExitCode::from(EXIT_USAGE);
            }
        };
        return ExitCode::from(run_frames(frames));
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            return print_help(error);
        }
        Err(error) => {
            let failure = Failure::cli(
                "cli.invalid-arguments",
                error.to_string().trim().to_owned(),
                None,
                EXIT_USAGE,
            );
            emit_early_failure(failure, preparse_json);
            return ExitCode::from(EXIT_USAGE);
        }
    };

    ExitCode::from(run(cli))
}

fn print_help(help: clap::Error) -> ExitCode {
    if help.print().is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_OPERATIONAL)
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

    if cli.version {
        let profile = Renderer::profile_identity(DEFAULT_COMPATIBILITY_REVISION)
            .expect("default revision is supported");
        let text = format!(
            "excalidraw-render {}\ncompatibility revision {}\nExcalidraw {}\nreference profile {}\n",
            env!("CARGO_PKG_VERSION"),
            DEFAULT_COMPATIBILITY_REVISION,
            profile.excalidraw_release(),
            profile.profile_digest()
        );
        return write_stdout(text.as_bytes());
    }
    if cli.licenses {
        return write_stdout(license_bundle().as_bytes());
    }

    let revisions = Renderer::supported_revisions()
        .iter()
        .map(|&revision| {
            let profile =
                Renderer::profile_identity(revision).expect("listed revision is supported");
            RevisionEntry {
                compatibility_revision: revision,
                excalidraw_release: profile.excalidraw_release(),
                reference_profile_digest: profile.profile_digest(),
                default: revision == DEFAULT_COMPATIBILITY_REVISION,
            }
        })
        .collect::<Vec<_>>();
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
    if report_output(cli) != "stderr" && path_entry_exists(Path::new(report_output(cli))) {
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
                EXIT_USAGE,
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
    })?;

    let revision = match &cli.compatibility_revision {
        Some(value) => value.parse::<CompatibilityRevision>().map_err(|_| {
            Failure::cli(
                "cli.invalid-arguments",
                "compatibility revision must be a nonnegative integer",
                Some(CliDetails::CliArgument {
                    argument: "--compatibility-revision".to_owned(),
                    value: Some(value.clone()),
                }),
                EXIT_USAGE,
            )
        })?,
        None => DEFAULT_COMPATIBILITY_REVISION,
    };
    let renderer = Renderer::for_revision(revision)
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))?;
    let profile = renderer.selected_profile_identity();
    *identity = Some(SelectedIdentity {
        revision,
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

    let scene_bytes = read_scene_bounded(input, MAX_SCENE_BYTES)?;
    let scene = SceneDocument::parse(&scene_bytes)
        .map_err(|error| Failure::library(error.descriptor(), EXIT_OPERATIONAL))?;

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
        .frame_options(FrameOptions::new(
            !cli.no_frame_rendering,
            !cli.no_frame_clipping,
            !cli.no_frame_names,
            !cli.no_frame_outlines,
        ))
        .metadata_options(MetadataOptions::new(
            cli.embed_scene,
            cli.embed_provenance,
            cli.embed_diagnostics,
        ))
        .supplemental_assets(assets)
        .resource_limits(ResourceLimits::default());
    if let Some(fallback) = cli.pdf_raster_fallback {
        builder = builder.pdf_options(PdfOptions::new(match fallback {
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
            "explicit format does not match the output extension",
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
    Failure::cli(
        "cli.path-conflict",
        "input, asset, output, and report paths must not alias",
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

fn check_destinations(output: &str, report: &str, force: bool) -> Result<(), Failure> {
    for (role, path) in [
        (DeliveryRole::Output, output),
        (DeliveryRole::Report, report),
    ] {
        if (role == DeliveryRole::Output && path == "-")
            || (role == DeliveryRole::Report && path == "stderr")
        {
            continue;
        }
        if path_entry_exists(Path::new(path)) && !force {
            return Err(Failure::cli(
                "cli.destination-exists",
                format!("{} destination already exists", role.as_str()),
                Some(CliDetails::Delivery {
                    role,
                    operation: CliOperation::Create,
                    path: path.to_owned(),
                    artifact_state: DeliveryState::None,
                }),
                EXIT_USAGE,
            ));
        }
    }
    Ok(())
}

fn read_scene_bounded(path: &str, limit: u64) -> Result<Vec<u8>, Failure> {
    let mut reader: Box<dyn Read> = if path == "-" {
        Box::new(io::stdin().lock())
    } else {
        Box::new(
            File::open(path)
                .map_err(|_| io_failure(IoRole::Input, None, CliOperation::Open, path))?,
        )
    };
    let capacity = usize::try_from(limit.min(1024 * 1024)).unwrap_or(1024 * 1024);
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| io_failure(IoRole::Input, None, CliOperation::Read, path))?;
    let measured = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if measured > limit {
        return Err(Failure::cli(
            "scene.input-too-large",
            "scene input exceeds the fixed parser transport ceiling",
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
    let file = File::open(path)
        .map_err(|_| io_failure(IoRole::Asset, Some(asset_id), CliOperation::Open, path))?;
    let stream_limit = per_asset_limit.min(aggregate_remaining);
    let capacity = usize::try_from(stream_limit.min(1024 * 1024)).unwrap_or(1024 * 1024);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(stream_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| io_failure(IoRole::Asset, Some(asset_id), CliOperation::Read, path))?;
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
) -> Failure {
    let code = if role == IoRole::Input {
        "cli.input-read-failed"
    } else {
        "cli.asset-read-failed"
    };
    Failure::cli(
        code,
        format!(
            "failed to {} {}",
            operation.as_str(),
            if role == IoRole::Input {
                "input"
            } else {
                "asset"
            }
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
    let report_bytes = success_report_bytes(cli.report_format, &report, &diagnostics);

    let output = cli.output.as_deref().expect("output validated");
    let mut staged_output = if output == "-" {
        None
    } else {
        match StagedFile::new(Path::new(output), &artifact, cli.force) {
            Ok(staged) => Some(staged),
            Err(operation) => {
                return emit_failure(
                    cli,
                    Some(identity),
                    Some(format),
                    delivery_failure(
                        DeliveryRole::Output,
                        operation,
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
            Err(operation) => {
                return emit_failure(
                    cli,
                    Some(identity),
                    Some(format),
                    delivery_failure(
                        DeliveryRole::Report,
                        operation,
                        report_output,
                        DeliveryState::None,
                        diagnostics.clone(),
                    ),
                )
            }
        }
    };

    if let Some(staged) = staged_output.take() {
        if let Err(operation) = staged.deliver() {
            return emit_failure(
                cli,
                Some(identity),
                Some(format),
                delivery_failure(
                    DeliveryRole::Output,
                    operation,
                    output,
                    DeliveryState::None,
                    diagnostics.clone(),
                ),
            );
        }
    } else if let Err((operation, artifact_state)) =
        write_stream_tracked(&mut io::stdout().lock(), &artifact)
    {
        return emit_failure(
            cli,
            Some(identity),
            Some(format),
            delivery_failure(
                DeliveryRole::Output,
                operation,
                output,
                artifact_state,
                diagnostics.clone(),
            ),
        );
    }

    if let Some(staged) = staged_report.take() {
        if let Err(operation) = staged.deliver() {
            return emergency_report_failure(
                cli,
                identity,
                format,
                operation,
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

fn stage_report_file(cli: &Cli, bytes: &[u8]) -> Result<Option<StagedFile>, CliOperation> {
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
) -> Vec<u8> {
    match mode {
        ReportFormat::Json => json_line(report),
        ReportFormat::Human => diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}\n", diagnostic.code(), diagnostic.message()))
            .collect::<String>()
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
    let bytes = failure_report_bytes(cli.report_format, identity, format, failure);
    let report_output = report_output(cli);
    if report_output == "stderr"
        || cli.report_format == ReportFormat::None
        || !report_destination_is_safe(cli)
    {
        let _ = write_stream(&mut io::stderr().lock(), &bytes);
    } else if let Err(operation) =
        StagedFile::new(Path::new(report_output), &bytes, cli.force).and_then(StagedFile::deliver)
    {
        let fallback = delivery_failure(
            DeliveryRole::Report,
            operation,
            report_output,
            DeliveryState::None,
            diagnostics,
        );
        let emergency = failure_report_bytes(
            emergency_report_format(cli.report_format),
            identity,
            format,
            fallback,
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
    if report == "stderr" || (path_entry_exists(Path::new(report)) && !cli.force) {
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

fn failure_report_bytes(
    mode: ReportFormat,
    identity: Option<&SelectedIdentity>,
    format: Option<OutputFormat>,
    failure: Failure,
) -> Vec<u8> {
    if mode != ReportFormat::Json {
        let (family, code, message) = failure.error.human_parts();
        let mut text = String::new();
        if mode == ReportFormat::Human {
            for diagnostic in &failure.diagnostics {
                text.push_str(&format!(
                    "{}: {}\n",
                    diagnostic.code(),
                    diagnostic.message()
                ));
            }
        }
        text.push_str(&format!("{} {}: {}\n", family, code, message));
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

fn emit_early_failure(failure: Failure, json: bool) {
    let bytes = failure_report_bytes(
        if json {
            ReportFormat::Json
        } else {
            ReportFormat::Human
        },
        None,
        None,
        failure,
    );
    let _ = write_stream(&mut io::stderr().lock(), &bytes);
}

fn delivery_failure(
    role: DeliveryRole,
    operation: CliOperation,
    path: &str,
    artifact_state: DeliveryState,
    diagnostics: Vec<CompatibilityDiagnostic>,
) -> Failure {
    let mut failure = Failure::cli(
        if role == DeliveryRole::Output {
            "cli.output-write-failed"
        } else {
            "cli.report-write-failed"
        },
        format!("failed to deliver {}", role.as_str()),
        Some(CliDetails::Delivery {
            role,
            operation,
            path: path.to_owned(),
            artifact_state,
        }),
        EXIT_OPERATIONAL,
    );
    failure.diagnostics = diagnostics;
    failure
}

fn emergency_report_failure(
    cli: &Cli,
    identity: &SelectedIdentity,
    format: OutputFormat,
    operation: CliOperation,
    artifact_state: DeliveryState,
    diagnostics: Vec<CompatibilityDiagnostic>,
) -> u8 {
    let failure = delivery_failure(
        DeliveryRole::Report,
        operation,
        report_output(cli),
        artifact_state,
        diagnostics,
    );
    let bytes = failure_report_bytes(
        emergency_report_format(cli.report_format),
        Some(identity),
        Some(format),
        failure,
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
        Err(operation) => {
            let failure = delivery_failure(
                DeliveryRole::Report,
                operation,
                report_output,
                artifact_state,
                Vec::new(),
            );
            let bytes = failure_report_bytes(cli.report_format, None, None, failure);
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

fn write_stream(writer: &mut impl Write, bytes: &[u8]) -> Result<(), CliOperation> {
    write_stream_tracked(writer, bytes).map_err(|(operation, _)| operation)
}

fn write_stream_tracked(
    writer: &mut impl Write,
    mut bytes: &[u8],
) -> Result<(), (CliOperation, DeliveryState)> {
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
                return Err((CliOperation::Write, state(wrote_bytes)));
            }
            Ok(written) => {
                wrote_bytes = true;
                bytes = &bytes[written..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return Err((CliOperation::Write, state(wrote_bytes)));
            }
        }
    }
    loop {
        match writer.flush() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err((CliOperation::Flush, state(wrote_bytes))),
        }
    }
}

/// A fully written, synced temporary sibling of one destination. Only its path
/// is held, so staging many frames never holds one descriptor per frame.
struct StagedFile {
    temporary: TempPath,
    destination: PathBuf,
    replace: bool,
}

impl StagedFile {
    fn new(destination: &Path, bytes: &[u8], replace: bool) -> Result<Self, CliOperation> {
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
    ) -> Result<Self, CliOperation> {
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
            .map_err(|_| CliOperation::Create)?;
        file.write_all(bytes).map_err(|_| CliOperation::Write)?;
        file.flush()
            .and_then(|()| file.as_file().sync_all())
            .map_err(|_| CliOperation::Flush)?;
        Ok(Self {
            temporary: file.into_temp_path(),
            destination: destination.to_path_buf(),
            replace,
        })
    }

    /// Atomically publishes the staged file; without `replace` an existing
    /// destination, even one created after staging, is never overwritten.
    fn deliver(self) -> Result<(), CliOperation> {
        if self.replace {
            self.temporary
                .persist(&self.destination)
                .map_err(|_| CliOperation::Replace)
        } else {
            self.temporary
                .persist_noclobber(&self.destination)
                .map_err(|_| CliOperation::Create)
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
    format!("sha256:{:x}", Sha256::digest(bytes))
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
        CliMeasurement, CliOperation, CliPathEntry, DeliveryRole, DeliveryState, Failure,
        FormatArg, IoRole, RenderReport, ReportError, RevisionEntry, RevisionReport, StagedFile,
        EXIT_USAGE,
    };

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
            .unwrap_or_else(|operation| panic!("staging failed at {}", operation.as_str()));
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
            .unwrap_or_else(|operation| panic!("staging failed at {}", operation.as_str()));
        fs::write(&destination, b"late").expect("a racing writer creates the destination");

        assert!(matches!(staged.deliver(), Err(CliOperation::Create)));
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
            .unwrap_or_else(|operation| panic!("staging failed at {}", operation.as_str()));
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
            Err(CliOperation::Create)
        ));

        let staged = StagedFile::new(&scratch.path().join("result.svg"), b"artifact", false)
            .unwrap_or_else(|operation| panic!("staging failed at {}", operation.as_str()));
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
                    .unwrap_or_else(|operation| panic!("staging failed at {}", operation.as_str()))
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
