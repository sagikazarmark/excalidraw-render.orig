import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { canonicalize } from "./canonical-json.mjs";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const SURFACE_ROOT = resolve(REPO_ROOT, "tests/oracle/scene-surfaces");
const SCHEMA_PATH = resolve(SURFACE_ROOT, "scene-surface-v1.schema.json");
const MANIFEST_PATH = resolve(SURFACE_ROOT, "revision-1.json");
const MANIFEST_DIGEST_PATH = resolve(SURFACE_ROOT, "revision-1.sha256");
const EVIDENCE_PATH = resolve(SURFACE_ROOT, "evidence/revision-1-harmless-fields.json");
const EVIDENCE_DIGEST_PATH = resolve(SURFACE_ROOT, "evidence/revision-1-harmless-fields.sha256");
const PROFILE_DIGEST = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const ASSET_MANIFEST_DIGEST = "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const EXCALIDRAW_COMMIT = "a2ec2889babf7d2295469c6d90ebe77fae57df84";

const FORMATS = ["svg", "png", "pdf"];
const EFFECTS = ["input_repair", "visual", "semantic", "metadata"];
const RECOVERIES = [
  "repaired",
  "ignored",
  "placeholder",
  "omitted",
  "removed",
  "substituted",
  "flattened",
  "rasterized",
  "metadata_omitted",
];
const DIAGNOSTIC_CODES = [
  "normalization.missing-element-id",
  "normalization.duplicate-element-id",
  "normalization.missing-timestamp",
  "normalization.zero-rough-seed",
  "normalization.relationship-cycle",
  "compatibility.unsupported-element-type",
  "compatibility.malformed-element",
  "compatibility.unsupported-property",
  "compatibility.unsupported-property-value",
  "compatibility.known-element-replaced",
  "asset.supplement-conflict",
  "asset.media-type-mismatch",
  "asset.malformed-file-record",
  "image.missing-file",
  "image.forbidden-resource",
  "image.animation-flattened",
  "image.unsupported-format",
  "image.decode-failed",
  "image.unsafe-svg-content-removed",
  "link.unsafe-omitted",
  "color.invalid",
  "font.liberation-sans-substituted",
  "font.noto-emoji-substituted",
  "font.missing-glyph",
  "resource.element-complexity-replaced",
  "resource.element-work-exceeded",
  "resource.asset-bytes-exceeded",
  "resource.asset-work-exceeded",
  "resource.image-dimensions-exceeded",
  "resource.image-pixels-exceeded",
  "resource.nested-svg-complexity-exceeded",
  "resource.nested-asset-depth-exceeded",
  "resource.coordinate-range-exceeded",
  "resource.optional-metadata-omitted",
  "metadata.provenance-serialization-failed",
  "metadata.diagnostic-summary-serialization-failed",
  "metadata.scene-serialization-failed",
  "metadata.scene-compression-failed",
  "metadata.scene-embedding-failed",
  "metadata.manifest-serialization-failed",
  "metadata.manifest-embedding-failed",
  "pdf.raster-fallback",
];
const ELEMENT_KINDS = [
  "selection",
  "rectangle",
  "diamond",
  "ellipse",
  "text",
  "line",
  "arrow",
  "freedraw",
  "image",
  "frame",
  "magicframe",
  "iframe",
  "embeddable",
];
const RECORD_KINDS = [
  "scene",
  "app_state",
  "file_record",
  "frame_rendering",
  "element.common",
  "element.selection",
  "element.rectangle",
  "element.diamond",
  "element.ellipse",
  "element.text",
  "element.linear",
  "element.line",
  "element.arrow",
  "element.freedraw",
  "element.image",
  "element.frame",
  "element.magicframe",
  "element.iframe",
  "element.embeddable",
  "element.legacy_draw",
  "nested.roundness",
  "nested.bound_element",
  "nested.point",
  "nested.point_binding",
  "nested.fixed_point_binding",
  "nested.fixed_segment",
  "nested.image_crop",
  "nested.generation_data",
  "nested.local_svg_resource",
];

const COMMON_FIELDS = [
  "type",
  "id",
  "x",
  "y",
  "strokeColor",
  "backgroundColor",
  "fillStyle",
  "strokeWidth",
  "strokeStyle",
  "roundness",
  "roughness",
  "opacity",
  "width",
  "height",
  "angle",
  "seed",
  "version",
  "versionNonce",
  "index",
  "isDeleted",
  "groupIds",
  "frameId",
  "boundElements",
  "updated",
  "link",
  "locked",
  "customData",
  "strokeSharpness",
  "boundElementIds",
];

const APP_STATE_FIELDS = [
  "contextMenu",
  "showWelcomeScreen",
  "isLoading",
  "errorMessage",
  "activeEmbeddable",
  "newElement",
  "resizingElement",
  "multiElement",
  "selectionElement",
  "isBindingEnabled",
  "startBoundElement",
  "suggestedBindings",
  "frameToHighlight",
  "frameRendering",
  "editingFrame",
  "elementsToHighlight",
  "editingTextElement",
  "editingLinearElement",
  "activeTool",
  "penMode",
  "penDetected",
  "exportBackground",
  "exportEmbedScene",
  "exportWithDarkMode",
  "exportScale",
  "currentItemStrokeColor",
  "currentItemBackgroundColor",
  "currentItemFillStyle",
  "currentItemStrokeWidth",
  "currentItemStrokeStyle",
  "currentItemRoughness",
  "currentItemOpacity",
  "currentItemFontFamily",
  "currentItemFontSize",
  "currentItemTextAlign",
  "currentItemStartArrowhead",
  "currentItemEndArrowhead",
  "currentHoveredFontFamily",
  "currentItemRoundness",
  "currentItemArrowType",
  "viewBackgroundColor",
  "scrollX",
  "scrollY",
  "cursorButton",
  "scrolledOutside",
  "name",
  "isResizing",
  "isRotating",
  "zoom",
  "openMenu",
  "openPopup",
  "openSidebar",
  "openDialog",
  "defaultSidebarDockedPreference",
  "lastPointerDownWith",
  "selectedElementIds",
  "hoveredElementIds",
  "previousSelectedElementIds",
  "selectedElementsAreBeingDragged",
  "shouldCacheIgnoreZoom",
  "toast",
  "zenModeEnabled",
  "theme",
  "gridSize",
  "gridStep",
  "gridModeEnabled",
  "viewModeEnabled",
  "selectedGroupIds",
  "editingGroupId",
  "width",
  "height",
  "offsetTop",
  "offsetLeft",
  "fileHandle",
  "collaborators",
  "stats",
  "currentChartType",
  "pasteDialog",
  "pendingImageElementId",
  "showHyperlinkPopup",
  "selectedLinearElement",
  "snapLines",
  "originSnapOffset",
  "objectsSnapModeEnabled",
  "userToFollow",
  "followedBy",
  "isCropping",
  "croppingElementId",
  "searchMatches",
  "isSidebarDocked",
];

const SPECIFIC_FIELDS = {
  "element.arrow": ["startArrowhead", "endArrowhead", "elbowed", "fixedSegments", "startIsSpecial", "endIsSpecial"],
  "element.embeddable": ["validated"],
  "element.frame": ["name"],
  "element.freedraw": ["points", "pressures", "simulatePressure", "lastCommittedPoint"],
  "element.iframe": ["customData/generationData"],
  "element.image": ["fileId", "status", "scale", "crop"],
  "element.linear": [
    "points",
    "lastCommittedPoint",
    "startBinding",
    "endBinding",
  ],
  "element.line": ["startArrowhead", "endArrowhead"],
  "element.magicframe": ["name"],
  "element.text": [
    "fontSize",
    "fontFamily",
    "text",
    "textAlign",
    "verticalAlign",
    "containerId",
    "originalText",
    "autoResize",
    "lineHeight",
    "font",
    "rawText",
    "baseline",
  ],
  "frame_rendering": ["enabled", "clip", "name", "outline"],
  "nested.bound_element": ["id", "type"],
  "nested.fixed_point_binding": ["elementId", "focus", "gap", "fixedPoint"],
  "nested.fixed_segment": ["start", "end", "index"],
  "nested.generation_data": ["status", "html", "message", "code"],
  "nested.image_crop": ["x", "y", "width", "height", "naturalWidth", "naturalHeight"],
  "nested.point": ["0", "1"],
  "nested.point_binding": ["elementId", "focus", "gap"],
  "nested.roundness": ["type", "value"],
};

const SCHEMA = {
  $id: "https://github.com/sagikazarmark/excalidraw-render.orig/blob/main/tests/oracle/scene-surfaces/scene-surface-v1.schema.json",
  $schema: "https://json-schema.org/draft/2020-12/schema",
  additionalProperties: false,
  properties: {
    authorities: { items: { $ref: "#/$defs/authority" }, type: "array" },
    compatibilityRevision: { const: 1 },
    conservativeUnmatchedPathRule: { $ref: "#/$defs/conservativeRule" },
    coverage: { $ref: "#/$defs/coverage" },
    domainCatalog: { additionalProperties: { $ref: "#/$defs/domain" }, type: "object" },
    elementKinds: { items: { $ref: "#/$defs/elementKind" }, type: "array" },
    evidenceCatalog: { items: { $ref: "#/$defs/evidence" }, type: "array" },
    fallbackRecipes: { items: { $ref: "#/$defs/recipe" }, type: "array" },
    fieldDispositions: { items: { $ref: "#/$defs/field" }, type: "array" },
    noticeCatalog: { items: { $ref: "#/$defs/notice" }, type: "array" },
    normalizationRules: { items: { $ref: "#/$defs/normalizationRule" }, type: "array" },
    recordKinds: { items: { $ref: "#/$defs/recordKind" }, type: "array" },
    referenceProfile: { $ref: "#/$defs/referenceProfile" },
    registryMappings: { items: { $ref: "#/$defs/mapping" }, type: "array" },
    rendererOwnedRecipeReferences: { items: { $ref: "#/$defs/rendererRecipeReference" }, type: "array" },
    schema: { const: "excalidraw-render/scene-surface/v1" },
    schemaDefinition: { $ref: "#/$defs/schemaDefinition" },
    sourceCatalog: { items: { $ref: "#/$defs/source" }, type: "array" },
  },
  required: [
    "schema",
    "schemaDefinition",
    "compatibilityRevision",
    "referenceProfile",
    "authorities",
    "sourceCatalog",
    "evidenceCatalog",
    "domainCatalog",
    "recordKinds",
    "fieldDispositions",
    "elementKinds",
    "fallbackRecipes",
    "noticeCatalog",
    "normalizationRules",
    "registryMappings",
    "rendererOwnedRecipeReferences",
    "conservativeUnmatchedPathRule",
    "coverage",
  ],
  type: "object",
  $defs: {
    authority: {
      additionalProperties: false,
      properties: {
        id: { type: "string" },
        title: { type: "string" },
        url: { pattern: "^https://github\\.com/", type: "string" },
      },
      required: ["id", "title", "url"],
      type: "object",
    },
    coverage: {
      additionalProperties: false,
      properties: {
        elementKinds: { items: { type: "string" }, type: "array" },
        fieldDispositionCount: { type: "number" },
        officialFallbackCount: { type: "number" },
        recordKinds: { items: { type: "string" }, type: "array" },
        status: { const: "complete" },
        unresolved: { maxItems: 0, type: "array" },
        upstreamTypeInventory: { additionalProperties: { items: { type: "string" }, type: "array" }, type: "object" },
        upstreamTypeInventoryDigest: { pattern: "^sha256:[0-9a-f]{64}$", type: "string" },
        validation: { type: "string" },
      },
      required: ["elementKinds", "fieldDispositionCount", "officialFallbackCount", "recordKinds", "status", "unresolved", "upstreamTypeInventory", "upstreamTypeInventoryDigest", "validation"],
      type: "object",
    },
    conservativeRule: {
      additionalProperties: false,
      properties: {
        diagnostic: {
          additionalProperties: false,
          properties: {
            code: { const: "compatibility.unsupported-property" },
            effects: { items: { enum: EFFECTS }, type: "array" },
            recovery: { const: "ignored" },
            severity: { const: "warning" },
          },
          required: ["code", "effects", "recovery", "severity"],
          type: "object",
        },
        malformedKnownValue: {
          additionalProperties: false,
          properties: { code: { const: "compatibility.unsupported-property-value" }, rule: { type: "string" } },
          required: ["code", "rule"],
          type: "object",
        },
        matching: { type: "string" },
        participation: {
          additionalProperties: false,
          properties: {
            appState: { type: "string" },
            deletedElement: { type: "string" },
            fileRecord: { type: "string" },
            knownElement: { type: "string" },
            metadataEmbedding: { type: "string" },
            root: { type: "string" },
            unreferencedFileRecord: { type: "string" },
          },
          required: ["appState", "deletedElement", "fileRecord", "knownElement", "metadataEmbedding", "root", "unreferencedFileRecord"],
          type: "object",
        },
        preservation: { type: "string" },
        suppression: {
          additionalProperties: false,
          properties: {
            malformedRecord: { type: "string" },
            unknownElement: { type: "string" },
            unknownProperty: { type: "string" },
          },
          required: ["malformedRecord", "unknownElement", "unknownProperty"],
          type: "object",
        },
      },
      required: ["diagnostic", "malformedKnownValue", "matching", "participation", "preservation", "suppression"],
      type: "object",
    },
    domain: {
      additionalProperties: false,
      properties: {
        constraints: { items: { type: "string" }, type: "array" },
        jsonTypes: { items: { enum: ["null", "boolean", "number", "string", "array", "object"] }, type: "array" },
        values: { items: {}, type: "array" },
      },
      required: ["jsonTypes", "constraints", "values"],
      type: "object",
    },
    evidence: {
      additionalProperties: false,
      properties: {
        classifications: { items: { type: "string" }, type: "array" },
        digest: { pattern: "^sha256:[0-9a-f]{64}$", type: "string" },
        id: { type: "string" },
        kind: { enum: ["canonical-oracle", "upstream-reference", "approved-compatibility-exception"] },
        path: { type: ["string", "null"] },
        result: { const: "pass" },
        sourceIds: { items: { type: "string" }, type: "array" },
        url: { type: ["string", "null"] },
      },
      required: ["id", "kind", "classifications", "sourceIds", "path", "url", "digest", "result"],
      type: "object",
    },
    fallback: {
      additionalProperties: false,
      properties: {
        action: { enum: ["accept", "default", "repair", "substitute", "ignore", "omit", "replace", "fatal"] },
        code: { type: ["string", "null"] },
        effects: { items: { enum: EFFECTS }, type: "array" },
        formatRules: { anyOf: [{ $ref: "#/$defs/formatRuleSet" }, { type: "null" }] },
        recipe: { type: ["string", "null"] },
        recovery: { anyOf: [{ enum: RECOVERIES }, { type: "null" }] },
        value: {},
        when: { type: "string" },
      },
      required: ["action", "code", "effects", "formatRules", "recovery", "recipe", "value", "when"],
      type: "object",
    },
    field: {
      additionalProperties: false,
      properties: {
        acceptedDomain: { type: "string" },
        disposition: { enum: ["supported", "static-export-irrelevant", "opaque-metadata"] },
        evidenceIds: { items: { type: "string" }, type: "array" },
        fallbackPrecedence: { const: "first matching valueRule; otherwise missing fallback when absent; otherwise invalid fallback when outside acceptedDomain; otherwise accept; a non-null fallback/valueRule formatRules selects the format-specific recovery instead of the scalar recovery" },
        harmlessNewerBehavior: { type: "boolean" },
        invalid: { $ref: "#/$defs/fallback" },
        missing: { $ref: "#/$defs/fallback" },
        normalization: { type: "string" },
        path: { pattern: "^/", type: "string" },
        recordKind: { type: "string" },
        required: { type: "boolean" },
        scope: { $ref: "#/$defs/scope" },
        sourceIds: { items: { type: "string" }, minItems: 1, type: "array" },
        valueRules: { items: { $ref: "#/$defs/valueRule" }, type: "array" },
      },
      required: [
        "recordKind",
        "path",
        "acceptedDomain",
        "disposition",
        "required",
        "normalization",
        "scope",
        "missing",
        "invalid",
        "sourceIds",
        "evidenceIds",
        "fallbackPrecedence",
        "harmlessNewerBehavior",
        "valueRules",
      ],
      type: "object",
    },
    elementKind: {
      additionalProperties: false,
      properties: {
        behavior: { type: "string" },
        ignoredInheritedPaths: { items: { type: "string" }, type: "array" },
        kind: { type: "string" },
        recordKind: { type: "string" },
        status: { enum: ["supported", "static-export-irrelevant"] },
        unknownValueFallback: { type: "string" },
      },
      required: ["behavior", "ignoredInheritedPaths", "kind", "recordKind", "status", "unknownValueFallback"],
      type: "object",
    },
    mapping: {
      additionalProperties: false,
      properties: {
        code: { type: "string" },
        detailKinds: { items: { enum: ["none", "compatibility", "asset", "resource_limit", "missing_glyph", "metadata", "nested_source", "pdf_raster_fallback"] }, minItems: 1, type: "array" },
        effectsConditional: { items: { type: "string" }, type: "array" },
        effectsRequired: { items: { enum: EFFECTS }, type: "array" },
        formatRules: { $ref: "#/$defs/formatRuleSet" },
        pointerRule: { type: "string" },
        recipeSelection: { type: "string" },
        severity: { enum: ["info", "warning"] },
        sourceRule: { type: "string" },
      },
      required: ["code", "detailKinds", "severity", "effectsRequired", "effectsConditional", "formatRules", "pointerRule", "sourceRule", "recipeSelection"],
      type: "object",
    },
    formatRule: {
      additionalProperties: false,
      properties: {
        action: { type: "string" },
        recovery: { anyOf: [{ enum: RECOVERIES }, { type: "null" }] },
        when: { type: "string" },
      },
      required: ["action", "recovery"],
      type: "object",
    },
    formatRuleSet: {
      additionalProperties: false,
      properties: {
        pdf: { anyOf: [{ $ref: "#/$defs/formatRule" }, { items: { $ref: "#/$defs/formatRule" }, minItems: 1, type: "array" }] },
        png: { anyOf: [{ $ref: "#/$defs/formatRule" }, { items: { $ref: "#/$defs/formatRule" }, minItems: 1, type: "array" }] },
        svg: { anyOf: [{ $ref: "#/$defs/formatRule" }, { items: { $ref: "#/$defs/formatRule" }, minItems: 1, type: "array" }] },
      },
      type: "object",
    },
    notice: {
      additionalProperties: false,
      properties: {
        assetIdentity: { type: "string" },
        id: { type: "string" },
        requiredFormats: { items: { enum: FORMATS }, type: "array" },
        sourceIds: { items: { type: "string" }, minItems: 1, type: "array" },
        text: { type: "string" },
        textSha256: { pattern: "^[0-9a-f]{64}$", type: "string" },
      },
      required: ["id", "assetIdentity", "text", "textSha256", "requiredFormats", "sourceIds"],
      type: "object",
    },
    normalizationRule: {
      additionalProperties: false,
      properties: {
        code: { type: ["string", "null"] },
        effects: { items: { enum: EFFECTS }, type: "array" },
        pointerRule: { type: "string" },
        recovery: { enum: RECOVERIES },
        rule: { type: "string" },
        sourceIds: { items: { type: "string" }, minItems: 1, type: "array" },
      },
      required: ["code", "effects", "pointerRule", "recovery", "rule", "sourceIds"],
      type: "object",
    },
    recipe: {
      additionalProperties: false,
      properties: {
        content: { $ref: "#/$defs/recipeContent" },
        contentDigest: { pattern: "^sha256:[0-9a-f]{64}$", type: "string" },
        evidenceIds: { items: { type: "string" }, minItems: 1, type: "array" },
        kind: { enum: ["official", "official-derived-safety-exception"] },
        noticeIds: { items: { type: "string" }, type: "array" },
        recipeId: { pattern: "^[a-z0-9.-]+@sha256:[0-9a-f]{64}$", type: "string" },
        sourceIds: { items: { type: "string" }, minItems: 1, type: "array" },
      },
      required: ["recipeId", "kind", "contentDigest", "content", "noticeIds", "sourceIds", "evidenceIds"],
      type: "object",
    },
    recipeContent: {
      anyOf: [
        {
          additionalProperties: false,
          properties: {
            format: { $ref: "#/$defs/recipeFormat" },
            glyph: { type: "object" },
            inherit: { items: { type: "string" }, type: "array" },
            kind: { enum: ["image", "image-error"] },
            paint: { type: "object" },
            sourceBehavior: { type: "object" },
          },
          required: ["format", "glyph", "inherit", "kind", "paint", "sourceBehavior"],
          type: "object",
        },
        {
          additionalProperties: false,
          properties: {
            baseRecipe: { type: "string" },
            format: { $ref: "#/$defs/recipeFormat" },
            inherit: { items: { type: "string" }, type: "array" },
            kind: { enum: ["forbidden-resource", "nested-resource"] },
            placement: { type: "object" },
            reuse: { items: { type: "string" }, type: "array" },
          },
          required: ["baseRecipe", "format", "inherit", "kind", "placement", "reuse"],
          type: "object",
        },
        {
          additionalProperties: false,
          properties: {
            format: { $ref: "#/$defs/recipeFormat" },
            kind: { const: "iframe-embeddable" },
            label: { type: "object" },
            links: { type: "object" },
            notices: { type: "object" },
            shell: { type: "object" },
          },
          required: ["format", "kind", "label", "links", "notices", "shell"],
          type: "object",
        },
      ],
    },
    recipeFormat: {
      additionalProperties: false,
      properties: { pdf: { enum: ["paint", "omit"] }, png: { enum: ["paint", "omit"] }, svg: { enum: ["paint", "omit"] } },
      required: ["pdf", "png", "svg"],
      type: "object",
    },
    recordKind: {
      additionalProperties: false,
      properties: {
        fieldPaths: { items: { type: "string" }, type: "array" },
        id: { type: "string" },
        inherits: { items: { type: "string" }, type: "array" },
        unmatchedProperties: { type: "string" },
      },
      required: ["fieldPaths", "id", "inherits", "unmatchedProperties"],
      type: "object",
    },
    referenceProfile: {
      additionalProperties: false,
      properties: {
        assetManifestDigest: { pattern: "^sha256:[0-9a-f]{64}$", type: "string" },
        excalidrawRelease: { const: "v0.18.1" },
        profileDigest: { const: `sha256:${PROFILE_DIGEST}` },
        sourceCommit: { const: EXCALIDRAW_COMMIT },
      },
      required: ["assetManifestDigest", "excalidrawRelease", "profileDigest", "sourceCommit"],
      type: "object",
    },
    rendererRecipeReference: {
      additionalProperties: false,
      properties: {
        definitionAuthority: { const: "outcomes" },
        recipeId: { enum: ["renderer.unknown-element.v1", "renderer.malformed-element.v1", "renderer.known-element-unavailable.v1"] },
      },
      required: ["definitionAuthority", "recipeId"],
      type: "object",
    },
    schemaDefinition: {
      additionalProperties: false,
      properties: {
        digest: { pattern: "^sha256:[0-9a-f]{64}$", type: "string" },
        path: { type: "string" },
        validationCommand: { type: "string" },
      },
      required: ["digest", "path", "validationCommand"],
      type: "object",
    },
    scope: {
      additionalProperties: false,
      properties: {
        effects: { items: { enum: EFFECTS }, type: "array" },
        formats: { items: { enum: FORMATS }, type: "array" },
        options: { items: { type: "string" }, type: "array" },
        participation: { type: "string" },
        reachability: { type: "string" },
        states: { items: { type: "string" }, type: "array" },
      },
      required: ["effects", "formats", "options", "participation", "reachability", "states"],
      type: "object",
    },
    source: {
      additionalProperties: false,
      properties: {
        contentSha256: { anyOf: [{ pattern: "^[0-9a-f]{64}$", type: "string" }, { type: "null" }] },
        gitBlobSha1: { anyOf: [{ pattern: "^[0-9a-f]{40}$", type: "string" }, { type: "null" }] },
        id: { type: "string" },
        immutableRevision: { type: "string" },
        kind: { enum: ["upstream-source", "upstream-test", "license", "reference-profile", "project-decision"] },
        path: { type: ["string", "null"] },
        url: { pattern: "^https://", type: "string" },
      },
      required: ["id", "kind", "url", "immutableRevision", "path", "gitBlobSha1", "contentSha256"],
      type: "object",
    },
    valueRule: {
      additionalProperties: false,
      properties: {
        code: { type: ["string", "null"] },
        formatRules: { anyOf: [{ $ref: "#/$defs/formatRuleSet" }, { type: "null" }] },
        recovery: { anyOf: [{ enum: RECOVERIES }, { type: "null" }] },
        value: {},
        when: { type: "string" },
      },
      required: ["code", "formatRules", "recovery", "when"],
      type: "object",
    },
  },
};

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(canonicalize(value), "utf8");
}

