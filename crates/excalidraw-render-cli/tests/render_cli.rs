use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use excalidraw_render::Renderer;
use tempfile::TempDir;

#[test]
fn help_is_successful_stdout_for_scene_and_frame_commands() {
    for args in [
        vec!["--help"],
        vec!["-h"],
        vec!["frames", "--help"],
        vec!["frames", "-h"],
        vec!["--report-format", "json", "--help"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .args(&args)
            .output()
            .expect("print help");
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
        assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
    }
}

#[test]
fn help_describes_the_commands_with_examples() {
    let scene = cli(&["--help"]);
    assert!(scene.status.success());
    let scene = String::from_utf8(scene.stdout).expect("help is UTF-8");
    assert!(scene.contains("Usage: excalidraw-render [OPTIONS] [INPUT]"));
    assert!(scene.contains("frames"), "{scene}");
    assert!(scene.contains("Examples:"), "{scene}");
    assert!(scene.contains("excalidraw-render drawing.excalidraw -o drawing.png --scale 2"));

    for args in [vec!["help", "frames"], vec!["frames", "--help"]] {
        let frames = cli(&args);
        assert!(frames.status.success(), "{args:?}: {frames:?}");
        let frames = String::from_utf8(frames.stdout).expect("help is UTF-8");
        assert!(
            frames.contains("Usage: excalidraw-render frames [OPTIONS]"),
            "{args:?}: {frames}"
        );
        assert!(frames.contains("--name-regex"), "{frames}");
        assert!(frames.contains("Examples:"), "{frames}");
        assert!(
            frames.contains("Every frame, including AI-generated (magic) frames, is exported"),
            "{frames}"
        );
        assert!(
            frames.contains("Each frame is one line of four tab-separated columns"),
            "{frames}"
        );
    }

    // Help speaks the user's language: the single compatibility revision is
    // not a choice to advertise, and the renderer's internal vocabulary stays
    // out of it.
    for args in [
        vec!["--help"],
        vec!["-h"],
        vec!["frames", "--help"],
        vec!["frames", "-h"],
    ] {
        let help = cli(&args);
        let help = String::from_utf8(help.stdout).expect("help is UTF-8");
        for jargon in [
            "--compatibility-revision",
            "--list-revisions",
            "revision",
            "profile",
            "evidence",
            "oracle",
            "admitted",
            "reference",
            "live frame",
            "compatibility diagnostics",
        ] {
            assert!(!help.contains(jargon), "{args:?} mentions {jargon}: {help}");
        }
    }

    let bare_help = cli(&["help"]);
    assert!(bare_help.status.success());
    assert!(String::from_utf8_lossy(&bare_help.stdout).contains("Usage:"));

    // No arguments at all is a usage error that shows the help.
    let nothing = cli(&[]);
    assert_eq!(nothing.status.code(), Some(2));
    assert!(nothing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&nothing.stderr).contains("Usage:"));
}

#[test]
fn unknown_options_and_short_version_use_the_command_grammar() {
    let version = cli(&["-V"]);
    assert!(version.status.success(), "{version:?}");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!(
            "excalidraw-render {} (Excalidraw 0.18.1)\n",
            env!("CARGO_PKG_VERSION")
        )
    );

    for args in [vec!["--bogus"], vec!["scene.excalidraw", "--bogus"]] {
        let bogus = cli(&args);
        assert_eq!(bogus.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&bogus.stderr);
        assert!(
            stderr.starts_with(
                "error: unexpected argument '--bogus' found [cli.invalid-arguments]\n"
            ),
            "{args:?}: {stderr}"
        );
        assert!(stderr.contains("Usage:"), "{stderr}");
    }

    let frames_bogus = cli(&["frames", "scene.excalidraw", "--padding", "3"]);
    assert_eq!(frames_bogus.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&frames_bogus.stderr)
        .starts_with("error: unexpected argument '--padding' found [cli.invalid-arguments]\n"));
}

#[test]
fn a_scene_file_named_like_the_subcommand_stays_addressable() {
    let scratch = temp_directory_named("frames-named-scene");
    let temp = scratch.path();
    fs::copy(corpus_scene("basic-shapes.excalidraw"), temp.join("frames"))
        .expect("copy scene named frames");
    for (args, output) in [
        (vec!["./frames", "-o", "dot.svg"], "dot.svg"),
        (vec!["-o", "dashes.svg", "--", "frames"], "dashes.svg"),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .current_dir(temp)
            .args(&args)
            .output()
            .expect("render the scene named frames");
        assert!(result.status.success(), "{args:?}: {result:?}");
        assert!(temp.join(output).is_file(), "{args:?}");
    }
}

#[test]
fn human_errors_name_the_path_the_reason_and_the_fix() {
    let scratch = temp_directory_named("human-errors");
    let temp = scratch.path();

    let missing = temp.join("missing.excalidraw");
    let reason = fs::File::open(&missing).expect_err("the input is missing");
    let result = cli(&[missing.to_str().unwrap(), "-o", "x.svg"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: cannot open input '{}': {reason} [cli.input-read-failed]\n",
            missing.display()
        )
    );

    let existing = temp.join("existing.svg");
    fs::write(&existing, b"old").expect("create existing output");
    let result = render_command(&existing)
        .output()
        .expect("render over output");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: output '{}' already exists [cli.destination-exists]\nhint: use --force to overwrite it\n",
            existing.display()
        )
    );

    // A missing directory is found before rendering, so no diagnostics of a
    // render that could never be delivered precede the error.
    let nested = temp.join("no-such-directory").join("out.svg");
    let reason = fs::read_dir(temp.join("no-such-directory")).expect_err("missing directory");
    let result = render_command(&nested)
        .output()
        .expect("render into a missing directory");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: cannot create output '{}': {reason} [cli.output-write-failed]\n\
             hint: directory '{}' does not exist; create it first\n",
            nested.display(),
            temp.join("no-such-directory").display()
        )
    );
    assert!(!temp.join("no-such-directory").exists());

    // A directory is never a scene output, even with --force.
    let directory = temp.join("directory");
    fs::create_dir(&directory).expect("create a directory output");
    for force in [false, true] {
        let mut command = render_command(&directory);
        command.args(["--format", "svg"]);
        if force {
            command.arg("--force");
        }
        let result = command.output().expect("render onto a directory");
        assert_eq!(result.status.code(), Some(2), "force={force}");
        assert_eq!(
            String::from_utf8_lossy(&result.stderr),
            format!(
                "error: output '{}' is a directory [cli.invalid-output-directory]\n\
                 hint: pass a file path to -o, or use `excalidraw-render frames INPUT -o \
                 DIRECTORY --format FORMAT` to write one file per frame\n",
                directory.display()
            ),
            "force={force}"
        );
        assert_no_staging_files(&directory);
        assert_no_staging_files(temp);
    }
    // A trailing separator names a directory whether or not it exists yet.
    let named = format!("{}/", temp.join("not-yet").display());
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .args(["-o", &named, "--format", "svg", "--report-format", "json"])
        .output()
        .expect("render onto a directory name");
    assert_eq!(result.status.code(), Some(2));
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("JSON directory-output report");
    assert_eq!(report["error"]["code"], "cli.invalid-output-directory");
    assert_eq!(report["error"]["details"]["argument"], "--output");
    assert_eq!(report["error"]["details"]["value"], named.as_str());
    assert!(!temp.join("not-yet").exists());

    let broken = temp.join("broken.excalidraw");
    fs::write(
        &broken,
        b"{\"type\":\"excalidraw\",\n  \"version\":2,\n  \"elements\":[1,,]}",
    )
    .expect("write broken JSON");
    let result = cli(&[broken.to_str().unwrap(), "-o", "-", "-f", "svg"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: cannot parse '{}': scene document is not valid JSON at line 3, column 17 [scene.invalid-json]\n",
            broken.display()
        )
    );

    // Like Excalidraw, a scene without `appState` or `files` renders with
    // their defaults, silently.
    let no_files = temp.join("no-files.excalidraw");
    fs::write(
        &no_files,
        br#"{"type":"excalidraw","version":2,"elements":[]}"#,
    )
    .expect("write a scene without app state or a files map");
    let result = cli(&[no_files.to_str().unwrap(), "-o", "-", "-f", "svg"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stderr), "");

    let bare = temp.join("bare.excalidraw");
    fs::write(&bare, br#"{"type":"excalidraw"}"#).expect("write a scene missing several members");
    let result = cli(&[bare.to_str().unwrap(), "-o", "-", "-f", "svg"]);
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("[scene.invalid-envelope]\nhint: ")
            && stderr.contains(
                "hint: add the missing members \"version\": 2, \"elements\": [] to the scene"
            ),
        "{stderr}"
    );

    let huge = temp.join("huge.png");
    let result = render_command(&huge)
        .args(["--scale", "20"])
        .output()
        .expect("render an oversized PNG");
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.starts_with("error: ") && stderr.contains("[resource.raster-surface-exceeded]\n"),
        "{stderr}"
    );
    // The scene fits unscaled, so the fix is a lower scale.
    assert!(stderr.contains("hint: use --scale "), "{stderr}");

    // A sizing that rounds the output to nothing names the cause and the fix.
    for (name, sizing, unit) in [
        ("tiny.png", ["--scale", "1e-300"], "pixels"),
        ("tiny-max.png", ["--max-dimension", "0.5"], "pixels"),
        ("tiny.pdf", ["--scale", "1e-300"], "points"),
    ] {
        let result = render_command(&temp.join(name))
            .args(sizing)
            .output()
            .expect("render a zero-size artifact");
        assert_eq!(result.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains(&format!("rounds to zero {unit} at the requested size")),
            "{stderr}"
        );
        assert!(
            stderr.contains("hint: use a larger --scale or --max-dimension\n"),
            "{stderr}"
        );
    }

    // `none` keeps failures to one line, without hints.
    let result = render_command(&existing)
        .args(["--report-format", "none"])
        .output()
        .expect("render over output without a report");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: output '{}' already exists [cli.destination-exists]\n",
            existing.display()
        )
    );
}

