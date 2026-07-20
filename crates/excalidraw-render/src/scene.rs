use std::collections::{BTreeSet, TryReserveError};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::{ErrorCode, ErrorDescriptor, ErrorDetails, Measurement, ValueKind};

const MAX_INPUT_BYTES: u64 = 67_108_864;
const MAX_JSON_DEPTH: u64 = 128;
const MAX_JSON_STRUCTURE_ENTRIES: u64 = 1_048_576;
const MAX_TOTAL_STRING_BYTES: u64 = 67_108_864;
const MAX_STRING_BYTES: u64 = 67_108_864;
const MAX_OBJECT_KEY_BYTES: u64 = 4 * 1024;
const MAX_NUMBER_TOKEN_BYTES: u64 = 128;
const MAX_PARSE_WORKING_BYTES: u64 = 256 * 1024 * 1024;

const SCALAR_NODE_BYTES: u64 = 64;
const CONTAINER_NODE_BYTES: u64 = 128;
const ACTIVE_FRAME_BYTES: u64 = 64;
const TOKEN_SCRATCH_BASE_BYTES: u64 = 64;
const ALLOCATION_QUANTUM_BYTES: u64 = 64;

const INPUT_BYTES_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_INPUT_TOO_LARGE,
    message: "scene document exceeds the fixed parser input ceiling",
    field: "max_input_bytes",
    configured: MAX_INPUT_BYTES,
    value_kind: ValueKind::Bytes,
};
const JSON_DEPTH_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_JSON_DEPTH_EXCEEDED,
    message: "JSON nesting exceeds the fixed parser depth ceiling",
    field: "max_json_depth",
    configured: MAX_JSON_DEPTH,
    value_kind: ValueKind::Depth,
};
const JSON_STRUCTURE_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_JSON_STRUCTURE_EXCEEDED,
    message: "JSON structure exceeds the fixed parser entry ceiling",
    field: "max_json_structure_entries",
    configured: MAX_JSON_STRUCTURE_ENTRIES,
    value_kind: ValueKind::Count,
};
const TOTAL_STRING_BYTES_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_STRING_TOO_LARGE,
    message: "decoded strings exceed the fixed aggregate ceiling",
    field: "max_total_string_bytes",
    configured: MAX_TOTAL_STRING_BYTES,
    value_kind: ValueKind::Bytes,
};
const STRING_BYTES_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_STRING_TOO_LARGE,
    message: "decoded string exceeds the fixed per-string ceiling",
    field: "max_string_bytes",
    configured: MAX_STRING_BYTES,
    value_kind: ValueKind::Bytes,
};
const OBJECT_KEY_BYTES_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_OBJECT_KEY_TOO_LARGE,
    message: "object key exceeds the fixed parser key ceiling",
    field: "max_object_key_bytes",
    configured: MAX_OBJECT_KEY_BYTES,
    value_kind: ValueKind::Bytes,
};
const NUMBER_TOKEN_BYTES_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_NUMBER_TOKEN_TOO_LARGE,
    message: "number token exceeds the fixed parser token ceiling",
    field: "max_number_token_bytes",
    configured: MAX_NUMBER_TOKEN_BYTES,
    value_kind: ValueKind::Bytes,
};
const PARSE_WORKING_BYTES_LIMIT: ParserLimit = ParserLimit {
    code: ErrorCode::SCENE_PARSE_MEMORY_EXCEEDED,
    message: "canonical parser live bytes exceed the fixed parser memory ceiling",
    field: "max_parse_working_bytes",
    configured: MAX_PARSE_WORKING_BYTES,
    value_kind: ValueKind::Bytes,
};

#[derive(Clone, Copy)]
struct ParserLimit {
    code: ErrorCode,
    message: &'static str,
    field: &'static str,
    configured: u64,
    value_kind: ValueKind,
}

/// An admitted Excalidraw scene document.
#[derive(Clone)]
pub struct SceneDocument {
    _root: Arc<JsonValue>,
}

impl SceneDocument {
    /// Parses and admits a scene document from UTF-8 JSON bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, SceneParseError> {
        let input_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if input_bytes > MAX_INPUT_BYTES {
            return Err(SceneParseError::limit(
                INPUT_BYTES_LIMIT,
                Measurement::Attempted { value: input_bytes },
                None,
            ));
        }
        if std::str::from_utf8(bytes).is_err() {
            return Err(SceneParseError::new(
                ErrorCode::SCENE_INVALID_UTF8,
                "scene document is not valid UTF-8",
                None,
                None,
            ));
        }

