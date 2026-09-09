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
            push_json_string_content(&mut manifest, &notice.text);
            manifest.push_str("\"}");
            separator = ",";
        }
        manifest.push(']');
        Ok(Self(manifest))
    }
}

pub(super) fn comic_shanns_notice() -> Option<&'static str> {
    const START: &str = "Comic Shanns MIT license\n==============================================================================\n\n";
    const END: &str = "\n\n==============================================================================\nSIL Open Font License 1.1";
    let bundle = excalidraw_render_assets_base::license_bundle();
    let start = bundle.find(START)?.checked_add(START.len())?;
    let tail = bundle.get(start..)?;
    tail.get(..tail.find(END)?)
}

pub(super) fn liberation_sans_notice() -> Option<&'static str> {
    const START: &str = "liberation-sans.ofl-full\n";
    const END: &str = "\n\nnoto-emoji.ofl-full\n";
    let bundle = excalidraw_render_assets_base::license_bundle();
    let start = bundle.find(START)?.checked_add(START.len())?;
    let tail = bundle.get(start..)?;
    let end = tail.find(END)?;
    tail.get(..end)
}

pub(super) fn noto_emoji_notice() -> Option<&'static str> {
    const START: &str = "noto-emoji.ofl-full\n";
    let bundle = excalidraw_render_assets_base::license_bundle();
    let start = bundle.find(START)?.checked_add(START.len())?;
    bundle.get(start..)
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