function pathForManifest(path) {
  return relative(REPO_ROOT, path).split(sep).join("/");
}

function compareOrWrite(mode, path, expected) {
  if (mode === "update") {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, expected);
    return;
  }
  const actual = readFileSync(path);
  if (!actual.equals(expected)) {
    throw new Error(`${pathForManifest(path)} is not canonical or is stale; run scene-surface update`);
  }
}

function source(id, kind, path, gitBlobSha1, contentSha256 = null, repository = "excalidraw/excalidraw", revision = EXCALIDRAW_COMMIT) {
  return {
    contentSha256,
    gitBlobSha1,
    id,
    immutableRevision: revision,
    kind,
    path,
    url: path
      ? `https://github.com/${repository}/blob/${revision}/${path}`
      : `https://github.com/${repository}/tree/${revision}`,
  };
}

function domain(jsonTypes, constraints = [], values = []) {
  return { constraints, jsonTypes, values };
}

function scope(effects, participation, formats = FORMATS, options = [], states = [], reachability = "selected record participates in the requested static export") {
  return { effects, formats, options, participation, reachability, states };
}

function fallback(action, {
  code = null,
  effects = [],
  formatRules = null,
  recipe = null,
  recovery = null,
  value = null,
  when = "the field is absent or outside its accepted domain",
} = {}) {
  return { action, code, effects, formatRules, recipe, recovery, value, when };
}

function field(recordKind, path, acceptedDomain, disposition, {
  evidenceIds = [],
  harmlessNewerBehavior = false,
  invalid = fallback("ignore", { recovery: "ignored" }),
  missing = fallback("accept", { when: "the optional field is absent" }),
  normalization = "none",
  required = false,
  scope: fieldScope = scope([], "containing-record"),
  sourceIds = ["excalidraw.element-types"],
  valueRules = [],
} = {}) {
  return {
    acceptedDomain,
    disposition,
    evidenceIds,
    fallbackPrecedence: "first matching valueRule; otherwise missing fallback when absent; otherwise invalid fallback when outside acceptedDomain; otherwise accept; a non-null fallback/valueRule formatRules selects the format-specific recovery instead of the scalar recovery",
    harmlessNewerBehavior,
    invalid,
    missing,
    normalization,
    path: `/${path}`,
    recordKind,
    required,
    scope: fieldScope,
    sourceIds,
    valueRules: valueRules.map((rule) => ({ formatRules: null, ...rule })),
  };
}

const SOURCE_CATALOG = [
  source("excalidraw.element-types", "upstream-source", "packages/excalidraw/element/types.ts", "596586743415c9185ce6022c03918a3216f69390"),
  source("excalidraw.types", "upstream-source", "packages/excalidraw/types.ts", "0562736cd0070a8188b1c1788a149cbe24f6145f"),
  source("excalidraw.data-types", "upstream-source", "packages/excalidraw/data/types.ts", "f5f5535a5e0658b5dfc433f0b79eff10415d414a"),
  source("excalidraw.restore", "upstream-source", "packages/excalidraw/data/restore.ts", "c4e45b025a073f9ff9af1e3fedb322862c25a4cd"),
  source("excalidraw.app-state", "upstream-source", "packages/excalidraw/appState.ts", "644949e7c8d69385dc1c0f1dc31982752756a790"),
  source("excalidraw.public-export", "upstream-source", "packages/utils/export.ts", "22287ce7e6693704a738d3c9c7e3551bc181d79e"),
  source("excalidraw.scene-export", "upstream-source", "packages/excalidraw/scene/export.ts", "7b3325690291d72de8e57bfd587f7103b01c9125"),
  source("excalidraw.shape", "upstream-source", "packages/excalidraw/scene/Shape.ts", "9c49db0c21c20150bd90917513d49b310207d77b", "fa39770b6e373718db623ec066daa3d536c86071f382f879124b4b736746da6e"),
  source("excalidraw.static-svg", "upstream-source", "packages/excalidraw/renderer/staticSvgScene.ts", "b14faf7f4401bbad09bc28ec86d5073949e63e19", "6ac60a185c8e7f5aba7f9abd85f4a102f4eb4d0520497590af365471b2675ca3"),
  source("excalidraw.static-canvas", "upstream-source", "packages/excalidraw/renderer/staticScene.ts", "90ed8af430ca94b9eb0b0f04c7016075327fcb01"),
  source("excalidraw.render-element", "upstream-source", "packages/excalidraw/renderer/renderElement.ts", "d93469c2d3b8fce8a4647d07406908c4bcad6358", "b45a76dfb1e429732e3e1d54888389e78da6797d24ba8addd0ed442c1e7e9bd5"),
  source("excalidraw.embeddable", "upstream-source", "packages/excalidraw/element/embeddable.ts", "8265a0b13d8eeb88fdc0993e7ddd702a60dc4456", "e27969acba740323a4a128ee16b315d99e353ff8e7d4036f1c455bf3a07fb5af"),
  source("excalidraw.text-wrapping", "upstream-source", "packages/excalidraw/element/textWrapping.ts", "1913f6ed3750e6dfd45dd49a539e0c73bfe165de"),
  source("excalidraw.new-element", "upstream-source", "packages/excalidraw/element/newElement.ts", "7d1f1491073a70baf023f14cf07b7ba5e6a9bd11"),
  source("excalidraw.font-metadata", "upstream-source", "packages/excalidraw/fonts/FontMetadata.ts", "e93380e81ed829202f1cf05665b7275860277b6a"),
  source("excalidraw.url", "upstream-source", "packages/excalidraw/data/url.ts", "2ab553bb8f4a9c254bfd3171e3bfcab7b397c6e6"),
  source("excalidraw.fractional-index", "upstream-source", "packages/excalidraw/fractionalIndex.ts", "dfb8a4672ed42202b9b0c8f8c2ce5ed4d781a3cd"),
  source("excalidraw.image", "upstream-source", "packages/excalidraw/element/image.ts", "32644b64d1b07ea8006b38225ca5cd5f6059ddb9"),
  source("excalidraw.constants", "upstream-source", "packages/excalidraw/constants.ts", "616ced1a76c02f4d5066a3f625eefdebadc60920"),
  source("excalidraw.liberation-sans-ttf", "upstream-source", "scripts/woff2/assets/LiberationSans-Regular.ttf", "006f615ebe209d681f66f24e90f5bfd9e24e69bc", "42231919b6322858a1e80dfdaf671b0a530694392f50633604a6eff06ddb1628"),
  source("excalidraw.liberation-sans-2048-ttf", "upstream-source", "scripts/woff2/assets/LiberationSans-Regular-2048.ttf", "ff514cddee1f6ee4a3b56c9d47504564ee2af7ac", "dee2f5580b02465fda0e2ff2efdfe2b3fef5900129d3a9c2e91fa5bf6e3e376c"),
  source("excalidraw.noto-emoji-ttf", "upstream-source", "scripts/woff2/assets/NotoEmoji-Regular.ttf", "f68173bd469f7a731ab876cc6082f6c3ad79d6fb", "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634"),
  source("excalidraw.noto-emoji-2048-ttf", "upstream-source", "scripts/woff2/assets/NotoEmoji-Regular-2048.ttf", "608947be4f490112a8fdc4a5fe1cf97b6c8bb140", "ac40f7eade56dfc94ad2e9bdfc0333d4667036551c133cdf8f8ef652456ca002"),
  source("excalidraw.restore-tests", "upstream-test", "packages/excalidraw/tests/data/restore.test.ts", "37a27ac6a59fe16f03ac02178fdd4f375b770205"),
  source("excalidraw.restore-snapshot", "upstream-test", "packages/excalidraw/tests/data/__snapshots__/restore.test.ts.snap", "7dd0c01c3f36e326fe98a6fc2cdee51c6854c55f", "ce904e4246e529137970c3b8d8b7e38684b99a595eb18264b591c6d261ed6278"),
  source("excalidraw.export-snapshot", "upstream-test", "packages/excalidraw/tests/scene/__snapshots__/export.test.ts.snap", "fdf514d90cd26a893321959bcb975e22719574d5", "b8def935aaf60e4f31d35da6e27f4616f4563533cd89c5af3339997f17cab016"),
  source("font-awesome.image", "upstream-source", "svgs/solid/image.svg", "c32f68b886e481f2602f1103caf72da9f58a2ec6", "f3a4dff85d862e91501af10000d758898ca2bc3055ea451423ddedac7f41c1d9", "FortAwesome/Font-Awesome", "7d3d774145ac38663f6d1effc6def0334b68ab7e"),
  source("font-awesome.ban", "upstream-source", "svgs/solid/ban.svg", "716ce003051a3e8cc2724211871415d57a30dc09", "8b43245cb319c865fdc4ec865967bcd8b4e833c5b9e108adad750d3db3b64ca9", "FortAwesome/Font-Awesome", "7d3d774145ac38663f6d1effc6def0334b68ab7e"),
  source("font-awesome.license", "license", "LICENSE.txt", "f31bef92b64e9d493204176354b2d7f83d8d7eae", "e779748dfe75e84f974df3c7bc07f842011a100159158b0f1f49b2f2a5a515cb", "FortAwesome/Font-Awesome", "7d3d774145ac38663f6d1effc6def0334b68ab7e"),
  source("liberation-sans.license", "license", "LICENSE", "aba73e8a403084a93a245ca00e4a0db007886e0a", "93fed46019c38bbe566b479d22148e2e8a1e85ada614accb0211c37b2c61c19b", "liberationfonts/liberation-fonts", "4b0192046158094654e865245832c66d2104219e"),
  source("noto-emoji.license", "license", "ofl/notoemoji/OFL.txt", "d09d3d0e09169f6747754e6b25faf5ba5ef6402d", "500bb1ccf43df7bbb522112f9133a52b16e1c35e809632f5d8609b179152de5b", "google/fonts", "a73b9ab0a5df191bcfed817159a903911ea7958a"),
  {
    contentSha256: PROFILE_DIGEST,
    gitBlobSha1: null,
    id: "project.reference-profile",
    immutableRevision: PROFILE_DIGEST,
    kind: "reference-profile",
    path: "tests/oracle/reference-profiles/revision-1.json",
    url: "https://github.com/sagikazarmark/excalidraw-render.orig/blob/12119b3c9055b75e20470afa2d64f7f72e1f045e/tests/oracle/reference-profiles/revision-1.json",
  },
  {
    contentSha256: ASSET_MANIFEST_DIGEST,
    gitBlobSha1: null,
    id: "project.reference-assets",
    immutableRevision: ASSET_MANIFEST_DIGEST,
    kind: "reference-profile",
    path: "tests/oracle/reference-profiles/revision-1-assets.json",
    url: "https://github.com/sagikazarmark/excalidraw-render.orig/blob/12119b3c9055b75e20470afa2d64f7f72e1f045e/tests/oracle/reference-profiles/revision-1-assets.json",
  },
  {
    contentSha256: "69d91937938fb764c65958b217c34eb6c930c271da6984fd81c29d39ddcd7e89",
    gitBlobSha1: null,
    id: "project.compatibility-decision",
    immutableRevision: "issue-comment-5009029451",
    kind: "project-decision",
    path: null,
    url: "https://github.com/sagikazarmark/excalidraw-render.orig/issues/3#issuecomment-5009029451",
  },
];

