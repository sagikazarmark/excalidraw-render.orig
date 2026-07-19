use excalidraw_render_compare::{CanonicalRgba8, RasterError};

#[test]
fn canonical_raster_validates_layout_and_normalizes_invisible_rgb() {
    assert_eq!(
        CanonicalRgba8::new(0, 1, Vec::new()),
        Err(RasterError::ZeroDimension)
    );
    assert_eq!(
        CanonicalRgba8::new(2, 1, vec![0; 4]),
        Err(RasterError::InvalidBufferLength {
            expected: 8,
            actual: 4,
        })
    );

    let raster = CanonicalRgba8::new(1, 1, vec![200, 100, 50, 0]).unwrap();
    assert_eq!(raster.rgba(), &[0, 0, 0, 0]);
}

#[test]
fn canonical_raster_projects_normative_alpha_black_and_white_vectors() {
    let raster = CanonicalRgba8::new(
        4,
        1,
        vec![
            200, 100, 50, 0, // invisible RGB is canonicalized before projection
            255, 128, 1, 1, 10, 20, 30, 128, 255, 254, 253, 255,
        ],
    )
    .unwrap();

    let views = raster.views();
    assert_eq!(
        views.alpha(),
        &[0, 0, 0, 255, 1, 1, 1, 255, 128, 128, 128, 255, 255, 255, 255, 255,]
    );
    assert_eq!(
        views.black(),
        &[0, 0, 0, 255, 1, 1, 0, 255, 5, 10, 15, 255, 255, 254, 253, 255,]
    );
    assert_eq!(
        views.white(),
        &[255, 255, 255, 255, 255, 255, 254, 255, 132, 137, 142, 255, 255, 254, 253, 255,]
    );
}
