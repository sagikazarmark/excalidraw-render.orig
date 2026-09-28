use crate::common::lower_hex;
use excalidraw_render::license_bundle;
use sha2::{Digest, Sha256};

const LICENSE_BUNDLE_SHA256: &str =
    "cadd243b2228954efb8f61f62d492bcde65a23f8e7e96a2db5a6cc42ce78a9f2";

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
    assert!(bundle.contains(include_str!(
        "../../../excalidraw-fonts/license-parts/skia-BSD-3-Clause.txt"
    )));
    assert!(bundle.ends_with('\n'));
    assert_eq!(
        lower_hex(&Sha256::digest(bundle.as_bytes())),
        LICENSE_BUNDLE_SHA256
    );
}