const AUTHORITIES = [
  ["compatibility", "Define the static-export compatibility contract", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/3#issuecomment-5009029451"],
  ["outcomes", "Define public outcomes and serialized contracts", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/16#issuecomment-5014728242"],
  ["admission", "Define scene admission and forward-compatibility boundaries", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/18#issuecomment-5013009810"],
  ["metadata", "Define canonical scene preservation and artifact metadata", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/20#issuecomment-5013112249"],
  ["font-assets", "Approve the distributable font asset plan", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/23#issuecomment-5013022993"],
  ["pdf", "Clarify vector-first PDF conformance and fallback boundaries", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/25#issuecomment-5013218898"],
  ["raster", "Choose compatibility revision 1 raster normalization and SVG passthrough policy", "https://github.com/sagikazarmark/excalidraw-render.orig/issues/28#issuecomment-5013258726"],
].map(([id, title, url]) => ({ id, title, url }));

const DOMAIN_CATALOG = {
  any_json: domain(["null", "boolean", "number", "string", "array", "object"]),
  arrow_type: domain(["string"], [], ["sharp", "round", "elbow"]),
  arrowhead: domain(["null", "string"], [], [null, "arrow", "bar", "dot", "circle", "circle_outline", "triangle", "triangle_outline", "diamond", "diamond_outline", "crowfoot_one", "crowfoot_many", "crowfoot_one_or_many"]),
  boolean: domain(["boolean"]),
  bound_element_type: domain(["string"], [], ["arrow", "text"]),
  bound_element_array: domain(["null", "array"], ["every item is a nested.bound_element record"]),
  css_color: domain(["string"], ["accepted by pinned Chromium CSS color parser"]),
  chart_type: domain(["string"], [], ["bar", "line"]),
  cursor_button: domain(["string"], [], ["up", "down"]),
  data_url: domain(["string"], ["bounded canonical data: URL with a validated local payload"]),
  element_type: domain(["string"], [], ELEMENT_KINDS),
  fill_style: domain(["string"], [], ["hachure", "cross-hatch", "solid", "zigzag"]),
  finite_number: domain(["number"], ["finite"]),
  finite_number_array: domain(["array"], ["every item finite"]),
  freedraw_points: domain(["array"], ["zero or more nested.point records"]),
  fixed_segment_array: domain(["null", "array"], ["every item is a nested.fixed_segment record"]),
  font_family: domain(["number"], ["integer"], [1, 2, 3, 5, 6, 7, 8, 9]),
  frame_rendering: domain(["object"], ["closed frame_rendering record"]),
  image_crop: domain(["null", "object"], ["null or nested.image_crop record"]),
  image_mime: domain(["string"], [], ["image/svg+xml", "image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp", "image/x-icon", "image/avif", "image/jfif", "application/octet-stream"]),
  image_scale: domain(["array"], ["exactly two items", "each item is -1 or 1"]),
  image_status: domain(["string"], [], ["pending", "saved", "error"]),
  hyperlink_popup: domain(["boolean", "string"], ["boolean value may only be false"], [false, "info", "editor"]),
  integer: domain(["number"], ["integer"]),
  literal_excalidraw: domain(["string"], [], ["excalidraw"]),
  literal_v2: domain(["number"], ["integer"], [2]),
  nonempty_string: domain(["string"], ["nonempty"]),
  nonzero_integer: domain(["number"], ["integer", "!= 0"]),
  nonnegative_integer: domain(["number"], ["integer", ">= 0"]),
  nonnegative_number: domain(["number"], ["finite", ">= 0"]),
  nullable_boolean: domain(["null", "boolean"]),
  nullable_font_family: domain(["null", "number"], ["when non-null, integer"], [null, 1, 2, 3, 5, 6, 7, 8, 9]),
  nullable_point: domain(["null", "array"], ["null or nested.point record"]),
  nullable_record: domain(["null", "object"]),
  nullable_string: domain(["null", "string"]),
  pointer_type: domain(["string"], [], ["mouse", "pen", "touch"]),
  object: domain(["object"]),
  opacity: domain(["number"], ["finite", ">= 0", "<= 100"]),
  open_menu: domain(["null", "string"], [], [null, "canvas", "shape"]),
  open_popup: domain(["null", "string"], [], [null, "canvasBackground", "elementBackground", "elementStroke", "fontFamily"]),
  point: domain(["array"], ["exactly two finite numbers"]),
  point_array: domain(["array"], ["at least two nested.point records"]),
  point_binding: domain(["null", "object"], ["null or nested.point_binding/nested.fixed_point_binding record"]),
  positive_number: domain(["number"], ["finite", "> 0"]),
  pressure_array: domain(["array"], ["finite values in [0,1]", "same length as points when simulatePressure is false"]),
  roundness: domain(["null", "object"], ["null or nested.roundness record"]),
  roundness_type: domain(["number"], ["integer"], [1, 2, 3]),
  scene_elements: domain(["array"], ["members are retained as arbitrary JSON and classified at render time"]),
  string: domain(["string"]),
  string_array: domain(["array"], ["every item is a string"]),
  stroke_style: domain(["string"], [], ["solid", "dashed", "dotted"]),
  stroke_roundness: domain(["string"], [], ["round", "sharp"]),
  theme: domain(["string"], [], ["light", "dark"]),
  text_align: domain(["string"], [], ["left", "center", "right"]),
  vertical_align: domain(["string"], [], ["top", "middle", "bottom"]),
};

function readEvidence() {
  const bytes = readFileSync(EVIDENCE_PATH);
  const digest = readFileSync(EVIDENCE_DIGEST_PATH, "ascii").trim();
  if (sha256(bytes) !== digest) {
    throw new Error("scene-surface evidence digest does not match its canonical bytes");
  }
  const evidence = JSON.parse(bytes);
  if (!Buffer.from(canonicalize(evidence), "utf8").equals(bytes) || evidence.result !== "pass") {
    throw new Error("scene-surface evidence is not canonical or did not pass");
  }
  const expectedModes = ["dark", "default", "embeddedScene", "framesDisabled", "maximumDimension", "transparentBackground"];
  for (const item of evidence.cases) {
    assertExactSet(Object.keys(item.modes), expectedModes, `evidence modes for ${item.id}`);
    for (const [mode, result] of Object.entries(item.modes)) {
      if (!result.comparison.pngVisualProjectionIdentity || !result.comparison.svgVisualProjectionIdentity || !result.comparison.requestSetIdentity) {
        throw new Error(`evidence case ${item.id}/${mode} did not establish visual and request-set identity`);
      }
    }
  }
  return { digest, evidence };
}

function buildEvidenceCatalog(evidence, digest) {
  const path = pathForManifest(EVIDENCE_PATH);
  const records = [];
  for (const item of evidence.cases) {
    records.push({
      classifications: item.classifications,
      digest: `sha256:${digest}`,
      id: `oracle.${item.id}`,
      kind: "canonical-oracle",
      path,
      result: "pass",
      sourceIds: ["project.reference-profile"],
      url: null,
    });
  }
  for (const item of evidence.snapshots) {
    records.push({
      classifications: item.classifications,
      digest: `sha256:${digest}`,
      id: `oracle.${item.id}`,
      kind: "canonical-oracle",
      path,
      result: "pass",
      sourceIds: ["project.reference-profile"],
      url: null,
    });
  }
  records.push({
    classifications: ["restore defaults", "legacy migrations", "unknown-property preservation"],
    digest: "sha256:ce904e4246e529137970c3b8d8b7e38684b99a595eb18264b591c6d261ed6278",
    id: "upstream.restore-reference",
    kind: "upstream-reference",
    path: null,
    result: "pass",
    sourceIds: ["excalidraw.restore-tests", "excalidraw.restore-snapshot"],
    url: "https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/data/__snapshots__/restore.test.ts.snap",
  });
  records.push({
    classifications: ["official SVG static-export structure", "image omission", "semantic links", "frame appearance"],
    digest: "sha256:b8def935aaf60e4f31d35da6e27f4616f4563533cd89c5af3339997f17cab016",
    id: "upstream.svg-export-reference",
    kind: "upstream-reference",
    path: null,
    result: "pass",
    sourceIds: ["excalidraw.export-snapshot"],
    url: "https://github.com/excalidraw/excalidraw/blob/a2ec2889babf7d2295469c6d90ebe77fae57df84/packages/excalidraw/tests/scene/__snapshots__/export.test.ts.snap",
  });
  records.push({
    classifications: ["recipe:official.forbidden-resource", "recipe:official.nested-resource"],
    digest: "sha256:69d91937938fb764c65958b217c34eb6c930c271da6984fd81c29d39ddcd7e89",
    id: "decision.offline-resource-exception",
    kind: "approved-compatibility-exception",
    path: null,
    result: "pass",
    sourceIds: ["project.compatibility-decision", "excalidraw.render-element", "font-awesome.image", "font-awesome.ban"],
    url: "https://github.com/sagikazarmark/excalidraw-render.orig/issues/3#issuecomment-5009029451",
  });
  return records;
}

function evidenceIndex(catalog) {
  const result = new Map();
  for (const evidence of catalog) {
    for (const classification of evidence.classifications) {
      const current = result.get(classification) ?? [];
      current.push(evidence.id);
      result.set(classification, current);
    }
  }
  return result;
}

function buildFieldDispositions(evidenceByClassification) {
  const fields = [];
  const add = (...args) => fields.push(field(...args));
  const allVisual = scope(["visual"], "participating-record");
  const globalVisual = scope(["visual"], "global", FORMATS, [], [], "root and app state always participate");
  const irrelevant = scope([], "containing-record", FORMATS, [], [], "preserved in the canonicalized original scene document but not consumed by static export");
  const ignored = fallback("ignore", { recovery: "ignored" });
  const unsupportedValue = (value, effects = ["input_repair", "visual"], recovery = "substituted") => fallback("substitute", {
    code: "compatibility.unsupported-property-value",
    effects,
    recovery,
    value,
  });
  const ev = (classification) => evidenceByClassification.get(classification) ?? [];

  add("scene", "type", "literal_excalidraw", "supported", {
    invalid: fallback("fatal", { code: "scene.invalid-envelope" }),
    missing: fallback("fatal", { code: "scene.invalid-envelope" }),
    required: true,
    scope: scope([], "global"),
    sourceIds: ["excalidraw.data-types"],
  });
  add("scene", "version", "literal_v2", "supported", {
    invalid: fallback("fatal", { code: "scene.invalid-envelope" }),
    missing: fallback("fatal", { code: "scene.invalid-envelope" }),
    required: true,
    scope: scope([], "global"),
    sourceIds: ["excalidraw.data-types", "excalidraw.constants"],
  });
  for (const [name, domainId] of [["elements", "scene_elements"], ["appState", "object"], ["files", "object"]]) {
    add("scene", name, domainId, "supported", {
      invalid: fallback("fatal", { code: "scene.invalid-envelope" }),
      missing: fallback("fatal", { code: "scene.invalid-envelope" }),
      required: true,
      scope: scope([], "global"),
      sourceIds: ["excalidraw.data-types"],
    });
  }
  add("scene", "source", "string", "supported", {
    invalid: ignored,
    normalization: "Preserve the source value in the canonicalized original scene document; it affects only opt-in embedded-scene metadata and never visual rendering.",
    scope: scope(["metadata"], "global", FORMATS, ["embedded_scene"], [], "participates only in the embedded canonicalized original scene document"),
    sourceIds: ["excalidraw.data-types", "excalidraw.scene-export"],
  });
  add("scene", "scrollToContent", "boolean", "static-export-irrelevant", {
    invalid: ignored,
    scope: irrelevant,
    sourceIds: ["excalidraw.data-types"],
  });
  add("scene", "libraryItems", "any_json", "opaque-metadata", {
    invalid: ignored,
    scope: irrelevant,
    sourceIds: ["excalidraw.data-types"],
  });

  const supportedAppState = {
    exportBackground: ["boolean", true, ["visual"]],
    exportEmbedScene: ["boolean", false, ["metadata"]],
    exportScale: ["positive_number", 1, ["visual"]],
    exportWithDarkMode: ["boolean", false, ["visual"]],
    frameRendering: ["frame_rendering", { clip: true, enabled: true, name: true, outline: true }, ["visual"]],
    viewBackgroundColor: ["css_color", "#ffffff", ["visual"]],
  };
  for (const [name, [domainId, defaultValue, effects]] of Object.entries(supportedAppState)) {
    add("app_state", name, domainId, "supported", {
      invalid: name === "viewBackgroundColor"
        ? fallback("substitute", { code: "color.invalid", effects: ["input_repair", "visual"], recovery: "substituted", value: defaultValue })
        : unsupportedValue(defaultValue, ["input_repair", ...effects]),
      missing: fallback("default", { value: defaultValue }),
      normalization: "The public render request overrides the scene value when explicitly supplied; otherwise the revision-1 default is used.",
      scope: scope(effects, "global", FORMATS, [name]),
      sourceIds: ["excalidraw.app-state", "excalidraw.public-export", "excalidraw.scene-export"],
    });
  }
  for (const name of ["enabled", "clip", "name", "outline"]) {
    add("frame_rendering", name, "boolean", "supported", {
      invalid: unsupportedValue(true),
      missing: fallback("default", { value: true }),
      scope: globalVisual,
      sourceIds: ["excalidraw.types", "excalidraw.scene-export"],
    });
  }

  const appBoolean = new Set([
    "showWelcomeScreen", "isLoading", "isBindingEnabled", "penMode", "penDetected",
    "scrolledOutside", "isResizing", "isRotating", "defaultSidebarDockedPreference",
    "selectedElementsAreBeingDragged", "shouldCacheIgnoreZoom", "zenModeEnabled",
    "gridModeEnabled", "viewModeEnabled", "objectsSnapModeEnabled", "isCropping",
  ]);
  const appNumber = new Set([
    "currentItemStrokeWidth", "currentItemRoughness", "currentItemOpacity", "currentItemFontFamily",
    "currentItemFontSize", "scrollX", "scrollY", "gridSize", "gridStep", "width", "height",
    "offsetTop", "offsetLeft",
  ]);
  const appString = new Set([
    "currentItemStrokeColor", "currentItemBackgroundColor", "currentItemFillStyle",
    "currentItemStrokeStyle", "currentItemTextAlign", "currentItemStartArrowhead",
    "currentItemEndArrowhead", "currentHoveredFontFamily", "currentItemRoundness",
    "currentItemArrowType", "cursorButton", "name", "openMenu", "openPopup", "lastPointerDownWith",
    "theme", "currentChartType", "pendingImageElementId", "showHyperlinkPopup", "editingGroupId",
    "editingFrame", "croppingElementId", "isSidebarDocked",
  ]);
  const appSpecificDomain = {
    currentChartType: "chart_type",
    currentHoveredFontFamily: "nullable_font_family",
    currentItemBackgroundColor: "string",
    currentItemArrowType: "arrow_type",
    currentItemEndArrowhead: "arrowhead",
    currentItemFillStyle: "fill_style",
    currentItemFontFamily: "font_family",
    currentItemRoundness: "stroke_roundness",
    currentItemStrokeColor: "string",
    currentItemStartArrowhead: "arrowhead",
    currentItemStrokeStyle: "stroke_style",
    currentItemTextAlign: "text_align",
    cursorButton: "cursor_button",
    editingFrame: "nullable_string",
    croppingElementId: "nullable_string",
    isSidebarDocked: "boolean",
    lastPointerDownWith: "pointer_type",
    openMenu: "open_menu",
    openPopup: "open_popup",
    pendingImageElementId: "nullable_string",
    showHyperlinkPopup: "hyperlink_popup",
    theme: "theme",
  };
  for (const name of APP_STATE_FIELDS) {
    if (Object.hasOwn(supportedAppState, name)) continue;
    const classification = `app_state:/${name}`;
    const isNewer = ["gridModeEnabled", "gridSize", "gridStep"].includes(name);
    const acceptedDomain = appSpecificDomain[name] ?? (appBoolean.has(name)
      ? "boolean"
      : appNumber.has(name)
        ? "finite_number"
        : appString.has(name)
          ? "nullable_string"
          : "any_json");
    add("app_state", name, acceptedDomain, acceptedDomain === "any_json" ? "opaque-metadata" : "static-export-irrelevant", {
      evidenceIds: isNewer ? ev(classification) : [],
      harmlessNewerBehavior: isNewer,
      invalid: ignored,
      normalization: "Static export fixes editor overlays, viewport state, selection state, and editor-only controls independently of persisted app state.",
      scope: irrelevant,
      sourceIds: ["excalidraw.types", "excalidraw.app-state", "excalidraw.public-export"],
    });
  }

  add("file_record", "$key", "nonempty_string", "supported", {
    invalid: fallback("omit", { code: "asset.malformed-file-record", effects: ["visual"], recovery: "omitted" }),
    required: true,
    scope: scope(["visual"], "referenced-authoritative-file"),
    sourceIds: ["excalidraw.types", "excalidraw.static-svg"],
  });
  add("file_record", "id", "nonempty_string", "supported", {
    invalid: fallback("repair", { code: "compatibility.unsupported-property-value", effects: ["input_repair"], recovery: "repaired", value: "map key" }),
    missing: fallback("repair", { code: "compatibility.unsupported-property-value", effects: ["input_repair"], recovery: "repaired", value: "map key" }),
    required: true,
    scope: scope(["visual", "semantic"], "referenced-authoritative-file"),
    sourceIds: ["excalidraw.types", "excalidraw.static-svg"],
  });
  add("file_record", "mimeType", "image_mime", "supported", {
    invalid: fallback("repair", { code: "asset.media-type-mismatch", effects: ["input_repair"], recovery: "repaired", value: "validated media kind" }),
    required: true,
    scope: scope(["input_repair", "visual"], "referenced-authoritative-file"),
    sourceIds: ["excalidraw.types", "excalidraw.constants"],
  });
  add("file_record", "dataURL", "data_url", "supported", {
    invalid: fallback("replace", {
      code: "compatibility.unsupported-property-value",
      effects: ["visual"],
      formatRules: {
        pdf: { action: "omit malformed source image", recovery: "omitted" },
        png: { action: "paint status-selected official image fallback", recovery: "placeholder" },
        svg: { action: "omit malformed source image", recovery: "omitted" },
      },
      recovery: null,
      value: "format-specific official image fallback",
    }),
    missing: fallback("replace", {
      code: "asset.malformed-file-record",
      effects: ["visual"],
      formatRules: {
        pdf: { action: "omit malformed source image", recovery: "omitted" },
        png: { action: "paint status-selected official image fallback", recovery: "placeholder" },
        svg: { action: "omit malformed source image", recovery: "omitted" },
      },
      recovery: null,
      value: "format-specific official image fallback",
    }),
    normalization: "Direct SVG wraps exact validated raster payload octets in a canonical data URL without decoding; PNG and PDF select the reference default frame and apply pinned orientation/color/HDR interpretation before deterministic static encoding. Sanitized local SVG follows bounded nested-resource rules.",
    required: true,
    scope: scope(["visual", "semantic"], "referenced-authoritative-file"),
    sourceIds: ["excalidraw.types", "excalidraw.image", "excalidraw.static-svg"],
    valueRules: [
      {
        code: "image.forbidden-resource",
        formatRules: {
          pdf: [{ action: "paint forbidden-resource recipe", recovery: "placeholder", when: "safe finite image bounds exist" }, { action: "omit source image", recovery: "omitted", when: "safe finite image bounds are unavailable" }],
          png: [{ action: "paint forbidden-resource recipe", recovery: "placeholder", when: "safe finite image bounds exist" }, { action: "omit source image", recovery: "omitted", when: "safe finite image bounds are unavailable" }],
          svg: [{ action: "paint forbidden-resource recipe", recovery: "placeholder", when: "safe finite image bounds exist" }, { action: "omit source image", recovery: "omitted", when: "safe finite image bounds are unavailable" }],
        },
        recovery: null,
        when: "value is a string that is not a local data: URL",
      },
      {
        code: "image.unsupported-format",
        formatRules: {
          pdf: { action: "omit source image", recovery: "omitted" },
          png: { action: "paint status-selected official image fallback", recovery: "placeholder" },
          svg: { action: "omit source image", recovery: "omitted" },
        },
        recovery: null,
        when: "canonical data: decoding succeeds but the validated media family is not accepted by revision 1",
      },
      {
        code: "image.decode-failed",
        formatRules: {
          pdf: { action: "omit source image", recovery: "omitted" },
          png: { action: "paint status-selected official image fallback", recovery: "placeholder" },
          svg: { action: "omit source image", recovery: "omitted" },
        },
        recovery: null,
        when: "validated accepted media bytes are malformed or fail deterministic decoding/sanitization",
      },
    ],
  });
  for (const name of ["created", "lastRetrieved", "version"]) {
    add("file_record", name, "nonnegative_integer", "static-export-irrelevant", {
      evidenceIds: ev(`file_record:/${name}`),
      harmlessNewerBehavior: true,
      invalid: ignored,
      missing: name === "created"
        ? fallback("repair", { code: "normalization.missing-timestamp", effects: ["input_repair"], recovery: "repaired", value: 0 })
        : fallback("accept", { when: "the optional lifecycle metadata is absent" }),
      scope: irrelevant,
      sourceIds: ["excalidraw.types"],
    });
  }

  const commonDefinitions = {
    type: ["element_type", fallback("replace", { code: "compatibility.malformed-element", effects: ["visual"], recovery: "placeholder", recipe: "renderer.malformed-element.v1" }), null],
    id: ["nonempty_string", fallback("repair", { code: "compatibility.unsupported-property-value", effects: ["input_repair"], recovery: "repaired", value: "derived deterministic ID" }), "derived deterministic ID"],
    x: ["finite_number", unsupportedValue(0), 0],
    y: ["finite_number", unsupportedValue(0), 0],
    strokeColor: ["css_color", fallback("substitute", { code: "color.invalid", effects: ["input_repair", "visual"], recovery: "substituted", value: "#1e1e1e" }), "#1e1e1e"],
    backgroundColor: ["css_color", fallback("substitute", { code: "color.invalid", effects: ["input_repair", "visual"], recovery: "substituted", value: "transparent" }), "transparent"],
    fillStyle: ["fill_style", unsupportedValue("solid"), "solid"],
    strokeWidth: ["positive_number", unsupportedValue(2), 2],
    strokeStyle: ["stroke_style", unsupportedValue("solid"), "solid"],
    roundness: ["roundness", unsupportedValue(null), null],
    roughness: ["nonnegative_number", unsupportedValue(1), 1],
    opacity: ["opacity", unsupportedValue(100), 100],
    width: ["finite_number", unsupportedValue(0), 0],
    height: ["finite_number", unsupportedValue(0), 0],
    angle: ["finite_number", unsupportedValue(0), 0],
    seed: ["nonzero_integer", unsupportedValue("nonzero deterministic seed"), 1],
    isDeleted: ["boolean", unsupportedValue(false, ["input_repair"]), false],
    groupIds: ["string_array", unsupportedValue([], ["input_repair"]), []],
    frameId: ["nullable_string", unsupportedValue(null, ["input_repair", "visual"]), null],
    boundElements: ["bound_element_array", unsupportedValue([], ["input_repair", "visual", "semantic"]), []],
    link: ["nullable_string", fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["semantic"], recovery: "omitted", value: null }), null],
  };
  for (const [name, [domainId, invalid, defaultValue]] of Object.entries(commonDefinitions)) {
    const missing = name === "type"
      ? fallback("replace", { code: "compatibility.malformed-element", effects: ["visual"], recovery: "placeholder", recipe: "renderer.malformed-element.v1" })
      : name === "id"
        ? fallback("repair", { code: "normalization.missing-element-id", effects: ["input_repair"], recovery: "repaired", value: defaultValue })
        : fallback("default", { value: defaultValue });
    const valueRules = name === "type"
      ? [
          { code: "compatibility.malformed-element", recovery: "placeholder", when: "value is absent, empty, or not a string and safe finite bounds are recoverable" },
          { code: "compatibility.malformed-element", recovery: "omitted", when: "value is absent, empty, or not a string and safe finite bounds are unavailable" },
          { code: "compatibility.unsupported-element-type", recovery: "placeholder", when: "value is a nonempty unrecognized string and safe finite bounds are recoverable" },
          { code: "compatibility.unsupported-element-type", recovery: "omitted", when: "value is a nonempty unrecognized string and safe finite bounds are unavailable" },
        ]
      : name === "seed"
        ? [{ code: "normalization.zero-rough-seed", recovery: "repaired", value: "nonzero seed derived from compatibility revision and normalized element identity", when: "value is integer zero" }]
        : name === "link"
          ? [{
              code: "link.unsafe-omitted",
              formatRules: {
                pdf: { action: "omit URI annotation", recovery: "omitted" },
                svg: { action: "omit anchor", recovery: "omitted" },
              },
              recovery: null,
              when: "value is a string rejected by the pinned URL sanitizer and the selected format consumes links; PNG is silent",
            }]
        : [];
    add("element.common", name, domainId, "supported", {
      invalid,
      missing,
      normalization: ["width", "height"].includes(name) ? "Negative dimensions shift the origin and become absolute before rendering." : "Pinned restore/default behavior followed by deterministic revision-1 repairs.",
      scope: name === "link"
        ? scope(["semantic"], "participating-linked-element", ["svg", "pdf"], [], [], "link channel participates only in semantic SVG/PDF output; PNG is silent")
        : allVisual,
      sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.shape"],
      valueRules,
    });
  }
  for (const [name, domainId] of [["version", "integer"], ["versionNonce", "integer"], ["updated", "nonnegative_integer"], ["locked", "boolean"]]) {
    add("element.common", name, domainId, "static-export-irrelevant", {
      invalid: ignored,
      missing: name === "updated"
        ? fallback("repair", { code: "normalization.missing-timestamp", effects: ["input_repair"], recovery: "repaired", value: 0 })
        : fallback("default", { value: name === "version" ? 1 : name === "locked" ? false : 0 }),
      normalization: name === "updated" ? "Missing required timestamps use canonical zero and normalization.missing-timestamp." : "Preserved but not painted.",
      scope: irrelevant,
      sourceIds: ["excalidraw.element-types", "excalidraw.restore"],
    });
  }
  add("element.common", "index", "nullable_string", "static-export-irrelevant", {
    evidenceIds: ev("element.common:/index"),
    harmlessNewerBehavior: true,
    invalid: ignored,
    normalization: "Repair invalid fractional indices in source array order without reordering the array.",
    scope: irrelevant,
    sourceIds: ["excalidraw.element-types", "excalidraw.fractional-index"],
  });
  add("element.common", "customData", "any_json", "opaque-metadata", {
    invalid: ignored,
    scope: irrelevant,
    sourceIds: ["excalidraw.element-types"],
  });
  add("element.common", "strokeSharpness", "string", "supported", {
    invalid: unsupportedValue(null),
    normalization: "Legacy round/sharp values migrate to the corresponding roundness record before rendering.",
    scope: allVisual,
    sourceIds: ["excalidraw.restore"],
  });
  add("element.common", "boundElementIds", "string_array", "supported", {
    invalid: unsupportedValue([], ["input_repair", "visual", "semantic"]),
    normalization: "When boundElements is absent, legacy IDs migrate to arrow bound-element records.",
    scope: scope(["input_repair", "visual", "semantic"], "participating-element"),
    sourceIds: ["excalidraw.restore"],
  });

  const textDefinitions = {
    fontSize: ["positive_number", 20, ["visual"]],
    fontFamily: ["font_family", 5, ["visual", "semantic"]],
    text: ["string", "", ["visual", "semantic"]],
    textAlign: ["text_align", "left", ["visual"]],
    verticalAlign: ["vertical_align", "top", ["visual"]],
    containerId: ["nullable_string", null, ["visual", "semantic"]],
    originalText: ["string", "source text", ["visual"]],
    lineHeight: ["positive_number", "pinned family default", ["visual"]],
  };
  for (const [name, [domainId, defaultValue, effects]] of Object.entries(textDefinitions)) {
    add("element.text", name, domainId, "supported", {
      invalid: unsupportedValue(defaultValue, ["input_repair", ...effects]),
      missing: fallback("default", { value: defaultValue }),
      normalization: name === "text" ? "Empty text is marked deleted by pinned restore behavior." : "Pinned text restore/default behavior.",
      scope: scope(effects, "participating-text-element"),
      sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.static-svg", "excalidraw.render-element"],
    });
  }
  add("element.text", "autoResize", "boolean", "static-export-irrelevant", {
    evidenceIds: ev("element.text:/autoResize"),
    harmlessNewerBehavior: true,
    invalid: fallback("ignore", { recovery: "ignored" }),
    normalization: "The public export wrapper does not refresh persisted text dimensions.",
    scope: irrelevant,
    sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.public-export"],
  });
  add("element.text", "font", "string", "supported", {
    invalid: unsupportedValue("independent fontSize/fontFamily defaults"),
    normalization: "Legacy '<size>px <family>' text migrates to fontSize and fontFamily.",
    scope: scope(["input_repair", "visual"], "participating-text-element"),
    sourceIds: ["excalidraw.restore"],
  });
  for (const name of ["rawText", "baseline"]) {
    add("element.text", name, "any_json", "static-export-irrelevant", {
      evidenceIds: ev(`element.text:/${name}`),
      harmlessNewerBehavior: true,
      invalid: fallback("ignore", { recovery: "ignored" }),
      normalization: "Legacy text metadata is removed before static rendering.",
      scope: irrelevant,
      sourceIds: ["excalidraw.restore"],
    });
  }

  const linearDefinitions = {
    points: ["point_array", "[[0,0],[width,height]]", ["input_repair", "visual"]],
    lastCommittedPoint: ["nullable_point", null, []],
    startBinding: ["point_binding", null, ["input_repair", "visual"]],
    endBinding: ["point_binding", null, ["input_repair", "visual"]],
  };
  for (const [name, [domainId, defaultValue, effects]] of Object.entries(linearDefinitions)) {
    const isLast = name === "lastCommittedPoint";
    add("element.linear", name, domainId, isLast ? "static-export-irrelevant" : "supported", {
      invalid: isLast ? fallback("ignore", { recovery: "ignored" }) : unsupportedValue(defaultValue, effects),
      missing: fallback("default", { value: defaultValue }),
      normalization: isLast
        ? "Always restored to null before static rendering."
        : name === "points"
          ? "Synthesize missing short geometry, move the first point to [0,0], and repair origin and dimensions."
          : "Pinned linear-element restore behavior.",
      scope: isLast ? irrelevant : scope(effects, "participating-linear-element"),
      sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.shape"],
    });
  }
  for (const [recordKind, endDefault] of [["element.line", null], ["element.arrow", "arrow"]]) {
    for (const [name, defaultValue] of [["startArrowhead", null], ["endArrowhead", endDefault]]) {
      add(recordKind, name, "arrowhead", recordKind === "element.arrow" ? "supported" : "static-export-irrelevant", {
        invalid: unsupportedValue(defaultValue),
        missing: fallback("default", { value: defaultValue }),
        normalization: "Pinned kind-specific arrowhead restore/default behavior.",
        scope: recordKind === "element.arrow" ? scope(["visual"], "participating-arrow") : irrelevant,
        sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.shape"],
      });
    }
  }
  add("element.arrow", "elbowed", "boolean", "supported", {
    invalid: unsupportedValue(false),
    missing: fallback("default", { value: false }),
    scope: scope(["visual"], "participating-arrow"),
    sourceIds: ["excalidraw.element-types", "excalidraw.shape"],
  });
  for (const [name, domainId] of [["fixedSegments", "fixed_segment_array"], ["startIsSpecial", "nullable_boolean"], ["endIsSpecial", "nullable_boolean"]]) {
    add("element.arrow", name, domainId, "static-export-irrelevant", {
      evidenceIds: ev(`element.arrow:/${name}`),
      harmlessNewerBehavior: true,
      invalid: fallback("ignore", { recovery: "ignored" }),
      normalization: "Persisted elbow-routing metadata does not replace the authoritative points array in static rendering.",
      scope: irrelevant,
      sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.shape"],
    });
  }

  for (const [name, domainId, defaultValue, effects] of [
    ["points", "freedraw_points", [], ["visual"]],
    ["pressures", "pressure_array", [], ["visual"]],
    ["simulatePressure", "boolean", true, ["visual"]],
  ]) {
    add("element.freedraw", name, domainId, "supported", {
      invalid: name === "points"
        ? fallback("replace", { code: "compatibility.unsupported-property-value", effects, recovery: "placeholder", recipe: "renderer.known-element-unavailable.v1" })
        : unsupportedValue(defaultValue, ["input_repair", ...effects]),
      missing: fallback("default", { value: defaultValue }),
      scope: scope(effects, "participating-freedraw-element"),
      sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.shape"],
    });
  }
  add("element.freedraw", "lastCommittedPoint", "nullable_point", "static-export-irrelevant", {
    invalid: fallback("ignore", { recovery: "ignored" }),
    normalization: "Always restored to null before static rendering.",
    scope: irrelevant,
    sourceIds: ["excalidraw.element-types", "excalidraw.restore"],
  });

  for (const [name, domainId, defaultValue, effects] of [
    ["fileId", "nullable_string", null, ["visual"]],
    ["status", "image_status", "pending", ["visual"]],
    ["scale", "image_scale", [1, 1], ["visual"]],
    ["crop", "image_crop", null, ["visual"]],
  ]) {
    add("element.image", name, domainId, "supported", {
      invalid: unsupportedValue(defaultValue, ["input_repair", ...effects]),
      missing: name === "fileId"
        ? fallback("replace", {
            code: "image.missing-file",
            effects,
            formatRules: {
              pdf: { action: "omit source image paint", recovery: "omitted" },
              png: { action: "paint status-selected official image fallback", recovery: "placeholder" },
              svg: { action: "omit source image paint", recovery: "omitted" },
            },
            recovery: null,
            value: null,
          })
        : fallback("default", { value: defaultValue }),
      scope: scope(effects, "participating-image-element"),
      sourceIds: ["excalidraw.element-types", "excalidraw.restore", "excalidraw.image", "excalidraw.static-svg"],
      valueRules: name === "fileId"
        ? [{
            code: "image.missing-file",
            formatRules: {
              pdf: { action: "omit source image paint", recovery: "omitted" },
              png: { action: "paint status-selected official image fallback", recovery: "placeholder" },
              svg: { action: "omit source image paint", recovery: "omitted" },
            },
            recovery: null,
            when: "value is null or names no authoritative scene/supplemental payload",
          }]
        : [],
    });
  }
  for (const recordKind of ["element.frame", "element.magicframe"]) {
    add(recordKind, "name", "nullable_string", "supported", {
      invalid: unsupportedValue(null),
      missing: fallback("default", { value: null }),
      scope: scope(["visual", "semantic"], "participating-frame-when-frame-name-rendering-is-enabled", FORMATS, ["frameRendering.name"]),
      sourceIds: ["excalidraw.element-types", "excalidraw.scene-export"],
    });
  }
  add("element.iframe", "customData/generationData", "any_json", "opaque-metadata", {
    evidenceIds: ev("element.iframe:/customData/generationData"),
    harmlessNewerBehavior: true,
    invalid: fallback("ignore", { recovery: "ignored" }),
    normalization: "Generation state and generated HTML are never executed by a static export.",
    scope: irrelevant,
    sourceIds: ["excalidraw.element-types", "excalidraw.embeddable"],
  });
  add("element.embeddable", "validated", "any_json", "static-export-irrelevant", {
    invalid: fallback("ignore", { recovery: "ignored" }),
    normalization: "Legacy editor validation state does not affect the offline static placeholder.",
    scope: irrelevant,
    sourceIds: ["excalidraw.embeddable"],
  });
  add("element.legacy_draw", "type", "string", "supported", {
    invalid: fallback("replace", { code: "compatibility.unsupported-element-type", effects: ["visual", "semantic"], recovery: "placeholder", recipe: "renderer.unknown-element.v1" }),
    normalization: "The exact legacy value 'draw' migrates to line before linear normalization.",
    required: true,
    scope: scope(["input_repair", "visual"], "participating-legacy-draw-element"),
    sourceIds: ["excalidraw.restore"],
  });

  for (const [name, domainId, defaultValue] of [["type", "roundness_type", null], ["value", "nonnegative_number", null]]) {
    add("nested.roundness", name, domainId, "supported", {
      invalid: unsupportedValue(defaultValue),
      missing: fallback("default", { value: defaultValue }),
      scope: scope(["visual"], "owning-element-consumes-roundness"),
      sourceIds: ["excalidraw.element-types", "excalidraw.shape"],
    });
  }
  for (const [name, domainId] of [["id", "nonempty_string"], ["type", "bound_element_type"]]) {
    add("nested.bound_element", name, domainId, "supported", {
      invalid: fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["input_repair", "visual", "semantic"], recovery: "omitted" }),
      missing: fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["input_repair", "visual", "semantic"], recovery: "omitted" }),
      required: true,
      scope: scope(["visual", "semantic"], "owning-element-and-target-participate"),
      sourceIds: ["excalidraw.element-types", "excalidraw.restore"],
    });
  }
  for (const name of ["0", "1"]) {
    add("nested.point", name, "finite_number", "supported", {
      invalid: fallback("replace", { code: "compatibility.unsupported-property-value", effects: ["visual"], recovery: "placeholder", recipe: "renderer.known-element-unavailable.v1" }),
      missing: fallback("replace", { code: "compatibility.unsupported-property-value", effects: ["visual"], recovery: "placeholder", recipe: "renderer.known-element-unavailable.v1" }),
      required: true,
      scope: scope(["visual"], "owning-geometry-participates"),
      sourceIds: ["excalidraw.element-types", "excalidraw.shape"],
    });
  }
  for (const recordKind of ["nested.point_binding", "nested.fixed_point_binding"]) {
    for (const [name, domainId] of [["elementId", "nonempty_string"], ["focus", "finite_number"], ["gap", "finite_number"]]) {
      add(recordKind, name, domainId, "supported", {
        invalid: fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["input_repair", "visual"], recovery: "omitted" }),
        missing: name === "focus"
          ? fallback("default", { value: 0, when: "focus is absent or falsy under pinned binding normalization" })
          : fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["input_repair", "visual"], recovery: "omitted" }),
        normalization: name === "focus" ? "Pinned binding normalization silently defaults absent or falsy focus to 0 before compatibility classification." : "none",
        required: true,
        scope: scope(["input_repair", "visual"], "owning-linear-element-participates"),
        sourceIds: ["excalidraw.element-types", "excalidraw.restore"],
      });
    }
  }
  add("nested.fixed_point_binding", "fixedPoint", "point", "supported", {
    invalid: unsupportedValue([0, 0]),
    missing: fallback("omit", { recovery: "omitted", value: "binding becomes null", when: "an elbow-arrow binding has no non-null own fixedPoint and is silently removed by pinned binding normalization" }),
    normalization: "Components within 1e-4 of 0.5 migrate to 0.5001.",
    required: true,
    scope: scope(["input_repair", "visual"], "owning-elbow-arrow-participates"),
    sourceIds: ["excalidraw.element-types", "excalidraw.restore"],
    valueRules: [{
      code: null,
      recovery: "omitted",
      when: "value is null; pinned isFixedPointBinding rejects it and silently removes the elbow binding before compatibility classification",
    }],
  });
  for (const [name, domainId] of [["start", "point"], ["end", "point"], ["index", "nonnegative_integer"]]) {
    add("nested.fixed_segment", name, domainId, "static-export-irrelevant", {
      evidenceIds: ev("element.arrow:/fixedSegments"),
      harmlessNewerBehavior: true,
      invalid: fallback("ignore", { recovery: "ignored" }),
      scope: irrelevant,
      sourceIds: ["excalidraw.element-types", "excalidraw.shape"],
    });
  }
  for (const name of ["x", "y", "width", "height", "naturalWidth", "naturalHeight"]) {
    add("nested.image_crop", name, name === "x" || name === "y" ? "nonnegative_number" : "positive_number", "supported", {
      invalid: fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["visual"], recovery: "omitted", value: "crop disabled" }),
      missing: fallback("omit", { code: "compatibility.unsupported-property-value", effects: ["visual"], recovery: "omitted", value: "crop disabled" }),
      required: true,
      scope: scope(["visual"], "owning-image-and-authoritative-file-participate"),
      sourceIds: ["excalidraw.element-types", "excalidraw.static-svg", "excalidraw.render-element"],
    });
  }
  for (const [name, domainId] of [["status", "string"], ["html", "string"], ["message", "string"], ["code", "string"]]) {
    add("nested.generation_data", name, domainId, "opaque-metadata", {
      evidenceIds: ev("element.iframe:/customData/generationData"),
      harmlessNewerBehavior: true,
      invalid: fallback("ignore", { recovery: "ignored" }),
      scope: irrelevant,
      sourceIds: ["excalidraw.element-types", "excalidraw.embeddable"],
    });
  }
  add("nested.local_svg_resource", "*", "any_json", "supported", {
    invalid: fallback("replace", { code: "image.forbidden-resource", effects: ["visual", "semantic"], recovery: "placeholder", value: "smallest safe finite nested viewport" }),
    normalization: "Sanitize locally, remove active content, and recursively admit only bounded local data payloads. XML identity uses nested ordinal paths, not invented JSON pointers.",
    scope: scope(["visual", "semantic"], "referenced-nested-resource"),
    sourceIds: ["excalidraw.static-svg", "excalidraw.image"],
  });

  return fields;
}

