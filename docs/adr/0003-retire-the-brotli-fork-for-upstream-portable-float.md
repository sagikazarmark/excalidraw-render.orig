---
status: accepted
---

# Retire the brotli fork in favor of upstream `portable-float`

`crates/brotli-deterministic` exists because released rust-brotli 8.0.4 lets the
platform choose floating-point evaluation order, which breaks byte-identical
WOFF2 output across the five supported targets. Upstream PR #259 adds a
`portable-float` feature that fixes this in tree, so the fork stops being
necessary once a release carries it.

Retirement runs in three stages.

**Stage A (branch only).** Point `ttf2woff2-deterministic` at
`brotli = { git, rev = "fb3104d", default-features = false, features = ["std", "portable-float"] }`
with the profile identity string unchanged, and prove byte-identity: `catalog
verify` plus `cargo test --lib font_subset` on native and wasm, and a green
five-target aggregate at unchanged pins. Because `cargo package --locked
--workspace` runs in CI on every push and a git dependency cannot be packaged,
the stage-A branch comments out the packaging steps so the target jobs reach the
aggregate. The branch never merges; it only banks the proof, together with a
scripted mechanical diff of the fork against upstream 8.0.4.

Stage A was executed on branch `brotli-portable-float-stage-a`, which does not
merge. On `x86_64-unknown-linux-gnu` the swap reproduces the fork's output byte
for byte: `catalog verify` passes, all 971 per-cell `generatedWoff2Sha256` pins
match unchanged, and both the `generatedBytesSha256` and `resourceTracesSha256`
measurements are identical to the fork's. The four remaining targets need the
CI aggregate; the branch records what is still unproven. The mechanical diff
lives on `main` as `tools/brotli-fork-diff.sh`, and against released `8.0.4` it
reports one substantive divergence — `src/enc/util.rs`, where `log2` and `powf`
are replaced by software implementations so evaluation order cannot vary by
platform. That is precisely what `portable-float` now provides.

**Stage B.** Request an upstream rust-brotli release containing PR #259.

**Stage C (on release).** Pin `=8.0.x`, delete `crates/brotli-deterministic`,
and perform exactly one identity-string plus full evidence refresh — the
identity string feeds every subset font's name table, so it regenerates all
font-subset evidence and must never be refreshed twice. Spec and profile edits
land in the same change-set, all other evidence-touching work freezes during it,
and stage C is serialized behind the ADR-0002 work. At stage C, re-run the
mechanical diff against the released tag and compare decompressed sfnt tables
(excluding the rewritten name table) before and after the refresh, so the
identity-string delta is isolated from any encoder delta.

Stage B closed on 2026-09-02: upstream released PR #259 as **rust-brotli
9.0.0**, not an `8.0.x` patch. The major bump is carried by the dependency
closure — `brotli-decompressor ~6.0`, `alloc-no-stdlib 3`, `alloc-stdlib 0.3` —
not by the encoder, so the stage-C pin is `=9.0.0` with `default-features =
false, features = ["std", "portable-float", "disable-timer"]` (upstream
ttf2woff2 0.13.2 depends on `brotli ^8` with `disable-timer`). `wuff` keeps
decoding through `brotli-decompressor 5.0.3`, so the two decompressor majors
coexist in the lock until wuff moves; only the encoder path changed.

Stage C's first half was executed on branch `brotli-portable-float-stage-c`
with the identity string deliberately unchanged, because the byte-identity
proof has to precede the single refresh. The mechanical diff against the
`9.0.0` tag (`a51b65e`) confirms the fork has nothing left to keep: the only
divergence on the encoder path is `src/enc/util.rs`, where the fork's hard patch
is exactly the code upstream now selects under `portable-float` (`FastLog2` →
`FastLog2u64`, `FastPow2` → repeated squaring); the rest of `src/enc/` differs
by a `to_int` → `to_simd` rename, import and `cfg` reshuffles, and added test
modules, and `bin/`, `concat/`, `ffi/` are not on the renderer's path. The
control run against `8.0.4` still reports `util.rs` alone. The fork and
`tools/brotli-fork-diff.sh` are deleted in the same commit; both diffs are
reproducible from the parent commit. With a crates.io dependency, `cargo package
--locked` now runs unmodified in the workflow, so stage A's packaging
workaround is not needed.

Proven on that branch, on the pinned 1.97.1 toolchain: on
`x86_64-unknown-linux-gnu`, `catalog verify` passes, all 971 per-cell
`generatedWoff2Sha256` pins match, and the measurement is identical to the
committed cross-target file (`generatedBytesSha256 6733d051…514f`,
`resourceTracesSha256 a98ec840…dd42`, 971/1198). On `wasm32-unknown-unknown`,
`complete_font_subset_qualification_corpus_in_wasm` — which asserts every one
of those per-cell pins — passes in the pinned Chromium 149.0.7827.55, and the
packaged public facade passes in Chromium against the packaged crates. The
packaged-CLI check, every `conformance verify-*` gate, fmt and clippy pass, and
the comparator digest is unmoved, as ADR-0005 predicts. `aarch64-apple-darwin`,
`x86_64-apple-darwin` and `x86_64-pc-windows-msvc` are read off the `native`
jobs' target reports on the first workflow run after the merge; each report
must carry the same two digests.

