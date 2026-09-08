use excalidraw_render::license_bundle;
use sha2::{Digest, Sha256};

const LICENSE_BUNDLE_SHA256: &str =
    "a3236318603e4035951ad166ad827ef9fd3a98d9eeae9996e804aa6048d20077";

#[test]
fn public_license_bundle_returns_the_complete_static_utf8_bundle() {
    let bundle: &'static str = license_bundle();

    assert!(bundle.starts_with("Excalidraw Render License Bundle\n"));
    assert!(bundle.contains("Apache License\n                           Version 2.0"));
    assert!(bundle.contains("Copyright (c) 2020 Excalidraw"));
    assert!(bundle.contains("Copyright (c) 2019 Preet Shihn"));
    assert!(bundle.contains("Copyright (c) 2021 Stephen Ruiz Ltd"));
    assert!(bundle.contains("Copyright (c) 2020 Preet Shihn"));
    assert!(bundle.contains("Copyright (c) 2018 Shannon Miwa"));
    assert!(bundle.contains("SIL OPEN FONT LICENSE Version 1.1"));
    assert!(bundle.contains("Creative Commons Attribution 4.0 International Public License"));
    assert!(bundle.contains("Font Awesome Free 5.15.4 by @fontawesome"));
    assert!(bundle.contains("Excalifont and Virgil provenance is retained together"));
    assert!(bundle.contains("Copyright (c) 2020 LXGW"));
    assert!(bundle.ends_with('\n'));
    assert_eq!(
        format!("{:x}", Sha256::digest(bundle.as_bytes())),
        LICENSE_BUNDLE_SHA256
    );
}
