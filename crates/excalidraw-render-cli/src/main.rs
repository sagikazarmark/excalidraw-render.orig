use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
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
    about = "Render Excalidraw scene documents to SVG, PNG, or PDF"
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

struct ArtifactDescriptor {
    media_type: &'static str,
    byte_length: usize,
    sha256: String,
}

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

struct CliPathEntry {
    role: &'static str,
    asset_id: Option<String>,
    path: String,
}

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

struct RevisionReport {
    schema_version: u32,
    renderer_version: &'static str,
    revisions: Vec<RevisionEntry>,
}

struct RevisionEntry {
    compatibility_revision: CompatibilityRevision,
    excalidraw_release: &'static str,
    reference_profile_digest: &'static str,
    default: bool,
}

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
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
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
        ReportFormat::Json => revision_json_line(&RevisionReport {
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

fn success_report_bytes(
    mode: ReportFormat,
    report: &RenderReport<'_>,
    diagnostics: &[CompatibilityDiagnostic],
) -> Vec<u8> {
    match mode {
        ReportFormat::Json => render_json_line(report),
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
    render_json_line(&RenderReport {
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

struct StagedFile {
    temporary: PathBuf,
    destination: PathBuf,
    replace: bool,
}

impl StagedFile {
    fn new(destination: &Path, bytes: &[u8], replace: bool) -> Result<Self, CliOperation> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("output");
        for attempt in 0..100_u32 {
            let temporary = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), attempt));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(mut file) => {
                    if file.write_all(bytes).is_err() {
                        let _ = fs::remove_file(&temporary);
                        return Err(CliOperation::Write);
                    }
                    if file.flush().is_err() || file.sync_all().is_err() {
                        let _ = fs::remove_file(&temporary);
                        return Err(CliOperation::Flush);
                    }
                    return Ok(Self {
                        temporary,
                        destination: destination.to_path_buf(),
                        replace,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(CliOperation::Create),
            }
        }
        Err(CliOperation::Create)
    }

    fn deliver(mut self) -> Result<(), CliOperation> {
        if self.replace {
            fs::rename(&self.temporary, &self.destination).map_err(|_| CliOperation::Replace)?;
        } else {
            fs::hard_link(&self.temporary, &self.destination).map_err(|_| CliOperation::Create)?;
            if fs::remove_file(&self.temporary).is_ok() {
                self.temporary.clear();
            }
        }
        if self.replace {
            self.temporary.clear();
        }
        Ok(())
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        if !self.temporary.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.temporary);
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

fn revision_json_line(report: &RevisionReport) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"{\"schema_version\":");
    bytes.extend_from_slice(report.schema_version.to_string().as_bytes());
    bytes.extend_from_slice(b",\"renderer_version\":");
    push_json_string(&mut bytes, report.renderer_version);
    bytes.extend_from_slice(b",\"revisions\":[");
    for (index, revision) in report.revisions.iter().enumerate() {
        if index != 0 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(b"{\"compatibility_revision\":");
        bytes.extend_from_slice(revision.compatibility_revision.get().to_string().as_bytes());
        bytes.extend_from_slice(b",\"excalidraw_release\":");
        push_json_string(&mut bytes, revision.excalidraw_release);
        bytes.extend_from_slice(b",\"reference_profile_digest\":");
        push_json_string(&mut bytes, revision.reference_profile_digest);
        bytes.extend_from_slice(b",\"default\":");
        bytes.extend_from_slice(if revision.default { b"true" } else { b"false" });
        bytes.push(b'}');
    }
    bytes.extend_from_slice(b"]}\n");
    bytes
}

fn render_json_line(report: &RenderReport<'_>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"{\"schema_version\":");
    bytes.extend_from_slice(report.schema_version.to_string().as_bytes());
    bytes.extend_from_slice(b",\"status\":");
    push_json_string(&mut bytes, report.status);
    bytes.extend_from_slice(b",\"renderer_version\":");
    push_json_string(&mut bytes, report.renderer_version);
    bytes.extend_from_slice(b",\"compatibility_revision\":");
    if let Some(revision) = report.compatibility_revision {
        bytes.extend_from_slice(revision.get().to_string().as_bytes());
    } else {
        bytes.extend_from_slice(b"null");
    }
    bytes.extend_from_slice(b",\"excalidraw_release\":");
    push_optional_json_string(&mut bytes, report.excalidraw_release);
    bytes.extend_from_slice(b",\"reference_profile_digest\":");
    push_optional_json_string(&mut bytes, report.reference_profile_digest);
    bytes.extend_from_slice(b",\"format\":");
    if let Some(format) = report.format {
        push_json_string(&mut bytes, format.as_str());
    } else {
        bytes.extend_from_slice(b"null");
    }
    bytes.extend_from_slice(b",\"artifact\":");
    if let Some(artifact) = &report.artifact {
        bytes.extend_from_slice(b"{\"media_type\":");
        push_json_string(&mut bytes, artifact.media_type);
        bytes.extend_from_slice(b",\"byte_length\":");
        bytes.extend_from_slice(artifact.byte_length.to_string().as_bytes());
        bytes.extend_from_slice(b",\"sha256\":");
        push_json_string(&mut bytes, &artifact.sha256);
        bytes.push(b'}');
    } else {
        bytes.extend_from_slice(b"null");
    }
    bytes.extend_from_slice(b",\"error\":");
    if let Some(error) = &report.error {
        push_report_error(&mut bytes, error);
    } else {
        bytes.extend_from_slice(b"null");
    }
    bytes.extend_from_slice(b",\"diagnostics\":[");
    for (index, diagnostic) in report.diagnostics.iter().enumerate() {
        if index != 0 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(
            &serde_json::to_vec(diagnostic).expect("compatibility diagnostic serializes"),
        );
    }
    bytes.extend_from_slice(b"]}\n");
    bytes
}

fn push_report_error(bytes: &mut Vec<u8>, error: &ReportError) {
    match error {
        ReportError::Library(error) => bytes.extend_from_slice(
            &serde_json::to_vec(error).expect("library error descriptor serializes"),
        ),
        ReportError::Cli(error) => {
            bytes.extend_from_slice(b"{\"family\":");
            push_json_string(bytes, error.family);
            bytes.extend_from_slice(b",\"category\":");
            push_optional_json_string(bytes, error.category);
            bytes.extend_from_slice(b",\"code\":");
            push_json_string(bytes, error.code);
            bytes.extend_from_slice(b",\"message\":");
            push_json_string(bytes, &error.message);
            bytes.extend_from_slice(b",\"format\":");
            if let Some(format) = error.format {
                push_json_string(bytes, format.as_str());
            } else {
                bytes.extend_from_slice(b"null");
            }
            bytes.extend_from_slice(b",\"element_id\":");
            push_optional_json_string(bytes, error.element_id.as_deref());
            bytes.extend_from_slice(b",\"json_pointer\":");
            push_optional_json_string(bytes, error.json_pointer.as_deref());
            bytes.extend_from_slice(b",\"details\":");
            if let Some(details) = &error.details {
                push_cli_details(bytes, details);
            } else {
                bytes.extend_from_slice(b"null");
            }
            bytes.push(b'}');
        }
    }
}

fn push_cli_details(bytes: &mut Vec<u8>, details: &CliDetails) {
    bytes.push(b'{');
    match details {
        CliDetails::ParseLimit {
            field,
            configured,
            value_kind,
            measurement,
        } => {
            bytes.extend_from_slice(b"\"kind\":\"parse_limit\",\"field\":");
            push_json_string(bytes, field);
            push_limit_details(bytes, *configured, value_kind, measurement);
        }
        CliDetails::ResourceLimit {
            field,
            configured,
            value_kind,
            measurement,
            recovery,
        } => {
            bytes.extend_from_slice(b"\"kind\":\"resource_limit\",\"field\":");
            push_json_string(bytes, field);
            push_limit_details(bytes, *configured, value_kind, measurement);
            bytes.extend_from_slice(b",\"recovery\":");
            push_optional_json_string(bytes, *recovery);
        }
        CliDetails::CliArgument { argument, value } => {
            bytes.extend_from_slice(b"\"kind\":\"cli_argument\",\"argument\":");
            push_json_string(bytes, argument);
            bytes.extend_from_slice(b",\"value\":");
            push_optional_json_string(bytes, value.as_deref());
        }
        CliDetails::CliPath { entries } => {
            bytes.extend_from_slice(b"\"kind\":\"cli_path\",\"entries\":[");
            for (index, entry) in entries.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                bytes.extend_from_slice(b"{\"role\":");
                push_json_string(bytes, entry.role);
                bytes.extend_from_slice(b",\"asset_id\":");
                push_optional_json_string(bytes, entry.asset_id.as_deref());
                bytes.extend_from_slice(b",\"path\":");
                push_json_string(bytes, &entry.path);
                bytes.push(b'}');
            }
            bytes.push(b']');
        }
        CliDetails::CliIo {
            role,
            asset_id,
            operation,
            path,
        } => {
            bytes.extend_from_slice(b"\"kind\":\"cli_io\",\"role\":");
            push_json_string(bytes, role.as_str());
            bytes.extend_from_slice(b",\"asset_id\":");
            push_optional_json_string(bytes, asset_id.as_deref());
            bytes.extend_from_slice(b",\"operation\":");
            push_json_string(bytes, operation.as_str());
            bytes.extend_from_slice(b",\"path\":");
            push_json_string(bytes, path);
        }
        CliDetails::Delivery {
            role,
            operation,
            path,
            artifact_state,
        } => {
            bytes.extend_from_slice(b"\"kind\":\"delivery\",\"role\":");
            push_json_string(bytes, role.as_str());
            bytes.extend_from_slice(b",\"operation\":");
            push_json_string(bytes, operation.as_str());
            bytes.extend_from_slice(b",\"path\":");
            push_json_string(bytes, path);
            bytes.extend_from_slice(b",\"artifact_state\":");
            push_json_string(bytes, artifact_state.as_str());
        }
    }
    bytes.push(b'}');
}

fn push_limit_details(
    bytes: &mut Vec<u8>,
    configured: u64,
    value_kind: &str,
    measurement: &CliMeasurement,
) {
    bytes.extend_from_slice(b",\"configured\":");
    bytes.extend_from_slice(configured.to_string().as_bytes());
    bytes.extend_from_slice(b",\"value_kind\":");
    push_json_string(bytes, value_kind);
    bytes.extend_from_slice(b",\"measurement\":");
    match measurement {
        CliMeasurement::AtLeast { value } => {
            bytes.extend_from_slice(b"{\"kind\":\"at_least\",\"value\":");
            bytes.extend_from_slice(value.to_string().as_bytes());
            bytes.push(b'}');
        }
    }
}

fn push_optional_json_string(bytes: &mut Vec<u8>, value: Option<&str>) {
    if let Some(value) = value {
        push_json_string(bytes, value);
    } else {
        bytes.extend_from_slice(b"null");
    }
}

fn push_json_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&serde_json::to_vec(value).expect("CLI string serializes"));
}

