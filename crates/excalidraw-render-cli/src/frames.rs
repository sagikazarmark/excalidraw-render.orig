use crate::*;

#[derive(Parser)]
#[command(
    name = "excalidraw-render frames",
    disable_version_flag = true,
    about = "Render each selected Excalidraw frame to a separate artifact"
)]
pub(crate) struct FramesCli {
    /// Input Excalidraw scene document, or - for stdin.
    #[arg(allow_hyphen_values = true)]
    input: String,
    /// Output directory. Not required in `--list` mode, which writes nothing.
    #[arg(
        short,
        long,
        allow_hyphen_values = true,
        required_unless_present = "list"
    )]
    output: Option<String>,
    #[arg(short, long, value_enum, required_unless_present = "list")]
    format: Option<FormatArg>,
    #[arg(long)]
    compatibility_revision: Option<String>,
    #[arg(long)]
    no_background: bool,
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
    name_regex: Option<String>,
    #[arg(long, default_value_t = 256)]
    max_frames: u32,
    #[arg(long)]
    allow_partial: bool,
    /// Report the frames this selection would export without rendering or
    /// writing anything.
    #[arg(long)]
    list: bool,
}

impl FramesCli {
    /// Clap requires both outside `--list` mode, so the render path never sees
    /// them absent.
    fn output(&self) -> &str {
        self.output
            .as_deref()
            .expect("--output is required outside list mode")
    }

    fn format(&self) -> FormatArg {
        self.format.expect("--format is required outside list mode")
    }

    fn report_cli(&self) -> Cli {
        Cli {
            input: Some(self.input.clone()),
            output: self.output.clone(),
            format: self.format,
            compatibility_revision: self.compatibility_revision.clone(),
            no_background: self.no_background,
            padding: None,
            scale: self.scale,
            max_dimension: self.max_dimension,
            dark_mode: self.dark_mode,
            embed_scene: self.embed_scene,
            embed_provenance: self.embed_provenance,
            embed_diagnostics: self.embed_diagnostics,
            no_frame_rendering: false,
            no_frame_clipping: false,
            no_frame_names: false,
            no_frame_outlines: false,
            asset: self.asset.clone(),
            asset_mime: self.asset_mime.clone(),
            pdf_raster_fallback: self.pdf_raster_fallback,
            report_format: self.report_format,
            report_output: self.report_output.clone(),
            force: self.force,
            list_revisions: false,
            version: false,
            licenses: false,
        }
    }
}

struct FrameCliEntry {
    ordinal: u32,
    kind: &'static str,
    name: Option<String>,
    output_path: String,
    artifact: Option<ArtifactDescriptor>,
    error: Option<ReportError>,
    diagnostics: Vec<CompatibilityDiagnostic>,
    delivery_state: DeliveryState,
}

struct PreparedFrameBatch {
    entries: Vec<(FrameCliEntry, Option<StagedFile>)>,
    global_diagnostics: Vec<CompatibilityDiagnostic>,
    had_failure: bool,
}

struct FrameListEntry {
    ordinal: u32,
    kind: &'static str,
    name: Option<String>,
    basename: String,
}

pub(crate) fn run_frames(frames: FramesCli) -> u8 {
    if frames.list {
        return run_frames_list(&frames);
    }
    let cli = frames.report_cli();
    let mut identity = None;
    let format = OutputFormat::from(frames.format());
    let result = prepare_and_render_frames(&frames, &mut identity);
    match result {
        Ok(batch) => deliver_frame_batch(
            &cli,
            identity.as_ref().expect("frame identity selected"),
            format,
            batch.entries,
            batch.global_diagnostics,
            batch.had_failure,
            frames.allow_partial,
        ),
        Err(failure) => emit_failure(&cli, identity.as_ref(), Some(format), failure),
    }
}

/// `--list` is a dry run: it applies the same selection the export would apply
/// and reports it, but reads nothing beyond the scene, writes no artifact, and
/// creates no output directory. An empty selection is a legitimate answer here,
/// so unlike the export it succeeds instead of failing `cli.frame-no-match`.
fn run_frames_list(frames: &FramesCli) -> u8 {
    let cli = frames.report_cli();
    let mut identity = None;
    match prepare_frame_list(frames, &mut identity) {
        Ok(entries) => deliver_frame_list(&cli, identity.as_ref(), &entries),
        Err(failure) => emit_failure(&cli, identity.as_ref(), None, failure),
    }
}

