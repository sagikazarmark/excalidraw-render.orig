use crate::compatibility::{CompatibilityRevision, ReferenceProfileIdentity};

#[derive(Debug)]
pub(crate) struct RevisionCatalog {
    profile: ReferenceProfileIdentity,
    assets: &'static excalidraw_render_assets_base::RevisionCatalog,
}

impl RevisionCatalog {
    pub(crate) const fn profile(&'static self) -> &'static ReferenceProfileIdentity {
        &self.profile
    }

    pub(crate) const fn asset_catalog(
        &'static self,
    ) -> &'static excalidraw_render_assets_base::RevisionCatalog {
        self.assets
    }
}

pub(crate) static REVISION_1: RevisionCatalog = RevisionCatalog {
    profile: ReferenceProfileIdentity::new(
        CompatibilityRevision::REVISION_1,
        excalidraw_render_assets_base::revision_1().excalidraw_release,
        excalidraw_render_assets_base::revision_1().reference_profile_digest,
    ),
    assets: excalidraw_render_assets_base::revision_1(),
};

pub(crate) const SUPPORTED_REVISIONS: &[CompatibilityRevision] =
    &[CompatibilityRevision::REVISION_1];

pub(crate) fn for_revision(revision: CompatibilityRevision) -> Option<&'static RevisionCatalog> {
    match revision {
        CompatibilityRevision::REVISION_1 => Some(&REVISION_1),
        _ => None,
    }
}
