//! Scene document admission: the envelope, malformed input, duplicate members,
//! and the fixed depth, key and number-token ceilings. The structure, input-byte
//! and parser-memory ceilings, which build 64 MiB and million-entry scenes,
//! are `limits_scene_admission`.

use crate::common::parse_limits::assert_parse_limit;
use excalidraw_render::{ErrorCode, ErrorFamily, Measurement, SceneDocument, ValueKind};

const CANONICAL_EMPTY_SCENE: &[u8] =
    br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;

#[test]
fn canonical_version_2_scene_documents_are_admitted() {
    SceneDocument::parse(CANONICAL_EMPTY_SCENE).expect("canonical scene document is admitted");

    let scene_with_opaque_content = br#"{
        "type": "excalidraw",
        "version": 2,
        "elements": [null, 17, {"type": "future-element", "unknown": true}],
        "appState": {"future": [1, 2.0, 3e4]},
        "files": {"broken-record": false},
        "source": "fixture",
        "futureRootMember": {"retained": true}
    }"#;
    SceneDocument::parse(scene_with_opaque_content)
        .expect("nested rendering content is not validated during admission");
}

#[test]
fn admitted_scene_documents_are_cloneable_and_safe_to_share_between_threads() {
    fn assert_clone_send_sync<T: Clone + Send + Sync>() {}
    assert_clone_send_sync::<SceneDocument>();

    let scene = std::sync::Arc::new(
        SceneDocument::parse(CANONICAL_EMPTY_SCENE).expect("canonical scene document is admitted"),
    );
    let other_thread = std::sync::Arc::clone(&scene);

    std::thread::spawn(move || other_thread.clone())
        .join()
        .expect("share admitted scene document with another thread");
}