function buildRecordKinds(fields) {
  const inheritance = {
    "element.arrow": ["element.linear", "element.common"],
    "element.embeddable": ["element.common"],
    "element.frame": ["element.common"],
    "element.freedraw": ["element.common"],
    "element.iframe": ["element.common"],
    "element.image": ["element.common"],
    "element.legacy_draw": ["element.linear", "element.common"],
    "element.line": ["element.linear", "element.common"],
    "element.magicframe": ["element.common"],
    "element.rectangle": ["element.common"],
    "element.diamond": ["element.common"],
    "element.ellipse": ["element.common"],
    "element.selection": ["element.common"],
    "element.text": ["element.common"],
    "nested.fixed_point_binding": ["nested.point_binding"],
  };
  return RECORD_KINDS.map((id) => ({
    fieldPaths: fields.filter((entry) => entry.recordKind === id).map((entry) => entry.path).sort(),
    id,
    inherits: inheritance[id] ?? [],
    unmatchedProperties: id === "nested.local_svg_resource"
      ? "Use nested ordinal paths and the local SVG sanitizer policy."
      : "Apply the conservative unmatched-path rule at the narrowest property boundary.",
  }));
}

function buildElementKinds() {
  const ignoredInherited = {
    selection: COMMON_FIELDS.map((name) => `/${name}`),
    rectangle: [],
    diamond: [],
    ellipse: ["/roundness"],
    text: ["/backgroundColor", "/fillStyle", "/strokeWidth", "/strokeStyle", "/roundness", "/roughness", "/seed"],
    line: [],
    arrow: ["/backgroundColor", "/fillStyle"],
    freedraw: ["/roundness", "/roughness", "/seed"],
    image: ["/strokeColor", "/backgroundColor", "/fillStyle", "/strokeWidth", "/strokeStyle", "/roughness", "/seed"],
    frame: ["/strokeColor", "/backgroundColor", "/fillStyle", "/strokeWidth", "/strokeStyle", "/roundness", "/roughness", "/seed", "/boundElements"],
    magicframe: ["/strokeColor", "/backgroundColor", "/fillStyle", "/strokeWidth", "/strokeStyle", "/roundness", "/roughness", "/seed", "/boundElements"],
    iframe: [],
    embeddable: [],
  };
  const ownRecord = {
    arrow: "element.arrow",
    embeddable: "element.embeddable",
    frame: "element.frame",
    freedraw: "element.freedraw",
    iframe: "element.iframe",
    image: "element.image",
    line: "element.line",
    magicframe: "element.magicframe",
    rectangle: "element.rectangle",
    diamond: "element.diamond",
    ellipse: "element.ellipse",
    selection: "element.selection",
    text: "element.text",
  };
  return ELEMENT_KINDS.map((kind) => ({
    behavior: kind === "selection"
      ? "Restore filters the editor-only selection element; it is preserved only in the canonicalized original scene document."
      : kind === "iframe" || kind === "embeddable"
        ? "Render the official offline shell and label in the final iframe-like paint pass."
        : kind === "image"
          ? "Resolve the authoritative local file and apply format-specific raster or local-SVG behavior."
          : "Render the complete understood static-export semantics for this kind.",
    ignoredInheritedPaths: ignoredInherited[kind],
    kind,
    recordKind: ownRecord[kind],
    status: kind === "selection" ? "static-export-irrelevant" : "supported",
    unknownValueFallback: kind === "selection"
      ? "omit silently"
      : "Use the narrowest field fallback; replace with renderer.known-element-unavailable.v1 only when no meaningful known rendering remains.",
  }));
}