        let mut parser = Parser::new(bytes);
        let root = parser.parse_value(1, false)?;
        parser.skip_whitespace();
        if parser.index != bytes.len() {
            return Err(parser.invalid_json(true));
        }
        validate_envelope(&root)?;

        Ok(Self {
            _root: Arc::new(root),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn canonicalized_original_bytes(&self) -> Result<Vec<u8>, TryReserveError> {
        let mut bytes = Vec::new();
        write_canonical_json(&self._root, &mut bytes)?;
        Ok(bytes)
    }

    pub(crate) fn has_no_element_occurrences(&self) -> bool {
        let JsonValue::Object(members) = &*self._root else {
            return false;
        };
        matches!(
            member_value(members, "elements"),
            Some(JsonValue::Array(elements)) if elements.is_empty()
        )
    }
}

impl fmt::Debug for SceneDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SceneDocument")
            .finish_non_exhaustive()
    }
}

/// Indicates that scene-document bytes could not be admitted.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneParseError {
    code: ErrorCode,
    message: String,
    json_pointer: Option<String>,
    details: Option<Box<ErrorDetails>>,
}

impl SceneParseError {
    fn new(
        code: ErrorCode,
        message: impl Into<String>,
        json_pointer: Option<String>,
        details: Option<ErrorDetails>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            json_pointer,
            details: details.map(Box::new),
        }
    }

    fn invalid_json(json_pointer: Option<String>) -> Self {
        Self::new(
            ErrorCode::SCENE_INVALID_JSON,
            "scene document is not valid JSON",
            json_pointer,
            None,
        )
    }

    fn invalid_envelope(json_pointer: &str) -> Self {
        Self::new(
            ErrorCode::SCENE_INVALID_ENVELOPE,
            "scene document does not have the required version-2 envelope",
            Some(json_pointer.to_owned()),
            None,
        )
    }

    fn limit(limit: ParserLimit, measurement: Measurement, json_pointer: Option<String>) -> Self {
        Self::new(
            limit.code,
            limit.message,
            json_pointer,
            Some(ErrorDetails::ParseLimit {
                field: limit.field,
                configured: limit.configured,
                value_kind: limit.value_kind,
                measurement,
            }),
        )
    }

    /// Returns the stable machine-readable error code.
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the narrowest reliable scene-document JSON pointer.
    pub fn json_pointer(&self) -> Option<&str> {
        self.json_pointer.as_deref()
    }

    /// Returns typed details required by this failure's stable code.
    pub fn details(&self) -> Option<&ErrorDetails> {
        self.details.as_deref()
    }

    /// Returns this failure's stable serializable descriptor.
    pub fn descriptor(&self) -> ErrorDescriptor {
        ErrorDescriptor::scene_parse(
            self.code,
            self.message.clone(),
            self.json_pointer.clone(),
            self.details.as_deref().cloned(),
        )
    }
}

impl fmt::Display for SceneParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SceneParseError {}

#[allow(dead_code)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(Box<str>),
    String(Box<str>),
    Array(Box<[JsonValue]>),
    Object(Box<[JsonMember]>),
}

#[allow(dead_code)]
struct JsonMember {
    key: Arc<str>,
    value: JsonValue,
}

struct ParsedString {
    value: String,
    scratch_bytes: u64,
}

