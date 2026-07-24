window.EXCALIDRAW_ASSET_PATH = `${window.location.origin}/assets/`;

const { exportToBlob, exportToSvg } = await import(
  "@excalidraw/excalidraw"
);

function validateScene(scene) {
  if (
    !scene ||
    scene.type !== "excalidraw" ||
    scene.version !== 2 ||
    !Array.isArray(scene.elements) ||
    !scene.appState ||
    !scene.files
  ) {
    throw new TypeError("scene must be an Excalidraw version 2 scene document");
  }
}

function selectedFrame(scene, ordinal) {
  const frames = scene.elements.filter(
    (element) =>
      element &&
      element.isDeleted !== true &&
      (element.type === "frame" || element.type === "magicframe"),
  );
  if (!Number.isSafeInteger(ordinal) || ordinal < 1 || ordinal > frames.length) {
    throw new RangeError("frame ordinal does not identify a live frame");
  }
  return frames[ordinal - 1];
}

async function blobToBase64(blob) {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

window.frameReferenceOracle = Object.freeze({
  async render(scene, ordinal, options) {
    validateScene(scene);
    const exportingFrame = selectedFrame(scene, ordinal);
    const hasScale = options.exportScale !== null;
    const hasMaximumDimension = options.maxWidthOrHeight !== null;
    if (hasScale === hasMaximumDimension) {
      throw new TypeError("exactly one sizing mode must be non-null");
    }
    const appState = {
      ...scene.appState,
      exportBackground: options.exportBackground,
      exportEmbedScene: options.exportEmbedScene,
      exportScale: options.exportScale ?? 1,
      exportWithDarkMode: options.exportWithDarkMode,
      frameRendering: {
        enabled: true,
        clip: true,
        name: false,
        outline: false,
      },
      viewBackgroundColor: options.viewBackgroundColor,
    };
    const common = {
      appState,
      elements: scene.elements,
      exportPadding: 0,
      exportingFrame,
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