#[test]
fn noncanonical_scene_envelopes_are_rejected_at_the_offending_or_expected_member() {
    let cases: &[(&[u8], &str)] = &[
        (br#"[]"#, ""),
        (
            br#"{"version":2,"elements":[],"appState":{},"files":{}}"#,
            "/type",
        ),
        (
            br#"{"type":"clipboard","version":2,"elements":[],"appState":{},"files":{}}"#,
            "/type",
        ),
        (
            br#"{"type":"excalidraw","elements":[],"appState":{},"files":{}}"#,
            "/version",
        ),
        (
            br#"{"type":"excalidraw","version":2.0,"elements":[],"appState":{},"files":{}}"#,
            "/version",
        ),
        (
            br#"{"type":"excalidraw","version":2,"elements":null,"appState":{},"files":{}}"#,
            "/elements",
        ),
        (
            br#"{"type":"excalidraw","version":2,"elements":[],"appState":[],"files":{}}"#,
            "/appState",
        ),
        (
            br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":null}"#,
            "/files",
        ),
    ];

    for &(bytes, expected_pointer) in cases {
        let error = SceneDocument::parse(bytes).expect_err("envelope must be rejected");
        let descriptor = error.descriptor();

        assert_eq!(descriptor.family(), ErrorFamily::SceneParse);
        assert_eq!(descriptor.category(), None);
        assert_eq!(descriptor.code(), ErrorCode::SCENE_INVALID_ENVELOPE);
        assert_eq!(descriptor.format(), None);
        assert_eq!(descriptor.element_id(), None);
        assert_eq!(descriptor.json_pointer(), Some(expected_pointer));
        assert_eq!(descriptor.details(), None);
    }
}

#[test]
fn invalid_utf8_and_malformed_json_return_typed_failures_with_reliable_pointers_only() {
    let invalid_utf8 = SceneDocument::parse(&[0xff]).expect_err("invalid UTF-8 must be rejected");
    assert_eq!(invalid_utf8.code(), ErrorCode::SCENE_INVALID_UTF8);
    assert_eq!(invalid_utf8.json_pointer(), None);
    assert_eq!(invalid_utf8.details(), None);
    assert_eq!(
        serde_json::to_string(&invalid_utf8.descriptor()).expect("serialize parse descriptor"),
        concat!(
            "{\"family\":\"scene_parse\",",
            "\"category\":null,",
            "\"code\":\"scene.invalid-utf8\",",
            "\"message\":\"scene document is not valid UTF-8\",",
            "\"format\":null,",
            "\"element_id\":null,",
            "\"json_pointer\":null,",
            "\"details\":null}"
        )
    );

    let malformed_nested_value = br#"{
        "type":"excalidraw","version":2,"elements":[truX],"appState":{},"files":{}
    }"#;
    let error = SceneDocument::parse(malformed_nested_value)
        .expect_err("malformed nested value must be rejected");
    assert_eq!(error.code(), ErrorCode::SCENE_INVALID_JSON);
    assert_eq!(error.json_pointer(), Some("/elements/0"));
    assert_eq!(error.details(), None);

    for malformed_token in [
        &br#"{"type":"excalidraw","version":2,"elements":[truefalse],"appState":{},"files":{}}"#[..],
        &br#"{"type":"excalidraw","version":2,"elements":[01],"appState":{},"files":{}}"#[..],
        &br#"{"type":"excalidraw","version":2,"elements":[{}x],"appState":{},"files":{}}"#[..],
    ] {
        let error = SceneDocument::parse(malformed_token)
            .expect_err("malformed value token must be rejected");
        assert_eq!(error.code(), ErrorCode::SCENE_INVALID_JSON);
        assert_eq!(error.json_pointer(), Some("/elements/0"));
    }

    for missing_array_value in [
        &br#"{"type":"excalidraw","version":2,"elements":[,],"appState":{},"files":{}}"#[..],
        &br#"{"type":"excalidraw","version":2,"elements":[null,],"appState":{},"files":{}}"#[..],
        &br#"{"type":"excalidraw","version":2,"elements":[wat],"appState":{},"files":{}}"#[..],
    ] {
        let error = SceneDocument::parse(missing_array_value)
            .expect_err("array syntax without an established value must be rejected");
        assert_eq!(error.code(), ErrorCode::SCENE_INVALID_JSON);
        assert_eq!(error.json_pointer(), Some("/elements"));
    }

    for invalid_number_suffix in [".", "e", "e+"] {
        let number = format!("{}{}", "1".repeat(128), invalid_number_suffix);
        let scene = format!(
            "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{number}}}"
        );
        let error = SceneDocument::parse(scene.as_bytes())
            .expect_err("invalid conditional number syntax must be rejected as JSON");
        assert_eq!(error.code(), ErrorCode::SCENE_INVALID_JSON);
        assert_eq!(error.json_pointer(), Some("/future"));
    }

    let lone_surrogate = br#"{
        "type":"excalidraw","version":2,"elements":[],"appState":{},"files":{},
        "future":"\uD800"
    }"#;
    let error = SceneDocument::parse(lone_surrogate).expect_err("lone surrogate must be rejected");
    assert_eq!(error.code(), ErrorCode::SCENE_INVALID_JSON);
    assert_eq!(error.json_pointer(), Some("/future"));

    let root_syntax = SceneDocument::parse(br#"{"#).expect_err("unfinished root must be rejected");
    assert_eq!(root_syntax.code(), ErrorCode::SCENE_INVALID_JSON);
    assert_eq!(root_syntax.json_pointer(), Some(""));

    let unknown_root_token =
        SceneDocument::parse(b"wat").expect_err("unknown root token must be rejected");
    assert_eq!(unknown_root_token.code(), ErrorCode::SCENE_INVALID_JSON);
    assert_eq!(unknown_root_token.json_pointer(), None);
}

#[test]
fn duplicate_object_members_are_rejected_at_the_later_member() {
    let duplicate_root_member = br#"{
        "type":"excalidraw","version":2,"elements":[],"appState":{},"files":{},"files":{}
    }"#;
    let error = SceneDocument::parse(duplicate_root_member)
        .expect_err("duplicate root member must be rejected");
    assert_eq!(error.code(), ErrorCode::SCENE_DUPLICATE_OBJECT_KEY);
    assert_eq!(error.json_pointer(), Some("/files"));
    assert_eq!(error.details(), None);

    let escaped_duplicate_member = br#"{
        "type":"excalidraw","version":2,"elements":[],"appState":{},"files":{},
        "future":{"a/b~c":1,"a\/b\u007ec":2}
    }"#;
    let error = SceneDocument::parse(escaped_duplicate_member)
        .expect_err("escape-equivalent duplicate member must be rejected");
    assert_eq!(error.code(), ErrorCode::SCENE_DUPLICATE_OBJECT_KEY);
    assert_eq!(error.json_pointer(), Some("/future/a~1b~0c"));
    assert_eq!(error.details(), None);
}

