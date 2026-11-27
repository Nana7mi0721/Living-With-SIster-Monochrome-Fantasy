#!/usr/bin/env python3
"""Fail-fast streaming Godot test runner with syntax pre-check and tree kill."""
from __future__ import annotations

import argparse
import os
import queue
import re
import shlex
import shutil
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

FATAL_LINE_PATTERNS = (
    re.compile(r"\bParse Error:"),
    re.compile(r"\bSCRIPT ERROR:"),
    re.compile(r"\bFailed to load script\b"),
    re.compile(r"\bCompile Error:"),
    re.compile(r"\bInvalid call\."),
    re.compile(r"\bASSERTION FAILED\b"),
    re.compile(r"\bUnhandled [Ee]xception\b"),
)

TSCN_GD_RE = re.compile(r'path="(res://[^"]+\.gd)"')
MSYS_PATH_RE = re.compile(r"^/([a-zA-Z])/(.+)$")


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _normalize_tool_path(raw: str) -> str:
    val = raw.strip().strip('"').strip("'")
    if not val:
        return ""
    if os.name == "nt":
        match = MSYS_PATH_RE.match(val)
        if match and not Path(val).exists():
            drive, rest = match.groups()
            win_path = f"{drive.upper()}:/{rest}"
            if Path(win_path).exists():
                return win_path
    return val


def _read_local_env(repo_root: Path) -> dict[str, str]:
    env_file = repo_root / "tools" / "local.env.sh"
    if not env_file.is_file():
        return {}
    result: dict[str, str] = {}
    for line in env_file.read_text(encoding="utf-8", errors="replace").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.startswith("export "):
            stripped = stripped[len("export ") :].strip()
        if "=" not in stripped:
            continue
        key, raw_val = stripped.split("=", 1)
        result[key.strip()] = _normalize_tool_path(raw_val)
    return result


def _find_godot(repo_root: Path, explicit: str | None) -> str:
    local_env = _read_local_env(repo_root)
    candidates = [
        explicit or "",
        os.environ.get("GODOT", ""),
        os.environ.get("GODOT_BIN", ""),
        local_env.get("GODOT", ""),
        local_env.get("GODOT_BIN", ""),
    ]
    for raw in candidates:
        norm = _normalize_tool_path(raw)
        if norm and Path(norm).exists():
            return norm
    for name in ("godot", "Godot_v4.7.2-stable_mono_win64_console.exe"):
        found = shutil.which(name)
        if found:
            return found
    raise FileNotFoundError(
        "Godot binary not found; set GODOT env var or tools/local.env.sh"
    )


def _child_env(repo_root: Path) -> dict[str, str]:
    env = os.environ.copy()
    local_env = _read_local_env(repo_root)
    dotnet = _normalize_tool_path(env.get("DOTNET") or env.get("DOTNET_BIN") or local_env.get("DOTNET", ""))
    if dotnet and Path(dotnet).exists():
        dotnet_dir = str(Path(dotnet).parent)
        env["DOTNET"] = dotnet
        env.setdefault("DOTNET_ROOT", dotnet_dir)
        env["PATH"] = dotnet_dir + os.pathsep + env.get("PATH", "")
    # Prevent out-of-proc Roslyn compiler servers from holding inherited pipes open.
    env.setdefault("UseSharedCompilation", "false")
    env.setdefault("MSBUILDDISABLENODEREUSE", "1")
    env.setdefault("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
    env.setdefault("DOTNET_NOLOGO", "1")
    env.setdefault("LWsmf_QUIET_HUD", "1")
    return env


def _discover_gd_scripts(project_dir: Path, targets: list[str], extra: list[str]) -> list[str]:
    scripts: list[str] = []
    seen: set[str] = set()
    tokens = targets + extra
    for idx, tok in enumerate(tokens):
        if tok == "--script" and idx + 1 < len(tokens):
            cand = tokens[idx + 1]
            if cand.endswith(".gd") and cand not in seen:
                seen.add(cand)
                scripts.append(cand)
        elif tok.endswith(".gd") and tok not in seen:
            seen.add(tok)
            scripts.append(tok)
        elif tok.endswith(".tscn"):
            rel = tok[len("res://") :] if tok.startswith("res://") else tok
            scene_path = (project_dir / rel).resolve()
            if scene_path.is_file():
                text = scene_path.read_text(encoding="utf-8", errors="replace")
                for match in TSCN_GD_RE.findall(text):
                    if match not in seen:
                        seen.add(match)
                        scripts.append(match)
    return scripts


def _kill_process_tree(proc: subprocess.Popen[str]) -> None:
    if proc.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/F", "/T", "/PID", str(proc.pid)],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    else:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    try:
        proc.wait(timeout=3.0)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2.0)