fn prepare_frame_list(
    frames: &FramesCli,
    identity: &mut Option<SelectedIdentity>,
) -> Result<Vec<FrameListEntry>, Failure> {
    if frames.report_format == ReportFormat::None {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "--list does not accept --report-format none",
            None,
            EXIT_USAGE,
        ));
    }
    let FramePreamble { regex, .. } = prepare_frame_preamble(frames, identity)?;

    let report_path = frames.report_output.as_deref().unwrap_or("stderr");
    if report_path != "stderr" {
        // List mode writes no artifact, so the only aliasing risk is the report
        // overwriting the scene it is reporting on.
        validate_paths(&frames.input, "-", report_path, &BTreeMap::new())?;
        if path_entry_exists(Path::new(report_path)) && !frames.force {
            return Err(Failure::cli(
                "cli.destination-exists",
                "report destination already exists",
                Some(CliDetails::Delivery {
                    role: DeliveryRole::Report,
                    operation: CliOperation::Create,
                    path: report_path.to_owned(),
                    artifact_state: DeliveryState::None,
                }),
                EXIT_USAGE,
            ));
        }
    }
    let scene_bytes = read_scene_bounded(&frames.input, MAX_SCENE_BYTES)?;
    let scene = SceneDocument::parse(&scene_bytes)
        .map_err(|error| Failure::library(error.descriptor(), EXIT_OPERATIONAL))?;
    let selected = select_frames(&scene, regex.as_ref());
    if selected.len() > frames.max_frames as usize {
        return Err(frame_limit_exceeded(frames));
    }
    let basenames = frame_output_basenames(&selected, frames.format.map(OutputFormat::from));
    Ok(selected
        .iter()
        .zip(basenames)
        .map(|(frame, basename)| FrameListEntry {
            ordinal: frame.ordinal(),
            kind: frame.kind().as_str(),
            name: frame.name().map(str::to_owned),
            basename,
        })
        .collect())
}

fn deliver_frame_list(
    cli: &Cli,
    identity: Option<&SelectedIdentity>,
    entries: &[FrameListEntry],
) -> u8 {
    let bytes = match cli.report_format {
        ReportFormat::Human => entries
            .iter()
            .map(|entry| {
                format!(
                    "{}\t{}\t{}\t{}\n",
                    entry.ordinal,
                    entry.kind,
                    entry.name.as_deref().unwrap_or(""),
                    entry.basename
                )
            })
            .collect::<String>()
            .into_bytes(),
        ReportFormat::Json => frame_list_json_line(identity, entries),
        // Rejected during preparation; list mode has nothing to report without
        // a report format.
        ReportFormat::None => Vec::new(),
    };
    deliver_report_bytes(cli, &bytes, DeliveryState::None)
}

fn frame_list_json_line(
    identity: Option<&SelectedIdentity>,
    entries: &[FrameListEntry],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"{\"schema_version\":1,\"status\":\"success\",\"renderer_version\":");
    push_json_string(&mut bytes, env!("CARGO_PKG_VERSION"));
    bytes.extend_from_slice(b",\"compatibility_revision\":");
    match identity {
        Some(identity) => {
            bytes.extend_from_slice(identity.revision.get().to_string().as_bytes());
            bytes.extend_from_slice(b",\"excalidraw_release\":");
            push_json_string(&mut bytes, identity.release);
            bytes.extend_from_slice(b",\"reference_profile_digest\":");
            push_json_string(&mut bytes, identity.digest);
        }
        None => {
            bytes.extend_from_slice(
                b"null,\"excalidraw_release\":null,\"reference_profile_digest\":null",
            );
        }
    }
    bytes.extend_from_slice(b",\"mode\":\"list\",\"frames\":[");
    for (index, entry) in entries.iter().enumerate() {
        if index != 0 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(b"{\"ordinal\":");
        bytes.extend_from_slice(entry.ordinal.to_string().as_bytes());
        bytes.extend_from_slice(b",\"kind\":");
        push_json_string(&mut bytes, entry.kind);
        bytes.extend_from_slice(b",\"name\":");
        push_optional_json_string(&mut bytes, entry.name.as_deref());
        bytes.extend_from_slice(b",\"output_basename\":");
        push_json_string(&mut bytes, &entry.basename);
        bytes.push(b'}');
    }
    bytes.extend_from_slice(b"]}\n");
    bytes
}

fn validate_frame_limit(frames: &FramesCli) -> Result<(), Failure> {
    if frames.max_frames == 0 || frames.max_frames > 16_384 {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "--max-frames must be between 1 and 16384",
            Some(CliDetails::CliArgument {
                argument: "--max-frames".to_owned(),
                value: Some(frames.max_frames.to_string()),
            }),
            EXIT_USAGE,
        ));
    }
    Ok(())
}

fn frame_name_regex(frames: &FramesCli) -> Result<Option<Regex>, Failure> {
    frames
        .name_regex
        .as_deref()
        .map(Regex::new)
        .transpose()
        .map_err(|_| {
            Failure::cli(
                "cli.invalid-name-regex",
                "--name-regex is not valid Rust regex syntax",
                Some(CliDetails::CliArgument {
                    argument: "--name-regex".to_owned(),
                    value: frames.name_regex.clone(),
                }),
                EXIT_USAGE,
            )
        })
}

/// Everything `--list` and an export must both settle before either touches the
/// filesystem, in the order the CLI failure ladder requires: report options,
/// selection bounds, revision identity, then request values and asset
/// declarations. `--list` is exempt from *reading* supplemental assets and from
/// writing, not from rejecting a command an export would reject.
struct FramePreamble {
    regex: Option<Regex>,
    renderer: Renderer,
    asset_paths: BTreeMap<String, String>,
    asset_mimes: BTreeMap<String, String>,
}

