#!/usr/bin/env python3
"""2x super-resolution pipeline for the live LWSMF art set.

Source of truth is ``work/media_manifest.txt`` (the 2,689 live retail entries)
resolved against ``work/retail_dat``. Images are upscaled 2x with the
Real-CUGAN ncnn Vulkan binary that ships inside Waifu2x-Extension-GUI, encoded
losslessly, and indexed in ``hd_manifest.json`` for the runtime.

Design notes:
  * 2x is the engine's native ratio, so the output needs no resampling.
  * The GUI's batch defaults (lossy encode, flattened folders, overwrite) are
    deliberately not used; this script drives the CLI directly.
  * Oversized art is copied through untouched (no manifest entry) so the
    runtime falls back to the 1x pak instead of failing to create a texture.
  * Alpha survives Real-CUGAN, but is asserted and repaired from the source if
    an engine build ever drops it.
  * This build's ``-f webp`` writer emits bit-valid files with corrupted pixel
    rows (rainbow noise). Always request PNG and encode lossless WebP in Pillow.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

import numpy as np
from pathlib import Path

from PIL import Image, ImageChops, ImageStat

SCALE = 2
TEXTURE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp", ".webp"}
MAX_LONG_SIDE = 8192          # 2x long side must stay under the GL texture limit
MAX_SOURCE_PIXELS = 4 * 1024 * 1024  # 2x area <= 16 MP => <= 64 MB RGBA8
# Real-CUGAN's Vulkan path drops the device when an input side is smaller than
# 64px (18x18 window skins, 606x35 bars). Those are flat graphics where Lanczos
# is both safer and visually exact, so they never reach the GPU.
ENGINE_MIN_SIDE = 64
WEBP_METHOD = 4          # method 6 costs ~5x the time for ~0.3% smaller files

DEFAULT_ENGINE_DIR = Path(
    r"D:\Program\Waifu2x-Extension-GUI\waifu2x-extension-gui\realcugan-ncnn-vulkan"
)

# Directory -> denoise level. -1 keeps source lines, 0 is conservative, 1..3
# trade detail for noise removal (the JPG backgrounds carry compression noise).
UI_DIRS = {
    "systemfile", "systemgraphic", "ficon", "kagura", "hword", "charachip",
    "battleeffect", "cutin", "fog_background",
}
PICTURE_DIRS = {"picture", "enemygraphic"}
ART_DIRS = {"cg", "hscene", "1.4.0", "dlc_data"}
SKIP_DIRS = {"mapchip"}


def denoise_for(rel: Path) -> int:
    top = rel.parts[0].lower()
    if top in PICTURE_DIRS:
        return -1
    if top in UI_DIRS:
        return 0
    # JPG backgrounds used to run at n=1, but that erases 1px linework (visible
    # smear on thin outlines); conservative denoise keeps compression noise low
    # without flattening the art.
    return 0


def group_key(rel: Path) -> str:
    return f"n{denoise_for(rel)}"


def read_manifest(path: Path) -> list[str]:
    entries: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        entries.append(line.replace("\\", "/"))
    return entries


def guard_reason(size: tuple[int, int]) -> str:
    w, h = size
    if max(w, h) * SCALE > MAX_LONG_SIDE:
        return f"2x long side {max(w, h) * SCALE} > {MAX_LONG_SIDE}"
    if w * h > MAX_SOURCE_PIXELS:
        return f"{w}x{h} exceeds {MAX_SOURCE_PIXELS} source pixels"
    return ""


def encode_best(image: Image.Image, out_dir: Path, stem: str) -> tuple[Path, int]:
    """Write a WebP-lossless candidate, plus PNG for small art where it may win."""
    out_dir.mkdir(parents=True, exist_ok=True)
    candidates: list[tuple[Path, int]] = []

    webp_path = out_dir / f"{stem}.webp"
    rgba = image.convert("RGBA") if "A" in image.getbands() else image.convert("RGB")
    rgba.save(webp_path, "WEBP", lossless=True, quality=100, method=WEBP_METHOD, exact=True)
    candidates.append((webp_path, webp_path.stat().st_size))

    if image.width * image.height <= 1_000_000:
        png_path = out_dir / f"{stem}.png"
        image.save(png_path, "PNG", optimize=True)
        candidates.append((png_path, png_path.stat().st_size))

    best = min(candidates, key=lambda pair: pair[1])
    for candidate, _ in candidates:
        if candidate != best[0]:
            candidate.unlink(missing_ok=True)
    return best


def repair_alpha(source: Image.Image, output: Image.Image) -> Image.Image:
    """Restore the source alpha channel scaled 2x when the engine dropped it."""
    if "A" not in source.getbands():
        return output
    if "A" in output.getbands():
        return output
    alpha = source.getchannel("A").resize(
        (source.width * SCALE, source.height * SCALE), Image.Resampling.LANCZOS
    )
    out = output.convert("RGBA")
    out.putalpha(alpha)
    return out


def record_image(
    rel: Path,
    image: Image.Image,
    out_root: Path,
    entries: dict,
    log_file: Path,
    engine_tag: str,
    fallback: bool,
    source_size: tuple[int, int],
) -> int:
    chosen, size_bytes = encode_best(image, out_root / rel.parent, rel.stem)
    out_rel = chosen.relative_to(out_root).as_posix()
    entries[rel.as_posix().lower()] = {
        "path": out_rel,
        "scale": float(SCALE),
        "w": image.width,
        "h": image.height,
        "bytes": size_bytes,
        "engine": engine_tag,
        "fallback": fallback,
    }
    with log_file.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps({
            "path": rel.as_posix(),
            "src": list(source_size),
            "dst": [image.width, image.height],
            "bytes": size_bytes,
            "codec": chosen.suffix,
            "fallback": fallback,
            "engine": engine_tag,
        }, ensure_ascii=False) + "\n")
    return size_bytes


def record_encoded(
    rel: Path,
    produced: Path,
    out_root: Path,
    entries: dict,
    log_file: Path,
    engine_tag: str,
    source_size: tuple[int, int],
    source_has_alpha: bool,
    source_alpha_opaque: bool = False,
) -> int | None:
    """Adopt the engine's own lossless WebP when it is dimensionally sound."""
    expected = (source_size[0] * SCALE, source_size[1] * SCALE)
    try:
        with Image.open(produced) as packed:
            if packed.size != expected:
                return None
            if source_has_alpha and "A" not in packed.getbands() and not source_alpha_opaque:
                return None
    except Exception:  # noqa: BLE001 - a corrupt output is a fallback
        return None
    dest = out_root / rel.parent / (rel.stem + produced.suffix.lower())
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists():
        dest.unlink()
    shutil.move(str(produced), str(dest))
    size_bytes = dest.stat().st_size
    entries[rel.as_posix().lower()] = {
        "path": dest.relative_to(out_root).as_posix(),
        "scale": float(SCALE),
        "w": expected[0],
        "h": expected[1],
        "bytes": size_bytes,
        "engine": engine_tag,
        "fallback": False,
    }
    with log_file.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps({
            "path": rel.as_posix(),
            "src": list(source_size),
            "dst": list(expected),
            "bytes": size_bytes,
            "codec": dest.suffix,
            "fallback": False,
            "engine": engine_tag,
        }, ensure_ascii=False) + "\n")
    return size_bytes


