use super::{font_subset, svg_fonts};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use write_fonts::read::{types::Tag, FontRef, TableProvider};
use write_fonts::FontBuilder;

const FIXED_SEED: u64 = 0x6a09_e667_f3bc_c908;
const PR_CASES: usize = 64;
const MAX_FUZZ_INPUT_BYTES: usize = 4_096;
const MAX_SCALARS: usize = 64;
const MAX_MUTATIONS: usize = 16;
const MAX_NAME_TABLE_BYTES: usize = 256;
const MAX_SOURCE_WOFF2_BYTES: usize = 100_000;
const MAX_DECODED_SFNT_BYTES: usize = 10_000_000;
const MAX_DERIVED_WOFF2_BYTES: usize = 1_000_000;
const REGRESSION_SCHEMA: &str = "excalidraw-render/svg-font-subset-fuzz-regressions/v1";
const PROFILE_SHA256: &str = "49015dfb91f9e89fcf1e7f3e16fd892e4c1715f5d49b6488d67037b7aac21c23";
const REQUIRED_RISKS: [Risk; 7] = [
    Risk::MalformedSourceWoff2,
    Risk::MalformedGeneratedSfnt,
    Risk::MalformedGeneratedWoff2,
    Risk::ClosureHeavyScalars,
    Risk::NameLegalRecords,
    Risk::TablePolicy,
    Risk::CacheMissHitReplay,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Valid([u8; 32]),
    Failure(crate::SvgFontSubsetStage),
}