fn prepare_frame_preamble(
    frames: &FramesCli,
    identity: &mut Option<SelectedIdentity>,
) -> Result<FramePreamble, Failure> {
    validate_report_options(&frames.report_cli())?;
    validate_frame_limit(frames)?;
    let regex = frame_name_regex(frames)?;
    let revision = frame_revision(frames)?;
    let renderer = Renderer::for_revision(revision)
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))?;
    let profile = renderer.selected_frame_profile_identity();
    *identity = Some(SelectedIdentity {
        revision,
        release: profile.excalidraw_release(),
        digest: profile.profile_digest(),
    });

    let (asset_paths, asset_mimes) = parse_asset_declarations(&frames.asset, &frames.asset_mime)?;
    let mut validation_assets = SupplementalAssets::builder();
    for asset_id in asset_paths.keys() {
        validation_assets = if let Some(media_type) = asset_mimes.get(asset_id) {
            validation_assets.asset_with_media_type(asset_id, Vec::new(), media_type)
        } else {
            validation_assets.asset(asset_id, Vec::new())
        };
    }
    let validation_assets = validation_assets
        .build()
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))?;
    // Without `--format` there is no artifact to shape, but the format-agnostic
    // request values still have to be validated; SVG stands in as the probe.
    build_frame_request(
        frames,
        frames.format.map_or(OutputFormat::Svg, OutputFormat::from),
        validation_assets,
    )?;
    Ok(FramePreamble {
        regex,
        renderer,
        asset_paths,
        asset_mimes,
    })
}

fn frame_revision(frames: &FramesCli) -> Result<CompatibilityRevision, Failure> {
    match &frames.compatibility_revision {
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
        }),
        None => Ok(DEFAULT_COMPATIBILITY_REVISION),
    }
}

fn select_frames<'scene>(
    scene: &'scene SceneDocument,
    regex: Option<&Regex>,
) -> Vec<excalidraw_render::FrameRef<'scene>> {
    scene
        .frames()
        .filter(|frame| {
            regex.is_none_or(|regex| {
                frame
                    .name()
                    .is_some_and(|name| !name.is_empty() && regex.is_match(name))
            })
        })
        .collect()
}

fn frame_limit_exceeded(frames: &FramesCli) -> Failure {
    Failure::cli(
        "cli.frame-limit-exceeded",
        "selected frame count exceeds --max-frames",
        Some(CliDetails::CliArgument {
            argument: "--max-frames".to_owned(),
            value: Some(frames.max_frames.to_string()),
        }),
        EXIT_OPERATIONAL,
    )
}

