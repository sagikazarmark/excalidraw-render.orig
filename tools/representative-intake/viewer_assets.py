"""Multi-face SVG viewer resources for representative evidence (#84).

Reproduces the Stage-4 single-face adapter (stage4-viewer-assets.mjs plus
`xtask conformance candidate-prepare-assets`) byte for byte, and additionally:

- declares every embedded face, so text may use more than one family;
- admits an official text family the exporter did not embed only through an
  observed substitution: the pinned reference browser's platform font for that
  family (CDP and fontconfig must agree, and the file must be a pinned system
  font). Those exact bytes are wrapped as WOFF2 with the renderer's encoder and
  injected into the official viewer SVGs. Geometry and text stay unchanged.

The approved Stage-4 adapter is not modified; its sources are bound by approved
candidate manifests.
"""
import base64
import json
from pathlib import Path
import re
import subprocess
import tempfile
import urllib.parse

import intake

ROOT = Path(__file__).resolve().parents[2]
FONTS_TOOL = ROOT / "tools/representative-intake/viewer-fonts"
IMAGE = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
RESOLUTION = "official-font-resolution.json"
SUBSTITUTED = "official-substituted.svg"
SCHEMA = "excalidraw-render/svg-viewer-asset-declaration/v1"
FONT_FACE = re.compile(r"@font-face\s*\{([^}]*)\}")
FORMAT_HINT = re.compile(r"(@font-face\s*\{[^}]*?src\s*:\s*url\(data:font/woff2;base64,[A-Za-z0-9+/=]+\))(?=\s*[;}])")


def fonts_tool(*args, stdin=None):
    binary = FONTS_TOOL / "target/release/representative-intake-viewer-fonts"
    subprocess.run(["cargo", "build", "--locked", "--release", "--quiet", "--manifest-path", str(FONTS_TOOL / "Cargo.toml")],
                   cwd=ROOT, check=True, timeout=600)
    result = subprocess.run([str(binary), *map(str, args)], cwd=ROOT, input=stdin, capture_output=True, timeout=120)
    intake.require(result.returncode == 0, f"viewer-fonts {args[0]} failed: {result.stderr.decode()}")
    return json.loads(result.stdout)


def field(css, name):
    match = re.search(fr"{name}\s*:\s*([^;]+)", css, re.IGNORECASE)
    return match[1].strip() if match else None


def unquote(value):
    return value.strip().strip("'\"")


def text_families(svg):
    """(complete font-family attribute, text content) for every text element."""
    texts = []
    for match in re.finditer(r"<text\b([^>]*)>(.*?)</text>", svg, re.DOTALL):
        family = re.search(r'\bfont-family="([^"]*)"', match[1])
        intake.require(family is not None, "text without an explicit font-family")
        texts.append((family[1], re.sub(r"<[^>]*>", "", match[2])))
    return texts


def primary(family):
    return unquote(family.split(",")[0])


def undeclared_families(svg):
    embedded = {unquote(field(m[1], "font-family")) for m in FONT_FACE.finditer(svg)}
    return sorted({primary(family) for family, _ in text_families(svg)} - embedded)


def observe(directory):
    """Measure substitutes for undeclared official families in the pinned container."""
    directory = Path(directory)
    official = (directory / "official.svg").read_text()
    if not undeclared_families(official):
        return None
    record = directory / RESOLUTION
    subprocess.run(["docker", "run", "--rm", "--init", "--ipc=host", "--network", "none", "--platform", "linux/amd64",
                    "--user", f"{subprocess.check_output(['id', '-u'], text=True).strip()}:{subprocess.check_output(['id', '-g'], text=True).strip()}",
                    "--env", "HOME=/tmp", "--env", "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1", "--env", "REFERENCE_ORACLE_CANONICAL=1",
                    "--mount", f"type=bind,src={ROOT},dst=/workspace", "--workdir", "/workspace/tools/reference-oracle", IMAGE,
                    "node", "../representative-intake/font-resolution.mjs", relative(directory / "official.svg"), relative(record)],
                   cwd=ROOT, check=True, capture_output=True, timeout=600)
    resolution = intake.decode(record.read_bytes())
    (directory / "assets").mkdir(exist_ok=True)
    for index, entry in enumerate(substitutions(resolution, official)):
        path = directory / "assets" / f"official-substitute-{index}{Path(entry['system_font']['path']).suffix}"
        data = subprocess.run(["docker", "run", "--rm", "--network", "none", "--platform", "linux/amd64", IMAGE,
                               "cat", entry["system_font"]["path"]], check=True, capture_output=True, timeout=300).stdout
        intake.require(intake.descriptor(data) == {k: entry["system_font"][k] for k in ("bytes", "sha256")}, "system font differs from the reference profile")
        path.write_bytes(data)
    return resolution