def lanczos_upscale(original: Image.Image) -> Image.Image:
    target = (original.width * SCALE, original.height * SCALE)
    mode = "RGBA" if "A" in original.getbands() else "RGB"
    return original.convert(mode).resize(target, Image.Resampling.LANCZOS)


def content_mae(source: Image.Image, upscaled: Image.Image) -> float:
    """Compare downscaled SR output with the source over a neutral backdrop."""
    size = source.size
    ref = source.convert("RGBA")
    got = upscaled.convert("RGBA").resize(size, Image.Resampling.BOX)
    canvas_ref = Image.new("RGBA", size, (24, 24, 24, 255))
    canvas_got = Image.new("RGBA", size, (24, 24, 24, 255))
    canvas_ref.alpha_composite(ref)
    canvas_got.alpha_composite(got)
    diff = ImageChops.difference(canvas_ref.convert("RGB"), canvas_got.convert("RGB"))
    stat = ImageStat.Stat(diff)
    return sum(stat.mean) / 3.0


def is_gray_source(source: Image.Image) -> bool:
    """True when opaque pixels carry no chroma (LWSMF art is overwhelmingly neutral)."""
    sample = source.convert("RGBA").resize((128, 128), Image.Resampling.BOX)
    arr = np.asarray(sample, dtype=np.int16)
    mask = arr[..., 3] > 64
    if mask.sum() < 10:
        return True
    spread = np.abs(arr[..., :3].max(2) - arr[..., :3].min(2))[mask]
    return float(spread.mean()) < 1.0