fn prepare_and_render_frames(
    frames: &FramesCli,
    identity: &mut Option<SelectedIdentity>,
) -> Result<PreparedFrameBatch, Failure> {
    if frames.output() == "-" {
        return Err(Failure::cli(
            "cli.invalid-arguments",
            "frame artifact output must be a directory and cannot be -",
            Some(CliDetails::CliArgument {
                argument: "--output".to_owned(),
                value: Some("-".to_owned()),
            }),
            EXIT_USAGE,
        ));
    }
    let FramePreamble {
        regex,
        renderer,
        asset_paths,
        asset_mimes,
    } = prepare_frame_preamble(frames, identity)?;

    let report_path = frames.report_output.as_deref().unwrap_or("stderr");
    validate_paths(&frames.input, frames.output(), report_path, &asset_paths)?;
    let output_directory = Path::new(frames.output());
    if path_entry_exists(output_directory) && !output_directory.is_dir() {
        return Err(Failure::cli(
            "cli.invalid-output-directory",
            "frame output path exists and is not a directory",
            Some(CliDetails::CliArgument {
                argument: "--output".to_owned(),
                value: Some(frames.output().to_owned()),
            }),
            EXIT_USAGE,
        ));
    }
    if report_path != "stderr" && path_entry_exists(Path::new(report_path)) && !frames.force {
        return Err(Failure::cli(
            "cli.destination-exists",
            "report destination already exists",
            Some(CliDetails::Delivery {
                role: DeliveryRole::Report,
                operation: CliOperation::Create,
                path: report_path.to_owned(),
                artifact_state: DeliveryState::None,
            }),
            EXIT_USAGE,
        ));
    }
    let scene_bytes = read_scene_bounded(&frames.input, MAX_SCENE_BYTES)?;
    let scene = SceneDocument::parse(&scene_bytes)
        .map_err(|error| Failure::library(error.descriptor(), EXIT_OPERATIONAL))?;
    let selected = select_frames(&scene, regex.as_ref());
    if selected.is_empty() {
        return Err(Failure::cli(
            "cli.frame-no-match",
            "scene contains no frames matching the requested selection",
            None,
            EXIT_OPERATIONAL,
        ));
    }
    if selected.len() > frames.max_frames as usize {
        return Err(frame_limit_exceeded(frames));
    }
    let basenames = frame_output_basenames(&selected, Some(OutputFormat::from(frames.format())));
    let destinations = basenames
        .iter()
        .map(|basename| output_directory.join(basename))
        .collect::<Vec<_>>();
    validate_frame_paths(
        &frames.input,
        &destinations,
        frames.report_output.as_deref().unwrap_or("stderr"),
        &asset_paths,
    )?;
    let destination_collisions = destinations
        .iter()
        .map(|destination| path_entry_exists(destination) && !frames.force)
        .collect::<Vec<_>>();

    let limits = ResourceLimits::default();
    let mut assets_builder = SupplementalAssets::builder();
    let mut total_asset_bytes = 0_u64;
    for (asset_id, path) in &asset_paths {
        let aggregate_remaining = limits
            .max_total_asset_bytes()
            .saturating_sub(total_asset_bytes);
        let bytes = match read_bounded_asset(
            path,
            asset_id,
            limits.max_asset_bytes(),
            limits.max_total_asset_bytes(),
            aggregate_remaining,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                if let Some(destination) = destinations
                    .iter()
                    .zip(&destination_collisions)
                    .find_map(|(destination, collision)| collision.then_some(destination))
                {
                    return Err(destination_exists(destination));
                }
                return Err(error);
            }
        };
        total_asset_bytes =
            total_asset_bytes.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        assets_builder = if let Some(media_type) = asset_mimes.get(asset_id) {
            assets_builder.asset_with_media_type(asset_id, bytes, media_type)
        } else {
            assets_builder.asset(asset_id, bytes)
        };
    }
    let assets = assets_builder
        .build()
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))?;
    let request = build_frame_request(frames, OutputFormat::from(frames.format()), assets)?;
    let mut cursor = match renderer.render_frames(&scene, selected.iter().copied(), &request) {
        Ok(cursor) => cursor,
        Err(error) => {
            if let Some(destination) = destinations
                .iter()
                .zip(&destination_collisions)
                .find_map(|(destination, collision)| collision.then_some(destination))
            {
                return Err(destination_exists(destination));
            }
            return Err(
                if error.kind() == FrameBatchErrorKind::SharedPreparationFailed {
                    Failure::library(error.descriptor(), EXIT_OPERATIONAL)
                } else {
                    Failure::cli(
                        "cli.invalid-frame-selection",
                        error.to_string(),
                        None,
                        EXIT_USAGE,
                    )
                },
            );
        }
    };

    let staging_parent = if output_directory.is_dir() {
        output_directory
    } else {
        output_directory
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    };
    let mut entries = Vec::new();
    let mut had_failure = false;
    for (((frame, destination), initial_collision), rendered) in selected
        .iter()
        .zip(destinations.iter())
        .zip(destination_collisions)
        .zip(cursor.by_ref())
    {
        debug_assert_eq!(frame.ordinal(), rendered.frame().ordinal());
        let diagnostics = rendered.target_diagnostics().to_vec();
        let mut entry = FrameCliEntry {
            ordinal: frame.ordinal(),
            kind: frame.kind().as_str(),
            name: frame.name().map(str::to_owned),
            output_path: destination.to_string_lossy().into_owned(),
            artifact: None,
            error: None,
            diagnostics,
            delivery_state: DeliveryState::None,
        };
        match rendered.into_result() {
            Ok(outcome) => {
                let bytes = outcome.artifact().bytes();
                entry.artifact = Some(ArtifactDescriptor {
                    media_type: outcome.artifact().media_type(),
                    byte_length: bytes.len(),
                    sha256: sha256(bytes),
                });
                if initial_collision || path_entry_exists(destination) && !frames.force {
                    had_failure = true;
                    entry.error = Some(frame_destination_error(destination));
                    entries.push((entry, None));
                    continue;
                }
                match StagedFile::new_at(staging_parent, destination, bytes, frames.force) {
                    Ok(staged) => entries.push((entry, Some(staged))),
                    Err(operation) => {
                        had_failure = true;
                        entry.error = Some(frame_cli_error(
                            "cli.output-write-failed",
                            "failed to stage frame artifact",
                            CliDetails::Delivery {
                                role: DeliveryRole::Output,
                                operation,
                                path: destination.to_string_lossy().into_owned(),
                                artifact_state: DeliveryState::None,
                            },
                        ));
                        entries.push((entry, None));
                    }
                }
            }
            Err(error) => {
                had_failure = true;
                entry.error = Some(if initial_collision {
                    frame_destination_error(destination)
                } else {
                    ReportError::Library(error.descriptor())
                });
                entries.push((entry, None));
            }
        }
    }
    let global_diagnostics = cursor.global_diagnostics().to_vec();
    Ok(PreparedFrameBatch {
        entries,
        global_diagnostics,
        had_failure,
    })
}

fn destination_exists(destination: &Path) -> Failure {
    Failure::cli(
        "cli.destination-exists",
        "output destination already exists",
        Some(CliDetails::Delivery {
            role: DeliveryRole::Output,
            operation: CliOperation::Create,
            path: destination.to_string_lossy().into_owned(),
            artifact_state: DeliveryState::None,
        }),
        EXIT_USAGE,
    )
}

fn frame_destination_error(destination: &Path) -> ReportError {
    frame_cli_error(
        "cli.destination-exists",
        "output destination already exists",
        CliDetails::Delivery {
            role: DeliveryRole::Output,
            operation: CliOperation::Create,
            path: destination.to_string_lossy().into_owned(),
            artifact_state: DeliveryState::None,
        },
    )
}

fn build_frame_request(
    frames: &FramesCli,
    format: OutputFormat,
    assets: SupplementalAssets,
) -> Result<FrameRenderRequest, Failure> {
    let sizing = frames
        .max_dimension
        .map(Sizing::MaxDimension)
        .unwrap_or_else(|| Sizing::Scale(frames.scale.unwrap_or(1.0)));
    let mut builder = FrameRenderRequest::builder(format)
        .include_background(!frames.no_background)
        .sizing(sizing)
        .color_mode(if frames.dark_mode {
            ColorMode::Dark
        } else {
            ColorMode::Light
        })
        .metadata_options(MetadataOptions::new(
            frames.embed_scene,
            frames.embed_provenance,
            frames.embed_diagnostics,
        ))
        .supplemental_assets(assets)
        .resource_limits(ResourceLimits::default());
    if let Some(fallback) = frames.pdf_raster_fallback {
        builder = builder.pdf_options(PdfOptions::new(match fallback {
            PdfFallbackArg::AllowIsolated => PdfRasterFallback::AllowIsolated,
            PdfFallbackArg::Forbid => PdfRasterFallback::Forbid,
        }));
    }
    builder
        .build()
        .map_err(|error| Failure::library(error.descriptor(), EXIT_USAGE))
}

