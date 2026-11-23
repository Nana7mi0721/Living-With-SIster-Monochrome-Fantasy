#!/usr/bin/env python3
"""Shared Brotli compression/decompression helper for LWSMF game_data/*.json.

Tries Python `brotli` / `brotlicffi`, then the `brotli` CLI, then `dotnet fsi`
with .NET's built-in `System.IO.Compression.BrotliStream` so no third-party
dependency is required in .NET-enabled build environments.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

BROTLI_JSON_NAMES = ("programs.json", "database.json", "map_tiles.json")

_DOTNET_FSI_SCRIPT = r"""
open System
open System.IO
open System.IO.Compression

let args = Environment.GetCommandLineArgs()
let mode = args.[args.Length - 4]
let inPath = args.[args.Length - 3]
let outPath = args.[args.Length - 2]
let quality = int args.[args.Length - 1]

if mode = "c" then
    let level =
        if quality >= 11 then CompressionLevel.SmallestSize
        elif quality >= 4 then CompressionLevel.Optimal
        else CompressionLevel.Fastest
    use src = File.OpenRead(inPath)
    use dst = File.Create(outPath)
    use br = new BrotliStream(dst, level)
    src.CopyTo(br)
else
    use src = File.OpenRead(inPath)
    use br = new BrotliStream(src, CompressionMode.Decompress)
    use dst = File.Create(outPath)
    br.CopyTo(dst)
"""


def _run_dotnet_fsi(mode: str, data: bytes, quality: int = 11) -> bytes:
    dotnet = shutil.which("dotnet")
    if not dotnet:
        raise RuntimeError("No Brotli provider found (install `brotli` Python package, `brotli` CLI, or `dotnet` SDK).")
    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        fsx = tmp_dir / "br.fsx"
        in_file = tmp_dir / "in.bin"
        out_file = tmp_dir / "out.bin"
        fsx.write_text(_DOTNET_FSI_SCRIPT, encoding="utf-8")
        in_file.write_bytes(data)
        subprocess.run(
            [dotnet, "fsi", "--nologo", str(fsx), mode, str(in_file), str(out_file), str(quality)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        return out_file.read_bytes()


def compress_bytes(data: bytes, quality: int = 11) -> bytes:
    try:
        import brotli  # type: ignore[import-not-found]

        return brotli.compress(data, quality=quality)
    except ImportError:
        pass
    try:
        import brotlicffi  # type: ignore[import-not-found]

        return brotlicffi.compress(data, quality=quality)
    except ImportError:
        pass
    cli = shutil.which("brotli")
    if cli:
        proc = subprocess.run([cli, f"--quality={quality}", "-c"], input=data, capture_output=True, check=True)
        return proc.stdout
    return _run_dotnet_fsi("c", data, quality=quality)


def decompress_bytes(data: bytes) -> bytes:
    try:
        import brotli  # type: ignore[import-not-found]

        return brotli.decompress(data)
    except ImportError:
        pass
    try:
        import brotlicffi  # type: ignore[import-not-found]

        return brotlicffi.decompress(data)
    except ImportError:
        pass
    cli = shutil.which("brotli")
    if cli:
        proc = subprocess.run([cli, "-d", "-c"], input=data, capture_output=True, check=True)
        return proc.stdout
    return _run_dotnet_fsi("d", data)


def compact_json_bytes(src: Path) -> bytes:
    data = json.loads(src.read_text(encoding="utf-8"))
    return json.dumps(data, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def stage_game_data_dir(src_dir: Path, dst_dir: Path, use_brotli: bool = False, quality: int = 11) -> int:
    dst_dir.mkdir(parents=True, exist_ok=True)
    written = 0
    for f in sorted(src_dir.glob("*.json")):
        if f.name in BROTLI_JSON_NAMES:
            raw = compact_json_bytes(f)
            if use_brotli:
                (dst_dir / f"{f.name}.br").write_bytes(compress_bytes(raw, quality=quality))
            else:
                (dst_dir / f.name).write_bytes(raw)
        else:
            (dst_dir / f.name).write_bytes(f.read_bytes())
        written += 1
    if written == 0:
        raise SystemExit(f"no json files found in {src_dir}")
    return written


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src", type=Path, required=True, help="source game_data directory")
    parser.add_argument("--dst", type=Path, required=True, help="destination game_data directory")
    parser.add_argument("--brotli", action="store_true", help="emit .json.br for large game_data JSONs")
    parser.add_argument("--quality", type=int, default=11, help="Brotli quality (0-11, default 11)")
    args = parser.parse_args(argv)
    count = stage_game_data_dir(args.src, args.dst, use_brotli=args.brotli, quality=args.quality)
    print(f"GAME_DATA_STAGED count={count} brotli={args.brotli} dst={args.dst}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