/// Hints say what the error line does not: never a restatement of it, and
/// never a fix that cannot help.
#[test]
fn hints_add_what_the_error_does_not_say() {
    let scratch = temp_directory_named("hints");
    let temp = scratch.path();
    let write = |name: &str, contents: &str| {
        let path = temp.join(name);
        fs::write(&path, contents).expect("write the input");
        path.to_str().expect("the scratch path is UTF-8").to_owned()
    };
    let stderr = |args: &[&str], exit: i32| {
        let result = cli(args);
        assert_eq!(result.status.code(), Some(exit), "{args:?}: {result:?}");
        String::from_utf8(result.stderr).expect("the report is UTF-8")
    };

    // A file from a newer Excalidraw is told apart from a malformed one.
    let newer = write(
        "newer.excalidraw",
        r#"{"type":"excalidraw","version":3,"elements":[]}"#,
    );
    assert_eq!(
        stderr(&[&newer, "-o", "-", "-f", "svg"], 1),
        format!(
            "error: cannot parse '{newer}': scene envelope version 3 is not supported; only \
             version 2 is [scene.invalid-envelope]\n\
             hint: the file may come from a newer Excalidraw than this renderer supports; \
             check for a newer excalidraw-render\n"
        )
    );
    // Other unsupported versions, a library and clipboard data are explained
    // by the error alone.
    for (name, contents) in [
        (
            "older.excalidraw",
            r#"{"type":"excalidraw","version":1,"elements":[]}"#,
        ),
        (
            "quoted.excalidraw",
            r#"{"type":"excalidraw","version":"2","elements":[]}"#,
        ),
        (
            "shapes.excalidrawlib",
            r#"{"type":"excalidrawlib","version":2,"libraryItems":[]}"#,
        ),
        (
            "clipboard.json",
            r#"{"type":"excalidraw/clipboard","elements":[]}"#,
        ),
    ] {
        let input = write(name, contents);
        let report = stderr(&[&input, "-o", "-", "-f", "svg"], 1);
        assert_eq!(report.lines().count(), 1, "{report}");
        assert!(report.starts_with("error: cannot parse "), "{report}");
    }

    // Empty input is reported as empty, not as a JSON syntax error.
    for (name, contents) in [("empty.excalidraw", ""), ("blank.excalidraw", " \n\t\n")] {
        let input = write(name, contents);
        assert_eq!(
            stderr(&[&input, "-o", "-", "-f", "svg"], 1),
            format!("error: cannot parse '{input}': it is empty [scene.invalid-json]\n")
        );
        // The JSON report keeps the library's error.
        let json = cli(&[&input, "-o", "-", "-f", "svg", "--report-format", "json"]);
        let report: serde_json::Value =
            serde_json::from_slice(&json.stderr).expect("JSON failure report");
        assert_eq!(
            report["error"]["message"],
            "scene document is not valid JSON"
        );
    }
    let empty_stdin = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["-", "-o", "-", "-f", "svg"])
        .stdin(Stdio::null())
        .output()
        .expect("render empty standard input");
    assert_eq!(empty_stdin.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&empty_stdin.stderr),
        "error: cannot parse standard input: it is empty [scene.invalid-json]\n"
    );

    // A missing INPUT says how to pipe a scene in.
    assert_eq!(
        stderr(&["-o", "out.svg"], 2),
        "error: INPUT is required [cli.invalid-arguments]\n\
         hint: name the scene file, or pass - to read it from standard input\n"
    );

    // A scene without frames points at the whole-scene render, not at a
    // listing that would be empty.
    let unframed = write(
        "unframed.excalidraw",
        r#"{"type":"excalidraw","version":2,"elements":[]}"#,
    );
    let frames = temp.join("frames");
    assert_eq!(
        stderr(
            &[
                "frames",
                &unframed,
                "-o",
                frames.to_str().unwrap(),
                "--format",
                "svg"
            ],
            1
        ),
        "error: the scene has no frames [cli.frame-no-match]\n\
         hint: render the whole scene instead: `excalidraw-render INPUT -o FILE`\n"
    );
    assert!(!frames.exists());
}

