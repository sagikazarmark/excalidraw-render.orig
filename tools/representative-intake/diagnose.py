"""Crop-context probe over immutable observations; never changes admission gates."""
import json
from pathlib import Path
import subprocess
import sys


def main():
    directory, output = map(Path, sys.argv[1:])
    output.mkdir()
    plan = json.loads((directory / "regions.json").read_bytes())
    crop = dict(plan["crops"][0])
    # Preserve all original typography pixels; add only reference context.
    crop["y"] -= 3
    crop["height"] += 3
    proposal = {**plan, "crops": [crop]}
    (output / "context.json").write_text(json.dumps(proposal))
    for name, reference, candidate, mode in [
        ("png-context", "official-png.rgba.zlib", "native.png", "canonical"),
        ("pdfium-context", "official-chromium.rgba.zlib", "native-pdfium.rgba.zlib", "canonical"),
        ("pdfjs-context", "official-chromium.rgba.zlib", "native-pdfjs.rgba.zlib", "pdfjs")]:
        subprocess.run(["cargo", "run", "--locked", "--release", "--manifest-path", "tools/representative-intake/compare/Cargo.toml", "--",
            str(directory / reference), str(directory / candidate), mode, str(output / "context.json"), str(output / f"{name}.json")], check=True)


if __name__ == "__main__": main()
