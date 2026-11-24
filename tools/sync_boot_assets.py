#!/usr/bin/env python3
"""Generate Android launcher/adaptive icons from godot/assets/boot/splash_static.png.

Uses Pillow when available and falls back to a pure-Python stdlib (zlib + struct)
PNG reader/writer so the script works in any Python 3 environment.
"""
from __future__ import annotations

import argparse
import struct
import sys
import zlib
from pathlib import Path

try:
    from PIL import Image as _PilImage
except ImportError:
    _PilImage = None

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = REPO_ROOT / "godot/assets/boot/splash_static.png"
PLACEHOLDER_SIZE = (1024, 576)
MAIN_ICON_SIZE = 192
ADAPTIVE_SIZE = 432
ADAPTIVE_SAFE_RATIO = 0.62
ADAPTIVE_SAFE_SIZE = int(round(ADAPTIVE_SIZE * ADAPTIVE_SAFE_RATIO))  # 268
BLACK_THRESHOLD = 8

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def write_png_rgba(path: Path, width: int, height: int, rgba: bytes | bytearray) -> None:
    expected = width * height * 4
    if len(rgba) != expected:
        raise ValueError(f"RGBA buffer size mismatch: got {len(rgba)}, expected {expected}")
    stride = width * 4
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        start = y * stride
        raw.extend(rgba[start : start + stride])
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    compressed = zlib.compress(bytes(raw), level=9)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        PNG_SIGNATURE
        + _png_chunk(b"IHDR", ihdr)
        + _png_chunk(b"IDAT", compressed)
        + _png_chunk(b"IEND", b"")
    )


def write_png_rgb(path: Path, width: int, height: int, rgb: bytes | bytearray) -> None:
    expected = width * height * 3
    if len(rgb) != expected:
        raise ValueError(f"RGB buffer size mismatch: got {len(rgb)}, expected {expected}")
    stride = width * 3
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        start = y * stride
        raw.extend(rgb[start : start + stride])
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    compressed = zlib.compress(bytes(raw), level=9)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        PNG_SIGNATURE
        + _png_chunk(b"IHDR", ihdr)
        + _png_chunk(b"IDAT", compressed)
        + _png_chunk(b"IEND", b"")
    )


def create_black_placeholder(path: Path, width: int = PLACEHOLDER_SIZE[0], height: int = PLACEHOLDER_SIZE[1]) -> None:
    write_png_rgb(path, width, height, bytes(width * height * 3))


def read_png_dimensions(path: Path) -> tuple[int, int]:
    header = path.read_bytes()[:24]
    if len(header) < 24 or header[:8] != PNG_SIGNATURE or header[12:16] != b"IHDR":
        raise ValueError(f"not a valid PNG file: {path}")
    return struct.unpack(">II", header[16:24])