function notice(id, assetIdentity, text, requiredFormats, sourceIds) {
  return {
    assetIdentity,
    id,
    requiredFormats,
    sourceIds,
    text,
    textSha256: sha256(Buffer.from(text, "utf8")),
  };
}

const OFL_BODY = `SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The fonts,
including any derivative works, can be bundled, embedded, redistributed
and/or sold with any software provided that any reserved names are not used
by derivative works. The fonts and derivatives, however, cannot be released
under any other type of license. The requirement for fonts to remain under
this license does not apply to any document created using the fonts or their
derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may include
source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical writer or
other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining a copy
of the Font Software, to use, study, copy, merge, embed, modify, redistribute,
and sell modified and unmodified copies of the Font Software, subject to the
following conditions:

1) Neither the Font Software nor any of its individual components, in Original
or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy contains
the above copyright notice and this license. These can be included either as
stand-alone text files, human-readable headers or in the appropriate
machine-readable metadata fields within text or binary files as long as those
fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font Name(s)
unless explicit written permission is granted by the corresponding Copyright
Holder. This restriction only applies to the primary font name as presented
to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font Software
shall not be used to promote, endorse or advertise any Modified Version,
except to acknowledge the contribution(s) of the Copyright Holder(s) and the
Author(s) or with their explicit written permission.

5) The Font Software, modified or unmodified, in part or in whole, must be
distributed entirely under this license, and must not be distributed under
any other license. The requirement for fonts to remain under this license does
not apply to any document created using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF COPYRIGHT, PATENT,
TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE FOR
ANY CLAIM, DAMAGES OR OTHER LIABILITY, INCLUDING ANY GENERAL, SPECIAL,
INDIRECT, INCIDENTAL, OR CONSEQUENTIAL DAMAGES, WHETHER IN AN ACTION OF
CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF THE USE OR INABILITY TO USE
THE FONT SOFTWARE OR FROM OTHER DEALINGS IN THE FONT SOFTWARE.`;

const LIBERATION_NOTICE = `Digitized data copyright (c) 2010 Google Corporation
with Reserved Font Arimo, Tinos and Cousine.
Copyright (c) 2012 Red Hat, Inc.
with Reserved Font Name Liberation.

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL

${OFL_BODY}`;

const NOTO_EMOJI_NOTICE = `Copyright 2013 Google LLC

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
https://scripts.sil.org/OFL

${OFL_BODY}`;

function buildNoticeCatalog() {
  return [
    notice(
      "font-awesome.image.cc-by-4.0",
      "font-awesome-free-5.15.4/solid/image@sha256:f3a4dff85d862e91501af10000d758898ca2bc3055ea451423ddedac7f41c1d9",
      "Font Awesome Free 5.15.4 by @fontawesome - https://fontawesome.com. \"Image\" icon source: https://github.com/FortAwesome/Font-Awesome/blob/7d3d774145ac38663f6d1effc6def0334b68ab7e/svgs/solid/image.svg. Licensed under CC BY 4.0: https://creativecommons.org/licenses/by/4.0/. Modified for Excalidraw by recoloring, scaling, and positioning; image-error/resource variants additionally transform and combine it with the \"Ban\" icon.",
      FORMATS,
      ["font-awesome.image", "font-awesome.license"],
    ),
    notice(
      "font-awesome.ban.cc-by-4.0",
      "font-awesome-free-5.15.4/solid/ban@sha256:8b43245cb319c865fdc4ec865967bcd8b4e833c5b9e108adad750d3db3b64ca9",
      "Font Awesome Free 5.15.4 by @fontawesome - https://fontawesome.com. \"Ban\" icon source: https://github.com/FortAwesome/Font-Awesome/blob/7d3d774145ac38663f6d1effc6def0334b68ab7e/svgs/solid/ban.svg. Licensed under CC BY 4.0: https://creativecommons.org/licenses/by/4.0/. Modified for Excalidraw by recoloring, transforming, scaling, positioning, and combining it with the \"Image\" icon in image-error/resource placeholders.",
      FORMATS,
      ["font-awesome.ban", "font-awesome.license"],
    ),
    notice(
      "liberation-sans.ofl-full",
      "fonts/Liberation/LiberationSans-Regular.woff2@sha256:006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8",
      LIBERATION_NOTICE,
      ["svg", "pdf"],
      ["liberation-sans.license", "project.reference-assets", "excalidraw.liberation-sans-ttf"],
    ),
    notice(
      "noto-emoji.ofl-full",
      "scripts/woff2/assets/NotoEmoji-Regular.ttf@sha256:0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634",
      NOTO_EMOJI_NOTICE,
      ["svg", "pdf"],
      ["noto-emoji.license", "excalidraw.noto-emoji-ttf"],
    ),
  ];
}

function makeRecipe(prefix, kind, content, noticeIds, sourceIds, evidenceIds) {
  const digest = sha256(canonicalBytes(content));
  return {
    content,
    contentDigest: `sha256:${digest}`,
    evidenceIds,
    kind,
    noticeIds,
    recipeId: `${prefix}@sha256:${digest}`,
    sourceIds,
  };
}

