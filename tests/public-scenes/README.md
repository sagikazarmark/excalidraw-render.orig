# Public scene pool

Upstream Excalidraw scene documents collected from public repositories whose
licenses permit redistribution. They are raw material for layer-3
("representative licensed or project-authored") conformance cases: nothing here
is manifested, sanitized, or approved, and no gate reads this directory.

Each subdirectory holds the upstream bytes verbatim:

- `scene.excalidraw` — the scene document exactly as published upstream
- `LICENSE` — the upstream license text, copied verbatim
- `NOTICE` — the upstream Apache-2.0 notice, where one exists
- `PROVENANCE.json` — repository, path, pinned commit, permalink, SPDX
  identifier, retrieval date, scene digest, and a feature census

Promoting one into `tests/conformance/cases/` still requires the sanitization,
manifest, provenance, and approval steps that section 14.2 of the renderer
specification demands. Copy the license and provenance across with the scene.

## Contents

| Scene | License | Upstream | Elements | Coverage |
|---|---|---|---|---|
| `agentstack-architecture` | Apache-2.0 | [i-am-bee/agentstack](https://github.com/i-am-bee/agentstack/blob/5077c9b4c2067ac96533d4a52aac322f12819f83/docs/stable/images/architecture-src.excalidraw) | 73 | frame plus embedded image; mixed hand-drawn and code fonts |
| `ai-on-eks-llama2-raytrain` | Apache-2.0 | [awslabs/ai-on-eks](https://github.com/awslabs/ai-on-eks/blob/93df6eed3baa828a42cbd85d5478b5b0f23cd211/website/docs/blueprints/excalidraw/llama2-raytrain.excalidraw) | 204 | largest case: 204 elements, 33 image elements, tinted background |
| `antibody-amino-acid-chain` | CC0-1.0 | [Jake-Jay/antibody-illustrations](https://github.com/Jake-Jay/antibody-illustrations/blob/7f899ed68ce16a1707e18b4b0d8ac54ef35e06a8/amino-acid-with-chain.excalidraw) | 129 | diamond- and ellipse-dense illustration; roughness 0/1/2 mixed; hachure and solid fills |
| `antibody-contact-maps` | CC0-1.0 | [Jake-Jay/antibody-illustrations](https://github.com/Jake-Jay/antibody-illustrations/blob/10f9e98105d3bcc7d93f8c752d96b095b4b3b7a9/contact-maps-from-attention-excalidraw.excalidraw) | 130 | cross-hatch fills over a dense line grid; 130 elements |
| `antibody-homolog-spectrum` | CC0-1.0 | [Jake-Jay/antibody-illustrations](https://github.com/Jake-Jay/antibody-illustrations/blob/10f9e98105d3bcc7d93f8c752d96b095b4b3b7a9/homolog-spectrum.excalidraw) | 25 | small mixed illustration; hachure and solid fills |
| `elasticsearch-spec-overview` | Apache-2.0 | [elastic/elasticsearch-specification](https://github.com/elastic/elasticsearch-specification/blob/3d51338565d6558c96b55dad9142e4c2a500ca38/overview.excalidraw) | 97 | three font families (1/3/5) in one scene; 97 elements |
| `excali-local-slides` | MIT | [azzgo/excali-local](https://github.com/azzgo/excali-local/blob/5f63d1941ab85fe9343fea64f6d0b1225e0ef51d/demos/slides.excalidraw) | 32 | 8 frames and 16 image elements; frame-export batch coverage |
| `excalirender-sample` | MIT | [JonRC/excalirender](https://github.com/JonRC/excalirender/blob/dd2e231ccc7fc9d40755f8fccbc7748967b5a4d0/sample.excalidraw) | 6 | minimal smoke scene: one rectangle, diamond, ellipse, line, arrow, text |
| `k8s-breakglass-diagram` | Apache-2.0 | [telekom/k8s-breakglass](https://github.com/telekom/k8s-breakglass/blob/d5d046671f9a63e671c2e63d7b71f99355f4dad8/docs/diagram.excalidraw) | 44 | diamond, ellipse and line mix at moderate size |
| `leaflet-area-selection-ui` | MIT | [bopen/leaflet-area-selection](https://github.com/bopen/leaflet-area-selection/blob/74b55102dd8c4936367a997508e5dbc989450eb3/ui-components.excalidraw.json) | 7 | ellipse and line only; no text, exercises pure geometry |
| `litesql-ha-cluster` | Apache-2.0 | [litesql/ha](https://github.com/litesql/ha/blob/174bdf56ee3fa8c28b427f94e5fa2e2c2704f23b/ha.excalidraw) | 111 | embedded SVG image payload; font families 1/2/8; 111 elements |
| `music-server-architecture` | CC0-1.0 | [Seirea/excalidraws](https://github.com/Seirea/excalidraws/blob/33a2f94bd85f1b4e4a921f96cf2b3025ff9957a1/music-server.excalidraw) | 77 | all three fill styles, freedraw stroke, tinted view background, two font families |
| `perfect-freehand-strokes` | MIT | [steveruizok/perfect-freehand](https://github.com/steveruizok/perfect-freehand/blob/a2433b166608f1e36ca744a670cfc072a30ea4b2/packages/dev/src/state/excalidraw.json) | 18 | freedraw-heavy scene (8 strokes) with bound arrows and text |
| `varmq-queue-diagram` | MIT | [goptics/varmq](https://github.com/goptics/varmq/blob/58458daaf9fdd545025c89837a79400ab55b0ee2/assets/diagrams/varmq.excalidraw) | 89 | single frame, tinted view background, Excalifont-era fontFamily 5 |
| `velox-design` | MIT | [jpillora/velox](https://github.com/jpillora/velox/blob/44995d4038b41e720168f6e8bb614288dcee9ccd/go/design.excalidraw) | 53 | Comic Shanns fontFamily 8 throughout; rectangle/arrow/text architecture sketch |

## Renderer status at collection time

Every scene was run through `excalidraw-render <scene> --output <svg>` with the
release CLI on 2026-09-02. Thirteen exported (exit 0); two were rejected
(exit 1). Every result matches the specification — none of these are open bugs.

| Scene | Exit | Diagnostics |
|---|---|---|
| `leaflet-area-selection-ui` | 1 | `scene.invalid-envelope` |
| `perfect-freehand-strokes` | 1 | `scene.invalid-envelope` |
| `agentstack-architecture` | 0 | `image.decode-failed` |
| `elasticsearch-spec-overview` | 0 | `compatibility.unsupported-property`, `compatibility.unsupported-property-value` |
| `litesql-ha-cluster` | 0 | same pair, plus `font.liberation-sans-substituted` |
| `music-server-architecture` | 0 | `compatibility.unsupported-property`, `font.liberation-sans-substituted` |
| `excalirender-sample` | 0 | `normalization.missing-timestamp` |

The three non-obvious rows, each checked against the source:

- The two `scene.invalid-envelope` rejections are older excalidraw.com exports
  that omit the `files` key. The specification requires `files` to be present
  and permits it to be empty, and mandates `scene.invalid-envelope` when it is
  absent, so the rejection is correct. Upstream Excalidraw opens both files,
  which is what makes them useful: they are real-world negative-path input for
  a deliberate divergence, not candidates for a rendered parity case.
- `agentstack-architecture` carries a genuinely corrupt PNG. The chunk layout
  parses (400x400, 8-bit RGBA, IHDR/sRGB/IDAT/IEND), but the first IDAT chunk
  fails its CRC and the concatenated zlib stream aborts with `incorrect data
  check`. `image.decode-failed` plus a best-effort placeholder is the correct
  outcome. Our copy is byte-identical to upstream, so the corruption is
  upstream's; this is a ready-made corrupt-payload case.
- The `compatibility.*` and `normalization.*` rows are ordinary revision-1
  reporting on unknown or absent nested properties.

## Content review

Reviewed on 2026-09-02 before the pool was accepted; recorded here so it is not
repeated from scratch.

**Text.** Every text element in all 15 scenes was read. All are technical
diagram labels — architecture, Kubernetes, antibody biology, a Zig music
server. No personal data, credentials, internal hostnames, or objectionable
content. The `k8s-breakglass-diagram` authorization flow is schematic and
carries no real tokens.

**Embedded payloads.** Six across four scenes, all shown in
`embedded-payloads.png`:

| Scene | Payload | Notes |
|---|---|---|
| `ai-on-eks-llama2-raytrain` | AWS FSx logo, AWS compute icon | third-party product marks |
| `excali-local-slides` | two cartoon avatars | no marks |
| `litesql-ha-cluster` | project's own logo | wordmark is `fill:#fff` |
| `agentstack-architecture` | corrupt PNG | never decodes; see above |

**Third-party marks: kept, deliberately.** `ai-on-eks-llama2-raytrain` is the
only scene carrying product marks. It is retained: the scene is redistributed
under its upstream Apache-2.0 license with attribution intact, the marks appear
as the vendor's own icons in an architecture diagram, and the corpus is
internal test input rather than a published or promotional artifact. Revisit
this if the corpus is ever published on its own.

It also earns its place technically. Its 33 image elements resolve to 2
distinct file IDs, both present — 33 asset payload occurrences against 2
semantic asset identities. Nothing else in the pool exercises that
occurrence-versus-identity split.

**One caveat for parity work.** The `litesql-ha-cluster` logo draws its
wordmark in white, so it is invisible against the default white view
background. That is the asset's own doing, not a rendering defect, but it makes
the scene a weak choice for a visual-parity case.

## Verifying this directory

`./verify.sh` re-fetches every scene from its pinned upstream commit, compares
the SHA-256 against `PROVENANCE.json`, and re-runs the render to confirm the
exit status and diagnostics above still hold. It needs network access and a
built `target/release/excalidraw-render`.
