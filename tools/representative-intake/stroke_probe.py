"""Measure fixed interior straight-edge samples in the isolated client rectangle."""
import argparse
import json
from pathlib import Path
import zlib

from probe import png


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    width, height, candidate = png(args.directory / "light-0.png")
    reference = zlib.decompress((args.directory / "reference/light/official-png.rgba.zlib").read_bytes())
    rows = []
    for x, y in [(1350,y) for y in range(32,39)] + [(x,105) for x in range(1186,1194)]:
        offset = (y*width+x)*4
        rows.append({"x":x,"y":y,"reference":list(reference[offset:offset+4]),"native":list(candidate[offset:offset+4])})
    print(json.dumps({"width":width,"height":height,"samples":rows},indent=2))


if __name__ == "__main__": main()
