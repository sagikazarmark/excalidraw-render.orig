//! 165 byte-exact Xiaolai chunks for common CJK blocks, for `excalidraw-fonts`.

#![doc(hidden)]

/// Immutable whole chunks in upstream face order (catalog shard revision-1-001).
pub static BYTES: &[u8] = include_bytes!("../data/fonts.bin");
