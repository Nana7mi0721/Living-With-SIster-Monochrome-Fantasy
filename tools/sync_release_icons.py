#!/usr/bin/env python3
"""Derive release icons and installer artwork from local publish masters.

The source images live under the ignored work/ tree. Outputs are build artifacts;
packaging scripts stage them temporarily under godot/assets/boot/generated and
remove that directory after export.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:  # pragma: no cover - exercised only on incomplete toolchains
    Image = None
    ImageDraw = None

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE_DIR = REPO_ROOT / "work/retail_dat/_publish"
DEFAULT_OUT_DIR = REPO_ROOT / "work/out/release_icons"
PC_ICON_SIZES = ((16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256))


def _require_pillow() -> None:
    if Image is None:
        print("ERROR: Pillow is required to derive release icons (pip install pillow)", file=sys.stderr)
        raise SystemExit(2)


def _resampling() -> int:
    return getattr(Image, "Resampling", Image).LANCZOS


def _placeholder(size: tuple[int, int], rgba: tuple[int, int, int, int]) -> Image.Image:
    _require_pillow()
    img = Image.new("RGBA", size, rgba)
    draw = ImageDraw.Draw(img)
    margin = max(1, min(size) // 8)
    draw.rounded_rectangle(
        (margin, margin, size[0] - margin - 1, size[1] - margin - 1),
        radius=max(2, min(size) // 10),
        outline=(210, 214, 224, 255),
        width=max(1, min(size) // 64),
    )
    return img


def _open_source(source_dir: Path, name: str, allow_placeholder: bool) -> tuple[Image.Image, bool]:
    _require_pillow()
    path = source_dir / name
    if path.is_file():
        with Image.open(path) as raw:
            return raw.convert("RGBA"), False
    if not allow_placeholder:
        raise FileNotFoundError(f"publish source image not found: {path}")
    return _placeholder((1024, 1024), (38, 41, 50, 255)), True


def _center_square(img: Image.Image) -> Image.Image:
    side = min(img.width, img.height)
    left = (img.width - side) // 2
    top = (img.height - side) // 2
    return img.crop((left, top, left + side, top + side))


def _save_png(img: Image.Image, path: Path, mode: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.convert(mode).save(path, format="PNG", optimize=True)


def _save_ico(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    _center_square(img).convert("RGBA").save(path, format="ICO", sizes=PC_ICON_SIZES)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sync_release_icons(
    source_dir: Path,
    out_dir: Path,
    allow_placeholder: bool = False,
    stage_res: Path | None = None,
) -> list[Path]:
    _require_pillow()
    source_dir = source_dir.resolve()
    out_dir = out_dir.resolve()
    res_dir = out_dir / "res"
    installer_dir = out_dir / "installer"
    res_dir.mkdir(parents=True, exist_ok=True)
    installer_dir.mkdir(parents=True, exist_ok=True)

    mobile, mobile_placeholder = _open_source(source_dir, "icon_mobile.jpg", allow_placeholder)
    pc, pc_placeholder = _open_source(source_dir, "icon_pc.png", allow_placeholder)
    dockbar, dockbar_placeholder = _open_source(source_dir, "icon_pc_dockbar.png", allow_placeholder)
    corner, corner_placeholder = _open_source(source_dir, "icon_pc_corner.png", allow_placeholder)
    poster_path = source_dir / "setup_poster.png"
    poster_placeholder = not poster_path.is_file()
    if poster_placeholder and not allow_placeholder:
        raise FileNotFoundError(f"publish source image not found: {poster_path}")

    mobile_square = _center_square(mobile)
    outputs: list[Path] = []

    android_main = res_dir / "android_icon_192.png"
    mobile_square.resize((192, 192), _resampling()).convert("RGB").save(android_main, format="PNG", optimize=True)
    outputs.append(android_main)

    android_fg = res_dir / "android_adaptive_fg_432.png"
    fg_canvas = Image.new("RGBA", (432, 432), (0, 0, 0, 0))
    safe_side = int(round(432 * 0.62))
    safe_fg = mobile_square.convert("RGBA").resize((safe_side, safe_side), _resampling())
    offset = (432 - safe_side) // 2
    fg_canvas.paste(safe_fg, (offset, offset), safe_fg)
    fg_canvas.save(android_fg, format="PNG", optimize=True)
    outputs.append(android_fg)

    android_bg = res_dir / "android_adaptive_bg_432.png"
    _save_png(Image.new("RGBA", (432, 432), (0, 0, 0, 255)), android_bg, "RGB")
    outputs.append(android_bg)

    ios_icon = res_dir / "ios_icon_1024.png"
    _save_png(mobile_square.resize((1024, 1024), _resampling()), ios_icon, "RGB")
    outputs.append(ios_icon)

    pc_icon = res_dir / "icon_pc.ico"
    _save_ico(pc, pc_icon)
    outputs.append(pc_icon)

    dockbar_icon = res_dir / "icon_pc_dockbar.ico"
    _save_ico(dockbar, dockbar_icon)
    outputs.append(dockbar_icon)

    corner_icon = res_dir / "icon_pc_corner.ico"
    _save_ico(corner, corner_icon)
    outputs.append(corner_icon)

    for source in (dockbar_icon, corner_icon):
        raw_copy = source.with_suffix(".bin")
        shutil.copy2(source, raw_copy)
        outputs.append(raw_copy)

    setup_icon = installer_dir / "setup_icon.ico"
    shutil.copy2(pc_icon, setup_icon)
    outputs.append(setup_icon)

    setup_poster = installer_dir / "setup_poster.png"
    if not poster_placeholder:
        shutil.copy2(poster_path, setup_poster)
    else:
        _save_png(_placeholder((164, 314), (38, 41, 50, 255)), setup_poster, "RGBA")
    outputs.append(setup_poster)

    placeholder_used = any((mobile_placeholder, pc_placeholder, dockbar_placeholder, corner_placeholder, poster_placeholder))
    manifest = {
        "placeholder": placeholder_used,
        "sources": {
            "icon_mobile": mobile_placeholder,
            "icon_pc": pc_placeholder,
            "icon_pc_dockbar": dockbar_placeholder,
            "icon_pc_corner": corner_placeholder,
            "setup_poster": poster_placeholder,
        },
        "files": {path.relative_to(out_dir).as_posix(): _sha256(path) for path in outputs},
    }
    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    if stage_res is not None:
        stage_dir = stage_res.resolve()
        stage_dir.mkdir(parents=True, exist_ok=True)
        for path in res_dir.iterdir():
            if path.is_file():
                shutil.copy2(path, stage_dir / path.name)

    return outputs


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-dir",
        type=Path,
        default=Path(os.environ.get("LWSMF_PUBLISH_DIR", DEFAULT_SOURCE_DIR)),
        help="Directory containing icon_mobile.jpg, icon_pc.png, icon_pc_dockbar.png, "
        "icon_pc_corner.png and setup_poster.png",
    )
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR)
    parser.add_argument("--stage-res", type=Path, default=None, help="Temporary res:// staging directory")
    parser.add_argument("--allow-placeholder", action="store_true", help="Generate neutral placeholders for missing sources")
    args = parser.parse_args(argv)

    try:
        outputs = sync_release_icons(
            source_dir=args.source_dir,
            out_dir=args.out_dir,
            allow_placeholder=args.allow_placeholder,
            stage_res=args.stage_res,
        )
    except FileNotFoundError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 3

    manifest = json.loads((args.out_dir / "manifest.json").read_text(encoding="utf-8"))
    marker = "PLACEHOLDER" if manifest["placeholder"] else "OK"
    print(
        f"RELEASE_ICONS_{marker} outputs={len(outputs)} source={args.source_dir} out={args.out_dir}"
        + (f" stage={args.stage_res}" if args.stage_res else "")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
