# Six-case PNG requalification — ready for exact batch review

**Status:** proposed, not applied; all batch and prospective case approvals are
null. No live protected file changed. No full suite or commit ran.

## Exact batch subject

```text
sha256:3074ae5df1aa0c91743c68f4217f066370698f3d8970bac02dec5b1aacc00e84
```

This is the canonical digest of `approval-subject.json`, using the normal
`excalidraw-render/conformance-approval-subject/v1` envelope. It binds 116 packet
files, 261 live pre-application descriptors, the controlled diagnosis, fresh
release artifacts/reports, exact proposed limits/withdrawals, prospective case
subjects and explicit catalog/ownership migration plan. This README is explanatory
metadata excluded from that subject.

## Six prospective normal case subjects

Every case is `focused.<name>.v1`, variant `default`, scope `png/whole`.

| Case | Proposed action | Prospective canonical approval subject | Normal approval issue |
| --- | --- | --- | --- |
| embeddable-links | 410 / `3fefe50f5a0d41c1` | `sha256:c2f3d3e8499d5558487847915a9fdd963765225694bb0b711f8be525f5ce1001` | #103 |
| emoji-fallback | 1184 / `3fec457e1caa29ce` | `sha256:95d5200586291ee3dd68854ce3a456e19f43707d884dc6cb7b604c91ec80ca45` | #121 |
| bidi-text | 2375 / `3fec7b0fe0040f08` | `sha256:9fc60fe6485a8af4d2b7754b23d7acb3dc95b3bbe30fe2365d6a078302f2184f` | #152 |
| bidi-alignment | 1903 / `3feac3b632cb3ae3` | `sha256:5f4c10af4dc0fa04ee268a16163fc4e66e0ac7aa2a13d0c23b7fbcaa0d0446a5` | #152 |
| text-line-endings | Withdraw; 9 pixels, canonical pass | `sha256:2faeac908c14ee1c12e0357bcf2d29c26f107d0f92aa5e6571b41590ba66ddc1` | #110 |
| text-alignment-vertical | Withdraw; 20 pixels, canonical pass | `sha256:f9f652a17ea64b4db5615c58f1d83eca7ce0e40b55c30b43a5c5cd6bc5adc972` | #152 |

The prospective subjects were computed and verified by the **unchanged** normal
xtask in `/tmp/opencode/png-six-case-stage-v1`. The live six manifests and their
existing approvals remain untouched.

## Verification and preservation

- Twelve fresh native format captures (PNG/SVG for six cases), each repeated in
  two fresh release processes with byte-identical complete reports and artifacts.
- Unmodified release producer matches the approved v7 CLI executable:
  `d4252b1a73c41382f28392bfeff0ee3d37b549662099bf32aba404eeedc57869`.
- Comparator independently reproduces all twelve old/new measurements; current
  release pixels equal the diagnosis's current pixels exactly. All six effective
  prospective PNG gates pass, with two canonical passes requiring withdrawals.
- Isolated six-case SVG geometry/raster/PNG dependency check passes. The #168
  bidi-alignment SVG pin remains exactly 1190 / `3fea564b9c97ab4f`.
- All 2,155 pre-existing repository files are unchanged. Approved representative
  v7 still verifies as `sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05`.

Read-only packet/source/v7 verification:

```sh
python3 tests/conformance/proposals/png-device-phase-six-case-v1/prepare.py verify
```

## Review the migration constraint explicitly

The staged normal derivation removes only the two unowned withdrawn exception
obligations: **1876 → 1874 assignments; 1803 assigned/excluded; 73 → 71 gaps**.
The plan itemizes 122 authority and 12 case-owner manifest binding changes and
preserves every other criterion, selection, rationale and all three exclusions.

**All 168 semantic execution identities remain, but 18 `inputs_sha256` fields
must change.** The unchanged inventory protocol hashes the full case manifest,
including approval, into each SVG/PNG/PDF key. Literal preservation of all old
full keys is therefore impossible when six manifests change. The other 150 full
execution records remain identical. Exact final hashes depend on genuine future
approval comments and must be derived after approval; `preview/` hashes are
explicitly pending-approval values, not final authorities.

The parent should include this enumerated binding migration in the exact batch
approval request. If unchanged full input digests are a hard requirement, stop
before application and resolve that protocol conflict separately.

See [DIAGNOSIS.md](DIAGNOSIS.md) for the causal argument and apply sequence,
`migration-plan.json` for the exact deltas, `proposed/` for prospective protected
files, `before/` for immutable originals, and `preview/` for unapproved migration
previews. Record an actual batch decision on #85, then use genuine comments on
the normal registered issues above with each exact prospective subject. Do not
reuse old approvals or replace the normal case protocol with a #85 URL.

Native baseline refresh/#178, global ledger approval/closure and the fixed-size
moxcms allocation/resource gap remain separate and open.