def match_fatal_line(line: str) -> str | None:
    """Return a formatted fatal reason when line matches FATAL_LINE_PATTERNS."""
    for pat in FATAL_LINE_PATTERNS:
        if pat.search(line):
            return f"fatal error matched ({pat.pattern}): {line}"
    return None


def _stream_run(
    cmd: list[str],
    *,
    env: dict[str, str],
    timeout_sec: float,
    error_grace_sec: float,
    detect_fatal: bool = True,
) -> tuple[int, list[str], str | None]:
    popen_kwargs: dict[str, object] = {}
    if os.name == "nt":
        popen_kwargs["creationflags"] = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
    else:
        popen_kwargs["start_new_session"] = True

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
        env=env,
        **popen_kwargs,
    )
    line_q: queue.Queue[str | None] = queue.Queue()

    def _reader() -> None:
        assert proc.stdout is not None
        try:
            for raw in proc.stdout:
                line_q.put(raw.rstrip("\r\n"))
        finally:
            line_q.put(None)

    thread = threading.Thread(target=_reader, daemon=True)
    thread.start()

    lines: list[str] = []
    start = time.monotonic()
    fatal_reason: str | None = None
    fatal_at: float | None = None
    timed_out = False
    eof = False

    while not eof:
        now = time.monotonic()
        if timeout_sec > 0 and (now - start) >= timeout_sec:
            timed_out = True
            _kill_process_tree(proc)
            break
        if fatal_at is not None and (now - fatal_at) >= error_grace_sec:
            _kill_process_tree(proc)
            break

        try:
            item = line_q.get(timeout=0.1)
        except queue.Empty:
            if proc.poll() is not None and not thread.is_alive():
                eof = True
            continue

        if item is None:
            eof = True
            break

        lines.append(item)
        if detect_fatal and fatal_reason is None:
            matched = match_fatal_line(item)
            if matched is not None:
                fatal_reason = matched
                fatal_at = time.monotonic()

    thread.join(timeout=1.5)
    if proc.stdout is not None:
        proc.stdout.close()
    while True:
        try:
            extra = line_q.get_nowait()
        except queue.Empty:
            break
        if extra is not None:
            lines.append(extra)
            if detect_fatal and fatal_reason is None:
                matched = match_fatal_line(extra)
                if matched is not None:
                    fatal_reason = matched

    rc = proc.wait()
    if timed_out:
        return 124, lines, f"timeout after {timeout_sec:.1f}s"
    if fatal_reason is not None:
        return 1, lines, fatal_reason
    return rc, lines, None


def _write_log(log_path: Path | None, lines: list[str]) -> None:
    if log_path is None:
        return
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def _print_tail(lines: list[str], tail_lines: int = 40) -> None:
    snippet = lines[-tail_lines:] if len(lines) > tail_lines else lines
    for line in snippet:
        print(line, flush=True)