function buildFallbackRecipes() {
  const imagePath = "M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48zM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56zM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48z";
  const errorImagePath = "M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48ZM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56ZM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48Z";
  const banPath = "M256 8C119.034 8 8 119.033 8 256c0 136.967 111.034 248 248 248s248-111.034 248-248S392.967 8 256 8Zm130.108 117.892c65.448 65.448 70 165.481 20.677 235.637L150.47 105.216c70.204-49.356 170.226-44.735 235.638 20.676ZM125.892 386.108c-65.448-65.448-70-165.481-20.677-235.637L361.53 406.784c-70.203 49.356-170.226 44.736-235.638-20.676Z";
  const commonImagePaint = {
    background: "#E7E7E7",
    glyphHeight: "size",
    glyphWidth: "size",
    glyphX: "width/2-size/2",
    glyphY: "height/2-size/2",
    size: "min(min(width,height),min(min(width,height)*0.4,100))",
  };
  const image = makeRecipe(
    "official.image",
    "official",
    {
      format: { pdf: "omit", png: "paint", svg: "omit" },
      glyph: { paths: [{ d: imagePath, fill: "#888", transform: null }], viewBox: [0, 0, 512, 512] },
      inherit: ["element_transform", "element_opacity", "paint_order", "frame_clip"],
      kind: "image",
      paint: commonImagePaint,
      sourceBehavior: { crop: "ignored", roundness: "ignored", selection: "element.status != 'error'" },
    },
    ["font-awesome.image.cc-by-4.0"],
    ["excalidraw.render-element", "font-awesome.image"],
    ["oracle.official-pending-image-placeholder", "upstream.svg-export-reference"],
  );
  const imageError = makeRecipe(
    "official.image-error",
    "official",
    {
      format: { pdf: "omit", png: "paint", svg: "omit" },
      glyph: {
        paths: [
          { d: errorImagePath, fill: "#888", transform: [0.81709, 0, 0, 0.81709, 124.825, 145.825] },
          { d: banPath, fill: "#888", transform: [0.30366, 0, 0, 0.30366, 506.822, 60.065] },
        ],
        viewBox: [0, 0, 668, 668],
      },
      inherit: ["element_transform", "element_opacity", "paint_order", "frame_clip"],
      kind: "image-error",
      paint: commonImagePaint,
      sourceBehavior: { crop: "ignored", roundness: "ignored", selection: "element.status == 'error'" },
    },
    ["font-awesome.ban.cc-by-4.0", "font-awesome.image.cc-by-4.0"],
    ["excalidraw.render-element", "font-awesome.image", "font-awesome.ban"],
    ["oracle.official-error-image-placeholder", "upstream.svg-export-reference"],
  );
  const resourceViewportResolution = [
    "For a rejected top-level scene image, use its normalized local [0,0,width,height] bounds when width and height are finite and positive.",
    "For a nested image or nested svg occurrence, resolve its explicit x/y/width/height lengths against the nearest already-established viewport and use that occurrence-local rectangle only when every result is finite and both dimensions are positive.",
    "When a nested SVG payload is rejected before a child occurrence can be identified, use the owning scene image's normalized local bounds.",
    "A CSS declaration, filter input, paint server, font, or other resource slot without an independent finite viewport is omitted; never widen to a parent subtree merely to obtain bounds.",
    "The selected viewport is clipped by every existing resource, nested, owning-element, and frame clip; recovery never changes scene bounds.",
  ];
  const forbidden = makeRecipe(
    "official.forbidden-resource",
    "official-derived-safety-exception",
    {
      baseRecipe: imageError.recipeId,
      format: { pdf: "paint", png: "paint", svg: "paint" },
      inherit: ["resource_transform", "resource_opacity", "resource_clip", "owning_element_transform", "paint_order", "frame_clip"],
      kind: "forbidden-resource",
      placement: {
        coordinateSpace: "resource-local",
        scope: "rejected-occurrence",
        viewport: "smallest-safe-finite",
        viewportResolution: resourceViewportResolution,
        zeroOrUnbounded: "omit",
      },
      reuse: ["glyph", "paint"],
    },
    imageError.noticeIds,
    ["excalidraw.render-element", "font-awesome.image", "font-awesome.ban"],
    ["oracle.official-error-image-placeholder", "decision.offline-resource-exception"],
  );
  const nested = makeRecipe(
    "official.nested-resource",
    "official-derived-safety-exception",
    {
      baseRecipe: imageError.recipeId,
      format: { pdf: "paint", png: "paint", svg: "paint" },
      inherit: ["nested_resource_transform", "nested_resource_opacity", "nested_clip", "owning_element_transform", "paint_order", "frame_clip"],
      kind: "nested-resource",
      placement: {
        coordinateSpace: "nested-resource-local",
        recursion: "stop",
        scope: "rejected-nested-occurrence",
        viewport: "smallest-safe-finite",
        viewportResolution: resourceViewportResolution,
        zeroOrUnbounded: "omit",
      },
      reuse: ["glyph", "paint"],
    },
    imageError.noticeIds,
    ["excalidraw.render-element", "font-awesome.image", "font-awesome.ban"],
    ["oracle.official-error-image-placeholder", "decision.offline-resource-exception"],
  );
  const embeddable = makeRecipe(
    "official.embeddable-offline",
    "official",
    {
      format: { pdf: "paint", png: "paint", svg: "paint" },
      kind: "iframe-embeddable",
      label: {
        blockHeight: "line_count*font_size*1.15",
        blockPosition: "center(element_bounds)",
        blockWidth: "max(line_advance_widths)",
        color: "element.strokeColor != 'transparent' ? element.strokeColor : #000000",
        font: {
          ascender: 1854,
          descender: -434,
          fallbackFamily: "Noto Emoji",
          family: "Liberation Sans",
          liberationWoff2Sha256: "006a2b28cbbeeaec937d1b367d0949f5f48ef4b2e7b4e1d8dc7a799d2d639be8",
          lineHeight: 1.15,
          notoEmojiTtfSha256: "0ed584d111778b259f205a40807d78131de02a5b04dad4e24c43cc84f793b634",
          unitsPerEm: 2048,
        },
        fontSize: "raw=max(min(width/2,width/utf16_length(text)),width/30); effective=raw==0 ? 20 : raw",
        lineBaseline: "block_top+font_size*0.9216796875+line_index*font_size*1.15",
        opacity: 1,
        presence: { pdf: "always", png: "width!=0 && height!=0", svg: "always" },
        text: { embeddableEmpty: "Empty Web-Embed", embeddableLink: "element.link", iframe: "IFrame element" },
        textAlign: "center",
        verticalAlign: "middle",
        wrap: { algorithm: "excalidraw-v0.18.1.wrapText", maxWidth: "width-20", negativeMaxWidth: "no-wrap" },
      },
      links: { pdf: "safe-uri-annotation-over-transformed-element-bounds", png: "none", svg: "safe-anchor", unsafe: "omit" },
      notices: {
        liberationSans: "Carry the full Liberation Sans notice in SVG/PDF whenever its font bytes are embedded.",
        notoEmoji: "Carry the full Noto Emoji notice in SVG/PDF only when fallback glyphs cause its font bytes to be embedded.",
        png: "Rasterized font glyph pixels alone add no font notice.",
      },
      shell: {
        frameClip: { pdf: "none", png: "apply", svg: "none" },
        geometry: {
          cornerRadius: "roundness in [1,2] ? min(width,height)*0.25 : roundness==3 ? min(min(width,height)*0.25,roundness.value??32) : 0",
          roundedPath: "M r 0 L width-r 0 Q width 0,width r L width height-r Q width height,width-r height L r height Q 0 height,0 height-r L 0 r Q 0 0,r 0",
          unrounded: "roughjs.rectangle(0,0,width,height)",
        },
        opacity: { pdf: "element.opacity/100", png: "(containing_frame.opacity??100)*element.opacity/10000", svg: "element.opacity/100" },
        paint: {
          bothTransparent: { background: "#d3d3d3", fillStyle: "solid", roughness: 0, stroke: "transparent" },
          embeddableOther: "source",
          iframeOther: { background: "backgroundColor==transparent ? #f4f4f6 : backgroundColor", stroke: "strokeColor==transparent ? #000000 : strokeColor" },
        },
        paintOrder: "after-non-iframe-like-elements",
        rough: {
          dashed: "[8,8+strokeWidth]",
          dotted: "[1.5,6+strokeWidth]",
          fillWeight: "strokeWidth/2",
          hachureGap: "strokeWidth*4",
          library: "roughjs@4.6.4",
          nonSolidStrokeWidth: "strokeWidth+0.5",
          preserveVertices: "continuousPath || source.roughness < 2",
        },
        svgPrecision: 2,
        transform: "translate(element.x+export_offset.x,element.y+export_offset.y) rotate(element.angle_degrees,width/2,height/2)",
      },
    },
    ["liberation-sans.ofl-full", "noto-emoji.ofl-full"],
    ["excalidraw.embeddable", "excalidraw.shape", "excalidraw.text-wrapping", "excalidraw.new-element", "excalidraw.font-metadata", "project.reference-assets", "excalidraw.liberation-sans-ttf", "excalidraw.liberation-sans-2048-ttf", "excalidraw.noto-emoji-ttf", "excalidraw.noto-emoji-2048-ttf"],
    ["oracle.official-iframe-offline-placeholder", "oracle.official-empty-embeddable-placeholder", "oracle.official-linked-embeddable-placeholder"],
  );
  return [image, imageError, forbidden, nested, embeddable];
}

function buildRegistryMappings(recipes) {
  const byPrefix = Object.fromEntries(recipes.map((recipe) => [recipe.recipeId.split("@", 1)[0], recipe.recipeId]));
  const statusSelection = `status=='error' ? ${byPrefix["official.image-error"]} : ${byPrefix["official.image"]}`;
  const all = (action, recovery) => Object.fromEntries(FORMATS.map((format) => [format, { action, recovery }]));
  const ordinaryFormatRules = {
    pdf: { action: "omit top-level source occurrence", recovery: "omitted" },
    png: { action: "paint status-selected official image recipe", recovery: "placeholder" },
    svg: { action: "omit top-level source occurrence", recovery: "omitted" },
  };
  const boundedResourceRules = Object.fromEntries(FORMATS.map((format) => [format, [
    { action: "paint selected recipe in exact smallest safe finite bounds", recovery: "placeholder", when: "smallest safe finite bounds exist" },
    { action: "omit rejected resource occurrence", recovery: "omitted", when: "no safe finite viewport exists" },
  ]]));
  const localResourceRules = {
    pdf: [
      { action: "omit top-level source occurrence", recovery: "omitted", when: "top-level source image" },
      { action: "paint nested-resource recipe", recovery: "placeholder", when: "nested occurrence has a safe finite viewport" },
      { action: "omit nested occurrence", recovery: "omitted", when: "nested occurrence has no safe finite viewport" },
    ],
    png: [
      { action: "paint status-selected official image recipe", recovery: "placeholder", when: "top-level source image" },
      { action: "paint nested-resource recipe", recovery: "placeholder", when: "nested occurrence has a safe finite viewport" },
      { action: "omit nested occurrence", recovery: "omitted", when: "nested occurrence has no safe finite viewport" },
    ],
    svg: [
      { action: "omit top-level source occurrence", recovery: "omitted", when: "top-level source image" },
      { action: "paint nested-resource recipe", recovery: "placeholder", when: "nested occurrence has a safe finite viewport" },
      { action: "omit nested occurrence", recovery: "omitted", when: "nested occurrence has no safe finite viewport" },
    ],
  };
  const fieldFallbackRules = Object.fromEntries(FORMATS.map((format) => [format, [
    { action: "repair normalized field value", recovery: "repaired", when: "field disposition selects repair" },
    { action: "ignore field in understood rendering view", recovery: "ignored", when: "field disposition selects ignore" },
    { action: "paint field-specific compatibility placeholder", recovery: "placeholder", when: "field disposition selects placeholder" },
    { action: "omit affected field channel or occurrence", recovery: "omitted", when: "field disposition selects omission" },
    { action: "substitute field-specific value", recovery: "substituted", when: "field disposition selects substitution" },
  ]]));
  const mapping = (code, detailKinds, severity, effectsRequired, effectsConditional, formatRules, pointerRule, sourceRule, recipeSelection) => ({
    code,
    detailKinds,
    effectsConditional,
    effectsRequired,
    formatRules,
    pointerRule,
    recipeSelection,
    severity,
    sourceRule,
  });
  return [
    mapping("normalization.missing-element-id", ["compatibility"], "warning", ["input_repair"], [], all("derive deterministic ID", "repaired"), "expected id property", "one occurrence per participating element missing an ID", "none"),
    mapping("normalization.duplicate-element-id", ["compatibility"], "warning", ["input_repair"], ["add visual/semantic only when disambiguated relationships affect those channels"], all("rekey later duplicate", "repaired"), "later duplicate id property", "retain first occurrence; diagnose every later duplicate", "none"),
    mapping("normalization.missing-timestamp", ["compatibility"], "warning", ["input_repair"], [], all("substitute canonical zero timestamp", "repaired"), "expected updated/created property", "one occurrence per missing required timestamp", "none"),
    mapping("normalization.zero-rough-seed", ["compatibility"], "warning", ["input_repair", "visual"], [], all("derive deterministic nonzero seed", "repaired"), "participating element seed property", "integer zero only", "none"),
    mapping("normalization.relationship-cycle", ["compatibility"], "warning", ["input_repair"], ["add visual/semantic for the broken relationship's actual channels"], all("remove cycle-closing edge", "removed"), "cycle-closing relationship edge", "first edge in canonical source traversal", "none"),
    mapping("compatibility.unsupported-element-type", ["compatibility"], "warning", ["visual", "semantic"], [], boundedResourceRules, "whole unknown element boundary", "nonempty unrecognized type; descendants are not classified", "renderer.unknown-element.v1"),
    mapping("compatibility.malformed-element", ["compatibility"], "warning", ["visual"], ["add semantic when recoverable source semantics are lost"], boundedResourceRules, "whole malformed element occurrence", "absent/empty/non-string type or non-object element; descendants are not classified", "renderer.malformed-element.v1"),
    mapping("compatibility.unsupported-property", ["compatibility"], "warning", ["visual", "semantic", "metadata"], [], all("ignore unmatched property in understood rendering view", "ignored"), "narrowest unmatched property boundary", "only when containing record participates; descendants suppressed", "none"),
    mapping("compatibility.unsupported-property-value", ["compatibility"], "warning", [], ["exact effects and recovery are selected by the field disposition"], fieldFallbackRules, "narrowest recognized property", "wrong type, malformed recognized structure, or unsupported value not settled by pinned normalization", "field disposition recipe when applicable"),
    mapping("compatibility.known-element-replaced", ["compatibility"], "warning", ["visual"], ["add semantic when source carried a semantic channel"], boundedResourceRules, "known element boundary", "only when no narrower primary cause exists", "renderer.known-element-unavailable.v1"),
    mapping("asset.supplement-conflict", ["asset"], "warning", ["input_repair"], [], all("use scene-document payload", "ignored"), "authoritative /files entry or root for request-only source", "participating semantic asset identity", "none"),
    mapping("asset.media-type-mismatch", ["asset"], "warning", ["input_repair"], [], all("use validated media kind", "repaired"), "authoritative payload boundary", "participating semantic asset identity", "none"),
    mapping("asset.malformed-file-record", ["asset"], "warning", ["visual"], ["add semantic only when nested semantics are lost"], ordinaryFormatRules, "narrowest authoritative /files/<escaped-id> record pointer", "referenced authoritative file record; unreferenced malformed records are silent", statusSelection),
    mapping("image.missing-file", ["asset"], "warning", ["visual"], [], ordinaryFormatRules, "referring participating image element's /fileId pointer", "one occurrence per participating image reference", statusSelection),
    mapping("image.forbidden-resource", ["asset", "nested_source"], "warning", ["visual"], ["add semantic when nested semantic content is removed"], boundedResourceRules, "top-level asset pointer or owning image pointer plus nested ordinal path", "smallest rejected resource occurrence; never fetch", byPrefix["official.forbidden-resource"]),
    mapping("image.animation-flattened", ["asset"], "info", ["visual"], [], { pdf: { action: "paint reference default frame", recovery: "flattened" }, png: { action: "paint reference default frame", recovery: "flattened" } }, "referring participating image element's /fileId pointer", "one occurrence per actually multi-frame rendered PNG/PDF occurrence; direct SVG is silent", "none; accepted raster content"),
    mapping("image.unsupported-format", ["asset"], "warning", ["visual"], ["add semantic only when nested semantics are lost"], ordinaryFormatRules, "authoritative payload boundary, normally /files/<escaped-id>/dataURL", "validated authoritative payload occurrence", statusSelection),
    mapping("image.decode-failed", ["asset"], "warning", ["visual"], ["add semantic only when nested semantics are lost"], ordinaryFormatRules, "authoritative payload boundary, normally /files/<escaped-id>/dataURL", "validated authoritative payload occurrence", statusSelection),
    mapping("image.unsafe-svg-content-removed", ["nested_source"], "warning", ["semantic"], ["add visual only when static appearance changes"], all("remove unsafe nested content", "removed"), "owning image pointer plus narrowest nested ordinal path", "one occurrence per removed unsafe nested boundary", "none unless a separate rejected resource mapping selects nested-resource"),
    mapping("link.unsafe-omitted", ["compatibility"], "warning", ["semantic"], [], { pdf: { action: "omit URI annotation", recovery: "omitted" }, svg: { action: "omit anchor", recovery: "omitted" } }, "source element link property", "all participating linked elements and generated labels; PNG is silent", "none"),
    mapping("color.invalid", ["compatibility"], "warning", ["input_repair", "visual"], [], all("substitute field-specific reference default", "substituted"), "invalid color property", "one occurrence per consumed invalid color", "none"),
    mapping("font.liberation-sans-substituted", ["compatibility"], "info", ["visual"], [], all("use bundled Liberation Sans", "substituted"), "affected text, frame name, or generated label owner", "one occurrence per affected text/label source", `none for user text/frame names; ${byPrefix["official.embeddable-offline"]} for embeddable labels`),
    mapping("font.noto-emoji-substituted", ["compatibility"], "info", ["visual"], [], all("use bundled Noto Emoji", "substituted"), "affected text, frame name, or generated label owner", "only when Noto supplies at least one glyph", `none for user text/frame names; ${byPrefix["official.embeddable-offline"]} for embeddable labels`),
    mapping("font.missing-glyph", ["missing_glyph"], "warning", ["visual"], [], all("paint deterministic replacement glyph", "substituted"), "affected text, frame name, or generated label owner", "one occurrence per affected Unicode scalar under registry ordering", "none"),
    mapping("resource.element-complexity-replaced", ["resource_limit"], "warning", ["visual"], ["add semantic when replaced/omitted content carried semantic output"], boundedResourceRules, "participating element boundary", "local semantic transaction only", "renderer.known-element-unavailable.v1"),
    mapping("resource.element-work-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic when replaced/omitted content carried semantic output"], boundedResourceRules, "participating element boundary", "local semantic transaction only", "renderer.known-element-unavailable.v1"),
    mapping("resource.asset-bytes-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic for lost nested semantics"], localResourceRules, "authoritative asset occurrence", "top-level uses status-selected ordinary recipe; nested uses nested-resource", `${statusSelection}; nested=${byPrefix["official.nested-resource"]}`),
    mapping("resource.asset-work-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic for lost nested semantics"], localResourceRules, "authoritative asset occurrence", "top-level uses status-selected ordinary recipe; nested uses nested-resource", `${statusSelection}; nested=${byPrefix["official.nested-resource"]}`),
    mapping("resource.image-dimensions-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic for lost nested semantics"], localResourceRules, "authoritative image payload occurrence", "top-level uses status-selected ordinary recipe; nested uses nested-resource", `${statusSelection}; nested=${byPrefix["official.nested-resource"]}`),
    mapping("resource.image-pixels-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic for lost nested semantics"], localResourceRules, "authoritative image payload occurrence", "top-level uses status-selected ordinary recipe; nested uses nested-resource", `${statusSelection}; nested=${byPrefix["official.nested-resource"]}`),
    mapping("resource.nested-svg-complexity-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic for lost nested semantics"], boundedResourceRules, "owning image pointer plus narrowest known nested ordinal path", "smallest rejected nested occurrence", byPrefix["official.nested-resource"]),
    mapping("resource.nested-asset-depth-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic for lost nested semantics"], boundedResourceRules, "owning image pointer plus recursion-edge nested ordinal path", "stop recursion at rejected nested occurrence", byPrefix["official.nested-resource"]),
    mapping("resource.coordinate-range-exceeded", ["resource_limit"], "warning", ["visual"], ["add semantic when affected semantic output is lost"], boundedResourceRules, "narrowest affected element/property", "local semantic transaction only", "renderer.known-element-unavailable.v1"),
    mapping("resource.optional-metadata-omitted", ["resource_limit"], "warning", ["metadata"], [], all("omit optional metadata component", "metadata_omitted"), "root metadata component", "continue lower-priority metadata components", "none"),
    ...[
      "metadata.provenance-serialization-failed",
      "metadata.diagnostic-summary-serialization-failed",
      "metadata.scene-serialization-failed",
      "metadata.scene-compression-failed",
      "metadata.scene-embedding-failed",
      "metadata.manifest-serialization-failed",
      "metadata.manifest-embedding-failed",
    ].map((code) => mapping(code, ["metadata"], "warning", ["metadata"], [], all("omit failed optional metadata component", "metadata_omitted"), "root pointer", "component and operation are fixed by the code", "none")),
    mapping("pdf.raster-fallback", ["pdf_raster_fallback"], "warning", ["semantic"], [], { pdf: { action: "rasterize smallest approved isolated closure", recovery: "rasterized" } }, "outermost causative effect node", "one occurrence per emitted closure in canonical source order", "none; renderer-owned PDF projection"),
  ];
}