/// A PNG over the pixel limit is sized in pixels and megapixels, and the
/// hint names the largest `--max-dimension` (or, for a scene that fits
/// unscaled, `--scale`) that fits; one more does not. The JSON report keeps
/// the library's error.
#[test]
fn an_oversized_png_names_its_size_and_the_largest_that_fits() {
    let scratch = temp_directory_named("oversized-png");
    let temp = scratch.path();
    let rectangle = |name: &str, width: u32, height: u32| {
        let path = temp.join(name);
        fs::write(
            &path,
            format!(
                r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"r","type":"rectangle","x":0,"y":0,"width":{width},"height":{height},"roughness":0,"updated":1}}],"appState":{{}},"files":{{}}}}"#
            ),
        )
        .expect("write the scene");
        path.to_str().expect("the scratch path is UTF-8").to_owned()
    };
    let render = |args: &[&str]| {
        let result = cli(args);
        assert_eq!(result.status.code(), Some(1), "{args:?}: {result:?}");
        String::from_utf8(result.stderr).expect("the report is UTF-8")
    };

    // 7020 × 5020 with the default padding: too large unscaled.
    let large = rectangle("large.excalidraw", 7000, 5000);
    assert_eq!(
        render(&[&large, "-o", "-", "-f", "png"]),
        "error: the PNG would be 7020 × 5020 pixels (35.2 megapixels), over the \
         33.6-megapixel limit [resource.raster-surface-exceeded]\n\
         hint: use --max-dimension 6850, the largest size that fits, or render to SVG or \
         PDF instead\n"
    );
    // A maximum of 6850 is 6850 × 4898 = 33551300 pixels; one more is over.
    assert!(
        render(&[&large, "-o", "-", "-f", "png", "--max-dimension", "6851"]).starts_with(
            "error: the PNG would be 6851 × 4899 pixels (33.56 megapixels), over the \
             33.55-megapixel limit [resource.raster-surface-exceeded]\n"
        )
    );

    // 4020 × 3020 fits unscaled, so a lower scale is the fix for --scale 2.
    let medium = rectangle("medium.excalidraw", 4000, 3000);
    assert_eq!(
        render(&[&medium, "-o", "-", "-f", "png", "--scale", "2"]),
        "error: the PNG would be 8040 × 6040 pixels (48.6 megapixels), over the \
         33.6-megapixel limit [resource.raster-surface-exceeded]\n\
         hint: use --scale 1.66, the largest scale that fits, or render to SVG or PDF \
         instead\n"
    );
    assert!(
        render(&[&medium, "-o", "-", "-f", "png", "--scale", "1.67"])
            .contains("[resource.raster-surface-exceeded]")
    );

    // The report for programs is unchanged.
    let json = cli(&[&large, "-o", "-", "-f", "png", "--report-format", "json"]);
    let report: serde_json::Value =
        serde_json::from_slice(&json.stderr).expect("JSON failure report");
    assert_eq!(
        report["error"]["message"],
        "required PNG raster surface exceeds its configured limit: needs 35240400 pixels, \
         but max_raster_surface_pixels is 33554432"
    );
    assert_eq!(report["error"]["details"]["configured"], 33_554_432);
}

#[test]
fn image_inputs_are_recognized_and_explained() {
    let scratch = temp_directory_named("image-inputs");
    let temp = scratch.path();
    let png = temp.join("embedded.png");
    let svg = temp.join("plain.svg");
    render_command(&png)
        .arg("--embed-scene")
        .output()
        .expect("render a PNG with an embedded scene");
    render_command(&svg).output().expect("render an SVG");

    for (image, kind, code) in [
        (&png, "PNG", "scene.unsupported-input"),
        (&svg, "SVG", "scene.unsupported-input"),
    ] {
        let result = cli(&[image.to_str().unwrap(), "-o", "-", "-f", "svg"]);
        assert_eq!(result.status.code(), Some(1), "{kind}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.starts_with(&format!(
                "error: cannot parse '{}': it is {} {kind} image, not an Excalidraw scene [{code}]\n",
                image.display(),
                if kind == "SVG" { "an" } else { "a" }
            )),
            "{stderr}"
        );
        assert!(
            stderr.contains("hint: PNG and SVG input is not supported"),
            "{stderr}"
        );

        // The machine-readable report keeps the library's error.
        let json = cli(&[
            image.to_str().unwrap(),
            "-o",
            "-",
            "-f",
            "svg",
            "--report-format",
            "json",
        ]);
        let report: serde_json::Value =
            serde_json::from_slice(&json.stderr).expect("JSON failure report");
        assert_eq!(report["error"]["code"], code);
    }
}

#[test]
fn human_diagnostics_are_grouped_and_informational_ones_need_verbose() {
    let scratch = temp_directory_named("grouped-diagnostics");
    let temp = scratch.path();
    let input = temp.join("diagnosed.excalidraw");
    fs::write(
        &input,
        r#"{"type":"excalidraw","version":2,"elements":[
            {"id":"ar","type":"text","x":0,"y":0,"width":100,"height":35,"text":"مرحبا بالعالم","originalText":"مرحبا بالعالم","fontSize":28,"fontFamily":5,"lineHeight":1.25,"updated":1},
            {"id":"helvetica","type":"text","x":0,"y":50,"width":100,"height":35,"text":"Hello","originalText":"Hello","fontSize":28,"fontFamily":2,"lineHeight":1.25,"updated":1},
            {"id":"a","type":"rectangle","x":0,"y":100,"width":10,"height":10,"futureField":1},
            {"id":"b","type":"rectangle","x":20,"y":100,"width":10,"height":10,"futureField":1},
            {"id":"c","type":"rectangle","x":40,"y":100,"width":10,"height":10,"futureField":1},
            {"id":"d","type":"rectangle","x":60,"y":100,"width":10,"height":10,"futureField":1},
            {"id":"e","type":"rectangle","x":80,"y":100,"width":10,"height":10,"futureField":1}
        ],"appState":{},"files":{}}"#,
    )
    .expect("write diagnosed scene");

    let result = cli(&[input.to_str().unwrap(), "-o", "-", "-f", "svg"]);
    assert!(result.status.success(), "{result:?}");
    let stderr = String::from_utf8(result.stderr).expect("report is UTF-8");
    let lines = stderr.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "{stderr}");
    assert!(
        lines.iter().any(|line| line.starts_with(
            "warning: 12 characters have no glyph in the bundled fonts and were drawn as \"?\" "
        ) && line
            .contains("[font.missing-glyph] (element ar; characters U+0645, U+0631, ")
            && line.ends_with(" more)")),
        "{stderr}"
    );
    // The property each warning is about is named, once per group.
    assert!(
        lines.contains(
            &"warning: unrecognized element property was ignored: futureField \
              [compatibility.unsupported-property] (5 occurrences; elements a, b, c and 2 more)"
        ),
        "{stderr}"
    );
    for code in [
        "font.liberation-sans-substituted",
        "normalization.missing-timestamp",
    ] {
        assert!(!stderr.contains(code), "{stderr}");
    }

    let verbose = cli(&[input.to_str().unwrap(), "-o", "-", "-f", "svg", "--verbose"]);
    let verbose = String::from_utf8(verbose.stderr).expect("report is UTF-8");
    assert!(
        verbose.lines().any(|line| line.starts_with("info: ")
            && line.ends_with("[font.liberation-sans-substituted] (element helvetica)")),
        "{verbose}"
    );
    assert!(
        verbose.lines().any(|line| line
            == "info: missing required timestamp was repaired to canonical zero: updated \
                [normalization.missing-timestamp] (5 occurrences; elements a, b, c and 2 more)"),
        "{verbose}"
    );
    // A group of different properties names a few of them, and a property
    // outside the elements by its full path.
    let editor_only = temp.join("editor-only.excalidraw");
    fs::write(
        &editor_only,
        r#"{"type":"excalidraw","version":2,"elements":[
            {"id":"l","type":"line","x":0,"y":0,"width":10,"height":10,"points":[[0,0],[10,10]],"polygon":false,"updated":1}
        ],"appState":{"lockedMultiSelections":{}},"files":{}}"#,
    )
    .expect("write a scene with editor-only properties");
    let editor_only = cli(&[
        editor_only.to_str().unwrap(),
        "-o",
        "-",
        "-f",
        "svg",
        "--verbose",
    ]);
    assert!(editor_only.status.success(), "{editor_only:?}");
    let editor_only = String::from_utf8(editor_only.stderr).expect("report is UTF-8");
    assert!(
        editor_only.lines().any(|line| {
            line
            == "info: editor-only property was ignored: polygon, appState.lockedMultiSelections \
                [compatibility.unsupported-property] (2 occurrences; element l)"
        }),
        "{editor_only}"
    );

    // JSON keeps every occurrence.
    let json = cli(&[
        input.to_str().unwrap(),
        "-o",
        "-",
        "-f",
        "svg",
        "--report-format",
        "json",
    ]);
    let report: serde_json::Value = serde_json::from_slice(&json.stderr).expect("JSON report");
    assert_eq!(report["diagnostics"].as_array().map(Vec::len), Some(23));
}

