// Nested SVG sanitization and reference variants (#85 nested-SVG facets).
// Every row is a 24x24 SVG drawn as an image element of its natural size. A
// green ground and a red or blue square show what the reference draws. Each
// row declares its expectation:
//   match     - the renderer must equal the reference;
//   rejected  - the reference rejects the image;
//   divergent - the reference draws active or foreign content the renderer
//               removes (the declared divergence, spec section 8.2); the
//               renderer must equal its static twin `twin`, or, with no twin,
//               its pinned placeholder.
import assert from "node:assert/strict";
import { deflateSync, crc32 } from "node:zlib";

const HEAD = '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="24" height="24" viewBox="0 0 24 24"';
const svg = (body, { prolog = "", attrs = "" } = {}) =>
  `${prolog}${HEAD}${attrs}><rect width="24" height="24" fill="#20a040"/>${body}</svg>`;
const RED = '<rect x="4" y="4" width="16" height="16" fill="#e02020"/>';
const EXT = "http://example.invalid";

function png(rgb) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const size = Buffer.alloc(4); size.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
    return Buffer.concat([size, body, crc]);
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(2, 0); ihdr.writeUInt32BE(2, 4); ihdr.set([8, 2, 0, 0, 0], 8);
  const row = Buffer.from([0, ...rgb, ...rgb]);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(Buffer.concat([row, row]))), chunk("IEND", Buffer.alloc(0))]);
}
const b64 = s => Buffer.from(s).toString("base64");
const attr = s => s.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;");
const image = href => `<image x="4" y="4" width="16" height="16" href="${attr(href)}"/>`;