function buildConservativeRule() {
  return {
    diagnostic: {
      code: "compatibility.unsupported-property",
      effects: ["visual", "semantic", "metadata"],
      recovery: "ignored",
      severity: "warning",
    },
    malformedKnownValue: {
      code: "compatibility.unsupported-property-value",
      rule: "Use the manifest entry's exact field fallback at the narrowest property pointer and retain every other understood part of the record.",
    },
    matching: "Match by semantic record kind and path, never by property-name heuristics. An opaque-metadata entry alone admits arbitrary descendants.",
    participation: {
      appState: "global",
      deletedElement: "silent",
      fileRecord: "only when authoritative and referenced by participating content",
      knownElement: "when traversed as a non-deleted element for the requested whole-scene static export",
      metadataEmbedding: "does not make otherwise irrelevant rendering behavior participate",
      root: "global",
      unreferencedFileRecord: "silent",
    },
    preservation: "Preserve unmatched values in the canonicalized original scene document and ignore them in the understood rendering view.",
    suppression: {
      malformedRecord: "Use one malformed-record occurrence and do not classify descendants.",
      unknownElement: "Use one compatibility.unsupported-element-type occurrence and do not classify descendants.",
      unknownProperty: "Use one occurrence at the property boundary and do not classify descendants.",
    },
  };
}

function buildNormalizationRules() {
  return [
    {
      code: "normalization.missing-element-id",
      effects: ["input_repair"],
      pointerRule: "expected /elements/<index>/id",
      recovery: "repaired",
      rule: "Derive the normalized ID from compatibility revision, canonical original element content, and source array position.",
      sourceIds: ["excalidraw.restore"],
    },
    {
      code: "normalization.duplicate-element-id",
      effects: ["input_repair"],
      pointerRule: "later duplicate /elements/<index>/id",
      recovery: "repaired",
      rule: "Retain the first ID, deterministically rekey each later occurrence, and resolve ambiguous source references to the first occurrence; add visual/semantic effects only when disambiguated relationships affect those channels.",
      sourceIds: ["excalidraw.restore"],
    },
    {
      code: "normalization.missing-timestamp",
      effects: ["input_repair"],
      pointerRule: "expected required updated or created field",
      recovery: "repaired",
      rule: "Use canonical integer zero instead of current time.",
      sourceIds: ["excalidraw.restore", "excalidraw.types"],
    },
    {
      code: "normalization.zero-rough-seed",
      effects: ["input_repair", "visual"],
      pointerRule: "participating element /seed",
      recovery: "repaired",
      rule: "Derive a nonzero seed from compatibility revision and normalized element identity.",
      sourceIds: ["excalidraw.shape"],
    },
    {
      code: "normalization.relationship-cycle",
      effects: ["input_repair"],
      pointerRule: "cycle-closing relationship edge",
      recovery: "removed",
      rule: "Break the first cycle-closing edge in canonical source traversal order; add visual/semantic effects for channels the removed relationship actually affects.",
      sourceIds: ["excalidraw.restore"],
    },
    {
      code: null,
      effects: [],
      pointerRule: "element /index",
      recovery: "repaired",
      rule: "Repair absent, invalid, duplicate, or descending fractional indices in source array order without reordering paint order; this pinned migration is silent.",
      sourceIds: ["excalidraw.fractional-index"],
    },
    {
      code: null,
      effects: [],
      pointerRule: "element width/height and x/y",
      recovery: "repaired",
      rule: "Shift the origin for negative dimensions and replace dimensions with their absolute values before rendering; this pinned migration is silent.",
      sourceIds: ["excalidraw.restore"],
    },
  ];
}

function extractDeclaredFields(source, marker, endToken) {
  const markerOffset = source.indexOf(marker);
  if (markerOffset < 0) throw new Error(`pinned declaration marker not found: ${marker}`);
  const bodyOffset = source.indexOf("{", markerOffset) + 1;
  const endOffset = source.indexOf(endToken, bodyOffset);
  if (endOffset < 0) throw new Error(`pinned declaration terminator not found for: ${marker}`);
  return [...source.slice(bodyOffset, endOffset).matchAll(/^    ([A-Za-z_$][A-Za-z0-9_$]*)(?:\?)?:/gm)]
    .map((match) => match[1]);
}

function extractPinnedTypeInventory() {
  const declarationsRoot = resolve(ORACLE_ROOT, "node_modules/@excalidraw/excalidraw/dist/types/excalidraw");
  const elementTypes = readFileSync(resolve(declarationsRoot, "element/types.d.ts"), "utf8");
  const types = readFileSync(resolve(declarationsRoot, "types.d.ts"), "utf8");
  const inventory = {
    appState: extractDeclaredFields(types, "export interface AppState {", "\n}"),
    arrow: [
      ...extractDeclaredFields(elementTypes, "export type ExcalidrawArrowElement = ExcalidrawLinearElement & Readonly<{", "\n}>;"),
      ...extractDeclaredFields(elementTypes, "export type ExcalidrawElbowArrowElement = Merge<ExcalidrawArrowElement, {", "\n}>;"),
    ],
    common: extractDeclaredFields(elementTypes, "type _ExcalidrawElementBase = Readonly<{", "\n}>;"),
    fileRecord: extractDeclaredFields(types, "export type BinaryFileData = {", "\n};"),
    frame: extractDeclaredFields(elementTypes, "export type ExcalidrawFrameElement = _ExcalidrawElementBase & {", "\n};"),
    freedraw: extractDeclaredFields(elementTypes, "export type ExcalidrawFreeDrawElement = _ExcalidrawElementBase & Readonly<{", "\n}>;"),
    image: extractDeclaredFields(elementTypes, "export type ExcalidrawImageElement = _ExcalidrawElementBase & Readonly<{", "\n}>;"),
    linear: extractDeclaredFields(elementTypes, "export type ExcalidrawLinearElement = _ExcalidrawElementBase & Readonly<{", "\n}>;"),
    magicframe: extractDeclaredFields(elementTypes, "export type ExcalidrawMagicFrameElement = _ExcalidrawElementBase & {", "\n};"),
    text: extractDeclaredFields(elementTypes, "export type ExcalidrawTextElement = _ExcalidrawElementBase & Readonly<{", "\n}>;"),
  };
  for (const key of Object.keys(inventory)) inventory[key] = [...new Set(inventory[key])].sort();
  assertExactSet(inventory.appState, APP_STATE_FIELDS.filter((name) => name !== "isSidebarDocked"), "pinned AppState declaration");
  assertExactSet(inventory.common, COMMON_FIELDS.filter((name) => !["type", "strokeSharpness", "boundElementIds"].includes(name)), "pinned common-element declaration");
  assertExactSet(inventory.fileRecord, ["mimeType", "id", "dataURL", "created", "lastRetrieved", "version"], "pinned file-record declaration");
  return inventory;
}

function expectedFieldKeys(inventory) {
  const linearFields = inventory.linear.filter((name) => !["type", "startArrowhead", "endArrowhead"].includes(name));
  const arrowFields = [...new Set([
    "startArrowhead",
    "endArrowhead",
    ...inventory.arrow.filter((name) => !["type", "startBinding", "endBinding"].includes(name)),
  ])];
  const result = [
    ...["type", "version", "elements", "appState", "files", "source", "scrollToContent", "libraryItems"].map((name) => `scene:/${name}`),
    ...[...inventory.appState, "isSidebarDocked"].map((name) => `app_state:/${name}`),
    ...["$key", ...inventory.fileRecord].map((name) => `file_record:/${name}`),
    ...["type", ...inventory.common, "strokeSharpness", "boundElementIds"].map((name) => `element.common:/${name}`),
    ...inventory.text.filter((name) => name !== "type").map((name) => `element.text:/${name}`),
    ...["font", "rawText", "baseline"].map((name) => `element.text:/${name}`),
    ...linearFields.map((name) => `element.linear:/${name}`),
    ...["startArrowhead", "endArrowhead"].map((name) => `element.line:/${name}`),
    ...arrowFields.map((name) => `element.arrow:/${name}`),
    ...inventory.freedraw.filter((name) => name !== "type").map((name) => `element.freedraw:/${name}`),
    ...inventory.image.filter((name) => name !== "type").map((name) => `element.image:/${name}`),
    ...inventory.frame.filter((name) => name !== "type").map((name) => `element.frame:/${name}`),
    ...inventory.magicframe.filter((name) => name !== "type").map((name) => `element.magicframe:/${name}`),
    ...SPECIFIC_FIELDS["element.iframe"].map((name) => `element.iframe:/${name}`),
    ...SPECIFIC_FIELDS["element.embeddable"].map((name) => `element.embeddable:/${name}`),
    ...Object.entries(SPECIFIC_FIELDS)
      .filter(([recordKind]) => recordKind.startsWith("nested.") || recordKind === "frame_rendering")
      .flatMap(([recordKind, names]) => names.map((name) => `${recordKind}:/${name}`)),
    "element.legacy_draw:/type",
    "nested.local_svg_resource:/*",
  ];
  return [...new Set(result)].sort();
}

function assertUnique(items, key, subject) {
  const seen = new Set();
  for (const item of items) {
    const value = key(item);
    if (seen.has(value)) throw new Error(`duplicate ${subject}: ${value}`);
    seen.add(value);
  }
}

function assertExactSet(actual, expected, subject) {
  const left = [...new Set(actual)].sort();
  const right = [...new Set(expected)].sort();
  if (canonicalize(left) !== canonicalize(right)) {
    const missing = right.filter((item) => !left.includes(item));
    const extra = left.filter((item) => !right.includes(item));
    throw new Error(`${subject} coverage mismatch; missing=${missing.join(",")} extra=${extra.join(",")}`);
  }
}

function jsonSchemaTypeMatches(value, type) {
  if (type === "null") return value === null;
  if (type === "array") return Array.isArray(value);
  if (type === "object") return value !== null && typeof value === "object" && !Array.isArray(value);
  if (type === "number") return typeof value === "number" && Number.isFinite(value);
  return typeof value === type;
}

function resolveSchemaReference(root, reference) {
  if (!reference.startsWith("#/")) throw new Error(`unsupported JSON Schema reference ${reference}`);
  return reference.slice(2).split("/").reduce((value, segment) => value[segment.replaceAll("~1", "/").replaceAll("~0", "~")], root);
}

function validateJsonSchema(value, schema, root = schema, pointer = "$") {
  if (schema.$ref) return validateJsonSchema(value, resolveSchemaReference(root, schema.$ref), root, pointer);
  if (schema.anyOf) {
    const accepted = schema.anyOf.some((candidate) => {
      try {
        validateJsonSchema(value, candidate, root, pointer);
        return true;
      } catch {
        return false;
      }
    });
    if (!accepted) throw new Error(`${pointer} does not satisfy any JSON Schema branch`);
    return;
  }
  if (Object.hasOwn(schema, "const") && canonicalize(value) !== canonicalize(schema.const)) {
    throw new Error(`${pointer} does not equal its JSON Schema const`);
  }
  if (schema.enum && !schema.enum.some((candidate) => canonicalize(candidate) === canonicalize(value))) {
    throw new Error(`${pointer} is outside its JSON Schema enum`);
  }
  if (schema.type) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    if (!types.some((type) => jsonSchemaTypeMatches(value, type))) {
      throw new Error(`${pointer} has the wrong JSON Schema type`);
    }
  }
  if (typeof value === "string" && schema.pattern && !new RegExp(schema.pattern).test(value)) {
    throw new Error(`${pointer} does not match its JSON Schema pattern`);
  }
  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) {
      throw new Error(`${pointer} has fewer than ${schema.minItems} items`);
    }
    if (schema.maxItems !== undefined && value.length > schema.maxItems) {
      throw new Error(`${pointer} has more than ${schema.maxItems} items`);
    }
    if (schema.items) value.forEach((item, index) => validateJsonSchema(item, schema.items, root, `${pointer}/${index}`));
  }
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    for (const required of schema.required ?? []) {
      if (!Object.hasOwn(value, required)) throw new Error(`${pointer} is missing required property ${required}`);
    }
    for (const [key, member] of Object.entries(value)) {
      if (schema.properties?.[key]) {
        validateJsonSchema(member, schema.properties[key], root, `${pointer}/${key}`);
      } else if (schema.additionalProperties === false) {
        throw new Error(`${pointer} has unknown property ${key}`);
      } else if (schema.additionalProperties && typeof schema.additionalProperties === "object") {
        validateJsonSchema(member, schema.additionalProperties, root, `${pointer}/${key}`);
      }
    }
  }
}

function validateSchemaShape(manifest) {
  validateJsonSchema(manifest, SCHEMA);
  if (manifest === null || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new Error("manifest must be an object");
  }
  assertExactSet(Object.keys(manifest), SCHEMA.required, "top-level schema");
  if (manifest.schema !== "excalidraw-render/scene-surface/v1" || manifest.compatibilityRevision !== 1) {
    throw new Error("manifest schema identity or compatibility revision is invalid");
  }
  for (const name of ["authorities", "sourceCatalog", "evidenceCatalog", "recordKinds", "fieldDispositions", "elementKinds", "fallbackRecipes", "noticeCatalog", "normalizationRules", "registryMappings", "rendererOwnedRecipeReferences"]) {
    if (!Array.isArray(manifest[name])) throw new Error(`${name} must be an array`);
  }
}

function valueAtPath(value, path) {
  return path.split(".").reduce((current, segment) => current?.[segment], value);
}

function validateFormatRuleSet(rules, pointer) {
  const formats = Object.keys(rules);
  if (formats.length === 0 || formats.some((format) => !FORMATS.includes(format))) {
    throw new Error(`${pointer} must contain at least one known format`);
  }
  for (const format of formats) {
    const branches = Array.isArray(rules[format]) ? rules[format] : [rules[format]];
    for (const [index, branch] of branches.entries()) {
      if (!branch.action.trim()) throw new Error(`${pointer}/${format}/${index} has an empty action`);
      if (branch.recovery !== null && !RECOVERIES.includes(branch.recovery)) {
        throw new Error(`${pointer}/${format}/${index} has unknown recovery ${branch.recovery}`);
      }
    }
  }
}