def substitutions(resolution, official):
    """Validated substitutes, one per undeclared official family, in family order."""
    intake.require(resolution["schema"] == "excalidraw-render/representative-font-resolution/v1" and resolution["approval"] is None, "invalid font resolution record")
    intake.require(resolution["input"]["sha256"] == intake.descriptor(official.encode())["sha256"], "font resolution detached from official SVG")
    missing = undeclared_families(official)
    entries = [e for e in resolution["undeclared"] if e["requested_family"] in missing]
    intake.require(sorted(e["requested_family"] for e in entries) == missing, "every undeclared official family needs exactly one observed substitute")
    for entry in entries:
        intake.require(entry["agrees"] and entry["chromium"]["postScriptName"] == entry["fontconfig"]["postscript_name"], "Chromium and fontconfig disagree")
        intake.require(entry["system_font"]["path"] == entry["fontconfig"]["file"], "substitute is not the fontconfig match")
    pinned = intake.decode((ROOT / "tests/oracle/reference-profiles/revision-1-assets.json").read_bytes())["systemFonts"]["assets"]
    for entry in entries:
        intake.require(entry["system_font"] in pinned, "substitute is not a pinned system font")
    return sorted(entries, key=lambda e: e["requested_family"])


def relative(path):
    return str(Path(path).resolve().relative_to(ROOT))


def pretty_js(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def pretty_serde(value):
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True).encode()


