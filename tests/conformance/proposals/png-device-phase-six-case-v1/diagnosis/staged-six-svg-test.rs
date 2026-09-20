
#[test]
fn six_case_proposal_preserves_svg_geometry_raster_and_168_pin() {
    let selected = ["focused.embeddable-links.v1", "focused.emoji-fallback.v1", "focused.bidi-text.v1", "focused.bidi-alignment.v1", "focused.text-line-endings.v1", "focused.text-alignment-vertical.v1"];
    let variants: Vec<_> = registered_scene_variants(&repository_root()).into_iter().filter(|v| selected.contains(&v.case.as_str())).collect();
    assert_eq!(variants.len(), 6);
    for variant in variants {
        let errors = svg_divergences(&variant);
        assert!(errors.is_empty(), "{}: {:?}", variant.case, errors);
        println!("{}: geometry, SVG raster and direct-PNG dependency gates pass", variant.case);
    }
    let pin = &RASTERIZER_LIMITATIONS[0].pin;
    assert_eq!(pin.case, "focused.bidi-alignment.v1");
    assert_eq!(pin.changed_pixels, 1190);
    assert_eq!(pin.ssim_bits, 0x3fea_564b_9c97_ab4f);
}
