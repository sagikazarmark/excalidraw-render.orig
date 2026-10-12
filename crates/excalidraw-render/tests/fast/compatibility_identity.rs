use excalidraw_render::{
    CompatibilityRevision, FrameBatchError, FrameRef, FrameRefs, FrameRenderCursor,
    FrameRenderEntry, FrameRenderRequest, RenderError, RenderOutcome, RenderRequest, Renderer,
    RequestError, SceneDocument, SceneParseError,
};

const REVISION_1_REFERENCE_PROFILE_DIGEST: &str =
    "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";

#[test]
fn a_renderer_exposes_the_reference_profile_it_reproduces() {
    for renderer in [Renderer::new(), Renderer::default()] {
        let identity = renderer.profile_identity();
        assert_eq!(
            identity.compatibility_revision(),
            CompatibilityRevision::REVISION_1
        );
        assert_eq!(identity.compatibility_revision().get(), 1);
        assert_eq!(identity.excalidraw_release(), "v0.18.1");
        assert_eq!(
            identity.reference_profile_digest(),
            REVISION_1_REFERENCE_PROFILE_DIGEST
        );

        let frame = renderer.frame_profile_identity();
        assert_eq!(
            frame.compatibility_revision(),
            identity.compatibility_revision()
        );
        assert_eq!(frame.excalidraw_release(), identity.excalidraw_release());
        assert_ne!(
            frame.reference_profile_digest(),
            identity.reference_profile_digest()
        );
    }
}

#[test]
fn reference_profile_identity_is_serializable() {
    assert_eq!(CompatibilityRevision::REVISION_1.to_string(), "1");
    assert_eq!(
        serde_json::to_string(Renderer::new().profile_identity())
            .expect("serialize reference profile identity"),
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

/// Servers share one renderer and one admitted scene across worker threads,
/// so every value a render consumes or returns must cross threads.
#[test]
fn renderer_scenes_requests_and_results_are_safe_to_share_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_clone<T: Clone>() {}
    assert_send_sync::<Renderer>();
    assert_send_sync::<SceneDocument>();
    assert_send_sync::<RenderRequest>();
    assert_send_sync::<FrameRenderRequest>();
    assert_send_sync::<RenderOutcome>();
    assert_send_sync::<RenderError>();
    assert_clone::<Renderer>();
    assert_clone::<SceneDocument>();

    let renderer = std::sync::Arc::new(Renderer::new());
    let other_thread = std::sync::Arc::clone(&renderer);
    let revision =
        std::thread::spawn(move || other_thread.profile_identity().compatibility_revision())
            .join()
            .expect("read renderer identity on another thread");

    assert_eq!(revision, CompatibilityRevision::REVISION_1);
    assert_eq!(
        renderer.clone().profile_identity().compatibility_revision(),
        CompatibilityRevision::REVISION_1
    );
}

/// Every failure can be boxed into `Box<dyn Error + Send + Sync>` and sent to
/// another thread, and a frame batch's borrowed views and its cursor can be
/// handed to a worker for as long as the scene and request they borrow.
#[test]
fn errors_and_frame_batch_values_are_safe_to_send_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_thread_safe_error<T: std::error::Error + Send + Sync + 'static>() {}
    fn assert_borrowed_frame_values<'scene, 'request>(
        _: std::marker::PhantomData<(&'scene (), &'request ())>,
    ) {
        assert_send_sync::<FrameRef<'scene>>();
        assert_send_sync::<FrameRefs<'scene>>();
        assert_send_sync::<FrameRenderEntry<'scene>>();
        assert_send_sync::<FrameRenderCursor<'scene, 'request>>();
    }
    assert_thread_safe_error::<SceneParseError>();
    assert_thread_safe_error::<RequestError>();
    assert_thread_safe_error::<RenderError>();
    assert_thread_safe_error::<FrameBatchError>();
    assert_borrowed_frame_values(std::marker::PhantomData);
}
