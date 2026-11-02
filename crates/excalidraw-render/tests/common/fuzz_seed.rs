//! The seed of a scheduled fuzz expansion.
//!
//! Every fuzz family runs its PR smoke from a fixed seed, so the same cases
//! replay on every change. The nightly expansion takes `EXCALIDRAW_FUZZ_SEED`
//! (the workflow passes the run id) and mixes it into the family's fixed seed,
//! so each night searches a different region than the last and no two families
//! share a stream; a failure prints the seed that reproduces it. Unset, the
//! expansion keeps the fixed seed.

/// The seed a nightly expansion of the family with `fixed` seed runs from.
///
/// Reads `EXCALIDRAW_FUZZ_SEED` as a decimal or `0x`-prefixed integer, XORs it
/// into `fixed`, and prints the result. Panics on a malformed value, so a
/// workflow typo cannot silently fall back to the fixed seed.
pub fn nightly_seed(fixed: u64) -> u64 {
    let seed = match std::env::var("EXCALIDRAW_FUZZ_SEED") {
        Ok(value) => {
            let value = value.trim();
            let parsed = value
                .strip_prefix("0x")
                .map_or_else(|| value.parse::<u64>(), |hex| u64::from_str_radix(hex, 16));
            let parsed = parsed.unwrap_or_else(|error| {
                panic!("EXCALIDRAW_FUZZ_SEED {value:?} is not a decimal or 0x-prefixed integer: {error}")
            });
            // xorshift needs a non-zero state.
            match fixed ^ parsed {
                0 => fixed,
                mixed => mixed,
            }
        }
        Err(_) => fixed,
    };
    println!("fuzz seed {seed:016x}");
    seed
}