fn validate_frame_paths(
    input: &str,
    outputs: &[PathBuf],
    report: &str,
    assets: &BTreeMap<String, String>,
) -> Result<(), Failure> {
    for output in outputs {
        let output = output.to_string_lossy();
        if input != "-" && same_path(input, &output) {
            return Err(path_conflict([
                ("input", None, input),
                ("output", None, &output),
            ]));
        }
        if report != "stderr" && same_path(report, &output) {
            return Err(path_conflict([
                ("report", None, report),
                ("output", None, &output),
            ]));
        }
        for (id, asset) in assets {
            if same_path(asset, &output) {
                return Err(path_conflict([
                    ("asset", Some(id.as_str()), asset.as_str()),
                    ("output", None, &output),
                ]));
            }
        }
    }
    Ok(())
}

/// Derives the portable, collision-free basename for each selected frame.
/// `format` is `None` only in `--list` mode without an explicit `--format`,
/// where the stem is reported without an extension.
fn frame_output_basenames(
    frames: &[excalidraw_render::FrameRef<'_>],
    format: Option<OutputFormat>,
) -> Vec<String> {
    let mut used = BTreeSet::new();
    let mut basenames = Vec::with_capacity(frames.len());
    for frame in frames {
        let normalized_name = frame.name().map(|name| name.nfc().collect::<String>());
        let mut root = normalized_name
            .as_deref()
            .map(portable_frame_slug)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("frame-{}", frame.ordinal()));
        if is_reserved_device_name(&root) {
            root.insert(0, '_');
        }
        let stable_suffix = if root.len() > 120 {
            let digest = sha256(normalized_name.as_deref().unwrap_or(&root).as_bytes());
            format!("-{}", &digest["sha256:".len().."sha256:".len() + 8])
        } else {
            String::new()
        };
        let mut suffix = 1_u32;
        loop {
            let collision_suffix = (suffix > 1).then(|| format!("-{suffix}"));
            let reserved_bytes =
                stable_suffix.len() + collision_suffix.as_deref().map_or(0, str::len);
            let mut basename = truncate_basename(&root, 120 - reserved_bytes);
            basename.push_str(&stable_suffix);
            if let Some(collision_suffix) = collision_suffix {
                basename.push_str(&collision_suffix);
            }
            let key = unicase::UniCase::new(basename.nfc().collect::<String>())
                .to_folded_case()
                .nfc()
                .collect::<String>();
            if used.insert(key) {
                basenames.push(match format {
                    Some(format) => format!("{basename}.{}", format.as_str()),
                    None => basename,
                });
                break;
            }
            suffix += 1;
        }
    }
    basenames
}

fn portable_frame_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in name.chars() {
        if character.is_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    slug
}

fn truncate_basename(value: &str, max_bytes: usize) -> String {
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_owned()
}

