use crate::compatibility::{CompatibilityRevision, ReferenceProfileIdentity};
use crate::OutputFormat;

pub(crate) struct ManifestedNotice {
    pub(crate) asset_identity: String,
    pub(crate) text: String,
}

pub(crate) struct ManifestedFontAttribution {
    pub(crate) license_identity: String,
    pub(crate) text: String,
}

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

pub(crate) fn manifested_fallback_notices(
    fallback_kinds: &[&str],
    format: OutputFormat,
) -> Result<Vec<ManifestedNotice>, ()> {
    let manifested = scene_surface_manifest()?;
    let recipes = manifested["fallbackRecipes"].as_array().ok_or(())?;
    let mut notice_ids = Vec::new();
    for recipe in recipes {
        let kind = recipe["content"]["kind"].as_str().ok_or(())?;
        if !fallback_kinds.contains(&kind)
            || recipe["content"]["format"][format.as_str()] != "paint"
        {
            continue;
        }
        for notice_id in recipe["noticeIds"].as_array().ok_or(())? {
            let notice_id = notice_id.as_str().ok_or(())?;
            if !notice_ids.contains(&notice_id) {
                notice_ids.push(notice_id);
            }
        }
    }
    manifested_notices_from(&manifested, &notice_ids, format)
}

pub(crate) fn manifested_font_attribution(
    asset_identity: &str,
) -> Result<ManifestedFontAttribution, ()> {
    let catalog: serde_json::Value =
        serde_json::from_slice(REVISION_1.asset_catalog().catalog_json).map_err(|_| ())?;
    let asset = catalog["assets"]
        .as_array()
        .ok_or(())?
        .iter()
        .find(|asset| asset["identity"] == asset_identity)
        .ok_or(())?;
    let heading = match asset["family"].as_str().ok_or(())? {
        "Cascadia" => "Cascadia Code",
        "ComicShanns" => "Comic Shanns",
        "Excalifont" => "Excalifont and Virgil provenance",
        "Liberation" => "Liberation Sans",
        "Lilita" => "Lilita",
        "Nunito" => "Nunito",
        "Virgil" => "Virgil",
        "Xiaolai" => "Xiaolai SC",
        _ => return Err(()),
    };
    let line = excalidraw_render_assets_base::license_bundle()
        .split_once("Official rendering font notices\n")
        .and_then(|(_, notices)| notices.split_once("Revision 1 output-traveling notices"))
        .and_then(|(notices, _)| notices.split_once(&format!("\n{heading}\n")))
        .and_then(|(_, notice)| notice.lines().next())
        .ok_or(())?;
    Ok(ManifestedFontAttribution {
        license_identity: asset["licenseIdentity"].as_str().ok_or(())?.to_owned(),
        text: line
            .replace(
                " Licensed under the SIL Open Font License, Version 1.1.",
                "",
            )
            .replace(
                " Licensed under the MIT License included in this bundle.",
                "",
            ),
    })
}

fn scene_surface_manifest() -> Result<serde_json::Value, ()> {
    let scene_surface = REVISION_1
        .asset_catalog()
        .incorporated_artifacts
        .iter()
        .find(|artifact| artifact.repository_path == "tests/oracle/scene-surfaces/revision-1.json")
        .ok_or(())?;
    serde_json::from_slice(scene_surface.bytes).map_err(|_| ())
}

fn manifested_notices_from(
    manifested: &serde_json::Value,
    notice_ids: &[&str],
    format: OutputFormat,
) -> Result<Vec<ManifestedNotice>, ()> {
    let catalog = manifested["noticeCatalog"].as_array().ok_or(())?;
    let mut notices = Vec::new();
    notices
        .try_reserve_exact(notice_ids.len())
        .map_err(|_| ())?;
    for notice in catalog {
        let id = notice["id"].as_str().ok_or(())?;
        if !notice_ids.contains(&id) {
            continue;
        }
        let required_formats = notice["requiredFormats"].as_array().ok_or(())?;
        if !required_formats
            .iter()
            .any(|required| required == format.as_str())
        {
            continue;
        }
        notices.push(ManifestedNotice {
            asset_identity: notice["assetIdentity"].as_str().ok_or(())?.to_owned(),
            text: notice["text"].as_str().ok_or(())?.to_owned(),
        });
    }
    notices.sort_unstable_by(|left, right| left.asset_identity.cmp(&right.asset_identity));
    Ok(notices)
}
