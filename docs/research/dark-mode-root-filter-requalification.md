# Dark-mode root filter: native snapshot requalification (#84)

Status: evidence for review. No baseline is staged or approved here.

`a9d9e5e` (ADR-0012) makes SVG carry the reference's dark-mode root filter
over light colors and makes PDF round its lowered filter. The approved native
snapshot baseline (`tests/conformance/coverage/native-baseline/revision-1.json`)
has 168 keys.

## Isolation

Two canonical collections, `dagger api call packaging snapshots`, each rendering
every key twice in forward and reverse fresh processes:

- `9673a0f`, the parent: `target/parent-native-snapshots`
- `a9d9e5e`: `target/dark-native-snapshots` (`conformance verify-native-snapshots`
  verified all 168 candidates)

Every session and process order repeats exactly in both. Between them,
**exactly 8 keys change**: the dark-mode SVG and PDF of the four dark-mode
cases. **No report changes**; all PNG, all light keys and the other 160 keys are
byte-identical.

| Case | Variant | Format | `9673a0f` artifact | `a9d9e5e` artifact | `9673a0f` vs approved baseline |
| --- | --- | --- | --- | --- | --- |
| `focused.cjk-propagation.v1` | dark-mode | pdf | `e9cd2d7365330bee…` (203700) | `94307b50f7fdabb5…` (203707) | same |
| `focused.cjk-propagation.v1` | dark-mode | svg | `464a5490c3e5119d…` (23054) | `b23c0bed78232880…` (23094) | same |
| `focused.dark-mode-frames.v1` | dark-mode | pdf | `fd246a2aec1c5402…` (96887) | `8736fc8c974b1b45…` (96876) | same |
| `focused.dark-mode-frames.v1` | dark-mode | svg | `952409a0ee365879…` (17405) | `b14cf5f069b1e40c…` (17445) | same |
| `focused.dark-mode-images.v1` | dark-mode | pdf | `ab071dc7951d8bb8…` (26439) | `9435c029deb94fc5…` (26328) | differs |
| `focused.dark-mode-images.v1` | dark-mode | svg | `035185d6b15ae2a7…` (2654) | `b18ea2a8a20e9c27…` (2108) | same |
| `focused.ellipse-diamond-fills.v1` | dark-mode | pdf | `772e79cc4920a908…` (10369) | `421ebfb27c7485c6…` (10370) | same |
| `focused.ellipse-diamond-fills.v1` | dark-mode | svg | `ba3f6437eb1843a9…` (25898) | `bc7a30e0b4251b5f…` (25938) | same |

## Pre-existing baseline drift

At the parent, 35 keys already differ from the approved baseline. These are
outputs of earlier commits whose requalification approvals are pending on #85
(WebP composition and PNG projection, PDFium alpha consumers, image and case
input refreshes), plus 18 keys whose case input digests changed since the
baseline. They are not part of this change. `focused.dark-mode-images.v1`
dark-mode PDF is among them and changes again here.

## To requalify

After the pending #85 approvals, collect from a clean tree, stage, and approve
as `tests/conformance/coverage/README.md` describes:

```sh
dagger api call packaging snapshots --output target/native-snapshots-clean
cargo run --locked -p xtask -- conformance stage-native-baseline target/native-snapshots-clean target/new-native-baseline
cargo run --locked -p xtask -- conformance native-baseline-approval-subject
# maintainer: record-native-baseline-approval <login> <UTC-date> <#85 comment URL> <subject>
cargo run --locked -p xtask -- conformance verify-native-baseline
```

The approval covers these 8 dark-mode keys together with whatever the pending
approvals already cover.