export function nestedSvgFixtures() {
  const rows = [];
  const add = (id, group, expectation, source, twin) => rows.push({ id, group, expectation, ...(twin ? { twin } : {}), svg: source });
  const inner = svg('<rect x="2" y="2" width="20" height="20" fill="#2040e0"/>');
  const redPng = png([224, 32, 32]);

  add("base", "base", "match", svg(""));
  add("base-red", "base", "match", svg(RED));
  add("base-quarter", "base", "match", svg('<rect x="4" y="4" width="8" height="8" fill="#e02020"/>'));
  // Active content: never runs, in both.
  add("script", "sanitization", "match", svg(`${RED}<script>document.querySelector("rect").setAttribute("fill","#0000ff")</script>`));
  add("onload", "sanitization", "match", svg(RED, { attrs: ` onload="this.querySelector('rect').setAttribute('fill','#0000ff')"` }));
  add("link-javascript", "sanitization", "match", svg(`<a href="javascript:alert(1)">${RED}</a>`));
  add("xml-stylesheet", "sanitization", "match", svg(RED, { prolog: `<?xml-stylesheet href="${EXT}/a.css"?>` }));
  add("external-entity", "sanitization", "match", svg(`<desc>&x;</desc>${RED}`, { prolog: `<!DOCTYPE svg [<!ENTITY x SYSTEM "${EXT}/x">]>` }));
  // Internal entities expand; an undefined entity or a bomb rejects.
  add("internal-entity", "sanitization", "match", svg('<rect x="4" y="4" width="16" height="16" fill="&c;"/>', { prolog: '<!DOCTYPE svg [<!ENTITY c "#e02020">]>' }));
  add("entity-nested", "sanitization", "match", svg('<rect x="4" y="4" width="16" height="16" fill="&b;20"/>', { prolog: '<!DOCTYPE svg [<!ENTITY a "#e0"><!ENTITY b "&a;20">]>' }));
  add("entity-element", "sanitization", "match", svg("&r;", { prolog: `<!DOCTYPE svg [<!ENTITY r "<rect x='4' y='4' width='16' height='16' fill='#e02020'/>">]>` }));
  add("entity-undefined", "sanitization", "rejected", svg(`${RED}<rect x="4" y="4" width="8" height="8" fill="&nope;"/>`));
  const bomb = ['<!ENTITY a "aaaaaaaaaa">', ...Array.from({ length: 6 }, (_, i) => `<!ENTITY ${String.fromCharCode(98 + i)} "${`&${String.fromCharCode(97 + i)};`.repeat(10)}">`)].join("");
  add("entity-bomb", "sanitization", "rejected", svg(`<desc>&g;</desc>${RED}`, { prolog: `<!DOCTYPE svg [${bomb}]>` }));
  // External resources: a missing reference, in paint and elsewhere.
  add("fill-url-external", "sanitization", "match", svg(`<rect x="4" y="4" width="16" height="16" fill="url(${EXT}/a.svg#g)"/>`));
  add("fill-url-external-fallback", "sanitization", "match", svg(`<rect x="4" y="4" width="16" height="16" fill="url(${EXT}/a.svg#g) #2020e0"/>`));
  add("stroke-url-external", "sanitization", "match", svg(`<rect x="4" y="4" width="16" height="16" fill="#e02020" stroke="url(${EXT}/a.svg#g)" stroke-width="4"/>`));
  add("stroke-url-external-fallback", "sanitization", "match", svg(`<rect x="4" y="4" width="16" height="16" fill="#e02020" stroke="url(${EXT}/a.svg#g) #2020e0" stroke-width="4"/>`));
  add("style-url-external", "sanitization", "match", svg(`<rect x="4" y="4" width="16" height="16" fill="#e02020" style="fill:url(${EXT}/a.svg#g) #2020e0"/>`));
  add("style-url-external-no-fallback", "sanitization", "match", svg(`<rect x="4" y="4" width="16" height="16" fill="#e02020" style="fill:url(${EXT}/a.svg#g)"/>`));
  add("stylesheet-url-external-fallback", "sanitization", "match", svg(`<style>rect.a{fill:url(${EXT}/a.svg#g) #2020e0} rect.b{stroke:#e0e020}</style><rect class="a b" x="4" y="4" width="16" height="16" stroke-width="4"/>`));
  add("mask-url-external", "sanitization", "match", svg(`${RED}<rect x="4" y="4" width="8" height="8" fill="#2020e0" mask="url(${EXT}/m.svg#m)"/>`));
  add("external-use", "sanitization", "match", svg(`${RED}<use href="${EXT}/a.svg#r"/>`));
  add("xml-base", "sanitization", "match", svg(`<g xml:base="${EXT}/"><use href="a.svg#r"/></g>${RED}`));
  add("import", "sanitization", "match", svg(`<style>@import url(${EXT}/a.css); rect.a{fill:#e02020}</style><rect class="a" x="4" y="4" width="16" height="16" fill="#2020e0"/>`));
  add("import-middle", "sanitization", "match", svg(`<style>rect.a{fill:#2020e0} @import url(${EXT}/a.css); rect.b{fill:#e02020}</style><rect class="a b" x="4" y="4" width="16" height="16"/>`));
  add("font-face", "sanitization", "match", svg(`<style>@font-face{font-family:X;src:url(${EXT}/x.woff)} rect.a{fill:#e02020}</style><rect class="a" x="4" y="4" width="16" height="16"/>`));
  add("font-face-data", "sanitization", "match", svg('<style>@font-face{font-family:X;src:url(data:font/woff2;base64,AAAA)} rect.a{fill:#e02020}</style><rect class="a" x="4" y="4" width="16" height="16"/>'));
  // Declared divergences: animation state at draw time, foreignObject and the
  // external image the reference draws its own placeholder for.
  add("animate", "sanitization", "divergent", svg('<rect x="4" y="4" width="16" height="16" fill="#e02020"><animate attributeName="fill" from="#2020e0" to="#e0e020" dur="1s" fill="freeze"/></rect>'), "base-red");
  add("animate-later", "sanitization", "match", svg('<rect x="4" y="4" width="16" height="16" fill="#e02020"><animate attributeName="fill" from="#2020e0" to="#e0e020" begin="5s" dur="1s"/></rect>'));
  add("set", "sanitization", "divergent", svg('<rect x="4" y="4" width="16" height="16" fill="#e02020"><set attributeName="fill" to="#2020e0"/></rect>'), "base-red");
  add("animate-transform", "sanitization", "divergent", svg('<rect x="4" y="4" width="8" height="8" fill="#e02020"><animateTransform attributeName="transform" type="translate" from="8 8" to="0 0" dur="1s" fill="freeze"/></rect>'), "base-quarter");
  add("animate-motion", "sanitization", "divergent", svg('<rect x="4" y="4" width="8" height="8" fill="#e02020"><animateMotion path="M8,8 L0,0" dur="1s" fill="freeze"/></rect>'), "base-quarter");
  add("css-animation", "sanitization", "divergent", svg('<style>@keyframes k{from{fill:#2020e0}to{fill:#e0e020}} rect.a{animation:k 1s infinite}</style><rect class="a" x="4" y="4" width="16" height="16" fill="#e02020"/>'), "base-red");
  add("foreign-object", "sanitization", "divergent", svg('<foreignObject x="4" y="4" width="16" height="16"><div xmlns="http://www.w3.org/1999/xhtml" style="width:16px;height:16px;background:#e02020"></div></foreignObject>'), "base");
  add("external-image", "sanitization", "divergent", svg(`${RED}<image x="4" y="4" width="16" height="16" href="${EXT}/a.png"/>`));
  // Local references.
  add("use-fragment", "references", "match", svg('<defs><rect id="r" x="4" y="4" width="16" height="16" fill="#e02020"/></defs><use href="#r"/>'));
  add("gradient-fragment", "references", "match", svg('<defs><linearGradient id="g"><stop offset="0" stop-color="#e02020"/><stop offset="1" stop-color="#2020e0"/></linearGradient></defs><rect x="4" y="4" width="16" height="16" fill="url(#g)"/>'));
  add("fill-url-missing", "references", "match", svg('<rect x="4" y="4" width="16" height="16" fill="url(#missing)"/>'));
  add("fill-url-missing-fallback", "references", "match", svg('<rect x="4" y="4" width="16" height="16" fill="url(#missing) #2020e0"/>'));
  add("use-cycle", "references", "match", svg(`${RED}<defs><g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></defs><use href="#a"/>`));
  add("use-self", "references", "match", svg(`${RED}<use id="u" href="#u"/>`));
  add("use-cycle-drawn", "references", "match", svg(`<defs><g id="a"><rect x="4" y="4" width="8" height="8" fill="#2020e0"/><use href="#a"/></g></defs>${RED}<use href="#a"/>`));
  add("pattern-cycle", "references", "match", svg(`<defs><pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="4" height="4" fill="url(#p)"/></pattern></defs>${RED}<rect x="4" y="4" width="8" height="8" fill="url(#p)"/>`));
  add("mask-cycle", "references", "match", svg(`${RED}<defs><mask id="m"><rect width="24" height="24" fill="white" mask="url(#m)"/></mask></defs><rect x="4" y="4" width="8" height="8" fill="#2020e0" mask="url(#m)"/>`));
  add("clip-cycle", "references", "match", svg(`${RED}<defs><clipPath id="c" clip-path="url(#c)"><rect width="12" height="12"/></clipPath></defs><rect x="4" y="4" width="16" height="16" fill="#2020e0" clip-path="url(#c)"/>`));
  add("gradient-href-cycle", "references", "match", svg(`<defs><linearGradient id="a" href="#b"/><linearGradient id="b" href="#a"/></defs>${RED}<rect x="4" y="4" width="8" height="8" fill="url(#a) #2020e0"/>`));
  // Bounded data images, base64 and not, recursively sanitized.
  add("data-png", "references", "match", svg(image(`data:image/png;base64,${redPng.toString("base64")}`)));
  add("data-png-percent", "references", "match", svg(image(`data:image/png,${[...redPng].map(b => "%" + b.toString(16).padStart(2, "0")).join("")}`)));
  add("data-svg", "references", "match", svg(image(`data:image/svg+xml;base64,${b64(inner)}`)));
  add("data-svg-utf8", "references", "match", svg(image(`data:image/svg+xml,${encodeURIComponent(inner)}`)));
  add("data-svg-utf8-parameter", "references", "match", svg(image(`data:image/svg+xml;utf8,${inner.replaceAll("#", "%23")}`)));
  const scripted = svg('<script>1</script><rect x="2" y="2" width="20" height="20" fill="#2040e0"/>');
  add("data-svg-script", "references", "match", svg(image(`data:image/svg+xml;base64,${b64(scripted)}`)));
  add("data-svg-utf8-script", "references", "match", svg(image(`data:image/svg+xml,${encodeURIComponent(scripted)}`)));
  add("feimage-data", "references", "match", svg(`<filter id="f" x="0" y="0" width="1" height="1"><feImage href="data:image/png;base64,${redPng.toString("base64")}"/></filter><rect x="4" y="4" width="16" height="16" filter="url(#f)"/>`));

  assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
  for (const row of rows) if (row.twin) assert.ok(rows.some(r => r.id === row.twin && r.expectation === "match"), row.id);
  return rows.map(row => ({ ...row, size: [24, 24], svg: b64(row.svg) }));
}

export function sceneFor(row) {
  const [width, height] = row.size;
  return { type: "excalidraw", version: 2, source: "nested-svg", appState: { viewBackgroundColor: "#ffffff" },
    elements: [{ id: "image", type: "image", x: 0, y: 0, width, height, angle: 0, strokeColor: "transparent", backgroundColor: "transparent",
      fillStyle: "solid", strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [], frameId: null, roundness: null, seed: 1,
      version: 1, versionNonce: 1, isDeleted: false, boundElements: null, updated: 1, link: null, locked: false, status: "saved", fileId: "asset",
      scale: [1, 1], crop: null, index: "a0" }],
    files: { asset: { id: "asset", mimeType: "image/svg+xml", dataURL: `data:image/svg+xml;base64,${row.svg}`, created: 1 } } };
}
