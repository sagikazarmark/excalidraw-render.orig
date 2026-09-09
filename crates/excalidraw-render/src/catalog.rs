use std::collections::TryReserveError;

use excalidraw_render_assets_base::FontNotice;

use crate::compatibility::{CompatibilityRevision, ReferenceProfileIdentity};
use crate::scene_surface::SceneNoticePolicy;
use crate::OutputFormat;

#[derive(Debug)]
pub(crate) struct RevisionCatalog {
    profile: ReferenceProfileIdentity,
    frame_profile: ReferenceProfileIdentity,
    assets: &'static excalidraw_render_assets_base::RevisionCatalog,
    scene_surface: &'static crate::scene_surface::SceneSurfacePolicy,
}

impl RevisionCatalog {
    pub(crate) const fn profile(&'static self) -> &'static ReferenceProfileIdentity {
        &self.profile
    }

    pub(crate) const fn frame_profile(&'static self) -> &'static ReferenceProfileIdentity {
        &self.frame_profile
    }

    pub(crate) const fn asset_catalog(
        &'static self,
    ) -> &'static excalidraw_render_assets_base::RevisionCatalog {
        self.assets
    }

    pub(crate) const fn scene_surface(
        &'static self,
    ) -> &'static crate::scene_surface::SceneSurfacePolicy {
        self.scene_surface
    }
}

pub(crate) static REVISION_1: RevisionCatalog = RevisionCatalog {
    profile: ReferenceProfileIdentity::new(
        CompatibilityRevision::REVISION_1,
        excalidraw_render_assets_base::revision_1().excalidraw_release,
        excalidraw_render_assets_base::revision_1().reference_profile_digest,
    ),
    frame_profile: ReferenceProfileIdentity::new(
        CompatibilityRevision::REVISION_1,
        excalidraw_render_assets_base::revision_1().excalidraw_release,
        "sha256:63bf16ee197e7c6e79f917988e04aafec4c89c39de87b08ad10dd19712cde77e",
    ),
    assets: excalidraw_render_assets_base::revision_1(),
    scene_surface: &crate::scene_surface::SCENE_SURFACE_POLICY,
};

pub(crate) const SUPPORTED_REVISIONS: &[CompatibilityRevision] =
    &[CompatibilityRevision::REVISION_1];

pub(crate) fn for_revision(revision: CompatibilityRevision) -> Option<&'static RevisionCatalog> {
    match revision {
        CompatibilityRevision::REVISION_1 => Some(&REVISION_1),
        _ => None,
    }
}

/// The manifested notices every artifact of `format` that paints a fallback
/// recipe of one of `fallback_kinds` must carry, ordered by asset identity.
pub(crate) fn fallback_notices(
    fallback_kinds: &[&str],
    format: OutputFormat,
) -> Result<Vec<&'static SceneNoticePolicy>, TryReserveError> {
    let policy = REVISION_1.scene_surface();
    let mut notices = Vec::new();
    notices.try_reserve_exact(policy.notices.len())?;
    for notice in policy.notices {
        let owed = policy.fallback_recipes.iter().any(|recipe| {
            fallback_kinds.contains(&recipe.content_kind)
                && recipe.paints(format)
                && recipe.notices.contains(&notice.identity)
        });
        if owed && notice.requires(format) {
            notices.push(notice);
        }
    }
    notices.sort_unstable_by_key(|notice| notice.asset_identity);
    Ok(notices)
}

/// The notice covering the official rendering font `asset_identity` names.
pub(crate) fn font_notice(asset_identity: &str) -> Option<&'static FontNotice> {
    let assets = REVISION_1.asset_catalog();
    let record = assets.asset(asset_identity)?;
    assets.font_notice(record.family)
}

#[cfg(test)]
mod tests {
    use excalidraw_render_assets_base::FontLicense;

    use super::{fallback_notices, font_notice};
    use crate::OutputFormat;

    const IMAGE_NOTICE_ASSET: &str = "font-awesome-free-5.15.4/solid/image@sha256:f3a4dff85d862e91501af10000d758898ca2bc3055ea451423ddedac7f41c1d9";
    const BAN_NOTICE_ASSET: &str = "font-awesome-free-5.15.4/solid/ban@sha256:8b43245cb319c865fdc4ec865967bcd8b4e833c5b9e108adad750d3db3b64ca9";

    /// A PNG that paints the pending-image placeholder owes the "Image" icon
    /// notice; one that also paints the error placeholder owes the "Ban" icon
    /// notice too, ordered by asset identity. SVG and PDF omit both official
    /// image recipes, so they owe nothing for them.
    #[test]
    fn painted_fallback_recipes_select_their_notices_per_format() {
        let pending = fallback_notices(&["image"], OutputFormat::Png).unwrap();
        assert_eq!(
            pending
                .iter()
                .map(|notice| notice.asset_identity)
                .collect::<Vec<_>>(),
            [IMAGE_NOTICE_ASSET]
        );
        assert!(pending[0].text.contains("\"Image\" icon source"));

        let error = fallback_notices(&["image", "image-error"], OutputFormat::Png).unwrap();
        assert_eq!(
            error
                .iter()
                .map(|notice| notice.asset_identity)
                .collect::<Vec<_>>(),
            [BAN_NOTICE_ASSET, IMAGE_NOTICE_ASSET]
        );

        for format in [OutputFormat::Svg, OutputFormat::Pdf] {
            assert!(fallback_notices(&["image", "image-error"], format)
                .unwrap()
                .is_empty());
        }
        assert!(fallback_notices(&["unknown-kind"], OutputFormat::Png)
            .unwrap()
            .is_empty());
    }

    /// Every asset identity resolves to the attribution its family's bundle
    /// line states, without the license sentence, and to the terms the
    /// artifact carries with it.
    #[test]
    fn font_notices_resolve_by_asset_identity() {
        let cascadia = font_notice(
            "fonts/Cascadia/CascadiaCode-Regular.woff2@sha256:5bce041e0fcce617e35efbc335b1372d27a84acb65fb464c65ff80ab3424a885",
        )
        .expect("Cascadia is catalogued");
        assert_eq!(cascadia.license_identity, "font.cascadia-code.ofl-1.1");
        assert_eq!(cascadia.license, FontLicense::Ofl11);
        assert_eq!(
            cascadia.attribution,
            "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code."
        );
        assert!(cascadia.terms.starts_with(
            "This Font Software is licensed under the SIL Open Font License, Version 1.1."
        ));
        assert!(font_notice("fonts/Cascadia/CascadiaCode-Regular.woff2").is_none());
    }
}