def parse_args(argv: list[str]) -> tuple[argparse.Namespace, list[str]]:
    parser = argparse.ArgumentParser(
        description="Run a Godot scene/script fail-fast with streaming error detection.",
    )
    parser.add_argument("target", nargs="?", help="Scene or script path (e.g. res://tests_gd/Foo.tscn)")
    parser.add_argument("--godot", default=None, help="Override Godot executable path")
    parser.add_argument("--project", default=None, help="Godot project path (default: <repo>/godot)")
    parser.add_argument("--timeout", type=float, default=60.0, help="Wall-clock timeout in seconds (0 disables)")
    parser.add_argument("--quit-after", type=int, default=300, help="Pass --quit-after N for scene runs (0 disables)")
    parser.add_argument("--error-grace", type=float, default=1.5, help="Grace window before killing tree after fatal log")
    parser.add_argument("--marker", action="append", default=[], help="Required output substring (repeatable)")
    parser.add_argument("--log", default=None, help="Write captured stdout/stderr to this file")
    parser.add_argument("--windowed", action="store_true", help="Run without --headless")
    parser.add_argument("--rendering-driver", default=None, help="Godot rendering driver (e.g. opengl3)")
    parser.add_argument("--no-check-only", action="store_true", help="Skip GDScript --check-only preflight")
    parser.add_argument("--quiet-pass", action="store_true", help="Suppress full output on success")
    return parser.parse_known_args(argv)


def main(argv: list[str] | None = None) -> int:
    raw_argv = list(sys.argv[1:] if argv is None else argv)
    extra_after_sep: list[str] = []
    if "--" in raw_argv:
        sep_idx = raw_argv.index("--")
        extra_after_sep = raw_argv[sep_idx + 1 :]
        raw_argv = raw_argv[:sep_idx]

    args, unknown_args = parse_args(raw_argv)
    extra_args = unknown_args + extra_after_sep
    repo_root = _repo_root()
    project_dir = Path(args.project).resolve() if args.project else (repo_root / "godot")
    log_path = Path(args.log) if args.log else None

    try:
        godot_bin = _find_godot(repo_root, args.godot)
    except FileNotFoundError as exc:
        print(f"RUN_GODOT_TEST_FAIL: {exc}", file=sys.stderr)
        return 2

    env = _child_env(repo_root)
    targets = [args.target] if args.target else []
    is_import_mode = "--import" in extra_args

    if not args.no_check_only and not is_import_mode:
        for script_res in _discover_gd_scripts(project_dir, targets, extra_args):
            chk_cmd = [
                godot_bin,
                "--headless",
                "--path",
                str(project_dir),
                "--check-only",
                "--script",
                script_res,
            ]
            chk_rc, chk_lines, chk_err = _stream_run(
                chk_cmd, env=env, timeout_sec=15.0, error_grace_sec=0.5, detect_fatal=True
            )
            if chk_rc != 0 or chk_err is not None:
                _write_log(log_path, chk_lines)
                print(
                    f"RUN_GODOT_TEST_FAIL stage=check-only script={script_res} rc={chk_rc} reason={chk_err}",
                    file=sys.stderr,
                )
                _print_tail(chk_lines)
                return 1

    cmd = [godot_bin]
    if args.rendering_driver:
        cmd.extend(["--rendering-driver", args.rendering_driver])
    elif not args.windowed:
        cmd.append("--headless")
    cmd.extend(["--path", str(project_dir)])
    if args.quit_after > 0 and not is_import_mode:
        cmd.extend(["--quit-after", str(args.quit_after)])
    cmd.extend(extra_args)
    cmd.extend(targets)

    rc, lines, reason = _stream_run(
        cmd,
        env=env,
        timeout_sec=args.timeout,
        error_grace_sec=args.error_grace,
        detect_fatal=not is_import_mode,
    )
    _write_log(log_path, lines)

    joined = "\n".join(lines)
    missing_markers = [m for m in args.marker if m not in joined]
    if reason is not None or rc != 0 or missing_markers:
        detail = reason or (f"missing marker(s): {', '.join(missing_markers)}" if missing_markers else f"exit={rc}")
        print(
            f"RUN_GODOT_TEST_FAIL cmd={shlex.join(cmd)} rc={rc} reason={detail}",
            file=sys.stderr,
        )
        _print_tail(lines)
        return rc if rc != 0 else 1

    if not args.quiet_pass:
        for line in lines:
            print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
