"""Read-only localization of retained raster differences, not a parity gate."""
import argparse
from pathlib import Path
import struct
import zlib
import json


def png(path):
    data = path.read_bytes()
    cursor, packed = 8, b""
    while cursor < len(data):
        size = int.from_bytes(data[cursor:cursor+4], "big")
        kind, payload = data[cursor+4:cursor+8], data[cursor+8:cursor+8+size]
        if kind == b"IHDR":
            width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            assert (depth, color, compression, filtering, interlace) == (8, 6, 0, 0, 0)
        if kind == b"IDAT": packed += payload
        cursor += size + 12
    stream = zlib.decompress(packed)
    stride = width * 4
    output, previous = bytearray(), bytearray(stride)
    for y in range(height):
        kind = stream[y*(stride+1)]
        row = bytearray(stream[y*(stride+1)+1:(y+1)*(stride+1)])
        for x in range(stride):
            a, b, c = row[x-4] if x >= 4 else 0, previous[x], previous[x-4] if x >= 4 else 0
            p = a + b - c
            paeth = min(((abs(p-a),0,a),(abs(p-b),1,b),(abs(p-c),2,c)))[2]
            row[x] = (row[x] + [0,a,b,(a+b)//2,paeth][kind]) & 255
        output += row
        previous = row
    return width, height, output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    directory = args.directory
    width, height, native = png(directory / "native.png")
    reference = zlib.decompress((directory / "official-png.rgba.zlib").read_bytes())
    plan = json.loads((directory / "regions.json").read_bytes())
    results = []
    for crop in plan["crops"]:
        rows = []
        for y in range(crop["y"], crop["y"]+crop["height"]):
            exact = over_16 = 0
            for x in range(crop["x"], crop["x"]+crop["width"]):
                offset = (y*width+x)*4
                differences = [abs(reference[offset+i]-native[offset+i]) for i in range(4)]
                exact += max(differences)>0
                over_16 += max(differences)>16
            if exact: rows.append({"y":y,"different":exact,"delta_over_16":over_16})
        results.append({"crop":crop["id"],"rows":rows})
    print(json.dumps({"note":"raw channel deltas, not canonical Pixelmatch/SSIM", "width":width,"height":height,"results":results},indent=2))


if __name__ == "__main__": main()