struct Context {
    record: &'static excalidraw_render_assets_base::AssetRecord,
    source_woff2: Vec<u8>,
    source_sfnt: Vec<u8>,
    baseline_scalars: Vec<u32>,
    baseline_identity: String,
    baseline_sfnt: Vec<u8>,
    baseline_woff2: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegressionCorpus {
    schema: String,
    compatibility_revision: u64,
    originating_issue: u64,
    broader_fuzz_issue: u64,
    subset_profile_sha256: String,
    source_identity: String,
    source_sha256: String,
    fixed_seed_hex: String,
    bounds: Bounds,
    public_facade_cases: Vec<PublicFacadeCase>,
    cases: Vec<RegressionCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublicFacadeCase {
    id: String,
    scene_json: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bounds {
    max_input_bytes: usize,
    max_scalars: usize,
    max_mutations: usize,
    max_name_table_bytes: usize,
    max_source_woff2_bytes: usize,
    max_decoded_sfnt_bytes: usize,
    max_derived_woff2_bytes: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegressionCase {
    id: String,
    risk: Risk,
    input_hex: String,
    minimized: bool,
    complete_chunk_fallback: bool,
    partial_output: bool,
    expected: Expected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum Risk {
    MalformedSourceWoff2,
    MalformedGeneratedSfnt,
    MalformedGeneratedWoff2,
    ClosureHeavyScalars,
    NameLegalRecords,
    TablePolicy,
    CacheMissHitReplay,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Expected {
    Valid { woff2_sha256: String },
    Failure { stage: ExpectedStage },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExpectedStage {
    Decode,
    Plan,
    Sfnt,
    Encode,
    Validate,
}

impl ExpectedStage {
    const fn actual(self) -> crate::SvgFontSubsetStage {
        match self {
            Self::Decode => crate::SvgFontSubsetStage::Decode,
            Self::Plan => crate::SvgFontSubsetStage::Plan,
            Self::Sfnt => crate::SvgFontSubsetStage::Sfnt,
            Self::Encode => crate::SvgFontSubsetStage::Encode,
            Self::Validate => crate::SvgFontSubsetStage::Validate,
        }
    }
}

impl Context {
    fn new() -> Self {
        let record = excalidraw_render_assets_base::revision_1()
            .assets
            .iter()
            .find(|record| record.family == "Liberation")
            .expect("the fuzz seam has a qualified TrueType source");
        let source_woff2 = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("the fuzz source verifies")
            .expect("the fuzz source exists")
            .bytes()
            .to_vec();
        let source_sfnt = wuff::decompress_woff2(&source_woff2).expect("the fuzz source decodes");
        assert!(source_woff2.len() <= MAX_SOURCE_WOFF2_BYTES);
        assert!(source_sfnt.len() <= MAX_DECODED_SFNT_BYTES);
        let baseline_scalars =
            "office"
                .chars()
                .map(u32::from)
                .fold(Vec::new(), |mut scalars, scalar| {
                    if !scalars.contains(&scalar) {
                        scalars.push(scalar);
                    }
                    scalars
                });
        let baseline_identity = svg_fonts::semantic_identity(record, &baseline_scalars)
            .expect("the fuzz baseline has a semantic identity");
        let baseline = font_subset::derive(
            &source_woff2,
            &baseline_scalars,
            "ExcalidrawRender-9",
            &baseline_identity,
        )
        .expect("the fuzz baseline derives");
        let baseline_woff2 = baseline.woff2.to_vec();
        let baseline_sfnt =
            wuff::decompress_woff2(&baseline_woff2).expect("the fuzz baseline decodes");
        Self {
            record,
            source_woff2,
            source_sfnt,
            baseline_scalars,
            baseline_identity,
            baseline_sfnt,
            baseline_woff2,
        }
    }
}

#[test]
fn fixed_seed_font_subset_fuzz_smoke_is_bounded_and_deterministic() {
    assert_font_subset_fuzz_smoke();
}

#[test]
fn font_subset_fuzz_regression_corpus_is_manifested_and_replayed() {
    assert_font_subset_fuzz_regression_corpus();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "scheduled deterministic fuzz expansion"]
fn nightly_font_subset_fuzz_is_bounded_and_deterministic() {
    let cases = std::env::var("EXCALIDRAW_FONT_SUBSET_FUZZ_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(4_096)
        .min(16_384);
    assert_generated_cases(FIXED_SEED, cases);
}

pub(crate) fn assert_font_subset_fuzz_smoke() {
    assert_generated_cases(FIXED_SEED, PR_CASES);
}

fn assert_generated_cases(seed: u64, cases: usize) {
    let context = Context::new();
    let mut generator = Generator(seed);
    for index in 0..cases {
        let input = generator.input(index);
        let id = format!("seed {seed:016x} case {index}");
        let first = replay(&context, &input, &id);
        let second = replay(&context, &input, &id);
        assert_eq!(first, second, "fuzz case {index} is deterministic");
    }
}

fn exercise(context: &Context, input: &[u8]) -> Outcome {
    let input = &input[..input.len().min(MAX_FUZZ_INPUT_BYTES)];
    let (route, payload) = input
        .split_first()
        .map_or((0, &[][..]), |(route, rest)| (route % 9, rest));
    match route {
        0 => derive_outcome(
            payload,
            &context.baseline_scalars,
            "ExcalidrawRender-9",
            &context.baseline_identity,
        ),
        1 => {
            let candidate = candidate(&context.source_woff2, payload);
            derive_outcome(
                &candidate,
                &context.baseline_scalars,
                "ExcalidrawRender-9",
                &context.baseline_identity,
            )
        }
        2 => {
            let scalars = scalars(payload, &context.baseline_scalars);
            let identity = svg_fonts::semantic_identity(context.record, &scalars)
                .expect("bounded scalar sets have a semantic identity");
            derive_outcome(
                &context.source_woff2,
                &scalars,
                "ExcalidrawRender-9",
                &identity,
            )
        }
        3 => name_records_outcome(context, payload),
        4 => {
            let generated = candidate(&context.baseline_sfnt, payload);
            generated_sfnt_outcome(context, &generated)
        }
        5 => {
            let generated = candidate(&context.baseline_woff2, payload);
            generated_woff2_outcome(context, &generated)
        }
        6 => table_policy_outcome(context, payload),
        7 => cache_outcome(context, payload),
        8 => public_facade_outcome(payload),
        _ => unreachable!("route is reduced modulo nine"),
    }
}

fn public_facade_outcome(payload: &[u8]) -> Outcome {
    let mut text = payload
        .chunks(4)
        .take(MAX_SCALARS)
        .map(|chunk| {
            let mut bytes = [0_u8; 4];
            bytes[..chunk.len()].copy_from_slice(chunk);
            char::from_u32(u32::from_le_bytes(bytes) % 0x11_0000).unwrap_or('?')
        })
        .collect::<String>();
    if text.is_empty() {
        text.push('A');
    }
    let source = serde_json::to_vec(&serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "elements": [{
            "id": "font-subset-fuzz",
            "type": "text",
            "width": 320,
            "height": 80,
            "fontSize": 20,
            "fontFamily": 2,
            "text": text,
            "updated": 1,
        }],
        "appState": {},
        "files": {},
    }))
    .expect("bounded public-facade fuzz scene is serializable");
    let scene =
        crate::SceneDocument::parse(&source).expect("bounded public-facade fuzz scene is admitted");
    let request = crate::RenderRequest::builder(crate::OutputFormat::Svg)
        .build()
        .expect("public-facade fuzz request is valid");
    match crate::Renderer::new().render(&scene, &request) {
        Ok(outcome) => {
            let svg = std::str::from_utf8(outcome.artifact().bytes())
                .expect("public-facade fuzz SVG is UTF-8");
            assert!(
                svg.contains("svg-font-subset@sha256:"),
                "public-facade fuzz render used an unqualified font payload"
            );
            Outcome::Valid(Sha256::digest(outcome.artifact().bytes()).into())
        }
        Err(error) => match error.details() {
            Some(crate::ErrorDetails::SvgFontSubsetFailure { stage, .. }) => {
                Outcome::Failure(*stage)
            }
            details => {
                panic!("public-facade fuzz render failed outside font subsetting: {details:?}")
            }
        },
    }
}

fn derive_outcome(source: &[u8], scalars: &[u32], alias: &str, identity: &str) -> Outcome {
    if source.len() > MAX_SOURCE_WOFF2_BYTES {
        return Outcome::Failure(crate::SvgFontSubsetStage::Decode);
    }
    match font_subset::derive(source, scalars, alias, identity) {
        Ok(derived) => {
            if derived.decoded_sfnt_bytes > MAX_DECODED_SFNT_BYTES
                || derived.subset_sfnt_bytes > MAX_DECODED_SFNT_BYTES
                || derived.woff2.len() > MAX_DERIVED_WOFF2_BYTES
                || derived.subset_tables
                    > excalidraw_render_assets_base::FONT_SUBSET_TABLE_RULES_V1.len()
                || derived.woff2.as_ref() == source
            {
                return Outcome::Failure(crate::SvgFontSubsetStage::Validate);
            }
            Outcome::Valid(Sha256::digest(derived.woff2.as_ref()).into())
        }
        Err(error) => Outcome::Failure(error.stage),
    }
}

fn generated_sfnt_outcome(context: &Context, generated_sfnt: &[u8]) -> Outcome {
    if font_subset::validate_checksums(generated_sfnt).is_err()
        || FontRef::new(generated_sfnt).is_err()
    {
        return Outcome::Failure(crate::SvgFontSubsetStage::Validate);
    }
    let generated_woff2 = match ttf2woff2::encode(generated_sfnt, ttf2woff2::BrotliQuality::from(1))
    {
        Ok(generated_woff2) => generated_woff2,
        Err(_) => return Outcome::Failure(crate::SvgFontSubsetStage::Validate),
    };
    match derive_outcome(
        &generated_woff2,
        &context.baseline_scalars,
        "ExcalidrawRender-9",
        &context.baseline_identity,
    ) {
        valid @ Outcome::Valid(_) => valid,
        Outcome::Failure(_) => Outcome::Failure(crate::SvgFontSubsetStage::Validate),
    }
}

fn generated_woff2_outcome(context: &Context, generated_woff2: &[u8]) -> Outcome {
    match wuff::decompress_woff2(generated_woff2) {
        Ok(generated_sfnt) => generated_sfnt_outcome(context, &generated_sfnt),
        Err(_) => Outcome::Failure(crate::SvgFontSubsetStage::Validate),
    }
}

fn name_records_outcome(context: &Context, payload: &[u8]) -> Outcome {
    if payload.is_empty() {
        return derive_outcome(
            &context.source_woff2,
            &context.baseline_scalars,
            "ExcalidrawRender-9",
            &context.baseline_identity,
        );
    }
    let source = FontRef::new(&context.source_sfnt).expect("the fuzz source is a valid sfnt");
    let source_name = source
        .data_for_tag(Tag::new(b"name"))
        .expect("the fuzz source has a required name table");
    let payload = &payload[..payload.len().min(MAX_NAME_TABLE_BYTES)];
    let name = candidate(source_name.as_bytes(), payload);
    let mut builder = FontBuilder::new();
    builder.add_raw(Tag::new(b"name"), name);
    builder.copy_missing_tables(source);
    let source_sfnt = builder.build();
    let source_woff2 = match ttf2woff2::encode(&source_sfnt, ttf2woff2::BrotliQuality::from(1)) {
        Ok(source_woff2) => source_woff2,
        Err(_) => return Outcome::Failure(crate::SvgFontSubsetStage::Sfnt),
    };
    derive_outcome(
        &source_woff2,
        &context.baseline_scalars,
        "ExcalidrawRender-9",
        &context.baseline_identity,
    )
}

fn table_policy_outcome(context: &Context, payload: &[u8]) -> Outcome {
    let source = FontRef::new(&context.source_sfnt).expect("the fuzz source is a valid sfnt");
    let mut builder = FontBuilder::new();
    if payload.first().copied().unwrap_or_default() & 1 == 0 {
        builder.add_raw(Tag::new(b"FUZ0"), payload.to_vec());
        builder.copy_missing_tables(source);
    } else {
        for record in source.table_directory().table_records() {
            if record.tag() == Tag::new(b"name") {
                continue;
            }
            let table = source
                .data_for_tag(record.tag())
                .expect("a manifested table record has data");
            builder.add_raw(record.tag(), table.as_bytes().to_vec());
        }
    }
    let source_sfnt = builder.build();
    let source_woff2 = match ttf2woff2::encode(&source_sfnt, ttf2woff2::BrotliQuality::from(1)) {
        Ok(source_woff2) => source_woff2,
        Err(_) => return Outcome::Failure(crate::SvgFontSubsetStage::Encode),
    };
    derive_outcome(
        &source_woff2,
        &context.baseline_scalars,
        "ExcalidrawRender-9",
        &context.baseline_identity,
    )
}

fn cache_outcome(context: &Context, payload: &[u8]) -> Outcome {
    let scalars = scalars(payload, &context.baseline_scalars);
    let identity = svg_fonts::semantic_identity(context.record, &scalars)
        .expect("bounded scalar sets have a semantic identity");
    let mut session = font_subset::Session::new();
    let first = session.derive(
        context.record.identity,
        &context.source_woff2,
        &scalars,
        "ExcalidrawRender-9",
        &identity,
    );
    let second = session.derive(
        context.record.identity,
        &context.source_woff2,
        &scalars,
        "ExcalidrawRender-9",
        &identity,
    );
    match (first, second) {
        (Ok(first), Ok(second)) if first.woff2 == second.woff2 && session.stats() == (1, 1) => {
            Outcome::Valid(Sha256::digest(first.woff2.as_ref()).into())
        }
        (Err(first), Err(second)) if first.stage == second.stage && session.stats() == (0, 2) => {
            Outcome::Failure(first.stage)
        }
        _ => panic!("cache miss/hit replay changed the semantic outcome"),
    }
}

fn candidate(baseline: &[u8], payload: &[u8]) -> Vec<u8> {
    let Some((&mode, mutations)) = payload.split_first() else {
        return Vec::new();
    };
    if mode & 1 == 0 {
        return mutations.to_vec();
    }
    let mut candidate = baseline.to_vec();
    if candidate.is_empty() {
        return candidate;
    }
    for mutation in mutations.chunks(3).take(MAX_MUTATIONS) {
        let high = usize::from(mutation.first().copied().unwrap_or_default());
        let low = usize::from(mutation.get(1).copied().unwrap_or_default());
        let offset = ((high << 8) | low) % candidate.len();
        candidate[offset] ^= mutation.get(2).copied().unwrap_or(0xff);
    }
    candidate
}

fn scalars(payload: &[u8], fallback: &[u32]) -> Vec<u32> {
    let scalars = payload
        .chunks(4)
        .take(MAX_SCALARS)
        .map(|chunk| {
            let mut bytes = [0_u8; 4];
            bytes[..chunk.len()].copy_from_slice(chunk);
            u32::from_le_bytes(bytes) % 0x11_0000
        })
        .collect::<Vec<_>>();
    if scalars.is_empty() {
        fallback.to_vec()
    } else {
        scalars
    }
}

struct Generator(u64);

impl Generator {
    fn input(&mut self, index: usize) -> Vec<u8> {
        let length = match index {
            0 => MAX_FUZZ_INPUT_BYTES,
            1 | 4 | 5 | 6 => 2 + MAX_MUTATIONS * 3,
            2 | 7 => 1 + MAX_SCALARS * 4,
            3 => 1 + MAX_NAME_TABLE_BYTES,
            _ => 2 + (usize::from(self.next_u8()) % 127),
        };
        let mut input = Vec::with_capacity(length);
        input.push(u8::try_from(index % 9).expect("route count fits in u8"));
        input.extend((1..length).map(|_| self.next_u8()));
        input
    }

    fn next_u8(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.to_le_bytes()[0]
    }
}

pub(crate) fn assert_font_subset_fuzz_regression_corpus() {
    let corpus: RegressionCorpus = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/qualification/font-subset-fuzz-regressions.json"
    )))
    .expect("the font subset fuzz regression corpus is valid JSON");
    assert_eq!(corpus.schema, REGRESSION_SCHEMA);
    assert_eq!(corpus.compatibility_revision, 1);
    assert_eq!(corpus.originating_issue, 100);
    assert_eq!(corpus.broader_fuzz_issue, 83);
    assert_eq!(corpus.subset_profile_sha256, PROFILE_SHA256);
    assert_eq!(
        encode_hex(&excalidraw_render_assets_base::font_subset_profile_digest_v1()),
        PROFILE_SHA256
    );
    assert_eq!(corpus.fixed_seed_hex, format!("{FIXED_SEED:016x}"));
    assert_eq!(corpus.bounds.max_input_bytes, MAX_FUZZ_INPUT_BYTES);
    assert_eq!(corpus.bounds.max_scalars, MAX_SCALARS);
    assert_eq!(corpus.bounds.max_mutations, MAX_MUTATIONS);
    assert_eq!(corpus.bounds.max_name_table_bytes, MAX_NAME_TABLE_BYTES);
    assert_eq!(corpus.bounds.max_source_woff2_bytes, MAX_SOURCE_WOFF2_BYTES);
    assert_eq!(corpus.bounds.max_decoded_sfnt_bytes, MAX_DECODED_SFNT_BYTES);
    assert_eq!(
        corpus.bounds.max_derived_woff2_bytes,
        MAX_DERIVED_WOFF2_BYTES
    );

    let context = Context::new();
    assert_eq!(corpus.source_identity, context.record.identity);
    assert_eq!(corpus.source_sha256, context.record.complete_sha256);
    assert_public_facade_cases(&corpus.public_facade_cases);
    let mut ids = BTreeSet::new();
    let mut risks = BTreeSet::new();
    for case in corpus.cases {
        assert!(ids.insert(case.id.clone()), "duplicate regression id");
        assert!(REQUIRED_RISKS.contains(&case.risk));
        risks.insert(case.risk);
        assert!(case.minimized, "{} is not minimized", case.id);
        assert!(
            !case.complete_chunk_fallback,
            "{} permits fallback",
            case.id
        );
        assert!(!case.partial_output, "{} permits partial output", case.id);
        let input = decode_hex(&case.input_hex).expect("regression input is lowercase hex");
        assert!(
            input.len() <= MAX_FUZZ_INPUT_BYTES,
            "{} exceeds the input bound",
            case.id
        );
        let first = replay(&context, &input, &case.id);
        let second = replay(&context, &input, &case.id);
        assert_eq!(first, second, "{} is not deterministic", case.id);
        match (case.expected, first) {
            (Expected::Failure { stage }, Outcome::Failure(actual)) => {
                assert_eq!(
                    actual,
                    stage.actual(),
                    "{} failed at the wrong stage",
                    case.id
                );
            }
            (Expected::Valid { woff2_sha256 }, Outcome::Valid(actual)) => {
                assert_eq!(
                    encode_hex(&actual),
                    woff2_sha256,
                    "{} produced different validated bytes",
                    case.id
                );
            }
            (Expected::Failure { .. }, actual) | (Expected::Valid { .. }, actual) => {
                panic!("{} returned the wrong outcome: {actual:?}", case.id);
            }
        }
    }
    assert_eq!(
        risks,
        REQUIRED_RISKS.into_iter().collect(),
        "the regression corpus does not cover every required risk"
    );
}

fn assert_public_facade_cases(cases: &[PublicFacadeCase]) {
    assert!(!cases.is_empty());
    for case in cases {
        let scene =
            crate::SceneDocument::parse(case.scene_json.as_bytes()).unwrap_or_else(|error| {
                panic!("{} public-facade scene failed to parse: {error}", case.id)
            });
        let request = crate::RenderRequest::builder(crate::OutputFormat::Svg)
            .build()
            .expect("public-facade SVG request is valid");
        let first = crate::Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("{} public-facade render failed: {error}", case.id));
        let second = crate::Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("{} repeated render failed: {error}", case.id));
        assert_eq!(
            first.artifact().bytes(),
            second.artifact().bytes(),
            "{}",
            case.id
        );
        assert_eq!(first.report(), second.report(), "{}", case.id);
        let svg =
            std::str::from_utf8(first.artifact().bytes()).expect("public-facade SVG is UTF-8");
        assert!(svg.contains("svg-font-subset@sha256:"), "{}", case.id);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn replay(context: &Context, input: &[u8], id: &str) -> Outcome {
    std::panic::catch_unwind(|| exercise(context, input)).unwrap_or_else(|_| {
        panic!(
            "{id} input {} panicked instead of returning a typed outcome",
            encode_hex(input)
        )
    })
}

#[cfg(target_arch = "wasm32")]
fn replay(context: &Context, input: &[u8], _id: &str) -> Outcome {
    // A panic becomes a browser-WASM test trap and fails the smoke directly.
    exercise(context, input)
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(value.get(offset..offset + 2)?, 16).ok())
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