fn is_reserved_device_name(value: &str) -> bool {
    let value = value.to_ascii_uppercase();
    matches!(value.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || value
            .strip_prefix("COM")
            .or_else(|| value.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn frame_cli_error(
    code: &'static str,
    message: impl Into<String>,
    details: CliDetails,
) -> ReportError {
    ReportError::Cli(CliError {
        family: "cli",
        category: None,
        code,
        message: message.into(),
        format: None,
        element_id: None,
        json_pointer: None,
        details: Some(details),
    })
}

fn deliver_frame_batch(
    cli: &Cli,
    identity: &SelectedIdentity,
    format: OutputFormat,
    mut entries: Vec<(FrameCliEntry, Option<StagedFile>)>,
    global_diagnostics: Vec<CompatibilityDiagnostic>,
    mut had_failure: bool,
    allow_partial: bool,
) -> u8 {
    let publish_artifacts = !had_failure || allow_partial;
    if !publish_artifacts {
        for (_, staged) in &mut entries {
            staged.take();
        }
    } else {
        for (entry, staged) in &mut entries {
            if staged.is_some() {
                debug_assert!(matches!(entry.delivery_state, DeliveryState::None));
                entry.delivery_state = DeliveryState::Complete;
            }
        }
    }

    let (status, _) = frame_batch_delivery(&entries, had_failure);
    let mut report_bytes = frame_batch_report_bytes(
        cli.report_format,
        identity,
        format,
        status,
        &global_diagnostics,
        &entries,
    );
    let mut staged_report = match stage_report_file(cli, &report_bytes) {
        Ok(staged) => staged,
        Err(operation) => {
            return emergency_report_failure(
                cli,
                identity,
                format,
                operation,
                DeliveryState::None,
                frame_batch_diagnostics(&global_diagnostics, &entries),
            )
        }
    };

    // The first report is staged with projected success states, but publication
    // below must record only mutations that actually completed.
    for (entry, staged) in &mut entries {
        if staged.is_some() {
            entry.delivery_state = DeliveryState::None;
        }
    }

    if publish_artifacts {
        let output = Path::new(cli.output.as_deref().expect("frame output validated"));
        if !output.is_dir() && fs::create_dir(output).is_err() {
            for (entry, staged) in &mut entries {
                if staged.take().is_some() {
                    entry.delivery_state = DeliveryState::None;
                    entry.error = Some(frame_cli_error(
                        "cli.output-write-failed",
                        "failed to create frame output directory",
                        CliDetails::Delivery {
                            role: DeliveryRole::Output,
                            operation: CliOperation::Create,
                            path: entry.output_path.clone(),
                            artifact_state: DeliveryState::None,
                        },
                    ));
                }
            }
            had_failure = true;
        } else {
            let mut commit_failed = false;
            for (entry, staged) in &mut entries {
                let Some(staged) = staged.take() else {
                    continue;
                };
                if commit_failed {
                    had_failure = true;
                    entry.error = Some(frame_cli_error(
                        "cli.output-write-failed",
                        "frame artifact was not delivered after an earlier commit failure",
                        CliDetails::Delivery {
                            role: DeliveryRole::Output,
                            operation: if cli.force {
                                CliOperation::Replace
                            } else {
                                CliOperation::Create
                            },
                            path: entry.output_path.clone(),
                            artifact_state: DeliveryState::None,
                        },
                    ));
                    continue;
                }
                match staged.deliver() {
                    Ok(()) => entry.delivery_state = DeliveryState::Complete,
                    Err(operation) => {
                        had_failure = true;
                        entry.delivery_state = DeliveryState::None;
                        entry.error = Some(frame_cli_error(
                            "cli.output-write-failed",
                            "failed to deliver frame artifact",
                            CliDetails::Delivery {
                                role: DeliveryRole::Output,
                                operation,
                                path: entry.output_path.clone(),
                                artifact_state: DeliveryState::None,
                            },
                        ));
                        if !allow_partial {
                            commit_failed = true;
                        }
                    }
                }
            }
        }
        for (entry, staged) in &mut entries {
            if staged.take().is_some() {
                entry.delivery_state = DeliveryState::None;
            }
        }
    }

    let (status, artifact_state) = frame_batch_delivery(&entries, had_failure);
    let final_report_bytes = frame_batch_report_bytes(
        cli.report_format,
        identity,
        format,
        status,
        &global_diagnostics,
        &entries,
    );
    if final_report_bytes != report_bytes {
        staged_report.take();
        report_bytes = final_report_bytes;
        staged_report = match stage_report_file(cli, &report_bytes) {
            Ok(staged) => staged,
            Err(operation) => {
                return emergency_report_failure(
                    cli,
                    identity,
                    format,
                    operation,
                    artifact_state,
                    frame_batch_diagnostics(&global_diagnostics, &entries),
                )
            }
        };
    }
    let report_exit = deliver_frame_report(
        cli,
        identity,
        format,
        &report_bytes,
        staged_report,
        artifact_state,
        frame_batch_diagnostics(&global_diagnostics, &entries),
    );
    if report_exit != 0 {
        return report_exit;
    }
    if had_failure {
        EXIT_OPERATIONAL
    } else {
        0
    }
}

fn frame_batch_diagnostics(
    global: &[CompatibilityDiagnostic],
    entries: &[(FrameCliEntry, Option<StagedFile>)],
) -> Vec<CompatibilityDiagnostic> {
    global
        .iter()
        .chain(
            entries
                .iter()
                .flat_map(|(entry, _)| entry.diagnostics.iter()),
        )
        .cloned()
        .collect()
}

fn frame_batch_delivery(
    entries: &[(FrameCliEntry, Option<StagedFile>)],
    had_failure: bool,
) -> (&'static str, DeliveryState) {
    let complete = entries
        .iter()
        .filter(|(entry, _)| matches!(entry.delivery_state, DeliveryState::Complete))
        .count();
    let possibly_partial = entries
        .iter()
        .any(|(entry, _)| matches!(entry.delivery_state, DeliveryState::PossiblyPartial));
    let status = if had_failure {
        if complete == 0 && !possibly_partial {
            "failure"
        } else {
            "partial"
        }
    } else {
        "success"
    };
    let artifact_state = if complete == entries.len() && !possibly_partial {
        DeliveryState::Complete
    } else if complete == 0 && !possibly_partial {
        DeliveryState::None
    } else {
        DeliveryState::PossiblyPartial
    };
    (status, artifact_state)
}

fn frame_batch_report_bytes(
    mode: ReportFormat,
    identity: &SelectedIdentity,
    format: OutputFormat,
    status: &str,
    global_diagnostics: &[CompatibilityDiagnostic],
    entries: &[(FrameCliEntry, Option<StagedFile>)],
) -> Vec<u8> {
    match mode {
        ReportFormat::None if status == "success" => Vec::new(),
        ReportFormat::None => frame_batch_error_line(entries),
        ReportFormat::Human => {
            let mut text = String::new();
            for diagnostic in global_diagnostics.iter().chain(
                entries
                    .iter()
                    .flat_map(|(entry, _)| entry.diagnostics.iter()),
            ) {
                text.push_str(&format!(
                    "{}: {}\n",
                    diagnostic.code(),
                    diagnostic.message()
                ));
            }
            for (entry, _) in entries {
                if let Some(error) = &entry.error {
                    let (family, code, message) = error.human_parts();
                    text.push_str(&format!(
                        "{family} {code}: frame {} ({}): {message}\n",
                        entry.ordinal, entry.output_path
                    ));
                }
            }
            if status != "success" && entries.iter().all(|(entry, _)| entry.error.is_none()) {
                text.push_str("cli cli.output-write-failed: frame batch delivery failed\n");
            }
            text.into_bytes()
        }
        ReportFormat::Json => {
            let artifact_state = frame_batch_delivery(entries, status != "success").1;
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"{\"schema_version\":1,\"status\":");
            push_json_string(&mut bytes, status);
            bytes.extend_from_slice(b",\"renderer_version\":");
            push_json_string(&mut bytes, env!("CARGO_PKG_VERSION"));
            bytes.extend_from_slice(b",\"compatibility_revision\":");
            bytes.extend_from_slice(identity.revision.get().to_string().as_bytes());
            bytes.extend_from_slice(b",\"excalidraw_release\":");
            push_json_string(&mut bytes, identity.release);
            bytes.extend_from_slice(b",\"reference_profile_digest\":");
            push_json_string(&mut bytes, identity.digest);
            bytes.extend_from_slice(b",\"format\":");
            push_json_string(&mut bytes, format.as_str());
            bytes.extend_from_slice(b",\"delivery_state\":");
            push_json_string(&mut bytes, artifact_state.as_str());
            bytes.extend_from_slice(b",\"diagnostics\":[");
            push_diagnostics(&mut bytes, global_diagnostics);
            bytes.extend_from_slice(b"],\"frames\":[");
            for (index, (entry, _)) in entries.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                bytes.extend_from_slice(b"{\"ordinal\":");
                bytes.extend_from_slice(entry.ordinal.to_string().as_bytes());
                bytes.extend_from_slice(b",\"kind\":");
                push_json_string(&mut bytes, entry.kind);
                bytes.extend_from_slice(b",\"name\":");
                push_optional_json_string(&mut bytes, entry.name.as_deref());
                bytes.extend_from_slice(b",\"output_path\":");
                push_json_string(&mut bytes, &entry.output_path);
                bytes.extend_from_slice(b",\"artifact\":");
                if let Some(artifact) = &entry.artifact {
                    push_artifact_descriptor(&mut bytes, artifact);
                } else {
                    bytes.extend_from_slice(b"null");
                }
                bytes.extend_from_slice(b",\"error\":");
                if let Some(error) = &entry.error {
                    push_report_error(&mut bytes, error);
                } else {
                    bytes.extend_from_slice(b"null");
                }
                bytes.extend_from_slice(b",\"diagnostics\":[");
                push_diagnostics(&mut bytes, &entry.diagnostics);
                bytes.extend_from_slice(b"],\"delivery_state\":");
                push_json_string(&mut bytes, entry.delivery_state.as_str());
                bytes.push(b'}');
            }
            bytes.extend_from_slice(b"]}\n");
            bytes
        }
    }
}

fn frame_batch_error_line(entries: &[(FrameCliEntry, Option<StagedFile>)]) -> Vec<u8> {
    entries
        .iter()
        .find_map(|(entry, _)| entry.error.as_ref())
        .map_or_else(
            || b"cli cli.output-write-failed: frame batch delivery failed\n".to_vec(),
            |error| {
                let (family, code, message) = error.human_parts();
                format!("{family} {code}: {message}\n").into_bytes()
            },
        )
}

fn deliver_frame_report(
    cli: &Cli,
    identity: &SelectedIdentity,
    format: OutputFormat,
    bytes: &[u8],
    staged_report: Option<StagedFile>,
    artifact_state: DeliveryState,
    diagnostics: Vec<CompatibilityDiagnostic>,
) -> u8 {
    let report = report_output(cli);
    if let Some(staged) = staged_report {
        return match staged.deliver() {
            Ok(()) => 0,
            Err(operation) => emergency_report_failure(
                cli,
                identity,
                format,
                operation,
                artifact_state,
                diagnostics,
            ),
        };
    }
    debug_assert_eq!(report, "stderr");
    if report == "stderr" {
        return if bytes.is_empty() || write_stream(&mut io::stderr().lock(), bytes).is_ok() {
            0
        } else {
            EXIT_OPERATIONAL
        };
    }
    EXIT_OPERATIONAL
}

#[cfg(test)]
mod tests {
    use super::{
        frame_output_basenames, is_reserved_device_name, portable_frame_slug, truncate_basename,
    };
    use excalidraw_render::{OutputFormat, SceneDocument};

    /// A scene whose live frames carry `names` in order, a `null` name where
    /// the entry is `None`, parsed so its frame references can be listed.
    fn scene_with_frame_names(names: &[Option<&str>]) -> SceneDocument {
        let elements = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let name = match name {
                    Some(name) => serde_json::to_string(name).expect("name serializes"),
                    None => "null".to_owned(),
                };
                format!(
                    r#"{{"id":"frame-{index}","type":"frame","name":{name},"x":{x},"y":0,"width":20,"height":20,"updated":1}}"#,
                    x = index * 30
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        SceneDocument::parse(
            format!(
                r#"{{"type":"excalidraw","version":2,"elements":[{elements}],"appState":{{}},"files":{{}}}}"#
            )
            .as_bytes(),
        )
        .expect("frame scene is admitted")
    }

    /// The SVG basenames of every frame of a scene carrying `names`.
    fn svg_basenames(names: &[Option<&str>]) -> Vec<String> {
        let scene = scene_with_frame_names(names);
        let frames = scene.frames().collect::<Vec<_>>();
        frame_output_basenames(&frames, Some(OutputFormat::Svg))
    }

    #[test]
    fn frame_basenames_fold_case_and_normalize_unicode_before_deduplicating() {
        // "Straße" and "STRASSE" are one name under full case folding, so the
        // later frame takes a collision suffix while keeping its own spelling;
        // a decomposed "é" is the same name as the composed one and is written
        // composed.
        assert_eq!(
            svg_basenames(&[Some("Straße"), Some("STRASSE"), Some("strasse")]),
            ["Straße.svg", "STRASSE-2.svg", "strasse-3.svg"]
        );
        assert_eq!(
            svg_basenames(&[Some("caf\u{e9}"), Some("cafe\u{301}")]),
            ["caf\u{e9}.svg", "caf\u{e9}-2.svg"]
        );
    }

    #[test]
    fn long_frame_names_are_truncated_to_120_bytes_with_a_stable_hash_suffix() {
        // Two frames with the same 140-character name: both are cut to the
        // component budget, both carry the first eight hex digits of the
        // name's SHA-256, and the second still gets its collision suffix
        // inside the budget. `88e685e0` is `sha256("A" * 140)[..8]`.
        let long_name = "A".repeat(140);
        let basenames = svg_basenames(&[Some(&long_name), Some(&long_name)]);
        assert_eq!(
            basenames,
            [
                format!("{}-88e685e0.svg", "A".repeat(111)),
                format!("{}-88e685e0-2.svg", "A".repeat(109)),
            ]
        );
        for basename in &basenames {
            let stem = basename
                .strip_suffix(".svg")
                .expect("the basename carries the SVG extension");
            assert!(stem.len() <= 120, "{basename} exceeds the component budget");
        }

        // The hash is over the NFC name before slugging, so two long names
        // that slug alike but differ in spelling stay distinct files.
        let spaced = format!("{} B", "A".repeat(139));
        let dashed = format!("{}-B", "A".repeat(139));
        let distinct = svg_basenames(&[Some(&spaced), Some(&dashed)]);
        assert_ne!(distinct[0], distinct[1]);
        assert!(!distinct[1].ends_with("-2.svg"), "{}", distinct[1]);
    }

    #[test]
    fn reserved_device_names_are_prefixed_and_unnamed_frames_use_their_ordinal() {
        assert_eq!(
            svg_basenames(&[
                Some("CON"),
                Some("com1"),
                Some("Lpt9"),
                Some("COM10"),
                Some("Console")
            ]),
            [
                "_CON.svg",
                "_com1.svg",
                "_Lpt9.svg",
                "COM10.svg",
                "Console.svg"
            ]
        );
        // No name, a non-string name and a name with no alphanumerics all
        // fall back to the one-based frame ordinal.
        assert_eq!(
            svg_basenames(&[None, Some("!!!"), Some("Named"), Some(" ")]),
            ["frame-1.svg", "frame-2.svg", "Named.svg", "frame-4.svg"]
        );
    }

    #[test]
    fn list_mode_without_a_format_reports_bare_stems() {
        let scene = scene_with_frame_names(&[Some("Overview"), Some("Detail")]);
        let frames = scene.frames().collect::<Vec<_>>();
        assert_eq!(
            frame_output_basenames(&frames, None),
            ["Overview", "Detail"]
        );
        assert_eq!(
            frame_output_basenames(&frames, Some(OutputFormat::Png)),
            ["Overview.png", "Detail.png"]
        );
    }

    #[test]
    fn a_portable_slug_keeps_alphanumerics_and_joins_runs_with_one_dash() {
        assert_eq!(portable_frame_slug("Hello, World!"), "Hello-World");
        assert_eq!(
            portable_frame_slug("  leading and trailing  "),
            "leading-and-trailing"
        );
        assert_eq!(
            portable_frame_slug("a/b\\c:d*e?f\"g<h>i|j"),
            "a-b-c-d-e-f-g-h-i-j"
        );
        assert_eq!(portable_frame_slug("日本語 タイトル"), "日本語-タイトル");
        assert_eq!(portable_frame_slug("..."), "");
        assert_eq!(portable_frame_slug(""), "");
    }

    #[test]
    fn truncation_never_splits_a_character() {
        assert_eq!(truncate_basename("abcdef", 3), "abc");
        assert_eq!(truncate_basename("abc", 10), "abc");
        // Each "é" is two bytes: a three-byte budget keeps one whole character.
        assert_eq!(truncate_basename("ééé", 3), "é");
        assert_eq!(truncate_basename("ééé", 4), "éé");
        assert_eq!(truncate_basename("日本", 3), "日");
        assert_eq!(truncate_basename("abc", 0), "");
    }

    #[test]
    fn reserved_device_names_match_the_windows_set_case_insensitively() {
        for name in [
            "CON", "con", "PRN", "AUX", "NUL", "COM1", "com9", "LPT1", "lpt9",
        ] {
            assert!(is_reserved_device_name(name), "{name}");
        }
        for name in [
            "",
            "CON1",
            "COM0",
            "COM10",
            "LPT",
            "CONSOLE",
            "NULL",
            "AUXILIARY",
        ] {
            assert!(!is_reserved_device_name(name), "{name}");
        }
    }
}
