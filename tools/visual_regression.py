#!/usr/bin/env python3
"""Render core retail-save and presentation scenes in OpenGL3 into a contact sheet.

Requires Godot binary (resolved via GODOT / tools/local.env.sh / PATH) and PIL/Pillow.
Generates individual PNG captures and visual_contact_sheet.png under work/out/visual/
(or --out-dir), using temporary isolated LWsmf_SAVE_DIR so user saves stay untouched.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

GD_SCRIPT = """extends SceneTree

func _init() -> void:
\tcall_deferred("_run")

func _run() -> void:
\tvar out_dir := OS.get_environment("LWsmf_VISUAL_OUT")
\tvar root_dir := OS.get_environment("LWsmf_REPO_ROOT")
\tvar vp := SubViewport.new()
\tvp.size = Vector2i(1024, 576)
\tvp.render_target_update_mode = SubViewport.UPDATE_ALWAYS
\troot.add_child(vp)

\tvar game = load("res://presentation_gd/GameRoot.gd").new()
\tvp.add_child(game)

\tfor _i in range(1500):
\t\tgame._process(game.STEP)
\t\tif game.choice != null and game.choice.is_open:
\t\t\tbreak
\t\tif game.message != null and game.message.is_open():
\t\t\tgame.message.confirm()
\t\t\tgame._process(game.STEP)

\tawait process_frame
\tawait process_frame
\t_capture(vp, out_dir, "01_title_menu.png")

\t# Scenario 2: Live Nameplate + Message + Choice on Title
\tgame._on_message_options_changed(0, 0, 0, "妹妹", "")
\tgame._on_message("早上好！今天的早餐已经做好了，要现在吃吗？", 0)
\tgame._sync_active_nameplate()
\tif game.message != null:
\t\tgame.message.revealed = 1e9
\t\tgame.message.finished = true
\t\tgame.message.waiting_for_confirm = true
\tawait process_frame
\tawait process_frame
\t_capture(vp, out_dir, "02_dialog_nameplate.png")
\tgame._reset_presentation_transient_state()

\t# Scenario 3: Retail SaveData01.sav Evening Bedroom (HUD + Sprite + Icons)
\tvar sav1 := root_dir.path_join("work/all/save/SaveData01.sav")
\tif not FileAccess.file_exists(sav1):
\t\tsav1 = root_dir.path_join("work/retail_backup/Save/SaveData01.sav")
\tif FileAccess.file_exists(sav1):
\t\tgame.session.LoadRetailSave(sav1, 0)
\t\tfor _i in range(80):
\t\t\tgame._process(game.STEP)
\t\t\tif game.message != null and game.message.is_open():
\t\t\t\tgame.message.confirm()
\t\t\t\tgame._process(game.STEP)
\t\tawait process_frame
\t\tawait process_frame
\t\t_capture(vp, out_dir, "03_retail_save01_bedroom.png")

\t\t# Scenario 4: Save/Load Screen (CE94 + SaveData01 thumbnail / clean empty cards)
\t\tgame.vm.StartEventAt(94, 0)
\t\tfor _i in range(60):
\t\t\tgame._process(game.STEP)
\t\t\tif game.stage.picture_rect(12050).size.x > 0.0:
\t\t\t\tfor _j in range(25):
\t\t\t\t\tgame._process(game.STEP)
\t\t\t\tbreak
\t\tawait process_frame
\t\tawait process_frame
\t\t_capture(vp, out_dir, "04_save_load_ce94.png")

\t# Scenario 5: Retail SaveData08.sav Late-Game State
\tvar sav8 := root_dir.path_join("work/retail_backup/Save/SaveData08.sav")
\tif FileAccess.file_exists(sav8):
\t\tgame.session.LoadRetailSave(sav8, 0)
\t\tfor _i in range(40):
\t\t\tgame._process(game.STEP)
\t\tawait process_frame
\t\tawait process_frame
\t\t_capture(vp, out_dir, "05_retail_save08_late.png")

\tprint("VISUAL_REGRESSION_CAPTURE_OK")
\tquit(0)

func _capture(vp: SubViewport, out_dir: String, name: String) -> void:
\tvar img := vp.get_texture().get_image()
\tif img != null:
\t\timg.save_png(out_dir.path_join(name))
"""


def find_godot(repo_root: Path) -> str:
    for var in ("GODOT", "GODOT_BIN"):
        val = os.environ.get(var, "")
        if val and Path(val).exists():
            return val
    local_env = repo_root / "tools" / "local.env.sh"
    if local_env.is_file():
        for line in local_env.read_text(encoding="utf-8", errors="ignore").splitlines():
            if line.strip().startswith("export GODOT=") or line.strip().startswith("GODOT="):
                raw = line.split("=", 1)[1].strip().strip('"').strip("'")
                if Path(raw).exists():
                    return raw
    for name in ("godot", "Godot_v4.7.2-stable_mono_win64_console.exe"):
        found = shutil.which(name)
        if found:
            return found
    raise FileNotFoundError("Godot executable not found; set GODOT env var or tools/local.env.sh")


def build_contact_sheet(images: list[tuple[str, Path]], dest: Path) -> None:
    thumb_w, thumb_h = 512, 288
    pad = 16
    cols = 2
    rows = max(1, (len(images) + cols - 1) // cols)
    sheet = Image.new("RGB", (cols * (thumb_w + pad) + pad, rows * (thumb_h + 28 + pad) + pad), (28, 28, 32))
    draw = ImageDraw.Draw(sheet)
    for idx, (label, path) in enumerate(images):
        r, c = divmod(idx, cols)
        x = pad + c * (thumb_w + pad)
        y = pad + r * (thumb_h + 28 + pad)
        im = Image.open(path).convert("RGB").resize((thumb_w, thumb_h), Image.Resampling.LANCZOS)
        sheet.paste(im, (x, y))
        draw.text((x + 4, y + thumb_h + 4), label, fill=(235, 235, 240))
    sheet.save(dest)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Capture OpenGL3 retail-save screenshots into a contact sheet")
    parser.add_argument("--out-dir", default="work/out/visual", help="output directory for screenshots and contact sheet")
    args = parser.parse_args(argv)

    repo_root = Path(__file__).resolve().parent.parent
    out_dir = (repo_root / args.out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    godot = find_godot(repo_root)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        script_path = tmp_path / "capture_sheet.gd"
        script_path.write_text(GD_SCRIPT, encoding="utf-8")
        save_dir = tmp_path / "save"
        save_dir.mkdir()
        env = os.environ.copy()
        env["LWsmf_VISUAL_OUT"] = str(out_dir)
        env["LWsmf_REPO_ROOT"] = str(repo_root)
        env["LWsmf_SAVE_DIR"] = str(save_dir)
        env["LWsmf_QUIET_HUD"] = "1"
        proc = subprocess.run(
            [godot, "--path", str(repo_root / "godot"), "--rendering-driver", "opengl3", "-s", str(script_path)],
            env=env,
            capture_output=True,
            text=True,
            timeout=90,
            check=False,
        )
        if proc.returncode != 0 or "VISUAL_REGRESSION_CAPTURE_OK" not in proc.stdout:
            print(proc.stdout)
            print(proc.stderr, file=sys.stderr)
            return 1

    captured = sorted((p.stem, p) for p in out_dir.glob("*.png") if p.name != "visual_contact_sheet.png")
    if not captured:
        print("No screenshots captured", file=sys.stderr)
        return 1
    sheet_path = out_dir / "visual_contact_sheet.png"
    build_contact_sheet(captured, sheet_path)
    print(f"VISUAL_CONTACT_SHEET_OK count={len(captured)} sheet={sheet_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