def visible_chroma(source: Image.Image, upscaled: Image.Image) -> float:
    """Average channel spread under opaque source pixels; catches rainbow speckles."""
    sample = source.convert("RGBA").resize((128, 128), Image.Resampling.BOX)
    arr = np.asarray(sample, dtype=np.int16)
    mask = arr[..., 3] > 64
    out = np.asarray(upscaled.convert("RGBA").resize((128, 128), Image.Resampling.BOX), dtype=np.int16)
    spread = np.abs(out[..., :3].max(2) - out[..., :3].min(2))[mask]
    return float(spread.mean()) if spread.size else 0.0


def desaturate(image: Image.Image) -> Image.Image:
    """Keep 2x detail but drop colour noise on a neutral source, preserving alpha."""
    src = image.convert("RGBA")
    luma = src.convert("L")
    return Image.merge("RGBA", (luma, luma, luma, src.getchannel("A")))


def run_engine(engine: Path, model_dir: Path, in_dir: Path, out_dir: Path, denoise: int) -> bool:
    if not any(in_dir.iterdir()):
        return True
    command = [
        str(engine),
        "-i", str(in_dir),
        "-o", str(out_dir),
        "-s", str(SCALE),
        "-n", str(denoise),
        "-m", str(model_dir),
        "-f", "png",
        "-j", "1:1:1",
    ]
    try:
        completed = subprocess.run(command, capture_output=True, text=True, timeout=1800)
    except subprocess.TimeoutExpired:
        print("  engine timed out; remaining files fall back to Lanczos")
        return False
    if completed.returncode != 0:
        tail = (completed.stderr or completed.stdout).strip().splitlines()[-4:]
        print(f"  engine exited {completed.returncode}; remaining files fall back to Lanczos")
        if tail:
            print("  " + " | ".join(tail))
        return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="2x upscale the live LWSMF art set")
    parser.add_argument("--root", default="work/retail_dat")
    parser.add_argument("--manifest", default="work/media_manifest.txt")
    parser.add_argument("--out", default="work/hd")
    parser.add_argument("--engine-dir", default=os.environ.get("LWsmf_REALCUGAN", str(DEFAULT_ENGINE_DIR)))
    parser.add_argument("--engine-exe", default="realcugan-ncnn-vulkan_W2xEX.exe")
    parser.add_argument("--models", default="models-pro")
    parser.add_argument("--limit", type=int, default=0, help="only the first N per group (pilot)")
    parser.add_argument("--dirs", default="", help="comma separated top-level dirs to include")
    parser.add_argument("--only-jpg", action="store_true", help="reprocess JPEG sources only, keeping other manifest entries")
    parser.add_argument("--retry-fallback", action="store_true", help="re-run only engine entries that fell back to Lanczos")
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--log", default="work/out/hd_build.jsonl")
    args = parser.parse_args()

    root = Path(args.root)
    out_root = Path(args.out)
    engine_dir = Path(args.engine_dir)
    engine = engine_dir / args.engine_exe
    model_dir = engine_dir / args.models
    if not engine.exists():
        print(f"engine not found: {engine}", file=sys.stderr)
        return 2
    if not model_dir.is_dir():
        print(f"model dir not found: {model_dir}", file=sys.stderr)
        return 2

    wanted_dirs = {part.strip().lower() for part in args.dirs.split(",") if part.strip()}
    manifest_path = Path(args.manifest)
    previous: dict[str, dict] = {}
    manifest_file = out_root / "hd_manifest.json"
    if manifest_file.exists():
        previous = json.loads(manifest_file.read_text(encoding="utf-8")).get("entries", {})

    jobs: list[tuple[Path, str]] = []
    cpu_jobs: list[Path] = []
    skipped: list[dict] = []
    missing: list[str] = []
    for logical in read_manifest(manifest_path):
        rel = Path(logical)
        if rel.suffix.lower() not in TEXTURE_EXTENSIONS:
            continue
        if args.only_jpg and rel.suffix.lower() not in {".jpg", ".jpeg"}:
            continue
        top = rel.parts[0].lower()
        if top in SKIP_DIRS:
            skipped.append({"path": logical, "reason": "unused pixel-art directory"})
            continue
        if wanted_dirs and top not in wanted_dirs:
            continue
        source = root / rel
        if not source.is_file():
            missing.append(logical)
            continue
        try:
            with Image.open(source) as probe:
                size = probe.size
                mode = probe.mode
        except Exception as error:  # noqa: BLE001 - report and continue
            skipped.append({"path": logical, "reason": f"unreadable: {error}"})
            continue
        # Real-CUGAN is unreliable on L/LA masks and SD sprites; Lanczos keeps them clean.
        if mode in {"L", "LA"}:
            cpu_jobs.append(rel)
            continue
        reason = guard_reason(size)
        if reason:
            skipped.append({"path": logical, "reason": reason})
            continue
        short_side = min(size)
        if short_side < ENGINE_MIN_SIDE:
            cpu_jobs.append(rel)
            continue
        jobs.append((rel, group_key(rel)))

    if args.limit:
        grouped_count: dict[str, int] = {}
        limited: list[tuple[Path, str]] = []
        for rel, key in jobs:
            grouped_count[key] = grouped_count.get(key, 0) + 1
            if grouped_count[key] <= args.limit:
                limited.append((rel, key))
        jobs = limited

    retry_set: set[str] = set()
    if args.retry_fallback:
        retry_set = {
            key for key, entry in previous.items()
            if entry.get("fallback") and str(entry.get("engine", "")).startswith("realcugan")
        }
        jobs = [(rel, key) for rel, key in jobs if rel.as_posix().lower() in retry_set]
        print(f"retry fallback: {len(jobs)} engine image(s)")

    print(f"live images selected: {len(jobs)} engine + {len(cpu_jobs)} cpu | skipped by guards: {len(skipped)} | missing: {len(missing)}")
    if missing:
        Path("work/out/hd_missing.txt").write_text("\n".join(missing), encoding="utf-8")
    if args.dry_run:
        for rel, key in jobs[:20]:
            print(f"  {key}  {rel}")
        return 0

    log_file = Path(args.log)
    log_file.parent.mkdir(parents=True, exist_ok=True)
    started = time.time()
    done = 0
    entries = dict(previous)

    def already_current(rel: Path) -> bool:
        if args.force:
            return False
        if args.retry_fallback and rel.as_posix().lower() in retry_set:
            return False
        entry = previous.get(rel.as_posix().lower())
        if not entry:
            return False
        candidate = out_root / str(entry.get("path", ""))
        source = root / rel
        return candidate.is_file() and candidate.stat().st_mtime >= source.stat().st_mtime

    for rel in cpu_jobs:
        if already_current(rel):
            continue
        with Image.open(root / rel) as original:
            image = lanczos_upscale(original)
            record_image(rel, image, out_root, entries, log_file, "lanczos2x", True, original.size)
        done += 1
    if cpu_jobs:
        print(f"[cpu] lanczos2x on {len(cpu_jobs)} thin/small asset(s)")

    stage_root = Path(tempfile.mkdtemp(prefix="hd_stage_", dir=str(out_root.parent)))
    try:
        groups: dict[str, list[Path]] = {}
        for rel, key in jobs:
            groups.setdefault(key, []).append(rel)

        # The fp16 engine silently starts emitting rainbows after ~130 images in one
        # invocation; cap each batch so a long group always gets fresh GPU state.
        CHUNK = max(1, int(os.environ.get("LWsmf_HD_CHUNK", "48")))
        for key, rels in sorted(groups.items()):
            denoise = int(key[1:])
            for offset in range(0, len(rels), CHUNK):
                chunk = rels[offset : offset + CHUNK]
                stage_in = stage_root / f"{key}_{offset:05d}" / "in"
                stage_out = stage_root / f"{key}_{offset:05d}" / "out"
                stage_in.mkdir(parents=True)
                stage_out.mkdir(parents=True)
                index_to_rel: dict[str, Path] = {}
                staged = 0
                for rel in chunk:
                    source = root / rel
                    if already_current(rel):
                        continue
                    stem = f"f{staged:05d}"
                    target = stage_in / (stem + ".png")
                    with Image.open(source) as original:
                        mode = "RGBA" if "A" in original.getbands() else "RGB"
                        pixels = np.array(original.convert(mode), dtype=np.uint8)
                        # 255-white pixels make this fp16 engine emit NaN/rainbow rows; 249 is
                        # already too high for some RGBA CGs, 244 is the empirically safe cap.
                        white_cap = int(os.environ.get("LWsmf_HD_WHITE_CAP", "249"))
                        pixels[..., :3] = np.minimum(pixels[..., :3], white_cap)
                        Image.fromarray(pixels, mode).save(target, format="PNG")
                    index_to_rel[stem] = rel
                    staged += 1
                if staged == 0:
                    print(f"[{key} +{offset}] nothing to do")
                    continue

                print(f"[{key} +{offset}] engine pass on {staged} image(s), denoise={denoise}")
                t0 = time.time()
                engine_ok = run_engine(engine, model_dir, stage_in, stage_out, denoise)
                print(f"[{key} +{offset}] engine done in {time.time() - t0:.1f}s ok={engine_ok}")

                for stem, rel in sorted(index_to_rel.items()):
                    produced = stage_out / f"{stem}.png"
                    if not produced.exists():
                        produced = stage_out / f"{stem}.webp"
                    if not produced.exists():
                        produced = stage_out / f"{stem}{rel.suffix.lower()}"
                    engine_tag = f"realcugan/{args.models}/n{denoise}"
                    with Image.open(root / rel) as original:
                        src_size = original.size
                        src_alpha = "A" in original.getbands()
                        src_opaque = src_alpha and original.convert("RGBA").getchannel("A").getextrema() == (255, 255)
                        src_gray = is_gray_source(original)
                        adopted = None
                        if produced.exists() and produced.suffix.lower() == ".webp":
                            adopted = record_encoded(
                                rel, produced, out_root, entries, log_file, engine_tag,
                                src_size, src_alpha, src_opaque,
                            )
                        if adopted is None:
                            expected = (original.width * SCALE, original.height * SCALE)
                            image = None
                            mae_value = None
                            if produced.exists():
                                try:
                                    with Image.open(produced) as raw:
                                        raw.load()
                                        candidate = repair_alpha(original, raw.copy())
                                        if candidate.size == expected:
                                            mae_value = content_mae(original, candidate)
                                            if mae_value <= 25.0:
                                                image = candidate
                                                if src_gray:
                                                    chroma = visible_chroma(original, candidate)
                                                    if chroma > 15.0:
                                                        image = None
                                                    else:
                                                        image = desaturate(candidate)
                                except Exception:  # noqa: BLE001 - a broken engine output is a fallback
                                    image = None
                            fallback = image is None
                            if fallback:
                                print(f"  fallback {rel.as_posix()} mae={mae_value}")
                            if image is None:
                                image = lanczos_upscale(original)
                            if image.size != expected:
                                image = image.resize(expected, Image.Resampling.LANCZOS)
                            record_image(rel, image, out_root, entries, log_file, engine_tag, fallback, original.size)
                    done += 1
                    if done % 25 == 0:
                        print(f"  encoded {done}/{len(jobs) + len(cpu_jobs)}")
                for stale in stage_in.iterdir():
                    stale.unlink(missing_ok=True)
                for stale in stage_out.iterdir():
                    stale.unlink(missing_ok=True)
    finally:
        if os.environ.get("LWsmf_HD_KEEP_STAGE"):
            print(f"[stage] kept at {stage_root}", flush=True)
        else:
            shutil.rmtree(stage_root, ignore_errors=True)

    payload = {
        "version": 1,
        "scale": SCALE,
        "generated": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "engine": f"realcugan-ncnn-vulkan/{args.models}",
        "entries": dict(sorted(entries.items())),
    }
    for item in skipped:
        payload["entries"].pop(str(item["path"]).lower(), None)
    out_root.mkdir(parents=True, exist_ok=True)
    manifest_file.write_text(json.dumps(payload, ensure_ascii=False, indent=1), encoding="utf-8")
    Path("work/out/hd_skipped.json").write_text(
        json.dumps(skipped, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    total = sum(int(entry.get("bytes", 0)) for entry in entries.values())
    print(f"HD_MANIFEST_OK entries={len(entries)} total={total / (1 << 20):.1f} MiB elapsed={time.time() - started:.0f}s")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
