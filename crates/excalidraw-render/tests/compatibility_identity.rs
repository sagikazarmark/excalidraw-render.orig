use excalidraw_render::{CompatibilityRevision, Renderer, DEFAULT_COMPATIBILITY_REVISION};

const REVISION_1_REFERENCE_PROFILE_DIGEST: &str =
    "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";

#[test]
fn callers_can_select_and_discover_compatibility_revisions() {
    assert_eq!(
        DEFAULT_COMPATIBILITY_REVISION,
        CompatibilityRevision::REVISION_1
    );

    let default_renderer = Renderer::new();
    let revision_1_renderer = Renderer::for_revision(CompatibilityRevision::REVISION_1)
        .expect("compatibility revision 1 is supported");

    assert_eq!(
        Renderer::supported_revisions(),
        &[CompatibilityRevision::REVISION_1]
    );
    assert!(Renderer::supported_revisions()
        .windows(2)
        .all(|revisions| revisions[0] < revisions[1]));

    for renderer in [default_renderer, revision_1_renderer] {
        let identity = renderer.selected_profile_identity();
        assert_eq!(
            identity.compatibility_revision(),
            CompatibilityRevision::REVISION_1
        );
        assert_eq!(identity.excalidraw_release(), "v0.18.1");
        assert_eq!(
            identity.profile_digest(),
            REVISION_1_REFERENCE_PROFILE_DIGEST
        );
    }
}

#[test]
fn unsupported_revisions_return_the_typed_stable_failure() {
    let requested = CompatibilityRevision::new(2);
    let error = Renderer::for_revision(requested).expect_err("revision 2 is unsupported");

    assert_eq!(error.requested_revision(), requested);
    assert_eq!(
        serde_json::to_string(&error.descriptor()).expect("serialize error descriptor"),
        concat!(
            "{\"family\":\"unsupported_revision\",",
            "\"category\":null,",
            "\"code\":\"revision.unsupported\",",
            "\"message\":\"compatibility revision 2 is not supported\",",
            "\"format\":null,",
            "\"element_id\":null,",
            "\"json_pointer\":null,",
            "\"details\":{\"kind\":\"revision\",\"requested\":2}}"
        )
    );
}

#[test]
fn compatibility_revision_is_parseable_and_reference_profile_identity_is_serializable() {
    assert_eq!(
        "1".parse::<CompatibilityRevision>()
            .expect("parse compatibility revision"),
        CompatibilityRevision::REVISION_1
    );
    assert_eq!(
        CompatibilityRevision::from(1_u64),
        CompatibilityRevision::REVISION_1
    );

    let identity = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
        .expect("compatibility revision 1 has a reference profile");
    assert_eq!(
        serde_json::to_string(identity).expect("serialize reference profile identity"),
        format!(
            concat!(
                "{{\"compatibility_revision\":1,",
                "\"excalidraw_release\":\"v0.18.1\",",
                "\"reference_profile_digest\":\"{}\"}}"
            ),
            REVISION_1_REFERENCE_PROFILE_DIGEST
        )
    );
}

#[test]
fn renderer_is_cloneable_and_safe_to_share_between_threads() {
    fn assert_clone_send_sync<T: Clone + Send + Sync>() {}
    assert_clone_send_sync::<Renderer>();

    let renderer = std::sync::Arc::new(Renderer::new());
    let other_thread = std::sync::Arc::clone(&renderer);
    let selected_revision = std::thread::spawn(move || {
        other_thread
            .selected_profile_identity()
            .compatibility_revision()
    })
    .join()
    .expect("read renderer identity on another thread");

    assert_eq!(selected_revision, DEFAULT_COMPATIBILITY_REVISION);
    assert_eq!(
        renderer
            .clone()
            .selected_profile_identity()
            .compatibility_revision(),
        DEFAULT_COMPATIBILITY_REVISION
    );
}