def derive(directory):
    """Return {repository path: bytes} for every derived viewer resource."""
    directory = Path(directory)
    name = relative(directory)
    outputs, plans = {}, []
    with tempfile.TemporaryDirectory() as temporary:
        temporary = Path(temporary)
        for kind in ("official", "native"):
            svg = (directory / f"{kind}.svg").read_text()
            substituted = svg
            fonts, images = [], []
            for i, match in enumerate(FONT_FACE.finditer(svg)):
                css = match[1]
                url = unquote(re.search(r"url\(([^)]+)\)", css)[1])
                intake.require(url.startswith("data:font/woff2;base64,"), "undeclared font encoding")
                data = base64.b64decode(url.split(",", 1)[1])
                path = f"{name}/assets/{kind}-font-{i}.woff2"
                outputs[path] = data
                weight = field(css, "font-weight") or "400"
                fonts.append({"woff2": path, "source": {"media_type": "font/woff2", "sha256": intake.descriptor(data)["sha256"]},
                              "css": {"family": unquote(field(css, "font-family")), "style": field(css, "font-style") or "normal",
                                      "weight": 400 if weight == "normal" else 700 if weight == "bold" else int(weight),
                                      "stretch": field(css, "font-stretch") or "normal",
                                      "unicode_range": [v.strip() for v in (field(css, "unicode-range") or "U+0-10FFFF").split(",")]}})
            if kind == "official" and undeclared_families(svg):
                resolution = intake.decode((directory / RESOLUTION).read_bytes())
                rules = []
                for index, entry in enumerate(substitutions(resolution, svg)):
                    source = directory / "assets" / f"official-substitute-{index}{Path(entry['system_font']['path']).suffix}"
                    intake.require(intake.descriptor(source.read_bytes()) == {k: entry["system_font"][k] for k in ("bytes", "sha256")}, "retained substitute changed")
                    woff2 = temporary / f"substitute-{index}.woff2"
                    fonts_tool("encode", source, woff2)
                    data = woff2.read_bytes()
                    face = fonts_tool("decode", woff2, temporary / f"substitute-{index}.sfnt")
                    path = f"{name}/assets/official-font-{len(fonts)}.woff2"
                    outputs[path] = data
                    family = entry["requested_family"]
                    fonts.append({"woff2": path, "source": {"media_type": "font/woff2", "sha256": intake.descriptor(data)["sha256"]},
                                  "css": {"family": family, "style": face["style"], "weight": face["weight"], "stretch": face["stretch"],
                                          "unicode_range": ["U+0-10FFFF"]},
                                  "substitution": {"requested_family": family, "system_font": entry["system_font"],
                                                   "chromium": entry["chromium"], "retained": relative(source),
                                                   "resolution": {"path": f"{name}/{RESOLUTION}", **intake.descriptor((directory / RESOLUTION).read_bytes())}}})
                    rules.append(f"@font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{base64.b64encode(data).decode()}) format('woff2'); }}")
                # Chromium renders these exported bytes plus only the injected
                # face; the Rust inputs derive from the same document.
                closing = svg.find("</style>")
                intake.require(closing >= 0, "official SVG has no font style element for the substitute")
                substituted = svg[:closing] + "".join(f"\n      {rule}" for rule in rules) + svg[closing:]
                outputs[f"{name}/{SUBSTITUTED}"] = substituted.encode()
            view = FORMAT_HINT.sub(r"\1 format('woff2')", substituted)
            for i, match in enumerate(re.finditer(r'<image\b[^>]*\b(?:href|xlink:href)="([^"]+)"[^>]*>', svg)):
                meta, _, payload = match[1].partition(",")
                media = meta[5:].split(";")[0]
                data = base64.b64decode(payload) if meta.endswith(";base64") else urllib.parse.unquote_to_bytes(payload)
                path = f"{name}/assets/{kind}-image-{i}.{'svg' if media == 'image/svg+xml' else 'png'}"
                outputs[path] = data
                digest = intake.descriptor(data)["sha256"]
                images.append({"source": {"media_type": media, "sha256": digest}, "canonical": {"path": path, "media_type": media, "sha256": digest}})
            outputs[f"{name}/{kind}-view.svg"] = view.encode()
            plans.append({"kind": kind, "fonts": fonts, "images": images})

            # Rust viewer: each text names exactly its primary family, which
            # must be a declared face that covers every character.
            families = {font["css"]["family"]: font for font in fonts}
            rust = view
            for full, text in text_families(view):
                family = primary(full)
                intake.require(family in families, f"text family {family!r} has no declared face")
                font = families[family]
                sfnt = temporary / f"{kind}-cover-{family}.sfnt"
                if not sfnt.exists():
                    fonts_tool("decode", write(temporary, font["woff2"], outputs), sfnt)
                text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", '"').replace("&apos;", "'")
                missing = fonts_tool("covers", sfnt, stdin=text.encode())["missing"]
                intake.require(not missing, f"{kind} face {family!r} does not cover {missing}")
                rust = rust.replace(f'font-family="{full}"', f'font-family="{family}"')
            outputs[f"{name}/{kind}-rust.svg"] = rust.encode()

            declared = []
            for i, font in enumerate(fonts):
                woff2 = write(temporary, font["woff2"], outputs)
                sfnt_path = f"{name}/assets/{kind}-font-{i}.sfnt"
                face = fonts_tool("decode", woff2, temporary / f"{kind}-font-{i}.sfnt")
                outputs[sfnt_path] = (temporary / f"{kind}-font-{i}.sfnt").read_bytes()
                declared.append({"source": font["source"], "css": font["css"], "sfnt": {"path": sfnt_path, **face}})
            outputs[f"{name}/{kind}-assets.json"] = pretty_serde({"schema": SCHEMA, "fonts": declared, "images": images})
    outputs[f"{name}/viewer-assets-plan.json"] = pretty_js(plans)
    return outputs


def write(temporary, path, outputs):
    target = temporary / Path(path).name
    if not target.exists():
        target.write_bytes(outputs[path])
    return target


def collect(directory):
    observe(directory)
    for path, data in derive(directory).items():
        target = ROOT / path
        if target.exists():
            intake.require(target.read_bytes() == data, f"derived viewer resource differs: {path}")
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)


def verify(directory):
    for path, data in derive(directory).items():
        intake.require((ROOT / path).read_bytes() == data, f"derived viewer resource differs: {path}")


def chromium_input(directory, kind):
    """The SVG the Chromium viewer renders: exported bytes, or the substituted official document."""
    substituted = Path(directory) / SUBSTITUTED
    return substituted if kind == "official" and substituted.exists() else Path(directory) / f"{kind}.svg"


def main():
    import argparse
    import sys
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=("collect", "verify"))
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    relative(args.directory)
    try:
        (collect if args.command == "collect" else verify)(args.directory)
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
    print(json.dumps({"status": f"{args.command}ed", "approval": None}))


if __name__ == "__main__":
    main()
