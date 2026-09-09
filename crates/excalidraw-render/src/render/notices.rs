#[derive(Debug)]
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
        let notices = crate::catalog::fallback_notices(fallback_kinds, format)
            .map_err(|_| NoticeError::Memory)?;
        if notices.is_empty() {
            return Err(NoticeError::InvalidProvenance);
        }
        let mut manifest = String::new();
        manifest
            .try_reserve_exact(1_024)
            .map_err(|_| NoticeError::Memory)?;
        manifest.push('[');
        let mut separator = "";
        for notice in notices {
            manifest.push_str(separator);
            manifest.push_str("{\"asset_identity\":\"");
            manifest.push_str(notice.asset_identity);
            manifest.push_str("\",\"text\":\"");
            push_json_string_content(&mut manifest, notice.text);
            manifest.push_str("\"}");
            separator = ",";
        }
        manifest.push(']');
        Ok(Self(manifest))
    }
}

/// The sealed `liberation-sans.ofl-full` notice: the Liberation Sans
/// copyright lines followed by the OFL terms.
pub(super) fn liberation_sans_notice() -> Option<&'static str> {
    manifested_notice("liberation-sans.ofl-full")
}

/// The sealed `noto-emoji.ofl-full` notice: the Noto Emoji copyright line
/// followed by the OFL terms.
pub(super) fn noto_emoji_notice() -> Option<&'static str> {
    manifested_notice("noto-emoji.ofl-full")
}

fn manifested_notice(identity: &str) -> Option<&'static str> {
    crate::catalog::REVISION_1
        .scene_surface()
        .notice(identity)
        .map(|notice| notice.text)
}

pub(super) fn push_json_string_content(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character < '\u{20}' => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let value = character as u32;
                output.push_str("\\u00");
                output.push(char::from(HEX[((value >> 4) & 0x0f) as usize]));
                output.push(char::from(HEX[(value & 0x0f) as usize]));
            }
            character => output.push(character),
        }
    }
}