One defect in the gate itself surfaced while running the wasm leg with CI's
exact tooling: the measurement line is emitted with `println!`, which
`wasm32-unknown-unknown` discards and `wasm-bindgen-test 0.3.76` does not
capture, so `materialize-svg-font-subset-target wasm32-unknown-unknown` finds no
measurement and the five-target aggregate cannot assemble. The wasm test job
itself is the byte proof; the aggregate is the reviewable form of it. Emitting
the line through `console_log!` on wasm fixes it, but
`font_subset_qualification.rs` is digest-pinned by the approved qualification
(#99), so that fix belongs in the refresh change-set, which pays the re-approval
round anyway. Until then the aggregate cannot be the merge gate, and the
per-target reports are.

Amended 2026-09-06: with GitHub Actions unavailable, the target legs have to be
executed off-CI, and the wasm leg cannot produce a report at all while the line
is dropped. `crates/excalidraw-render/src/lib.rs` — not pinned by any approval
— now carries a `cfg(all(test, target_arch = "wasm32"))` `println!` shim that
routes to `wasm_bindgen_test::console_log!`; textual macro scoping applies it
inside the pinned module without changing a byte of it. The refresh still moves
the fix to the call site and deletes the shim in the same change-set.

Two decisions taken on 2026-09-05 re-sequence the rest of stage C.

The swap merges ahead of the refresh. The bytes are proven identical, so no
evidence moves and `catalog verify` holds; the cost is that every subset font's
name table keeps claiming `excalidraw-render-brotli-0.1.0(upstream-8.0.4)` until
the refresh, and nothing is published from `main` in that window — publication
was already blocked until stage C completes.

The refresh waits for an upstream ttf2woff2 release that carries
[0x6b/ttf2woff2#28](https://github.com/0x6b/ttf2woff2/pull/28): brotli 9 plus an
opt-in `portable-float` passthrough. `crates/ttf2woff2-deterministic` has a
`src/` identical to released 0.13.2 and exists only to reach the deterministic
brotli, so once that release exists it retires the same way — pinned exact
version, `default-features = false`, `features = ["portable-float"]` — and its
token `excalidraw-render-ttf2woff2-0.1.0(upstream-0.13.2)` changes in the same
single refresh as the brotli token. Depending on the fork by git in the meantime
was measured and rejected: `cargo package` normalizes a git dependency to its
registry version, where the feature does not exist, so the packaging step fails
on every push. The refresh change-set therefore carries the ttf2woff2 swap, both
new tokens, the wasm `console_log!` fix, and the full evidence regeneration, and
the aggregate runs for the first time on its evidence. If no ttf2woff2 release
lands by 2026-12-01, the refresh proceeds with the brotli token alone and the
ttf2woff2 fork stays until a later, separately decided refresh.

If no upstream release lands by 2026-12-01, the fallback is a zero-patch
re-vendor of upstream at `fb3104d`. It is not free: the identity token becomes
`excalidraw-render-brotli-0.2.0(upstream-fb3104d)` — the current token claims
`upstream-8.0.4`, which an `fb3104d` snapshot is not — and it consumes the same
single evidence refresh.

Interim stripping of the fork's unused surface (ffi, concat, bin, threading) is
deliberately not done: it is wasted effort if the upstream release lands, and is
resurrected only as part of the fallback.

First crates.io publication is blocked until stage C or the fallback completes,
because git dependencies cannot be published and `portable-float` does not exist
in released 8.0.4. That block is a consequence of this decision, not a technical
limit, and that has now been measured rather than assumed: on 2026-09-01
`cargo package --locked --workspace --exclude xtask --exclude
excalidraw-render-compare` succeeded **without** `--no-verify` — the
build-verifying run CI never performs — with the vendored fork in place. Every
tarball also fits the crates.io 10 MiB cap, though not by much:

| Crate | `.crate` size |
| --- | ---: |
| `excalidraw-render-assets-data-000` | 7.97 MiB |
| `excalidraw-render-assets-data-001` | 4.49 MiB |
| `excalidraw-render-assets-base` | 0.76 MiB |
| `excalidraw-render-brotli` | 0.69 MiB |
| `excalidraw-render` | 0.58 MiB |
| `excalidraw-render-cli` | 0.05 MiB |
| `excalidraw-render-ttf2woff2` | 0.03 MiB |
| `excalidraw-render-skera` | 0.01 MiB |

So publishing with the fork remains available at any time; retiring it is a
maintenance decision, not a precondition for release.