function validateRecipeContent(recipe) {
  const kind = recipe.content.kind;
  const commonFormatPaths = ["format.svg", "format.png", "format.pdf"];
  const requiredByKind = {
    image: ["glyph.paths", "glyph.viewBox", "paint.background", "paint.size", "paint.glyphX", "paint.glyphY", "sourceBehavior.selection"],
    "image-error": ["glyph.paths", "glyph.viewBox", "paint.background", "paint.size", "paint.glyphX", "paint.glyphY", "sourceBehavior.selection"],
    "forbidden-resource": ["baseRecipe", "placement.coordinateSpace", "placement.scope", "placement.viewport", "placement.viewportResolution", "placement.zeroOrUnbounded", "reuse"],
    "nested-resource": ["baseRecipe", "placement.coordinateSpace", "placement.recursion", "placement.scope", "placement.viewport", "placement.viewportResolution", "placement.zeroOrUnbounded", "reuse"],
    "iframe-embeddable": [
      "label.font.family", "label.font.fallbackFamily", "label.font.liberationWoff2Sha256", "label.font.notoEmojiTtfSha256",
      "label.fontSize", "label.lineBaseline", "label.text.iframe", "label.text.embeddableEmpty", "label.text.embeddableLink",
      "label.wrap.algorithm", "label.wrap.maxWidth", "links.svg", "links.png", "links.pdf", "links.unsafe",
      "notices.liberationSans", "notices.notoEmoji", "notices.png", "shell.geometry.cornerRadius", "shell.geometry.roundedPath",
      "shell.paint.bothTransparent", "shell.paint.iframeOther", "shell.paint.embeddableOther", "shell.paintOrder", "shell.rough.library", "shell.transform",
    ],
  };
  const required = [...commonFormatPaths, ...(requiredByKind[kind] ?? [])];
  if (!(kind in requiredByKind)) throw new Error(`recipe ${recipe.recipeId} has unknown content kind ${kind}`);
  for (const path of required) {
    if (valueAtPath(recipe.content, path) === undefined) throw new Error(`recipe ${recipe.recipeId} is missing content path ${path}`);
  }
  if ((kind === "image" || kind === "image-error") && recipe.content.glyph.paths.length !== (kind === "image" ? 1 : 2)) {
    throw new Error(`recipe ${recipe.recipeId} has the wrong glyph path count`);
  }
  const exactKeys = (value, expected, path) => {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`recipe ${recipe.recipeId} ${path} must be an object`);
    assertExactSet(Object.keys(value), expected, `recipe ${recipe.recipeId} ${path}`);
  };
  const finite = (value, path) => {
    if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`recipe ${recipe.recipeId} ${path} must be finite`);
  };
  const text = (value, path) => {
    if (typeof value !== "string" || value.length === 0) throw new Error(`recipe ${recipe.recipeId} ${path} must be a nonempty string`);
  };
  const textMembers = (value, paths) => paths.forEach((path) => text(valueAtPath(value, path), path));
  const finiteMembers = (value, paths) => paths.forEach((path) => finite(valueAtPath(value, path), path));
  if (!Array.isArray(recipe.content.inherit ?? []) || !(recipe.content.inherit ?? []).every((item) => typeof item === "string" && item.length > 0)) {
    throw new Error(`recipe ${recipe.recipeId} inherit must contain nonempty strings`);
  }
  if (kind === "image" || kind === "image-error") {
    exactKeys(recipe.content.glyph, ["paths", "viewBox"], "glyph");
    exactKeys(recipe.content.paint, ["background", "glyphHeight", "glyphWidth", "glyphX", "glyphY", "size"], "paint");
    exactKeys(recipe.content.sourceBehavior, ["crop", "roundness", "selection"], "sourceBehavior");
    if (!Array.isArray(recipe.content.glyph.viewBox) || recipe.content.glyph.viewBox.length !== 4 || !recipe.content.glyph.viewBox.every((value) => typeof value === "number" && Number.isFinite(value))) throw new Error(`recipe ${recipe.recipeId} has invalid viewBox`);
    textMembers(recipe.content, ["paint.background", "paint.glyphHeight", "paint.glyphWidth", "paint.glyphX", "paint.glyphY", "paint.size", "sourceBehavior.crop", "sourceBehavior.roundness", "sourceBehavior.selection"]);
    for (const [index, path] of recipe.content.glyph.paths.entries()) {
      exactKeys(path, ["d", "fill", "transform"], `glyph.paths/${index}`);
      if (typeof path.d !== "string" || path.d.length === 0 || path.fill !== "#888" || (path.transform !== null && (!Array.isArray(path.transform) || path.transform.length !== 6 || !path.transform.every((value) => typeof value === "number" && Number.isFinite(value))))) {
        throw new Error(`recipe ${recipe.recipeId} has invalid glyph path ${index}`);
      }
    }
  }
  if ((kind === "forbidden-resource" || kind === "nested-resource") && recipe.content.placement.viewportResolution.length !== 5) {
    throw new Error(`recipe ${recipe.recipeId} has incomplete viewport resolution rules`);
  }
  if (kind === "forbidden-resource" || kind === "nested-resource") {
    const placementKeys = kind === "nested-resource"
      ? ["coordinateSpace", "recursion", "scope", "viewport", "viewportResolution", "zeroOrUnbounded"]
      : ["coordinateSpace", "scope", "viewport", "viewportResolution", "zeroOrUnbounded"];
    exactKeys(recipe.content.placement, placementKeys, "placement");
    assertExactSet(recipe.content.reuse, ["glyph", "paint"], `recipe ${recipe.recipeId} reuse components`);
    textMembers(recipe.content, ["baseRecipe", "placement.coordinateSpace", "placement.scope", "placement.viewport", "placement.zeroOrUnbounded"]);
    if (kind === "nested-resource") text(recipe.content.placement.recursion, "placement.recursion");
    if (!recipe.content.placement.viewportResolution.every((item) => typeof item === "string" && item.length > 0)) throw new Error(`recipe ${recipe.recipeId} has invalid viewport resolution text`);
  }
  if (kind === "iframe-embeddable") {
    exactKeys(recipe.content.label, ["blockHeight", "blockPosition", "blockWidth", "color", "font", "fontSize", "lineBaseline", "opacity", "presence", "text", "textAlign", "verticalAlign", "wrap"], "label");
    exactKeys(recipe.content.label.font, ["ascender", "descender", "fallbackFamily", "family", "liberationWoff2Sha256", "lineHeight", "notoEmojiTtfSha256", "unitsPerEm"], "label.font");
    exactKeys(recipe.content.label.presence, FORMATS, "label.presence");
    exactKeys(recipe.content.label.text, ["embeddableEmpty", "embeddableLink", "iframe"], "label.text");
    exactKeys(recipe.content.label.wrap, ["algorithm", "maxWidth", "negativeMaxWidth"], "label.wrap");
    exactKeys(recipe.content.links, FORMATS.concat("unsafe"), "links");
    exactKeys(recipe.content.notices, ["liberationSans", "notoEmoji", "png"], "notices");
    exactKeys(recipe.content.shell, ["frameClip", "geometry", "opacity", "paint", "paintOrder", "rough", "svgPrecision", "transform"], "shell");
    exactKeys(recipe.content.shell.frameClip, FORMATS, "shell.frameClip");
    exactKeys(recipe.content.shell.geometry, ["cornerRadius", "roundedPath", "unrounded"], "shell.geometry");
    exactKeys(recipe.content.shell.opacity, FORMATS, "shell.opacity");
    exactKeys(recipe.content.shell.paint, ["bothTransparent", "embeddableOther", "iframeOther"], "shell.paint");
    exactKeys(recipe.content.shell.paint.bothTransparent, ["background", "fillStyle", "roughness", "stroke"], "shell.paint.bothTransparent");
    exactKeys(recipe.content.shell.paint.iframeOther, ["background", "stroke"], "shell.paint.iframeOther");
    exactKeys(recipe.content.shell.rough, ["dashed", "dotted", "fillWeight", "hachureGap", "library", "nonSolidStrokeWidth", "preserveVertices"], "shell.rough");
    textMembers(recipe.content, [
      "label.blockHeight", "label.blockPosition", "label.blockWidth", "label.color", "label.font.family", "label.font.fallbackFamily",
      "label.font.liberationWoff2Sha256", "label.font.notoEmojiTtfSha256", "label.fontSize", "label.lineBaseline",
      "label.presence.pdf", "label.presence.png", "label.presence.svg", "label.text.embeddableEmpty", "label.text.embeddableLink",
      "label.text.iframe", "label.textAlign", "label.verticalAlign", "label.wrap.algorithm", "label.wrap.maxWidth", "label.wrap.negativeMaxWidth",
      "links.pdf", "links.png", "links.svg", "links.unsafe", "notices.liberationSans", "notices.notoEmoji", "notices.png",
      "shell.frameClip.pdf", "shell.frameClip.png", "shell.frameClip.svg", "shell.geometry.cornerRadius", "shell.geometry.roundedPath",
      "shell.geometry.unrounded", "shell.opacity.pdf", "shell.opacity.png", "shell.opacity.svg", "shell.paint.bothTransparent.background",
      "shell.paint.bothTransparent.fillStyle", "shell.paint.bothTransparent.stroke", "shell.paint.embeddableOther",
      "shell.paint.iframeOther.background", "shell.paint.iframeOther.stroke", "shell.paintOrder", "shell.rough.dashed", "shell.rough.dotted",
      "shell.rough.fillWeight", "shell.rough.hachureGap", "shell.rough.library", "shell.rough.nonSolidStrokeWidth",
      "shell.rough.preserveVertices", "shell.transform",
    ]);
    finiteMembers(recipe.content, ["label.font.ascender", "label.font.descender", "label.font.lineHeight", "label.font.unitsPerEm", "label.opacity", "shell.paint.bothTransparent.roughness", "shell.svgPrecision"]);
    if (!/^[0-9a-f]{64}$/.test(recipe.content.label.font.liberationWoff2Sha256) || !/^[0-9a-f]{64}$/.test(recipe.content.label.font.notoEmojiTtfSha256)) {
      throw new Error(`recipe ${recipe.recipeId} has invalid font digest`);
    }
  }
}

function validateManifest(manifest, evidence, typeInventory) {
  validateSchemaShape(manifest);
  if (manifest.coverage.unresolved.length !== 0 || manifest.coverage.status !== "complete") {
    throw new Error("scene-surface coverage is not complete");
  }

  assertUnique(manifest.sourceCatalog, (item) => item.id, "source ID");
  assertUnique(manifest.evidenceCatalog, (item) => item.id, "evidence ID");
  assertUnique(manifest.fieldDispositions, (item) => `${item.recordKind}:${item.path}`, "semantic field path");
  assertUnique(manifest.fallbackRecipes, (item) => item.recipeId, "recipe ID");
  assertUnique(manifest.noticeCatalog, (item) => item.id, "notice ID");
  assertUnique(manifest.registryMappings, (item) => item.code, "registry code");
  assertExactSet(manifest.registryMappings.map((item) => item.code), DIAGNOSTIC_CODES, "diagnostic registry");

  assertExactSet(manifest.recordKinds.map((item) => item.id), RECORD_KINDS, "record kind");
  assertExactSet(manifest.elementKinds.map((item) => item.kind), ELEMENT_KINDS, "element kind");
  assertExactSet(manifest.fieldDispositions.map((item) => `${item.recordKind}:${item.path}`), expectedFieldKeys(typeInventory), "field disposition");

  const sourceIds = new Set(manifest.sourceCatalog.map((item) => item.id));
  const evidenceIds = new Set(manifest.evidenceCatalog.map((item) => item.id));
  const noticeIds = new Set(manifest.noticeCatalog.map((item) => item.id));
  const recipeIds = new Set(manifest.fallbackRecipes.map((item) => item.recipeId));
  const rendererRecipeIds = new Set(manifest.rendererOwnedRecipeReferences.map((item) => item.recipeId));
  const diagnosticCodes = new Set(manifest.registryMappings.map((item) => item.code));
  const domainIds = new Set(Object.keys(manifest.domainCatalog));
  for (const sourceRecord of manifest.sourceCatalog) {
    if (!sourceRecord.url.startsWith("https://") || !sourceRecord.immutableRevision) {
      throw new Error(`source ${sourceRecord.id} lacks an immutable pointer`);
    }
  }
  for (const evidenceRecord of manifest.evidenceCatalog) {
    for (const id of evidenceRecord.sourceIds) {
      if (!sourceIds.has(id)) throw new Error(`evidence ${evidenceRecord.id} references unknown source ${id}`);
    }
    if (evidenceRecord.kind === "approved-compatibility-exception") {
      const decisionSources = evidenceRecord.sourceIds
        .map((id) => manifest.sourceCatalog.find((sourceRecord) => sourceRecord.id === id))
        .filter((sourceRecord) => sourceRecord.kind === "project-decision");
      if (decisionSources.length !== 1 || evidenceRecord.digest !== `sha256:${decisionSources[0].contentSha256}`) {
        throw new Error(`compatibility-exception evidence ${evidenceRecord.id} is not bound to its immutable decision`);
      }
    }
  }
  for (const entry of manifest.fieldDispositions) {
    if (!domainIds.has(entry.acceptedDomain)) throw new Error(`${entry.recordKind}:${entry.path} references unknown domain`);
    for (const id of entry.sourceIds) {
      if (!sourceIds.has(id)) throw new Error(`${entry.recordKind}:${entry.path} references unknown source ${id}`);
    }
    for (const id of entry.evidenceIds) {
      if (!evidenceIds.has(id)) throw new Error(`${entry.recordKind}:${entry.path} references unknown evidence ${id}`);
    }
    if (entry.harmlessNewerBehavior && entry.evidenceIds.length === 0) {
      throw new Error(`${entry.recordKind}:${entry.path} classifies harmless newer behavior without focused evidence`);
    }
    for (const code of [entry.missing.code, entry.invalid.code, ...entry.valueRules.map((rule) => rule.code)].filter(Boolean)) {
      if (!code.startsWith("scene.") && !diagnosticCodes.has(code)) {
        throw new Error(`${entry.recordKind}:${entry.path} references unmapped diagnostic ${code}`);
      }
    }
    for (const recipe of [entry.missing.recipe, entry.invalid.recipe].filter(Boolean)) {
      if (!recipeIds.has(recipe) && !rendererRecipeIds.has(recipe)) {
        throw new Error(`${entry.recordKind}:${entry.path} references unknown recipe ${recipe}`);
      }
    }
  }
  for (const rule of manifest.normalizationRules) {
    for (const id of rule.sourceIds) {
      if (!sourceIds.has(id)) throw new Error(`normalization rule references unknown source ${id}`);
    }
    if (rule.code && !diagnosticCodes.has(rule.code)) throw new Error(`normalization rule references unmapped diagnostic ${rule.code}`);
  }
  for (const item of manifest.noticeCatalog) {
    if (sha256(Buffer.from(item.text, "utf8")) !== item.textSha256) throw new Error(`notice ${item.id} digest mismatch`);
    for (const id of item.sourceIds) {
      if (!sourceIds.has(id)) throw new Error(`notice ${item.id} references unknown source ${id}`);
    }
  }
  for (const recipe of manifest.fallbackRecipes) {
    const digest = sha256(canonicalBytes(recipe.content));
    if (recipe.contentDigest !== `sha256:${digest}` || recipe.recipeId !== `${recipe.recipeId.split("@sha256:", 1)[0]}@sha256:${digest}`) {
      throw new Error(`recipe ${recipe.recipeId} content digest mismatch`);
    }
    for (const id of recipe.noticeIds) {
      if (!noticeIds.has(id)) throw new Error(`recipe ${recipe.recipeId} references unknown notice ${id}`);
    }
    for (const id of recipe.sourceIds) {
      if (!sourceIds.has(id)) throw new Error(`recipe ${recipe.recipeId} references unknown source ${id}`);
    }
    for (const id of recipe.evidenceIds) {
      if (!evidenceIds.has(id)) throw new Error(`recipe ${recipe.recipeId} references unknown evidence ${id}`);
    }
    validateRecipeContent(recipe);
  }
  for (const recipeId of recipeIds) {
    if (!manifest.registryMappings.some((item) => item.recipeSelection.includes(recipeId))) {
      throw new Error(`recipe ${recipeId} has no public registry mapping`);
    }
  }
  const checkRecoveryValues = (value, pointer) => {
    if (Array.isArray(value)) return value.forEach((item, index) => checkRecoveryValues(item, `${pointer}/${index}`));
    if (value && typeof value === "object") {
      for (const [key, member] of Object.entries(value)) {
        if (key === "recovery" && member !== null && !RECOVERIES.includes(member)) {
          throw new Error(`${pointer}/${key} uses non-registry recovery ${member}`);
        }
        checkRecoveryValues(member, `${pointer}/${key}`);
      }
    }
  };
  checkRecoveryValues(manifest.registryMappings, "registryMappings");
  for (const [index, mapping] of manifest.registryMappings.entries()) {
    validateFormatRuleSet(mapping.formatRules, `registryMappings/${index}/formatRules`);
  }
  for (const [fieldIndex, entry] of manifest.fieldDispositions.entries()) {
    for (const [name, selectedFallback] of [["missing", entry.missing], ["invalid", entry.invalid]]) {
      if (selectedFallback.formatRules) validateFormatRuleSet(selectedFallback.formatRules, `fieldDispositions/${fieldIndex}/${name}/formatRules`);
    }
    for (const [ruleIndex, rule] of entry.valueRules.entries()) {
      if (rule.formatRules) validateFormatRuleSet(rule.formatRules, `fieldDispositions/${fieldIndex}/valueRules/${ruleIndex}/formatRules`);
    }
  }
  if (evidence.cases.length !== 7 || evidence.snapshots.length !== 5) {
    throw new Error("focused evidence case inventory is incomplete");
  }
  if (!manifest.rendererOwnedRecipeReferences.every((item) => item.definitionAuthority === "outcomes")) {
    throw new Error("renderer-owned recipes must be referenced from the public-outcomes decision");
  }
}

function buildManifest(schemaDigest, evidence, evidenceDigest, typeInventory) {
  const evidenceCatalog = buildEvidenceCatalog(evidence, evidenceDigest);
  const fieldDispositions = buildFieldDispositions(evidenceIndex(evidenceCatalog));
  const fallbackRecipes = buildFallbackRecipes();
  return {
    authorities: AUTHORITIES,
    compatibilityRevision: 1,
    conservativeUnmatchedPathRule: buildConservativeRule(),
    coverage: {
      elementKinds: ELEMENT_KINDS,
      fieldDispositionCount: fieldDispositions.length,
      officialFallbackCount: fallbackRecipes.length,
      recordKinds: RECORD_KINDS,
      status: "complete",
      upstreamTypeInventory: typeInventory,
      upstreamTypeInventoryDigest: `sha256:${sha256(canonicalBytes(typeInventory))}`,
      unresolved: [],
      validation: "JSON Schema shape plus dependency-free semantic validation; canonical RFC 8785 bytes; all source/evidence/notice/recipe references resolved; all official recipe contents digested.",
    },
    domainCatalog: DOMAIN_CATALOG,
    elementKinds: buildElementKinds(),
    evidenceCatalog,
    fallbackRecipes,
    fieldDispositions,
    noticeCatalog: buildNoticeCatalog(),
    normalizationRules: buildNormalizationRules(),
    recordKinds: buildRecordKinds(fieldDispositions),
    referenceProfile: {
      assetManifestDigest: `sha256:${ASSET_MANIFEST_DIGEST}`,
      excalidrawRelease: "v0.18.1",
      profileDigest: `sha256:${PROFILE_DIGEST}`,
      sourceCommit: EXCALIDRAW_COMMIT,
    },
    registryMappings: buildRegistryMappings(fallbackRecipes),
    rendererOwnedRecipeReferences: [
      { definitionAuthority: "outcomes", recipeId: "renderer.unknown-element.v1" },
      { definitionAuthority: "outcomes", recipeId: "renderer.malformed-element.v1" },
      { definitionAuthority: "outcomes", recipeId: "renderer.known-element-unavailable.v1" },
    ],
    schema: "excalidraw-render/scene-surface/v1",
    schemaDefinition: {
      digest: `sha256:${schemaDigest}`,
      path: pathForManifest(SCHEMA_PATH),
      validationCommand: "tools/reference-oracle/run-scene-surface.sh verify",
    },
    sourceCatalog: SOURCE_CATALOG,
  };
}

function main() {
  const mode = process.argv[2];
  if (mode !== "update" && mode !== "verify") {
    throw new Error("usage: node scripts/scene-surface.mjs <update|verify>");
  }
  const schemaBytes = canonicalBytes(SCHEMA);
  const schemaDigest = sha256(schemaBytes);
  const { digest: evidenceDigest, evidence } = readEvidence();
  const typeInventory = extractPinnedTypeInventory();
  const manifest = buildManifest(schemaDigest, evidence, evidenceDigest, typeInventory);
  validateManifest(manifest, evidence, typeInventory);
  const manifestBytes = canonicalBytes(manifest);
  const manifestDigest = sha256(manifestBytes);

  compareOrWrite(mode, SCHEMA_PATH, schemaBytes);
  compareOrWrite(mode, MANIFEST_PATH, manifestBytes);
  compareOrWrite(mode, MANIFEST_DIGEST_PATH, Buffer.from(`${manifestDigest}\n`, "ascii"));

  console.log(JSON.stringify({
    evidenceDigest,
    fieldDispositions: manifest.fieldDispositions.length,
    manifestDigest,
    manifestPath: pathForManifest(MANIFEST_PATH),
    recipes: manifest.fallbackRecipes.map((recipe) => recipe.recipeId),
    schemaDigest,
    validation: "pass",
  }, null, 2));
}

main();
