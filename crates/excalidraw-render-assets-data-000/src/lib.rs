//! Implementation-detail asset bytes for `excalidraw-render`.

#![doc(hidden)]

/// Immutable bytes in revision-1 shard 000.
pub static BYTES: &[u8] = include_bytes!("../data/revision-1-000.bin");