#[test]
fn max_dimension_on_svg_output_is_reported_as_ignored() {
    let result = cli(&[
        corpus_scene("basic-shapes.excalidraw").to_str().unwrap(),
        "-o",
        "-",
        "-f",
        "svg",
        "--max-dimension",
        "100",
    ]);
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).lines().next(),
        Some("warning: --max-dimension applies only to PNG and PDF output and was ignored; use --scale to resize SVG")
    );

    for args in [
        vec!["-f", "png"],
        vec!["-f", "svg", "--report-format", "json"],
    ] {
        let result = cli(&[
            &[
                corpus_scene("basic-shapes.excalidraw").to_str().unwrap(),
                "-o",
                "-",
                "--max-dimension",
                "100",
            ][..],
            &args,
        ]
        .concat());
        assert!(result.status.success());
        assert!(
            !String::from_utf8_lossy(&result.stderr).contains("--max-dimension"),
            "{args:?}"
        );
    }
}

/// Runs the CLI with `args` and captures its output.
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(args)
        .output()
        .expect("run excalidraw-render")
}

#[test]
fn standalone_commands_expose_revision_identity_and_exact_licenses() {
    let revisions = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["--list-revisions", "--report-format", "json"])
        .output()
        .expect("list revisions");
    assert!(revisions.status.success());
    assert!(revisions.stdout.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&revisions.stderr).expect("revision JSON report");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["renderer_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["revisions"][0]["compatibility_revision"], 1);
    assert_eq!(report["revisions"][0]["default"], true);

    // The plain version names the release; --verbose adds the exact identity.
    let version = cli(&["--version"]);
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).expect("version is UTF-8");
    assert_eq!(
        version,
        format!(
            "excalidraw-render {} (Excalidraw 0.18.1)\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    let profile = Renderer::new().profile_identity();
    for flag in ["--verbose", "-v"] {
        let verbose = cli(&["--version", flag]);
        assert!(verbose.status.success(), "{flag}");
        assert_eq!(
            String::from_utf8(verbose.stdout).expect("version is UTF-8"),
            format!(
                "{version}compatibility revision 1\nreference profile {}\n",
                profile.profile_digest()
            ),
            "{flag}"
        );
    }

    let licenses = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("--licenses")
        .output()
        .expect("print licenses");
    assert!(licenses.status.success());
    assert_eq!(
        licenses.stdout,
        excalidraw_render::license_bundle().as_bytes()
    );
    assert!(licenses.stderr.is_empty());
}

#[test]
fn only_one_valid_json_report_selection_changes_usage_error_transport() {
    let json = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["--report-format=json", "--not-an-option"])
        .output()
        .expect("run invalid JSON-mode command");
    assert_eq!(json.status.code(), Some(2));
    assert!(json.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&json.stderr).expect("JSON failure");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["error"]["family"], "cli");
    assert_eq!(report["error"]["code"], "cli.invalid-arguments");

    let duplicate = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            "--report-format=json",
            "--report-format",
            "json",
            "--not-an-option",
        ])
        .output()
        .expect("run duplicate report-mode command");
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(serde_json::from_slice::<serde_json::Value>(&duplicate.stderr).is_err());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("cli.invalid-arguments"));
}

#[test]
fn validation_uses_normative_failure_order_and_stable_exit_statuses() {
    let scratch = temp_directory_named("validation");
    let temp = scratch.path();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic-shapes.excalidraw");
    let output = temp.join("result.svg");

    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            "-",
            "--compatibility-revision",
            "999",
            "--padding=-1",
            "--report-format=json",
        ],
        2,
        "revision.unsupported",
    );
    // A revision past u64 is unsupported too, not malformed.
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            "-",
            "--compatibility-revision",
            "99999999999999999999",
            "--report-format=json",
        ],
        2,
        "revision.unsupported",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            "-",
            "--compatibility-revision",
            "1x",
            "--report-format=json",
        ],
        2,
        "cli.invalid-arguments",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            "-",
            "--padding=-1",
            "--report-format=json",
        ],
        2,
        "cli.format-required",
    );
    let format_required = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([fixture.to_str().unwrap(), "-o", "-", "--report-format=json"])
        .output()
        .expect("run format-required command");
    let format_required: serde_json::Value =
        serde_json::from_slice(&format_required.stderr).expect("JSON format failure");
    assert_eq!(format_required["error"]["details"]["kind"], "cli_argument");
    let invalid_none = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-format",
            "none",
            "--report-output",
            "stderr",
        ])
        .output()
        .expect("run invalid none-mode command");
    assert_eq!(invalid_none.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid_none.stderr).contains("[cli.invalid-arguments]"));
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-output=-",
            "--report-format=json",
        ],
        2,
        "cli.invalid-arguments",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            fixture.to_str().unwrap(),
            "--format",
            "svg",
            "--asset",
            "broken",
            "--report-format=json",
        ],
        2,
        "cli.invalid-asset-option",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--asset",
            "asset=-",
            "--report-format=json",
        ],
        2,
        "cli.invalid-asset-option",
    );
    let invalid_mime = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--asset-mime",
            "broken",
            "--report-format=json",
        ])
        .output()
        .expect("run malformed asset MIME declaration");
    let invalid_mime: serde_json::Value =
        serde_json::from_slice(&invalid_mime.stderr).expect("JSON failure report");
    assert_eq!(invalid_mime["error"]["details"]["argument"], "--asset-mime");
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            fixture.to_str().unwrap(),
            "--format",
            "svg",
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );

    fs::write(&output, b"old output").expect("create existing destination");
    let missing = temp.join("missing.excalidraw");
    assert_failure(
        &[
            missing.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-format=json",
        ],
        1,
        "cli.destination-exists",
    );
    assert_failure(
        &[
            missing.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--force",
            "--report-format=json",
        ],
        1,
        "cli.input-read-failed",
    );
    assert_eq!(fs::read(&output).unwrap(), b"old output");

    let original_fixture = fs::read(&fixture).expect("read fixture before conflict test");
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            temp.join("conflict.svg").to_str().unwrap(),
            "--report-output",
            fixture.to_str().unwrap(),
            "--report-format=json",
            "--force",
        ],
        2,
        "cli.path-conflict",
    );
    assert_eq!(fs::read(&fixture).unwrap(), original_fixture);

    let invalid_scene = temp.join("invalid.excalidraw");
    fs::write(&invalid_scene, b"not JSON").expect("write invalid scene");
    assert_failure(
        &[
            invalid_scene.to_str().unwrap(),
            "-o",
            temp.join("parse.svg").to_str().unwrap(),
            "--asset",
            "z=missing-z",
            "--report-format=json",
        ],
        1,
        "scene.invalid-json",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            temp.join("asset.svg").to_str().unwrap(),
            "--asset",
            "z=missing-z",
            "--asset",
            "a=missing-a",
            "--report-format=json",
        ],
        1,
        "cli.asset-read-failed",
    );

    let ordered_asset_failure = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            fixture.to_str().unwrap(),
            "-o",
            temp.join("ordered-asset.svg").to_str().unwrap(),
            "--asset",
            "z=missing-z",
            "--asset",
            "a=missing-a",
            "--report-format=json",
        ])
        .output()
        .expect("run ordered asset failure");
    let ordered_asset_failure: serde_json::Value =
        serde_json::from_slice(&ordered_asset_failure.stderr).expect("JSON asset failure");
    assert_eq!(ordered_asset_failure["error"]["details"]["asset_id"], "a");

    let none = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            missing.to_str().unwrap(),
            "-o",
            temp.join("none.svg").to_str().unwrap(),
            "--report-format",
            "none",
        ])
        .output()
        .expect("run none-mode failure");
    assert_eq!(none.status.code(), Some(1));
    assert!(serde_json::from_slice::<serde_json::Value>(&none.stderr).is_err());
    assert!(String::from_utf8_lossy(&none.stderr).contains("[cli.input-read-failed]"));
}