struct Parser<'a> {
    bytes: &'a [u8],
    index: usize,
    path: Vec<String>,
    structure_entries: u64,
    total_string_bytes: u64,
    working_bytes: u64,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            index: 0,
            path: Vec::new(),
            structure_entries: 0,
            total_string_bytes: 0,
            working_bytes: 0,
        }
    }

    fn parse_value(
        &mut self,
        depth: u64,
        pointer_is_reliable: bool,
    ) -> Result<JsonValue, SceneParseError> {
        self.skip_whitespace();
        let pointer = pointer_is_reliable.then(|| self.pointer());

        match self.peek() {
            Some(b'{') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.check_depth(depth, pointer.clone())?;
                self.admit_structure(pointer.clone())?;
                self.acquire(CONTAINER_NODE_BYTES, pointer.clone())?;
                self.parse_object(depth, pointer)
            }
            Some(b'[') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.check_depth(depth, pointer.clone())?;
                self.admit_structure(pointer.clone())?;
                self.acquire(CONTAINER_NODE_BYTES, pointer.clone())?;
                self.parse_array(depth, pointer)
            }
            Some(b'"') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.admit_structure(pointer.clone())?;
                self.acquire(SCALAR_NODE_BYTES, pointer.clone())?;
                let parsed = self.parse_string(false, pointer.clone())?;
                let storage_bytes = 64 + allocated_bytes(parsed.value.len());
                self.acquire(storage_bytes, pointer.clone())?;
                self.release(parsed.scratch_bytes);
                Ok(JsonValue::String(parsed.value.into_boxed_str()))
            }
            Some(b'-' | b'0'..=b'9') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.admit_structure(pointer.clone())?;
                self.acquire(SCALAR_NODE_BYTES, pointer.clone())?;
                let value = self.parse_number(pointer.clone())?;
                Ok(JsonValue::Number(value))
            }
            Some(b't') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.admit_structure(pointer.clone())?;
                self.acquire(SCALAR_NODE_BYTES, pointer.clone())?;
                self.parse_literal(b"true", pointer.clone())?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.admit_structure(pointer.clone())?;
                self.acquire(SCALAR_NODE_BYTES, pointer.clone())?;
                self.parse_literal(b"false", pointer.clone())?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'n') => {
                let pointer = pointer.or_else(|| Some(self.pointer()));
                self.admit_structure(pointer.clone())?;
                self.acquire(SCALAR_NODE_BYTES, pointer.clone())?;
                self.parse_literal(b"null", pointer.clone())?;
                Ok(JsonValue::Null)
            }
            _ => Err(SceneParseError::invalid_json(pointer)),
        }
    }

    fn parse_object(
        &mut self,
        depth: u64,
        object_pointer: Option<String>,
    ) -> Result<JsonValue, SceneParseError> {
        self.index += 1;
        self.acquire(ACTIVE_FRAME_BYTES, object_pointer.clone())?;
        self.skip_whitespace();
        if self.consume(b'}') {
            self.release(ACTIVE_FRAME_BYTES);
            return Ok(JsonValue::Object(Box::new([])));
        }

        let mut members = Vec::new();
        let mut seen = BTreeSet::<Arc<str>>::new();
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(SceneParseError::invalid_json(object_pointer));
            }

            let parsed_key = self.parse_string(true, object_pointer.clone())?;
            let key: Arc<str> = parsed_key.value.into();
            let member_pointer = self.pointer_with(&key);
            self.admit_structure(Some(member_pointer.clone()))?;
            if seen.contains(key.as_ref()) {
                return Err(SceneParseError::new(
                    ErrorCode::SCENE_DUPLICATE_OBJECT_KEY,
                    "scene document contains a duplicate object member",
                    Some(member_pointer),
                    None,
                ));
            }
            self.acquire(
                64 + allocated_bytes(key.len()),
                Some(member_pointer.clone()),
            )?;
            self.release(parsed_key.scratch_bytes);
            seen.insert(Arc::clone(&key));

            self.skip_whitespace();
            if !self.consume(b':') {
                return Err(SceneParseError::invalid_json(Some(member_pointer)));
            }

            self.path.push(key.to_string());
            let value = self.parse_value(depth + 1, true)?;
            self.require_value_delimiter(Some(self.pointer()))?;
            self.path.pop();
            members.push(JsonMember { key, value });

            self.skip_whitespace();
            if self.consume(b'}') {
                self.release(ACTIVE_FRAME_BYTES);
                return Ok(JsonValue::Object(members.into_boxed_slice()));
            }
            if !self.consume(b',') {
                return Err(SceneParseError::invalid_json(object_pointer));
            }
        }
    }

    fn parse_array(
        &mut self,
        depth: u64,
        array_pointer: Option<String>,
    ) -> Result<JsonValue, SceneParseError> {
        self.index += 1;
        self.acquire(ACTIVE_FRAME_BYTES, array_pointer.clone())?;
        self.skip_whitespace();
        if self.consume(b']') {
            self.release(ACTIVE_FRAME_BYTES);
            return Ok(JsonValue::Array(Box::new([])));
        }

        let mut values = Vec::new();
        let mut index = 0_u64;
        loop {
            self.skip_whitespace();
            if !self.next_token_starts_value() {
                return Err(SceneParseError::invalid_json(array_pointer.clone()));
            }
            self.path.push(index.to_string());
            let value = self.parse_value(depth + 1, true)?;
            self.require_value_delimiter(Some(self.pointer()))?;
            self.path.pop();
            values.push(value);

            self.skip_whitespace();
            if self.consume(b']') {
                self.release(ACTIVE_FRAME_BYTES);
                return Ok(JsonValue::Array(values.into_boxed_slice()));
            }
            if !self.consume(b',') {
                return Err(SceneParseError::invalid_json(array_pointer));
            }
            index += 1;
        }
    }

    fn parse_string(
        &mut self,
        is_key: bool,
        pointer: Option<String>,
    ) -> Result<ParsedString, SceneParseError> {
        self.index += 1;
        self.acquire(TOKEN_SCRATCH_BASE_BYTES, pointer.clone())?;
        let mut scratch_bytes = TOKEN_SCRATCH_BASE_BYTES;
        let mut value = String::new();

        loop {
            let byte = self
                .peek()
                .ok_or_else(|| SceneParseError::invalid_json(pointer.clone()))?;
            match byte {
                b'"' => {
                    self.index += 1;
                    return Ok(ParsedString {
                        value,
                        scratch_bytes,
                    });
                }
                b'\\' => {
                    self.index += 1;
                    let escape = self
                        .peek()
                        .ok_or_else(|| SceneParseError::invalid_json(pointer.clone()))?;
                    self.index += 1;
                    let character = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.parse_unicode_escape(pointer.clone())?,
                        _ => return Err(SceneParseError::invalid_json(pointer)),
                    };
                    self.push_string_character(
                        &mut value,
                        character,
                        is_key,
                        &mut scratch_bytes,
                        pointer.clone(),
                    )?;
                }
                0x00..=0x1f => return Err(SceneParseError::invalid_json(pointer)),
                _ => {
                    let start = self.index;
                    let run_limit = self.direct_string_run_limit(value.len(), is_key);
                    while self.index - start < run_limit
                        && matches!(self.peek(), Some(byte) if byte >= 0x20 && byte != b'"' && byte != b'\\')
                    {
                        self.index += 1;
                    }
                    while matches!(self.peek(), Some(0x80..=0xbf)) {
                        self.index += 1;
                    }
                    self.push_string_source_run(
                        &mut value,
                        start,
                        self.index,
                        is_key,
                        &mut scratch_bytes,
                        pointer.clone(),
                    )?;
                }
            }
        }
    }

    fn direct_string_run_limit(&self, current_string_bytes: usize, is_key: bool) -> usize {
        let current_string_bytes =
            u64::try_from(current_string_bytes).expect("parser input length fits in u64");
        let mut limit = MAX_STRING_BYTES - current_string_bytes + 1;
        limit = limit.min(MAX_TOTAL_STRING_BYTES - self.total_string_bytes + 1);
        if is_key {
            limit = limit.min(MAX_OBJECT_KEY_BYTES - current_string_bytes + 1);
        }
        usize::try_from(limit).expect("fixed parser string limits fit in usize")
    }

    fn parse_unicode_escape(&mut self, pointer: Option<String>) -> Result<char, SceneParseError> {
        let first = self.parse_hex_quad(pointer.clone())?;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            if self.bytes.get(self.index..self.index + 2) != Some(b"\\u") {
                return Err(SceneParseError::invalid_json(pointer));
            }
            self.index += 2;
            let second = self.parse_hex_quad(pointer.clone())?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(SceneParseError::invalid_json(pointer));
            }
            0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(SceneParseError::invalid_json(pointer));
        } else {
            u32::from(first)
        };

        char::from_u32(scalar).ok_or_else(|| SceneParseError::invalid_json(pointer))
    }

    fn parse_hex_quad(&mut self, pointer: Option<String>) -> Result<u16, SceneParseError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(hex_value)
                .ok_or_else(|| SceneParseError::invalid_json(pointer.clone()))?;
            self.index += 1;
            value = value * 16 + u16::from(digit);
        }
        Ok(value)
    }

    fn push_string_character(
        &mut self,
        value: &mut String,
        character: char,
        is_key: bool,
        scratch_bytes: &mut u64,
        pointer: Option<String>,
    ) -> Result<(), SceneParseError> {
        self.admit_decoded_string_bytes(
            value.len(),
            character.len_utf8(),
            is_key,
            scratch_bytes,
            pointer,
        )?;
        value.push(character);
        Ok(())
    }

    fn push_string_source_run(
        &mut self,
        value: &mut String,
        start: usize,
        end: usize,
        is_key: bool,
        scratch_bytes: &mut u64,
        pointer: Option<String>,
    ) -> Result<(), SceneParseError> {
        self.admit_decoded_string_bytes(value.len(), end - start, is_key, scratch_bytes, pointer)?;
        value.push_str(
            std::str::from_utf8(&self.bytes[start..end])
                .expect("input UTF-8 was validated before parsing"),
        );
        Ok(())
    }

    fn admit_decoded_string_bytes(
        &mut self,
        current_string_bytes: usize,
        added_bytes: usize,
        is_key: bool,
        scratch_bytes: &mut u64,
        pointer: Option<String>,
    ) -> Result<(), SceneParseError> {
        let decoded_bytes = u64::try_from(current_string_bytes + added_bytes)
            .expect("parser input length fits in u64");

        if is_key && decoded_bytes > MAX_OBJECT_KEY_BYTES {
            return Err(SceneParseError::limit(
                OBJECT_KEY_BYTES_LIMIT,
                Measurement::AtLeast {
                    value: decoded_bytes,
                },
                pointer,
            ));
        }
        if decoded_bytes > MAX_STRING_BYTES {
            return Err(SceneParseError::limit(
                STRING_BYTES_LIMIT,
                Measurement::AtLeast {
                    value: decoded_bytes,
                },
                pointer,
            ));
        }

        let added_bytes = u64::try_from(added_bytes).expect("parser input length fits in u64");
        let total_string_bytes = self.total_string_bytes + added_bytes;
        if total_string_bytes > MAX_TOTAL_STRING_BYTES {
            return Err(SceneParseError::limit(
                TOTAL_STRING_BYTES_LIMIT,
                Measurement::AtLeast {
                    value: total_string_bytes,
                },
                pointer,
            ));
        }

        let next_scratch_bytes = TOKEN_SCRATCH_BASE_BYTES + allocated_bytes(decoded_bytes as usize);
        let scratch_growth = next_scratch_bytes - *scratch_bytes;
        if self.working_bytes.saturating_add(scratch_growth) > MAX_PARSE_WORKING_BYTES {
            let first_crossing_growth =
                ((MAX_PARSE_WORKING_BYTES - self.working_bytes) / ALLOCATION_QUANTUM_BYTES + 1)
                    * ALLOCATION_QUANTUM_BYTES;
            return Err(SceneParseError::limit(
                PARSE_WORKING_BYTES_LIMIT,
                Measurement::Attempted {
                    value: self.working_bytes + first_crossing_growth,
                },
                pointer,
            ));
        }
        self.working_bytes += scratch_growth;
        *scratch_bytes = next_scratch_bytes;
        self.total_string_bytes = total_string_bytes;
        Ok(())
    }

    fn parse_number(&mut self, pointer: Option<String>) -> Result<Box<str>, SceneParseError> {
        let start = self.index;
        self.acquire(TOKEN_SCRATCH_BASE_BYTES, pointer.clone())?;
        let mut token_bytes = 0_u64;
        let mut scratch_bytes = TOKEN_SCRATCH_BASE_BYTES;

        if self.peek() == Some(b'-') {
            self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
        }

        match self.peek() {
            Some(b'0') => {
                self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.consume_number_byte(
                        &mut token_bytes,
                        &mut scratch_bytes,
                        pointer.clone(),
                    )?;
                }
            }
            _ => return Err(SceneParseError::invalid_json(pointer)),
        }

        if self.peek() == Some(b'.') {
            if !matches!(self.bytes.get(self.index + 1), Some(b'0'..=b'9')) {
                return Err(SceneParseError::invalid_json(pointer));
            }
            self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            let mut first_exponent_digit = self.index + 1;
            if matches!(self.bytes.get(first_exponent_digit), Some(b'+' | b'-')) {
                first_exponent_digit += 1;
            }
            if !matches!(self.bytes.get(first_exponent_digit), Some(b'0'..=b'9')) {
                return Err(SceneParseError::invalid_json(pointer));
            }
            self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.consume_number_byte(&mut token_bytes, &mut scratch_bytes, pointer.clone())?;
            }
        }

        self.acquire(64 + allocated_bytes(token_bytes as usize), pointer)?;
        self.release(scratch_bytes);
        Ok(std::str::from_utf8(&self.bytes[start..self.index])
            .expect("JSON number tokens are ASCII")
            .into())
    }

    fn consume_number_byte(
        &mut self,
        token_bytes: &mut u64,
        scratch_bytes: &mut u64,
        pointer: Option<String>,
    ) -> Result<(), SceneParseError> {
        self.index += 1;
        *token_bytes += 1;
        if *token_bytes > MAX_NUMBER_TOKEN_BYTES {
            return Err(SceneParseError::limit(
                NUMBER_TOKEN_BYTES_LIMIT,
                Measurement::AtLeast {
                    value: *token_bytes,
                },
                pointer,
            ));
        }

        let next_scratch_bytes = TOKEN_SCRATCH_BASE_BYTES + allocated_bytes(*token_bytes as usize);
        self.acquire(next_scratch_bytes - *scratch_bytes, pointer)?;
        *scratch_bytes = next_scratch_bytes;
        Ok(())
    }

    fn parse_literal(
        &mut self,
        expected: &[u8],
        pointer: Option<String>,
    ) -> Result<(), SceneParseError> {
        if self.bytes.get(self.index..self.index + expected.len()) != Some(expected) {
            return Err(SceneParseError::invalid_json(pointer));
        }
        self.index += expected.len();
        Ok(())
    }

    fn require_value_delimiter(&self, pointer: Option<String>) -> Result<(), SceneParseError> {
        if matches!(
            self.peek(),
            None | Some(b' ' | b'\t' | b'\n' | b'\r' | b',' | b']' | b'}')
        ) {
            Ok(())
        } else {
            Err(SceneParseError::invalid_json(pointer))
        }
    }

    fn check_depth(&self, depth: u64, pointer: Option<String>) -> Result<(), SceneParseError> {
        if depth <= MAX_JSON_DEPTH {
            return Ok(());
        }
        Err(SceneParseError::limit(
            JSON_DEPTH_LIMIT,
            Measurement::Attempted { value: depth },
            pointer,
        ))
    }

    fn admit_structure(&mut self, pointer: Option<String>) -> Result<(), SceneParseError> {
        let attempted = self.structure_entries + 1;
        if attempted > MAX_JSON_STRUCTURE_ENTRIES {
            return Err(SceneParseError::limit(
                JSON_STRUCTURE_LIMIT,
                Measurement::Attempted { value: attempted },
                pointer,
            ));
        }
        self.structure_entries = attempted;
        Ok(())
    }

    fn acquire(&mut self, bytes: u64, pointer: Option<String>) -> Result<(), SceneParseError> {
        let attempted = self.working_bytes.saturating_add(bytes);
        if attempted > MAX_PARSE_WORKING_BYTES {
            return Err(SceneParseError::limit(
                PARSE_WORKING_BYTES_LIMIT,
                Measurement::Attempted { value: attempted },
                pointer,
            ));
        }
        self.working_bytes = attempted;
        Ok(())
    }

    fn release(&mut self, bytes: u64) {
        self.working_bytes -= bytes;
    }

    fn invalid_json(&self, pointer_is_reliable: bool) -> SceneParseError {
        SceneParseError::invalid_json(pointer_is_reliable.then(|| self.pointer()))
    }

    fn pointer(&self) -> String {
        let mut pointer = String::new();
        for segment in &self.path {
            pointer.push('/');
            push_pointer_segment(&mut pointer, segment);
        }
        pointer
    }

    fn pointer_with(&self, segment: &str) -> String {
        let mut pointer = self.pointer();
        pointer.push('/');
        push_pointer_segment(&mut pointer, segment);
        pointer
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn next_token_starts_value(&self) -> bool {
        matches!(
            self.peek(),
            Some(b'{' | b'[' | b'"' | b'-' | b'0'..=b'9' | b't' | b'f' | b'n')
        )
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

fn validate_envelope(root: &JsonValue) -> Result<(), SceneParseError> {
    let JsonValue::Object(members) = root else {
        return Err(SceneParseError::invalid_envelope(""));
    };

    if !matches!(member_value(members, "type"), Some(JsonValue::String(value)) if &**value == "excalidraw")
    {
        return Err(SceneParseError::invalid_envelope("/type"));
    }
    if !matches!(member_value(members, "version"), Some(JsonValue::Number(value)) if &**value == "2")
    {
        return Err(SceneParseError::invalid_envelope("/version"));
    }
    if !matches!(member_value(members, "elements"), Some(JsonValue::Array(_))) {
        return Err(SceneParseError::invalid_envelope("/elements"));
    }
    if !matches!(
        member_value(members, "appState"),
        Some(JsonValue::Object(_))
    ) {
        return Err(SceneParseError::invalid_envelope("/appState"));
    }
    if !matches!(member_value(members, "files"), Some(JsonValue::Object(_))) {
        return Err(SceneParseError::invalid_envelope("/files"));
    }
    Ok(())
}

fn member_value<'a>(members: &'a [JsonMember], key: &str) -> Option<&'a JsonValue> {
    members
        .iter()
        .find(|member| &*member.key == key)
        .map(|member| &member.value)
}