#[test]
fn fixed_depth_key_and_number_boundaries_are_enforced() {
    let depth_128 = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{}{}}}",
        "[".repeat(127),
        "]".repeat(127)
    );
    SceneDocument::parse(depth_128.as_bytes()).expect("JSON depth 128 is admitted");

    let depth_129 = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{}{}}}",
        "[".repeat(128),
        "]".repeat(128)
    );
    let depth_pointer = format!("/future{}", "/0".repeat(127));
    assert_parse_limit(
        SceneDocument::parse(depth_129.as_bytes()).expect_err("JSON depth 129 is rejected"),
        ErrorCode::SCENE_JSON_DEPTH_EXCEEDED,
        Some(&depth_pointer),
        "max_json_depth",
        128,
        ValueKind::Depth,
        Measurement::Attempted { value: 129 },
    );

    let key_4096 = "k".repeat(4_096);
    let scene = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{{\"{key_4096}\":null}}}}"
    );
    SceneDocument::parse(scene.as_bytes()).expect("4 KiB object key is admitted");

    let key_4097 = "k".repeat(4_097);
    let scene = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{{\"{key_4097}\":null}}}}"
    );
    assert_parse_limit(
        SceneDocument::parse(scene.as_bytes()).expect_err("object key over 4 KiB is rejected"),
        ErrorCode::SCENE_OBJECT_KEY_TOO_LARGE,
        Some("/future"),
        "max_object_key_bytes",
        4_096,
        ValueKind::Bytes,
        Measurement::AtLeast { value: 4_097 },
    );

    let unicode_key_over_limit = format!("{}😀", "k".repeat(4_095));
    let scene = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{{\"{unicode_key_over_limit}\":null}}}}"
    );
    assert_parse_limit(
        SceneDocument::parse(scene.as_bytes())
            .expect_err("the decoded scalar that crosses the key ceiling is rejected"),
        ErrorCode::SCENE_OBJECT_KEY_TOO_LARGE,
        Some("/future"),
        "max_object_key_bytes",
        4_096,
        ValueKind::Bytes,
        Measurement::AtLeast { value: 4_099 },
    );

    let number_128 = "1".repeat(128);
    let scene = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{number_128}}}"
    );
    SceneDocument::parse(scene.as_bytes()).expect("128-byte number token is admitted");

    let number_129 = "1".repeat(129);
    let scene = format!(
        "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{{}},\"files\":{{}},\"future\":{number_129}}}"
    );
    let error =
        SceneDocument::parse(scene.as_bytes()).expect_err("129-byte number token is rejected");
    assert_eq!(
        serde_json::to_string(&error.descriptor()).expect("serialize parse-limit descriptor"),
        concat!(
            "{\"family\":\"scene_parse\",",
            "\"category\":null,",
            "\"code\":\"scene.number-token-too-large\",",
            "\"message\":\"number token exceeds the fixed parser token ceiling: ",
            "needs at least 129 bytes, but max_number_token_bytes is 128\",",
            "\"format\":null,",
            "\"element_id\":null,",
            "\"json_pointer\":\"/future\",",
            "\"details\":{\"kind\":\"parse_limit\",",
            "\"field\":\"max_number_token_bytes\",",
            "\"configured\":128,",
            "\"value_kind\":\"bytes\",",
            "\"measurement\":{\"kind\":\"at_least\",\"value\":129}}}"
        )
    );
    assert_parse_limit(
        error,
        ErrorCode::SCENE_NUMBER_TOKEN_TOO_LARGE,
        Some("/future"),
        "max_number_token_bytes",
        128,
        ValueKind::Bytes,
        Measurement::AtLeast { value: 129 },
    );
}