#[test]
fn rejects_existing_and_nonexistent_path_aliases_without_modifying_sources() {
    let scratch = temp_directory_named("path-aliases");
    let temp = scratch.path();
    let input = temp.join("input.excalidraw");
    fs::copy(corpus_scene("basic-shapes.excalidraw"), &input).expect("copy scene input");
    let alias = temp.join("input-alias.excalidraw");
    fs::hard_link(&input, &alias).expect("create input hard link");
    let original = fs::read(&input).expect("read original scene");

    assert_failure(
        &[
            input.to_str().unwrap(),
            "-o",
            alias.to_str().unwrap(),
            "--format=svg",
            "--force",
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );
    assert_eq!(fs::read(&input).unwrap(), original);

    assert_failure(
        &[
            "frames",
            input.to_str().unwrap(),
            "--output",
            input.to_str().unwrap(),
            "--format=svg",
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );
    assert_eq!(fs::read(&input).unwrap(), original);

    let output = temp.join("future.svg");
    let report_alias = temp.join("nested").join("..").join("future.svg");
    assert_failure(
        &[
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-output",
            report_alias.to_str().unwrap(),
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );
    assert!(!output.exists());
}

#[cfg(unix)]
#[test]
fn treats_dangling_symlink_destinations_as_existing_paths() {
    use std::os::unix::fs::symlink;

    let scratch = temp_directory_named("dangling-destination");
    let temp = scratch.path();
    let output = temp.join("result.svg");
    symlink(temp.join("missing-target"), &output).expect("create dangling destination symlink");

    assert_failure(
        &[
            corpus_scene("basic-shapes.excalidraw").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-format=json",
        ],
        1,
        "cli.destination-exists",
    );
}

/// A device or a pipe named as the output is written in place: there is no
/// file to protect, and replacing the device node would be wrong even with
/// `--force`.
#[cfg(unix)]
#[test]
fn writes_devices_and_pipes_in_place_without_force() {
    for args in [
        vec!["-o", "/dev/null", "-f", "svg"],
        vec!["-o", "/dev/null", "-f", "png", "--force"],
        vec![
            "-o",
            "-",
            "-f",
            "svg",
            "--report-format",
            "json",
            "--report-output",
            "/dev/null",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .arg(corpus_scene("basic-shapes.excalidraw"))
            .args(&args)
            .output()
            .expect("render to /dev/null");
        assert!(result.status.success(), "{args:?}: {result:?}");
        assert_eq!(String::from_utf8_lossy(&result.stderr), "", "{args:?}");
    }
    let null = fs::metadata("/dev/null").expect("/dev/null exists");
    assert!(!null.is_file(), "/dev/null was replaced by a file");

    let scratch = temp_directory_named("fifo-destination");
    let fifo = scratch.path().join("pipe");
    let made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("run mkfifo");
    assert!(made.success());
    let reader = {
        let fifo = fifo.clone();
        std::thread::spawn(move || fs::read(fifo).expect("read the pipe"))
    };
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .arg("-o")
        .arg(&fifo)
        .args(["-f", "svg"])
        .output()
        .expect("render to a pipe");
    if !result.status.success() {
        // Release the reader, still waiting for a writer.
        drop(fs::OpenOptions::new().write(true).open(&fifo));
    }
    let piped = reader.join().expect("the reader finishes");
    assert!(result.status.success(), "{result:?}");
    assert_eq!(String::from_utf8_lossy(&result.stderr), "");
    assert!(piped.starts_with(b"<svg"), "{}", piped.len());
    assert_no_staging_files(scratch.path());
}

#[test]
fn rejects_scene_and_asset_payloads_beyond_fixed_transport_bounds() {
    let scratch = temp_directory_named("bounded-reads");
    let temp = scratch.path();
    let oversized_scene = temp.join("oversized.excalidraw");
    fs::File::create(&oversized_scene)
        .and_then(|file| file.set_len(67_108_865))
        .expect("create oversized sparse scene");
    assert_failure(
        &[
            oversized_scene.to_str().unwrap(),
            "-o",
            temp.join("scene.svg").to_str().unwrap(),
            "--report-format=json",
        ],
        1,
        "scene.input-too-large",
    );

    let oversized_asset = temp.join("oversized.asset");
    fs::File::create(&oversized_asset)
        .and_then(|file| file.set_len(33_554_433))
        .expect("create oversized sparse asset");
    assert_failure(
        &[
            corpus_scene("basic-shapes.excalidraw").to_str().unwrap(),
            "-o",
            temp.join("asset.svg").to_str().unwrap(),
            "--asset",
            &format!("asset={}", oversized_asset.display()),
            "--report-format=json",
        ],
        1,
        "resource.asset-bytes-exceeded",
    );
}

#[test]
fn stages_both_paths_before_delivery_and_cleans_temporary_files() {
    let scratch = temp_directory_named("staging");
    let temp = scratch.path();
    let output = temp.join("result.svg");
    let unavailable_report = temp.join("missing-parent").join("report.json");
    let result = render_command(&output)
        .args([
            "--report-output",
            unavailable_report.to_str().unwrap(),
            "--report-format=json",
        ])
        .output()
        .expect("run report staging failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists(), "artifact must not precede report staging");
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("emergency JSON report");
    assert_eq!(report["error"]["code"], "cli.report-write-failed");
    assert_eq!(report["error"]["details"]["artifact_state"], "none");
    assert_no_staging_files(temp);
}

#[test]
fn delivers_artifact_before_report_and_emits_complete_emergency_json() {
    let scratch = temp_directory_named("report-delivery");
    let temp = scratch.path();
    let output = temp.join("result.svg");
    let report_destination = temp.join("report.json");
    fs::create_dir(&report_destination).expect("create undeliverable report destination");
    let result = render_command(&output)
        .args([
            "--report-output",
            report_destination.to_str().unwrap(),
            "--report-format=json",
            "--force",
        ])
        .output()
        .expect("run report delivery failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(fs::read(&output)
        .expect("delivered artifact")
        .starts_with(b"<svg "));
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("canonical emergency JSON report");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["artifact"], serde_json::Value::Null);
    assert_eq!(report["error"]["code"], "cli.report-write-failed");
    assert_eq!(report["error"]["details"]["artifact_state"], "complete");
    assert!(result.stderr.ends_with(b"\n"));
    assert_eq!(
        result.stderr.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    assert_no_staging_files(temp);
}

#[test]
fn failed_non_json_report_delivery_emits_one_emergency_line() {
    let scratch = temp_directory_named("non-json-emergency");
    let temp = scratch.path();
    let input = temp.join("diagnosed.excalidraw");
    fs::write(
        &input,
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"unknown","type":"future","x":0,"y":0,"width":100,"height":50,"updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("write diagnosed scene");
    let output = temp.join("result.svg");
    let report_destination = temp.join("report.txt");
    fs::create_dir(&report_destination).expect("create undeliverable report destination");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .args([
            "--report-output",
            report_destination.to_str().unwrap(),
            "--report-format=human",
            "--force",
        ])
        .output()
        .expect("run non-JSON report delivery failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(output.exists());
    assert_eq!(
        result.stderr.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("cli.report-write-failed"));
}

// `/dev/full` fails every write without moving a byte. Linux has it, and only
// Linux promises that a failed write moved nothing (`write_stream_tracked`).
#[cfg(target_os = "linux")]
#[test]
fn stdout_failure_before_first_byte_reports_that_no_artifact_bytes_escaped() {
    let scratch = temp_directory_named("closed-stdout");
    let temp = scratch.path();
    let report_path = temp.join("report.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"));
    command
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .args([
            "-o",
            "-",
            "--format=svg",
            "--report-output",
            report_path.to_str().unwrap(),
            "--report-format=json",
        ])
        .stderr(Stdio::piped());
    command.stdout(Stdio::from(
        fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("open always-failing stdout"),
    ));
    let child = command.spawn().expect("spawn renderer with closed stdout");
    let result = child.wait_with_output().expect("wait for stdout failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read stdout failure report"))
            .expect("JSON stdout failure report");
    assert_eq!(report["error"]["code"], "cli.output-write-failed");
    assert_eq!(report["error"]["details"]["artifact_state"], "none");
    assert_no_staging_files(temp);
}

#[test]
fn interrupted_stdout_after_bytes_escape_reports_possibly_partial() {
    let scratch = temp_directory_named("partial-stdout");
    let temp = scratch.path();
    let report_path = temp.join("report.json");
    // The artifact must outgrow the pipe buffer (64 KiB on Linux) so the
    // write is still in flight when the reader closes: a 4x PNG of the fixture
    // is ~240 KB, where its PDF fell under the buffer once every stream was
    // Flate (ADR-0008).
    let mut child = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .args([
            "-o",
            "-",
            "--format=png",
            "--scale=4",
            "--report-output",
            report_path.to_str().unwrap(),
            "--report-format=json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn renderer with interrupted stdout");
    let mut stdout = child.stdout.take().expect("take stdout pipe");
    let mut first_byte = [0_u8; 1];
    stdout
        .read_exact(&mut first_byte)
        .expect("receive one artifact byte");
    drop(stdout);
    let result = child
        .wait_with_output()
        .expect("wait for partial stdout failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read partial failure report"))
            .expect("JSON partial stdout failure report");
    assert_eq!(report["error"]["code"], "cli.output-write-failed");
    assert_eq!(
        report["error"]["details"]["artifact_state"],
        "possibly_partial"
    );
}

#[test]
fn a_reader_closing_stdout_early_ends_a_human_report_quietly() {
    // As in `interrupted_stdout_after_bytes_escape_reports_possibly_partial`,
    // the artifact outgrows the pipe buffer so the reader closes mid-write.
    for report in [&[][..], &["--report-format", "none"][..]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .arg(corpus_scene("basic-shapes.excalidraw"))
            .args(["-o", "-", "--format=png", "--scale=4"])
            .args(report)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn renderer with a short reader");
        let mut stdout = child.stdout.take().expect("take stdout pipe");
        let mut first_byte = [0_u8; 1];
        stdout
            .read_exact(&mut first_byte)
            .expect("receive one artifact byte");
        drop(stdout);
        let result = child.wait_with_output().expect("wait for the closed pipe");

        assert_eq!(result.status.code(), Some(1), "{report:?}");
        assert!(
            result.stderr.is_empty(),
            "{report:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

fn assert_failure(args: &[&str], exit: i32, code: &str) {
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(args)
        .output()
        .expect("run failing command");
    assert_eq!(
        result.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).unwrap_or_else(|error| {
            panic!(
                "JSON failure report for {args:?}: {error}; stderr={}",
                String::from_utf8_lossy(&result.stderr)
            )
        });
    assert_eq!(report["status"], "failure");
    assert_eq!(report["error"]["code"], code);
    assert_eq!(report["artifact"], serde_json::Value::Null);
}

#[test]
fn renders_fixture_to_svg_png_and_pdf_deterministically() {
    let scratch = temp_directory();
    let temp = scratch.path();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic-shapes.excalidraw");
    let svg = temp.join("basic.svg");
    let second_svg = temp.join("basic-second.svg");
    let png = temp.join("basic.png");
    let pdf = temp.join("basic.pdf");

    run(&fixture, &svg, &[]);
    let stdin_svg = run_stdio(&fixture, &["--format", "svg", "--report-format", "json"]);
    fs::write(&second_svg, &stdin_svg).expect("write captured SVG");
    run(&fixture, &png, &["--scale", "2"]);
    run(&fixture, &pdf, &[]);

    let svg_bytes = fs::read(&svg).expect("read SVG output");
    let second_svg_bytes = fs::read(&second_svg).expect("read second SVG output");
    let png_bytes = fs::read(&png).expect("read PNG output");
    let pdf_bytes = fs::read(&pdf).expect("read PDF output");

    assert!(!svg_bytes.is_empty());
    assert!(!png_bytes.is_empty());
    assert!(!pdf_bytes.is_empty());
    assert_eq!(svg_bytes, second_svg_bytes);

    let svg_text = String::from_utf8(svg_bytes).expect("SVG is UTF-8");
    assert!(svg_text.contains(r#"width="1510" height="1110""#));
    assert!(svg_text.starts_with("<svg "));
    assert!(svg_text.contains(r#"data-source-text="Text rendered by the complete CLI""#));

    let pixmap = tiny_skia::Pixmap::decode_png(&png_bytes).expect("decode PNG output");
    assert_eq!(pixmap.width(), 3020);
    assert_eq!(pixmap.height(), 2220);
    assert!(pdf_bytes.starts_with(b"%PDF-"));
    let pdf_text = String::from_utf8_lossy(&pdf_bytes);
    // Krilla names the embedded Excalifont program with its own subset tag
    // (ADR-0021).
    assert!(pdf_text.split("/BaseFont/").skip(1).any(|name| {
        name.len() > 7
            && name.as_bytes()[..6].iter().all(u8::is_ascii_uppercase)
            && name[6..].starts_with("+Excalifont-Regular")
    }));
}

#[test]
fn preserves_embedded_images_across_cli_formats() {
    let scratch = temp_directory_named("embedded-image");
    let temp = scratch.path();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/embedded-image.excalidraw");
    let svg = temp.join("image.svg");
    let png = temp.join("image.png");
    let pdf = temp.join("image.pdf");

    run(&fixture, &svg, &[]);
    run(&fixture, &png, &[]);
    run(&fixture, &pdf, &[]);

    let svg = fs::read_to_string(svg).expect("read SVG image output");
    assert!(svg.contains("data:image/png;base64,"));
    assert!(svg.contains(r#"width="10" height="10""#));

    let png = tiny_skia::Pixmap::load_png(png).expect("decode PNG image output");
    assert_eq!((png.width(), png.height()), (30, 30));
    assert_eq!(
        png.pixel(15, 15).expect("center pixel").demultiply(),
        tiny_skia::ColorU8::from_rgba(0, 0, 0, 255)
    );

    let pdf = fs::read(pdf).expect("read PDF image output");
    let pdf = String::from_utf8_lossy(&pdf);
    // The content stream is Flate, so the image is observed through its
    // XObject dictionary and the page's resource entry for it, in Krilla's
    // spelling (ADR-0021).
    assert!(pdf.contains("/Subtype/Image"));
    assert!(pdf.contains("/XObject<</x0 "));
}

#[test]
fn frames_list_reports_the_selection_without_rendering_anything() {
    let scratch = temp_directory_named("frames-list");
    let temp = scratch.path();
    let input = temp.join("scene.excalidraw");
    let output = temp.join("frames");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"Road Map","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"two","type":"frame","name":"Road Map","x":50,"y":0,"width":40,"height":30,"updated":1},
                {"id":"three","type":"magicframe","name":null,"x":100,"y":0,"width":40,"height":30,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    // Neither --output nor --format is required, and nothing is written.
    let human = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--list")
        .output()
        .expect("run frame listing");
    assert!(human.status.success());
    // The listing is the command's output: on stdout, so it can be piped.
    assert!(human.stderr.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&human.stdout),
        "1\tframe\tRoad Map\tRoad-Map\n2\tframe\tRoad Map\tRoad-Map-2\n3\tmagicframe\t\tframe-3\n"
    );
    assert!(!output.exists());

    // With a format, the derived basenames carry the extension the export would
    // have used, and the ordinals, kinds, and names match the export order.
    let json = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--format", "svg", "--report-format", "json"])
        .output()
        .expect("run frame listing");
    assert!(json.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("frame list JSON report");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["status"], "success");
    assert_eq!(report["mode"], "list");
    assert_eq!(report["compatibility_revision"], 1);
    assert_eq!(
        report["reference_profile_digest"],
        Renderer::new().frame_profile_identity().profile_digest()
    );
    assert_eq!(report["frames"].as_array().map(Vec::len), Some(3));
    assert_eq!(report["frames"][0]["ordinal"], 1);
    assert_eq!(report["frames"][0]["kind"], "frame");
    assert_eq!(report["frames"][0]["name"], "Road Map");
    assert_eq!(report["frames"][0]["output_basename"], "Road-Map.svg");
    assert_eq!(report["frames"][2]["kind"], "magicframe");
    assert_eq!(report["frames"][2]["name"], serde_json::Value::Null);
    assert_eq!(report["frames"][2]["output_basename"], "frame-3.svg");
    assert!(!output.exists());

    // --name-regex and --max-frames apply exactly as they do to an export.
    let filtered = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--name-regex", "^Road"])
        .output()
        .expect("run filtered frame listing");
    assert!(filtered.status.success());
    assert_eq!(
        String::from_utf8_lossy(&filtered.stdout),
        "1\tframe\tRoad Map\tRoad-Map\n2\tframe\tRoad Map\tRoad-Map-2\n"
    );

    let capped = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--max-frames", "2"])
        .output()
        .expect("run capped frame listing");
    assert_eq!(capped.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&capped.stderr).contains("cli.frame-limit-exceeded"));

    // An empty selection is a valid answer to a listing, unlike to an export.
    let empty = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args([
            "--list",
            "--name-regex",
            "^Missing$",
            "--report-format",
            "json",
        ])
        .output()
        .expect("run empty frame listing");
    assert!(empty.status.success());
    let empty_report: serde_json::Value =
        serde_json::from_slice(&empty.stdout).expect("empty frame list JSON report");
    assert_eq!(empty_report["frames"].as_array().map(Vec::len), Some(0));

    // A human listing says so explicitly instead of printing nothing.
    let empty_human = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--name-regex", "^Missing$"])
        .output()
        .expect("run empty human frame listing");
    assert!(empty_human.status.success());
    assert_eq!(
        String::from_utf8_lossy(&empty_human.stderr),
        "note: no frames match --name-regex '^Missing$'\n"
    );
    let frameless = temp.join("frameless.excalidraw");
    fs::copy(corpus_scene("basic-shapes.excalidraw"), &frameless).expect("copy frameless scene");
    let frameless = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&frameless)
        .arg("--list")
        .output()
        .expect("run frameless listing");
    assert!(frameless.status.success());
    assert_eq!(
        String::from_utf8_lossy(&frameless.stderr),
        "note: the scene has no frames\n"
    );

    let suppressed = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--report-format", "none"])
        .output()
        .expect("run suppressed frame listing");
    assert_eq!(suppressed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&suppressed.stderr).contains("cli.invalid-arguments"));

    // Outside --list, both --output and --format remain required.
    let missing = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--format", "svg"])
        .output()
        .expect("run frame export without an output");
    assert_eq!(missing.status.code(), Some(2));

    // A dry run is exempt from performing an export's work, not from rejecting
    // what an export would reject: a listing that succeeded here while the
    // matching export failed would be a false preview.
    for rejected in [
        vec!["--scale", "0"],
        vec!["--max-dimension", "-1"],
        vec!["--asset", "bogus-no-equals"],
        vec!["--asset-mime", "unmatched=image/png"],
    ] {
        let listed = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .arg("frames")
            .arg(&input)
            .arg("--list")
            .args(&rejected)
            .output()
            .expect("run rejected frame listing");
        let exported = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .arg("frames")
            .arg(&input)
            .arg("--output")
            .arg(temp.join("rejected"))
            .args(["--format", "svg"])
            .args(&rejected)
            .output()
            .expect("run rejected frame export");
        assert_eq!(
            listed.status.code(),
            exported.status.code(),
            "{rejected:?} is accepted by --list but rejected by an export"
        );
        assert_eq!(
            listed.status.code(),
            Some(2),
            "{rejected:?} is a usage error"
        );
    }
}

#[test]
fn frames_command_filters_names_and_writes_one_deterministically_named_artifact_per_frame() {
    let scratch = temp_directory_named("frames-command");
    let temp = scratch.path();
    let input = temp.join("scene.excalidraw");
    let output = temp.join("frames");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"Road Map","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"one-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"one","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
                {"id":"two","type":"frame","name":"Road Map","x":50,"y":0,"width":40,"height":30,"updated":1},
                {"id":"two-child","type":"rectangle","x":55,"y":5,"width":10,"height":10,"frameId":"two","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1},
                {"id":"three","type":"magicframe","name":null,"x":100,"y":0,"width":40,"height":30,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args(["--format", "svg", "--report-format", "json"])
        .output()
        .expect("run frame export");
    assert!(
        result.status.success(),
        "frame export failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(output.join("Road-Map.svg").is_file());
    assert!(output.join("Road-Map-2.svg").is_file());
    assert!(output.join("frame-3.svg").is_file());
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("frame batch JSON report");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["status"], "success");
    assert_eq!(report["format"], "svg");
    assert_eq!(report["frames"].as_array().map(Vec::len), Some(3));
    assert_eq!(report["frames"][0]["ordinal"], 1);
    assert_eq!(
        report["frames"][0]["output_path"],
        output.join("Road-Map.svg").to_string_lossy().as_ref()
    );
    assert_eq!(
        report["reference_profile_digest"],
        Renderer::new().frame_profile_identity().profile_digest()
    );

    let no_match_output = temp.join("no-match");
    let no_match = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--output")
        .arg(&no_match_output)
        .args(["--format", "svg", "--name-regex", "^Missing$"])
        .output()
        .expect("run no-match frame export");
    assert_eq!(no_match.status.code(), Some(1));
    assert!(!no_match_output.exists());
    assert!(String::from_utf8_lossy(&no_match.stderr).contains("cli.frame-no-match"));
}

#[test]
fn frame_exports_explain_bad_patterns_and_output_locations_before_rendering() {
    let scratch = temp_directory_named("frame-usage");
    let temp = scratch.path();
    let input = corpus_scene("basic-shapes.excalidraw");

    // The pattern's own syntax error says what to fix.
    let result = cli(&[
        "frames",
        input.to_str().unwrap(),
        "--list",
        "--name-regex",
        "[",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "error: --name-regex is not valid Rust regex syntax: unclosed character class \
         [cli.invalid-name-regex]\n"
    );

    // A file where the frame directory belongs is the mirror of a directory
    // given as a scene output.
    let file = temp.join("file");
    fs::write(&file, b"keep").expect("create a file output");
    let result = cli(&[
        "frames",
        input.to_str().unwrap(),
        "-o",
        file.to_str().unwrap(),
        "--format",
        "svg",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: frame output '{}' exists and is not a directory \
             [cli.invalid-output-directory]\n\
             hint: pass a directory to frames -o, or drop `frames` to render the whole scene \
             to one file\n",
            file.display()
        )
    );
    assert_eq!(fs::read(&file).unwrap(), b"keep");

    // Only the last directory is created, so a missing parent fails up front.
    let parent = temp.join("missing");
    let nested = parent.join("frames");
    let reason = fs::read_dir(&parent).expect_err("missing directory");
    let result = cli(&[
        "frames",
        input.to_str().unwrap(),
        "-o",
        nested.to_str().unwrap(),
        "--format",
        "svg",
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        format!(
            "error: cannot create output directory '{}': {reason} [cli.output-write-failed]\n\
             hint: directory '{}' does not exist; create it first\n",
            nested.display(),
            parent.display()
        )
    );
    assert!(!parent.exists());
    assert_no_staging_files(temp);
}

#[test]
fn frame_batches_are_stage_all_by_default_and_partial_only_when_requested() {
    let scratch = temp_directory_named("frame-publication-policy");
    let temp = scratch.path();
    let input = temp.join("scene.excalidraw");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"One","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"one-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"one","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
                {"id":"two","type":"frame","name":"Two","x":50,"y":0,"width":40,"height":30,"updated":1},
                {"id":"two-child","type":"rectangle","x":55,"y":5,"width":10,"height":10,"frameId":"two","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    let atomic = temp.join("atomic");
    fs::create_dir(&atomic).expect("create atomic output");
    fs::write(atomic.join("One.svg"), b"existing").expect("write collision");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg", "--report-format", "json"])
        .output()
        .expect("run atomic frame export");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(atomic.join("One.svg")).unwrap(), b"existing");
    assert!(!atomic.join("Two.svg").exists());
    assert_no_staging_files(&atomic);
    let report: serde_json::Value = serde_json::from_slice(&result.stderr).expect("atomic report");
    assert_eq!(report["status"], "failure");
    assert!(report["frames"][0]["artifact"].is_object());

    let partial = temp.join("partial");
    fs::create_dir(&partial).expect("create partial output");
    fs::write(partial.join("One.svg"), b"existing").expect("write collision");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&partial)
        .args([
            "--format",
            "svg",
            "--allow-partial",
            "--report-format",
            "json",
        ])
        .output()
        .expect("run partial frame export");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(partial.join("One.svg")).unwrap(), b"existing");
    assert!(partial.join("Two.svg").is_file());
    assert_no_staging_files(&partial);
    let report: serde_json::Value = serde_json::from_slice(&result.stderr).expect("partial report");
    assert_eq!(report["status"], "partial");
    assert_eq!(report["frames"][0]["delivery_state"], "none");
    assert_eq!(report["frames"][1]["delivery_state"], "complete");
    assert_eq!(report["frames"][0]["error"]["family"], "cli");
    assert_eq!(
        report["frames"][0]["error"]["category"],
        serde_json::Value::Null
    );
    assert_eq!(report["frames"][0]["error"]["details"]["kind"], "delivery");
    let raw_report = std::str::from_utf8(&result.stderr).expect("frame report is UTF-8");
    assert!(raw_report
        .starts_with("{\"schema_version\":1,\"status\":\"partial\",\"renderer_version\":"));
    assert!(raw_report.contains(
        "\"error\":{\"family\":\"cli\",\"category\":null,\"code\":\"cli.destination-exists\""
    ));

    let human = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg"])
        .output()
        .expect("run human collision report");
    assert_eq!(human.status.code(), Some(1));
    let human = String::from_utf8(human.stderr).expect("human report is UTF-8");
    assert!(human.contains("error: frame 1: output '"), "{human}");
    assert!(
        human.contains("already exists [cli.destination-exists]\n"),
        "{human}"
    );
    assert_eq!(
        human.matches("hint: use --force to overwrite").count(),
        1,
        "{human}"
    );
    assert!(!human.contains("frame-batch-"));

    // A re-run over a complete earlier export collides on every frame; the
    // fix is given once, for all of them.
    let rerun = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&partial)
        .args(["--format", "svg"])
        .output()
        .expect("re-run a frame export");
    assert_eq!(rerun.status.code(), Some(1));
    let rerun = String::from_utf8(rerun.stderr).expect("human report is UTF-8");
    assert_eq!(
        rerun
            .matches("already exists [cli.destination-exists]\n")
            .count(),
        2,
        "{rerun}"
    );
    assert!(
        rerun.ends_with("\nhint: use --force to overwrite the 2 existing frame files\n"),
        "{rerun}"
    );
    assert_eq!(rerun.matches("hint:").count(), 1, "{rerun}");

    let none = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg", "--report-format", "none"])
        .output()
        .expect("run suppressed collision report");
    assert_eq!(none.status.code(), Some(1));
    let none = String::from_utf8(none.stderr).expect("none report is UTF-8");
    assert!(
        none.contains("already exists [cli.destination-exists]\n"),
        "{none}"
    );
    assert_eq!(none.lines().count(), 1, "{none}");
    assert!(!none.contains("frame-batch-"));

    let missing_asset = temp.join("missing-asset.bin");
    let precedence = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg", "--report-format", "json", "--asset"])
        .arg(format!("missing={}", missing_asset.display()))
        .output()
        .expect("run collision and missing asset");
    assert_eq!(precedence.status.code(), Some(1));
    let precedence: serde_json::Value =
        serde_json::from_slice(&precedence.stderr).expect("precedence report is JSON");
    assert_eq!(precedence["error"]["code"], "cli.destination-exists");
}

#[test]
fn frame_batch_report_staging_failure_prevents_all_artifact_publication() {
    let scratch = temp_directory_named("frame-report-staging");
    let temp = scratch.path();
    let input = temp.join("scene.excalidraw");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"One","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"one-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"one","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    for (mode, allow_partial) in [("default", false), ("partial", true)] {
        let output = temp.join(format!("{mode}-frames"));
        let report = temp
            .join(format!("{mode}-missing-parent"))
            .join("report.json");
        let mut command = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"));
        command
            .arg("frames")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .args(["--format", "svg", "--report-format", "json"])
            .arg("--report-output")
            .arg(&report);
        if allow_partial {
            command.arg("--allow-partial");
        }
        let result = command.output().expect("run frame report staging failure");

        assert_eq!(result.status.code(), Some(1));
        assert!(
            !output.exists(),
            "{mode} mode published output before report staging"
        );
        assert!(!report.exists());
        let emergency: serde_json::Value =
            serde_json::from_slice(&result.stderr).expect("emergency JSON report");
        assert_eq!(emergency["error"]["code"], "cli.report-write-failed");
        assert_eq!(emergency["error"]["details"]["artifact_state"], "none");
        assert_no_staging_files(temp);
    }
}

fn run_stdio(input: &Path, extra_args: &[&str]) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["-", "--output", "-"])
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn excalidraw-render");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&fs::read(input).expect("read fixture"))
        .expect("write scene to stdin");
    let result = child.wait_with_output().expect("wait for renderer");
    assert!(
        result.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("JSON success report");
    assert_eq!(report["status"], "success");
    assert_eq!(report["format"], "svg");
    assert_eq!(report["artifact"]["byte_length"], result.stdout.len());
    assert!(report["artifact"]["sha256"]
        .as_str()
        .expect("artifact digest")
        .starts_with("sha256:"));
    result.stdout
}

fn run(input: &Path, output: &Path, extra_args: &[&str]) {
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(input)
        .arg("-o")
        .arg(output)
        .args(extra_args)
        .output()
        .expect("run excalidraw-render");

    assert!(
        result.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn render_command(output: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"));
    command
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .arg("-o")
        .arg(output);
    command
}

fn corpus_scene(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assert_no_staging_files(directory: &Path) {
    let staging_files = fs::read_dir(directory)
        .expect("read temp directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert!(
        staging_files.is_empty(),
        "staging files remain: {staging_files:?}"
    );
}

fn temp_directory() -> TempDir {
    temp_directory_named("render")
}

fn temp_directory_named(name: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(&format!("excalidraw-render-test-{name}-"))
        .tempdir()
        .expect("create test temp directory")
}