fn write_canonical_json(value: &JsonValue, bytes: &mut Vec<u8>) -> Result<(), TryReserveError> {
    match value {
        JsonValue::Null => extend_bytes(bytes, b"null")?,
        JsonValue::Bool(true) => extend_bytes(bytes, b"true")?,
        JsonValue::Bool(false) => extend_bytes(bytes, b"false")?,
        JsonValue::Number(number) => extend_bytes(bytes, number.as_bytes())?,
        JsonValue::String(string) => write_canonical_string(string, bytes)?,
        JsonValue::Array(values) => {
            push_byte(bytes, b'[')?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    push_byte(bytes, b',')?;
                }
                write_canonical_json(value, bytes)?;
            }
            push_byte(bytes, b']')?;
        }
        JsonValue::Object(members) => {
            push_byte(bytes, b'{')?;
            for (index, member) in members.iter().enumerate() {
                if index != 0 {
                    push_byte(bytes, b',')?;
                }
                write_canonical_string(&member.key, bytes)?;
                push_byte(bytes, b':')?;
                write_canonical_json(&member.value, bytes)?;
            }
            push_byte(bytes, b'}')?;
        }
    }
    Ok(())
}

fn write_canonical_string(value: &str, bytes: &mut Vec<u8>) -> Result<(), TryReserveError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    push_byte(bytes, b'"')?;
    for character in value.chars() {
        match character {
            '"' => extend_bytes(bytes, br#"\""#)?,
            '\\' => extend_bytes(bytes, br#"\\"#)?,
            '\u{0008}' => extend_bytes(bytes, br"\b")?,
            '\t' => extend_bytes(bytes, br"\t")?,
            '\n' => extend_bytes(bytes, br"\n")?,
            '\u{000c}' => extend_bytes(bytes, br"\f")?,
            '\r' => extend_bytes(bytes, br"\r")?,
            '\u{0000}'..='\u{001f}' => {
                let control = character as usize;
                extend_bytes(bytes, br"\u00")?;
                push_byte(bytes, HEX[control >> 4])?;
                push_byte(bytes, HEX[control & 0x0f])?;
            }
            _ => {
                let mut encoded = [0; 4];
                extend_bytes(bytes, character.encode_utf8(&mut encoded).as_bytes())?;
            }
        }
    }
    push_byte(bytes, b'"')?;
    Ok(())
}

fn push_byte(bytes: &mut Vec<u8>, byte: u8) -> Result<(), TryReserveError> {
    bytes.try_reserve(1)?;
    bytes.push(byte);
    Ok(())
}

fn extend_bytes(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), TryReserveError> {
    bytes.try_reserve(value.len())?;
    bytes.extend_from_slice(value);
    Ok(())
}

fn allocated_bytes(bytes: usize) -> u64 {
    if bytes == 0 {
        0
    } else {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        bytes.saturating_add(ALLOCATION_QUANTUM_BYTES - 1) / ALLOCATION_QUANTUM_BYTES
            * ALLOCATION_QUANTUM_BYTES
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn push_pointer_segment(pointer: &mut String, segment: &str) {
    for character in segment.chars() {
        match character {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            character => pointer.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SceneDocument;

    #[test]
    fn canonicalized_original_scene_document_uses_normative_json_spelling() {
        let source = br#"
        {
            "\u0074ype": "excalidraw",
            "version": 2,
            "elements": [
                null,
                true,
                false,
                {
                    "id": "deleted",
                    "isDeleted": true,
                    "unknown": "\/\u00e9\u2028\u2029\uD83D\uDE00",
                    "controls": "\u0000\b\t\n\f\r\u001F\"\\\/"
                }
            ],
            "appState": {"zoom": {"value": 1E+00}, "opaque": -0},
            "files": {
                "unused": {"mimeType": "image/png", "dataURL": "future"},
                "broken": false
            },
            "futureRoot": [3.00, 1e-2]
        }
        "#;
        let expected = concat!(
            r#"{"type":"excalidraw","version":2,"elements":[null,true,false,{"id":"deleted","isDeleted":true,"unknown":"/"#,
            "\u{00e9}\u{2028}\u{2029}\u{1f600}",
            r#"","controls":"\u0000\b\t\n\f\r\u001f\"\\/"}],"appState":{"zoom":{"value":1E+00},"opaque":-0},"files":{"unused":{"mimeType":"image/png","dataURL":"future"},"broken":false},"futureRoot":[3.00,1e-2]}"#,
        );

        let scene = SceneDocument::parse(source).expect("preservation vector is admitted");

        assert_eq!(
            scene
                .canonicalized_original_bytes()
                .expect("canonical scene serialization succeeds"),
            expected.as_bytes()
        );
    }

    #[test]
    fn canonicalized_original_scene_document_discards_whitespace_and_escape_spelling() {
        let escaped = br#" {
            "\u0074ype": "excalidraw",
            "version": 2,
            "elements": [{"label": "\u0061\/\u00e9"}],
            "appState": {},
            "files": {}
        } "#;
        let direct = concat!(
            r#"{"type":"excalidraw","version":2,"elements":[{"label":"a/"#,
            "\u{00e9}",
            r#""}],"appState":{},"files":{}}"#,
        );

        let escaped = SceneDocument::parse(escaped).expect("escaped vector is admitted");
        let direct_scene =
            SceneDocument::parse(direct.as_bytes()).expect("direct vector is admitted");

        assert_eq!(
            escaped
                .canonicalized_original_bytes()
                .expect("escaped scene serialization succeeds"),
            direct_scene
                .canonicalized_original_bytes()
                .expect("direct scene serialization succeeds")
        );
        assert_eq!(
            escaped
                .canonicalized_original_bytes()
                .expect("escaped scene serialization succeeds"),
            direct.as_bytes()
        );
    }

    #[test]
    fn canonicalized_original_scene_document_preserves_order_and_number_tokens() {
        let member_order_a = br#"{"type":"excalidraw","version":2,"elements":[],"appState":{"future":{"first":1,"second":2}},"files":{}}"#;
        let member_order_b = br#"{"type":"excalidraw","version":2,"elements":[],"appState":{"future":{"second":2,"first":1}},"files":{}}"#;
        let array_order_a =
            br#"{"type":"excalidraw","version":2,"elements":[1,2],"appState":{},"files":{}}"#;
        let array_order_b =
            br#"{"type":"excalidraw","version":2,"elements":[2,1],"appState":{},"files":{}}"#;
        let number_tokens = br#"{"type":"excalidraw","version":2,"elements":[0,-0,1,1.0,1e0,1E+00,10.00e-01],"appState":{},"files":{}}"#;

        let canonical = |source: &[u8]| {
            SceneDocument::parse(source)
                .expect("distinction vector is admitted")
                .canonicalized_original_bytes()
                .expect("distinction vector serialization succeeds")
        };

        assert_ne!(canonical(member_order_a), canonical(member_order_b));
        assert_eq!(canonical(member_order_a), member_order_a);
        assert_eq!(canonical(member_order_b), member_order_b);
        assert_ne!(canonical(array_order_a), canonical(array_order_b));
        assert_eq!(canonical(array_order_a), array_order_a);
        assert_eq!(canonical(array_order_b), array_order_b);
        assert_eq!(canonical(number_tokens), number_tokens);
    }
}
