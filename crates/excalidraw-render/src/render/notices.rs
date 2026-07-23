pub(super) struct MandatoryNotices(String);

pub(super) enum NoticeError {
    InvalidProvenance,
    Memory,
}

impl MandatoryNotices {
    pub(super) fn from_json(json: String) -> Self {
        Self(json)
    }

    pub(super) fn as_json(&self) -> &str {
        &self.0
    }

    pub(super) fn for_fallbacks(
        fallback_kinds: &[&str],
        format: crate::OutputFormat,
    ) -> Result<Self, NoticeError> {
        let notices = crate::catalog::manifested_fallback_notices(fallback_kinds, format)
            .map_err(|_| NoticeError::InvalidProvenance)?;
        if notices.is_empty() {
            return Err(NoticeError::InvalidProvenance);
        }
        let mut manifest = String::new();
        manifest
            .try_reserve_exact(1_024)
            .map_err(|_| NoticeError::Memory)?;
        manifest.push('[');
        let mut separator = "";
        for notice in &notices {
            manifest.push_str(separator);
            manifest.push_str("{\"asset_identity\":\"");
            manifest.push_str(&notice.asset_identity);
            manifest.push_str("\",\"text\":\"");
            super::text::push_json_string_content(&mut manifest, &notice.text);
            manifest.push_str("\"}");
            separator = ",";
        }
        manifest.push(']');
        Ok(Self(manifest))
    }
}