fn sha256(bytes: &[u8]) -> String {
    let digest = sha256_digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::from("sha256:");
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sha256_digest(input: &[u8]) -> [u8; 32] {
    let mut state = [
        0x6a09_e667_u32,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut blocks = input.chunks_exact(64);
    for block in &mut blocks {
        sha256_compress(&mut state, block);
    }
    let remainder = blocks.remainder();
    let mut final_blocks = [0_u8; 128];
    final_blocks[..remainder.len()].copy_from_slice(remainder);
    final_blocks[remainder.len()] = 0x80;
    let final_len = if remainder.len() < 56 { 64 } else { 128 };
    let bit_len = u64::try_from(input.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    final_blocks[final_len - 8..final_len].copy_from_slice(&bit_len.to_be_bytes());
    for block in final_blocks[..final_len].chunks_exact(64) {
        sha256_compress(&mut state, block);
    }
    let mut digest = [0_u8; 32];
    for (word, output) in state.iter().zip(digest.chunks_exact_mut(4)) {
        output.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn sha256_compress(state: &mut [u32; 8], block: &[u8]) {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut schedule = [0_u32; 64];
    for (index, word) in schedule[..16].iter_mut().enumerate() {
        let offset = index * 4;
        *word = u32::from_be_bytes([
            block[offset],
            block[offset + 1],
            block[offset + 2],
            block[offset + 3],
        ]);
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let choice = (e & f) ^ ((!e) & g);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let first = h
            .wrapping_add(sum1)
            .wrapping_add(choice)
            .wrapping_add(K[index])
            .wrapping_add(schedule[index]);
        let second = sum0.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (word, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(value);
    }
}