def _paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def read_png_rgba(path: Path) -> tuple[int, int, bytearray]:
    blob = path.read_bytes()
    if len(blob) < 24 or blob[:8] != PNG_SIGNATURE:
        raise ValueError(f"not a valid PNG file: {path}")
    pos = 8
    width = height = bit_depth = color_type = interlace = 0
    idat = bytearray()
    palette: bytes = b""
    trns: bytes = b""
    while pos + 8 <= len(blob):
        length, ctype = struct.unpack(">I4s", blob[pos : pos + 8])
        pos += 8
        data = blob[pos : pos + length]
        pos += length + 4
        if ctype == b"IHDR":
            width, height, bit_depth, color_type, _comp, _filt, interlace = struct.unpack(">IIBBBBB", data)
        elif ctype == b"PLTE":
            palette = data
        elif ctype == b"tRNS":
            trns = data
        elif ctype == b"IDAT":
            idat.extend(data)
        elif ctype == b"IEND":
            break

    if bit_depth != 8 or interlace != 0:
        raise ValueError(f"unsupported PNG format (bit_depth={bit_depth}, interlace={interlace}): {path}")
    bpp_map = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
    if color_type not in bpp_map:
        raise ValueError(f"unsupported PNG color_type={color_type}: {path}")
    bpp = bpp_map[color_type]
    stride = width * bpp
    raw = zlib.decompress(bytes(idat))
    if len(raw) < height * (stride + 1):
        raise ValueError(f"truncated PNG image data: {path}")

    prev = bytearray(stride)
    out = bytearray(width * height * 4)
    in_pos = 0
    for y in range(height):
        filt = raw[in_pos]
        in_pos += 1
        row = bytearray(raw[in_pos : in_pos + stride])
        in_pos += stride
        if filt == 1:
            for i in range(bpp, stride):
                row[i] = (row[i] + row[i - bpp]) & 0xFF
        elif filt == 2:
            for i in range(stride):
                row[i] = (row[i] + prev[i]) & 0xFF
        elif filt == 3:
            for i in range(stride):
                left = row[i - bpp] if i >= bpp else 0
                row[i] = (row[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif filt == 4:
            for i in range(stride):
                left = row[i - bpp] if i >= bpp else 0
                up = prev[i]
                up_left = prev[i - bpp] if i >= bpp else 0
                row[i] = (row[i] + _paeth(left, up, up_left)) & 0xFF
        elif filt != 0:
            raise ValueError(f"unsupported PNG filter={filt}: {path}")
        prev = row

        dst_base = y * width * 4
        if color_type == 6:
            out[dst_base : dst_base + width * 4] = row
        elif color_type == 2:
            for x in range(width):
                s = x * 3
                d = dst_base + x * 4
                out[d] = row[s]
                out[d + 1] = row[s + 1]
                out[d + 2] = row[s + 2]
                out[d + 3] = 255
        elif color_type == 0:
            for x in range(width):
                v = row[x]
                d = dst_base + x * 4
                out[d] = v
                out[d + 1] = v
                out[d + 2] = v
                out[d + 3] = 255
        elif color_type == 4:
            for x in range(width):
                v = row[x * 2]
                a = row[x * 2 + 1]
                d = dst_base + x * 4
                out[d] = v
                out[d + 1] = v
                out[d + 2] = v
                out[d + 3] = a
        elif color_type == 3:
            for x in range(width):
                idx = row[x]
                d = dst_base + x * 4
                p = idx * 3
                out[d] = palette[p] if p + 2 < len(palette) else 0
                out[d + 1] = palette[p + 1] if p + 2 < len(palette) else 0
                out[d + 2] = palette[p + 2] if p + 2 < len(palette) else 0
                out[d + 3] = trns[idx] if idx < len(trns) else 255
    return width, height, out


def _content_bbox_rgba(width: int, height: int, rgba: bytes | bytearray) -> tuple[int, int, int, int] | None:
    min_x = width
    min_y = height
    max_x = -1
    max_y = -1
    for y in range(height):
        row_off = y * width * 4
        for x in range(width):
            off = row_off + x * 4
            a = rgba[off + 3]
            if a == 0:
                continue
            if rgba[off] > BLACK_THRESHOLD or rgba[off + 1] > BLACK_THRESHOLD or rgba[off + 2] > BLACK_THRESHOLD:
                if x < min_x:
                    min_x = x
                if y < min_y:
                    min_y = y
                if x > max_x:
                    max_x = x
                if y > max_y:
                    max_y = y
    if max_x < min_x or max_y < min_y:
        return None
    return (min_x, min_y, max_x + 1, max_y + 1)


def _crop_rgba(
    width: int,
    rgba: bytes | bytearray,
    box: tuple[int, int, int, int],
) -> tuple[int, int, bytearray]:
    x0, y0, x1, y1 = box
    cw = x1 - x0
    ch = y1 - y0
    out = bytearray(cw * ch * 4)
    for y in range(ch):
        src_start = ((y0 + y) * width + x0) * 4
        dst_start = y * cw * 4
        out[dst_start : dst_start + cw * 4] = rgba[src_start : src_start + cw * 4]
    return cw, ch, out


def _resize_rgba_bilinear(src: bytes | bytearray, sw: int, sh: int, dw: int, dh: int) -> bytearray:
    out = bytearray(dw * dh * 4)
    scale_x = sw / dw
    scale_y = sh / dh
    for dy in range(dh):
        sy = (dy + 0.5) * scale_y - 0.5
        y0 = max(0, min(sh - 1, int(sy)))
        y1 = min(sh - 1, y0 + 1)
        fy = max(0.0, min(1.0, sy - y0))
        inv_fy = 1.0 - fy
        for dx in range(dw):
            sx = (dx + 0.5) * scale_x - 0.5
            x0 = max(0, min(sw - 1, int(sx)))
            x1 = min(sw - 1, x0 + 1)
            fx = max(0.0, min(1.0, sx - x0))
            inv_fx = 1.0 - fx
            w00 = inv_fx * inv_fy
            w10 = fx * inv_fy
            w01 = inv_fx * fy
            w11 = fx * fy
            p00 = (y0 * sw + x0) * 4
            p10 = (y0 * sw + x1) * 4
            p01 = (y1 * sw + x0) * 4
            p11 = (y1 * sw + x1) * 4
            dst = (dy * dw + dx) * 4
            for c in range(4):
                val = (
                    src[p00 + c] * w00
                    + src[p10 + c] * w10
                    + src[p01 + c] * w01
                    + src[p11 + c] * w11
                )
                out[dst + c] = max(0, min(255, int(round(val))))
    return out


def _fit_dimensions(src_w: int, src_h: int, max_size: int) -> tuple[int, int]:
    scale = min(max_size / src_w, max_size / src_h)
    return max(1, int(round(src_w * scale))), max(1, int(round(src_h * scale)))


def _blit_rgba(
    canvas: bytearray,
    canvas_w: int,
    src: bytes | bytearray,
    src_w: int,
    src_h: int,
    ox: int,
    oy: int,
    opaque_bg: bool,
) -> None:
    for y in range(src_h):
        for x in range(src_w):
            s = (y * src_w + x) * 4
            d = ((oy + y) * canvas_w + (ox + x)) * 4
            sa = src[s + 3]
            if opaque_bg:
                if sa == 255:
                    canvas[d] = src[s]
                    canvas[d + 1] = src[s + 1]
                    canvas[d + 2] = src[s + 2]
                    canvas[d + 3] = 255
                elif sa > 0:
                    inv = 255 - sa
                    canvas[d] = (src[s] * sa + canvas[d] * inv) // 255
                    canvas[d + 1] = (src[s + 1] * sa + canvas[d + 1] * inv) // 255
                    canvas[d + 2] = (src[s + 2] * sa + canvas[d + 2] * inv) // 255
                    canvas[d + 3] = 255
            else:
                canvas[d : d + 4] = src[s : s + 4]


def _sync_with_pillow(source: Path, out_dir: Path) -> list[Path]:
    assert _PilImage is not None
    resample = getattr(_PilImage, "Resampling", _PilImage).LANCZOS
    with _PilImage.open(source) as raw_img:
        img = raw_img.convert("RGBA")

    w, h = img.size
    rgba = img.tobytes()
    bbox = _content_bbox_rgba(w, h, rgba)
    content = img.crop(bbox) if bbox is not None else None

    main_path = out_dir / "android_icon_192.png"
    main_canvas = _PilImage.new("RGBA", (MAIN_ICON_SIZE, MAIN_ICON_SIZE), (0, 0, 0, 255))
    if content is not None:
        fw, fh = _fit_dimensions(content.width, content.height, MAIN_ICON_SIZE)
        resized = content.resize((fw, fh), resample)
        main_canvas.alpha_composite(resized, ((MAIN_ICON_SIZE - fw) // 2, (MAIN_ICON_SIZE - fh) // 2))
    main_canvas.convert("RGB").save(main_path, format="PNG", optimize=True)

    fg_path = out_dir / "android_adaptive_fg_432.png"
    fg_canvas = _PilImage.new("RGBA", (ADAPTIVE_SIZE, ADAPTIVE_SIZE), (0, 0, 0, 0))
    if content is not None:
        fw, fh = _fit_dimensions(content.width, content.height, ADAPTIVE_SAFE_SIZE)
        resized = content.resize((fw, fh), resample)
        fg_canvas.paste(resized, ((ADAPTIVE_SIZE - fw) // 2, (ADAPTIVE_SIZE - fh) // 2), resized)
    fg_canvas.save(fg_path, format="PNG", optimize=True)

    bg_path = out_dir / "android_adaptive_bg_432.png"
    bg_canvas = _PilImage.new("RGB", (ADAPTIVE_SIZE, ADAPTIVE_SIZE), (0, 0, 0))
    bg_canvas.save(bg_path, format="PNG", optimize=True)

    return [main_path, fg_path, bg_path]


def _sync_with_stdlib(source: Path, out_dir: Path) -> list[Path]:
    w, h, rgba = read_png_rgba(source)
    bbox = _content_bbox_rgba(w, h, rgba)
    content = _crop_rgba(w, rgba, bbox) if bbox is not None else None

    main_path = out_dir / "android_icon_192.png"
    main_canvas = bytearray(MAIN_ICON_SIZE * MAIN_ICON_SIZE * 4)
    for i in range(3, len(main_canvas), 4):
        main_canvas[i] = 255
    if content is not None:
        cw, ch, crgba = content
        fw, fh = _fit_dimensions(cw, ch, MAIN_ICON_SIZE)
        resized = _resize_rgba_bilinear(crgba, cw, ch, fw, fh)
        _blit_rgba(
            main_canvas,
            MAIN_ICON_SIZE,
            resized,
            fw,
            fh,
            (MAIN_ICON_SIZE - fw) // 2,
            (MAIN_ICON_SIZE - fh) // 2,
            opaque_bg=True,
        )
    main_rgb = bytearray(MAIN_ICON_SIZE * MAIN_ICON_SIZE * 3)
    for i in range(MAIN_ICON_SIZE * MAIN_ICON_SIZE):
        main_rgb[i * 3 : i * 3 + 3] = main_canvas[i * 4 : i * 4 + 3]
    write_png_rgb(main_path, MAIN_ICON_SIZE, MAIN_ICON_SIZE, main_rgb)

    fg_path = out_dir / "android_adaptive_fg_432.png"
    fg_canvas = bytearray(ADAPTIVE_SIZE * ADAPTIVE_SIZE * 4)
    if content is not None:
        cw, ch, crgba = content
        fw, fh = _fit_dimensions(cw, ch, ADAPTIVE_SAFE_SIZE)
        resized = _resize_rgba_bilinear(crgba, cw, ch, fw, fh)
        _blit_rgba(
            fg_canvas,
            ADAPTIVE_SIZE,
            resized,
            fw,
            fh,
            (ADAPTIVE_SIZE - fw) // 2,
            (ADAPTIVE_SIZE - fh) // 2,
            opaque_bg=False,
        )
    write_png_rgba(fg_path, ADAPTIVE_SIZE, ADAPTIVE_SIZE, fg_canvas)

    bg_path = out_dir / "android_adaptive_bg_432.png"
    write_png_rgb(bg_path, ADAPTIVE_SIZE, ADAPTIVE_SIZE, bytes(ADAPTIVE_SIZE * ADAPTIVE_SIZE * 3))

    return [main_path, fg_path, bg_path]


def sync_boot_assets(source: Path, out_dir: Path | None = None, use_pillow: bool = True) -> list[Path]:
    src = source.resolve()
    if not src.is_file():
        raise FileNotFoundError(f"boot splash source not found: {src}")
    target_dir = (out_dir or src.parent).resolve()
    target_dir.mkdir(parents=True, exist_ok=True)
    if use_pillow and _PilImage is not None:
        return _sync_with_pillow(src, target_dir)
    return _sync_with_stdlib(src, target_dir)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_SOURCE,
        help="Path to splash_static.png (default: godot/assets/boot/splash_static.png)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Output directory for derived Android icons (default: source directory)",
    )
    parser.add_argument(
        "--init-placeholder",
        action="store_true",
        help="Create a pure-black 1024x576 placeholder at --source if missing",
    )
    parser.add_argument(
        "--no-pillow",
        action="store_true",
        help="Force pure-Python stdlib PNG fallback instead of Pillow",
    )
    args = parser.parse_args(argv)

    if args.init_placeholder and not args.source.is_file():
        create_black_placeholder(args.source)
    outputs = sync_boot_assets(args.source, out_dir=args.out_dir, use_pillow=not args.no_pillow)
    print(f"SYNC_BOOT_ASSETS_OK source={args.source} outputs={len(outputs)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
