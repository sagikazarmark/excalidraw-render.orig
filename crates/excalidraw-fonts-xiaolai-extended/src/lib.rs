//! 44 byte-exact Xiaolai chunks for extended CJK blocks, for `excalidraw-fonts`.

#![doc(hidden)]

/// Immutable whole chunks in upstream face order (catalog shard revision-1-002).
pub static BYTES: &[u8] = include_bytes!("../data/fonts.bin");
