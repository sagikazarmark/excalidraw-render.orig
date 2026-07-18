window.EXCALIDRAW_ASSET_PATH = `${window.location.origin}/assets/`;

const { exportToBlob, exportToSvg } = await import(
  "@excalidraw/excalidraw"
);

const REQUIRED_ELEMENT_FIELDS = [
  "id",
  "type",
  "x",
  "y",
  "width",
  "height",
  "angle",
  "strokeColor",
  "backgroundColor",
  "fillStyle",
  "strokeWidth",
  "strokeStyle",
  "roughness",
  "opacity",
  "groupIds",
  "frameId",
  "index",
  "seed",
  "version",
  "versionNonce",
  "isDeleted",
  "boundElements",
  "updated",
  "link",
  "locked",
];

const REQUIRED_EXPORT_OPTIONS = [
  "exportBackground",
  "exportEmbedScene",
  "exportPadding",
  "exportScale",
  "exportWithDarkMode",
  "frameRendering",
  "maxWidthOrHeight",
  "viewBackgroundColor",
];

function requireOwnProperties(value, fields, subject) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${subject} must be an object`);
  }

  for (const field of fields) {
    if (!Object.hasOwn(value, field)) {
      throw new TypeError(`${subject} is missing ${field}`);
    }
  }
}

function validateScene(scene) {
  requireOwnProperties(scene, ["type", "version", "elements", "appState", "files"], "scene");
  if (scene.type !== "excalidraw" || scene.version !== 2) {
    throw new TypeError("scene must be an Excalidraw version 2 scene document");
  }
  if (!Array.isArray(scene.elements)) {
    throw new TypeError("scene.elements must be an array");
  }
  scene.elements.forEach((element, index) => {
    requireOwnProperties(element, REQUIRED_ELEMENT_FIELDS, `scene.elements[${index}]`);
  });
}

async function blobToBase64(blob) {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

window.referenceOracle = Object.freeze({
  async render(scene, options) {
    validateScene(scene);
    requireOwnProperties(options, REQUIRED_EXPORT_OPTIONS, "export options");
    requireOwnProperties(
      options.frameRendering,
      ["enabled", "clip", "name", "outline"],
      "export options.frameRendering",
    );
    const hasScale = options.exportScale !== null;
    const hasMaximumDimension = options.maxWidthOrHeight !== null;
    if (hasScale === hasMaximumDimension) {
      throw new TypeError("exactly one of exportScale or maxWidthOrHeight must be non-null");
    }

    const appState = {
      ...scene.appState,
      exportBackground: options.exportBackground,
      exportEmbedScene: options.exportEmbedScene,
      exportScale: options.exportScale ?? 1,
      exportWithDarkMode: options.exportWithDarkMode,
      frameRendering: options.frameRendering,
      viewBackgroundColor: options.viewBackgroundColor,
    };
    const common = {
      appState,
      elements: scene.elements,
      exportPadding: options.exportPadding,
      exportingFrame: null,
      files: scene.files,
    };

    const svg = await exportToSvg({
      ...common,
      renderEmbeddables: false,
      reuseImages: true,
      skipInliningFonts: undefined,
    });
    const png = await exportToBlob({
      ...common,
      ...(hasScale
        ? {
            getDimensions: (width, height) => ({
              height: height * options.exportScale,
              scale: options.exportScale,
              width: width * options.exportScale,
            }),
          }
        : { maxWidthOrHeight: options.maxWidthOrHeight }),
      mimeType: "image/png",
    });

    return {
      pngBase64: await blobToBase64(png),
      svg: new XMLSerializer().serializeToString(svg),
    };
  },
});
