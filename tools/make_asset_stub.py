"""Generate the non-copyright placeholder asset tree used when work/all is absent (CI).

The retail unpacked tree (work/all: Picture/, SystemFile/, ...) is not in Git (DECISIONS
D-11), so CI cannot index real assets. AssetDB falls back to
godot/tests/fixtures/asset_stub/ when the default root has no files. These stubs are
18x18 solid PNGs (the retail window skin is 18x18) plus a silent 16-bit WAV, so every
code path that touches a texture/audio stream still runs in CI.

Run: python tools/make_asset_stub.py
"""
import os
import struct
import zlib

ROOT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                    "godot", "tests", "fixtures", "asset_stub")
SIZE = 18


def png_bytes(rgba):
    raw = b"".join(b"\x00" + bytes(rgba) * SIZE for _ in range(SIZE))

    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b""))


def write_png(rel, rgba):
    path = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as handle:
        handle.write(png_bytes(rgba))
    print("wrote", rel, len(png_bytes(rgba)), "bytes")


def write_wav(rel, samples=8, rate=22050):
    path = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = b"\x00\x00" * samples
    header = (b"RIFF" + struct.pack("<I", 36 + len(data)) + b"WAVE"
              + b"fmt " + struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16)
              + b"data" + struct.pack("<I", len(data)))
    with open(path, "wb") as handle:
        handle.write(header + data)
    print("wrote", rel, len(header) + len(data), "bytes")


def main():
    write_png("Picture/face01.png", (208, 128, 96, 255))
    write_png("Picture/face02.png", (96, 144, 208, 255))
    write_png("SystemFile/WindowBase.png", (31, 36, 37, 223))
    write_png("SystemFile/TitleGraphic.png", (24, 24, 32, 255))
    write_png("SystemFile/Text_Pause.png", (245, 245, 245, 255))
    write_png("SystemFile/CursorBase.png", (245, 200, 96, 255))
    write_wav("Sound/stub.wav")
    print("ASSET_STUB_OK", ROOT)


if __name__ == "__main__":
    main()
