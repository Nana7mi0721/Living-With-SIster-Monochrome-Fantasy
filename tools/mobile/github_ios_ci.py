#!/usr/bin/env python3
"""Trigger, poll, and download unsigned iOS engine-only IPA artifacts from GitHub Actions.

Uses Python standard library HTTP requests so it works on Windows without requiring `gh` CLI.
Authentication is resolved in order:
  1. Explicit `--token` CLI argument
  2. Ignored local token file `work/out/github_token.txt` (if present)
  3. `GITHUB_TOKEN` / `GH_TOKEN` environment variables
  4. `gh auth token` (when GitHub CLI is installed and logged in)
  5. `git credential fill` for https://github.com (Windows Credential Manager / Git helper)
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

API_BASE = "https://api.github.com"
DEFAULT_WORKFLOW = "ios-unsigned.yml"
USER_AGENT = "lwsmf-release-packager"


class GitHubCIError(RuntimeError):
    """Raised when GitHub Actions API requests or artifact downloads fail."""


class _NoAuthCrossHostRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Strip Authorization header when redirected off api.github.com (e.g. Azure blob storage)."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # type: ignore[override]
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if redirected is None:
            return None
        old_host = urllib.parse.urlparse(req.full_url).netloc.lower()
        new_host = urllib.parse.urlparse(newurl).netloc.lower()
        if old_host != new_host:
            redirected.remove_header("Authorization")
            redirected.remove_header("Accept")
        return redirected


def parse_github_repo_from_remote(url: str) -> str | None:
    raw = url.strip()
    if not raw:
        return None
    m = re.match(r"^(?:https?://(?:[^@/]+@)?|ssh://git@|git@)github\.com[:/]([^/\s]+)/([^/\s]+?)(?:\.git)?/?$", raw, re.I)
    if not m:
        return None
    return f"{m.group(1)}/{m.group(2)}"


def detect_git_repo_and_branch(repo_root: Path) -> tuple[str | None, str]:
    repo_slug: str | None = None
    branch = "master"
    try:
        remote = subprocess.check_output(
            ["git", "-C", str(repo_root), "remote", "get-url", "origin"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        repo_slug = parse_github_repo_from_remote(remote)
    except Exception:  # noqa: BLE001
        pass
    try:
        b = subprocess.check_output(
            ["git", "-C", str(repo_root), "rev-parse", "--abbrev-ref", "HEAD"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        if b and b != "HEAD":
            branch = b
    except Exception:  # noqa: BLE001
        pass
    return repo_slug, branch


def iter_github_token_candidates(repo_root: Path, explicit_token: str = "") -> list[tuple[str, str]]:
    """Return ordered unique (token, source_label) candidates without exposing secrets."""
    candidates: list[tuple[str, str]] = []
    seen: set[str] = set()

    def add(tok: str | None, label: str) -> None:
        val = (tok or "").strip()
        if val and val not in seen:
            seen.add(val)
            candidates.append((val, label))

    add(explicit_token, "cli")

    token_file = repo_root / "work/out/github_token.txt"
    if token_file.is_file():
        add(token_file.read_text(encoding="utf-8", errors="replace"), "work/out/github_token.txt")

    for key in ("GITHUB_TOKEN", "GH_TOKEN"):
        add(os.environ.get(key, ""), f"env:{key}")

    gh_bin = shutil.which("gh")
    if gh_bin:
        try:
            proc = subprocess.run(
                [gh_bin, "auth", "token"],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=10,
                check=False,
            )
            if proc.returncode == 0:
                add(proc.stdout, "gh")
        except Exception:  # noqa: BLE001
            pass

    git_bin = shutil.which("git")
    if git_bin:
        try:
            env = os.environ.copy()
            env["GIT_TERMINAL_PROMPT"] = "0"
            env["GCM_INTERACTIVE"] = "never"
            proc = subprocess.run(
                [git_bin, "credential", "fill"],
                input="protocol=https\nhost=github.com\n\n",
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                env=env,
                timeout=10,
                check=False,
            )
            if proc.returncode == 0:
                for line in proc.stdout.splitlines():
                    if line.startswith("password="):
                        add(line.split("=", 1)[1], "git-credential")
                        break
        except Exception:  # noqa: BLE001
            pass

    return candidates


def resolve_github_token(repo_root: Path, explicit_token: str = "") -> tuple[str | None, str]:
    """Return the first available (token, source_label) without printing or logging the secret."""
    candidates = iter_github_token_candidates(repo_root, explicit_token)
    if candidates:
        return candidates[0]
    return None, "none"


def select_working_client(
    repo: str,
    repo_root: Path,
    explicit_token: str = "",
) -> tuple[GitHubActionsClient, str]:
    """Pick the first GitHub token candidate that authenticates successfully against /repos/<repo>."""
    candidates = iter_github_token_candidates(repo_root, explicit_token)
    last_err: GitHubCIError | None = None
    for tok, label in candidates:
        client = GitHubActionsClient(repo, tok)
        try:
            client.canonicalize_repo()
            return client, label
        except GitHubCIError as exc:
            last_err = exc
            if "HTTP 401" in str(exc):
                continue
            raise
    if last_err is not None:
        raise last_err
    raise GitHubCIError(
        "No valid GitHub token found. Set GITHUB_TOKEN / GH_TOKEN, run `gh auth login`, "
        "or save a fine-grained PAT (actions:read+write, contents:read) in work/out/github_token.txt."
    )


def save_local_github_token(repo_root: Path, token: str) -> Path:
    path = repo_root / "work/out/github_token.txt"
    path.parent.mkdir(parents=True, exist_ok=True)
    content = token.strip() + "\n"
    if os.name != "nt":
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(content)
        os.chmod(path, 0o600)
    else:
        path.write_text(content, encoding="utf-8")
    return path


class GitHubActionsClient:
    def __init__(self, repo: str, token: str | None) -> None:
        self.repo = repo
        self.token = token
        self._opener = urllib.request.build_opener(_NoAuthCrossHostRedirectHandler)

    def _headers(self, accept: str = "application/vnd.github+json") -> dict[str, str]:
        headers = {
            "Accept": accept,
            "User-Agent": USER_AGENT,
            "X-GitHub-Api-Version": "2022-11-28",
        }
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    def request_json(self, method: str, path: str, body: dict[str, Any] | None = None) -> tuple[int, Any]:
        url = path if path.startswith("http") else f"{API_BASE}{path}"
        data = json.dumps(body).encode("utf-8") if body is not None else None
        headers = self._headers()
        if data is not None:
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with self._opener.open(req, timeout=30) as resp:
                raw = resp.read()
                parsed = json.loads(raw.decode("utf-8")) if raw else {}
                return resp.status, parsed
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace").strip()
            msg = f"GitHub API {method} {path} failed (HTTP {exc.code})"
            if detail:
                try:
                    j = json.loads(detail)
                    if isinstance(j, dict) and j.get("message"):
                        msg += f": {j['message']}"
                except ValueError:
                    msg += f": {detail[:200]}"
            if exc.code in (401, 403):
                msg += " (check GITHUB_TOKEN / GH_TOKEN permissions: actions:write + actions:read)"
            raise GitHubCIError(msg) from exc
        except urllib.error.URLError as exc:
            raise GitHubCIError(f"GitHub API network error ({path}): {exc.reason}") from exc

    def download_file(self, url: str, dest: Path) -> None:
        req = urllib.request.Request(url, headers=self._headers(), method="GET")
        try:
            with self._opener.open(req, timeout=180) as resp, dest.open("wb") as out:
                shutil.copyfileobj(resp, out, length=1 << 20)
        except urllib.error.HTTPError as exc:
            raise GitHubCIError(f"Failed to download artifact (HTTP {exc.code})") from exc
        except urllib.error.URLError as exc:
            raise GitHubCIError(f"Network error downloading artifact: {exc.reason}") from exc

    def canonicalize_repo(self) -> str:
        """Follow GitHub repository rename redirects so POST workflow_dispatch hits the canonical repo."""
        _, data = self.request_json("GET", f"/repos/{self.repo}")
        if isinstance(data, dict) and data.get("full_name"):
            self.repo = str(data["full_name"])
        return self.repo

    def list_workflow_runs(
        self,
        workflow: str = DEFAULT_WORKFLOW,
        *,
        event: str | None = None,
        status: str | None = None,
        per_page: int = 15,
    ) -> list[dict[str, Any]]:
        params = [f"per_page={per_page}"]
        if event:
            params.append(f"event={urllib.parse.quote(event)}")
        if status:
            params.append(f"status={urllib.parse.quote(status)}")
        qs = "&".join(params)
        _, data = self.request_json("GET", f"/repos/{self.repo}/actions/workflows/{workflow}/runs?{qs}")
        runs = data.get("workflow_runs") if isinstance(data, dict) else []
        return [r for r in runs if isinstance(r, dict)]

    def dispatch_workflow(self, workflow: str, ref: str, version: str) -> None:
        status, _ = self.request_json(
            "POST",
            f"/repos/{self.repo}/actions/workflows/{workflow}/dispatches",
            {"ref": ref, "inputs": {"version": version}},
        )
        if status not in (200, 201, 204):
            raise GitHubCIError(f"Unexpected status dispatching {workflow}: HTTP {status}")

    def describe_failed_steps(self, run_id: int) -> list[str]:
        try:
            _, data = self.request_json("GET", f"/repos/{self.repo}/actions/runs/{run_id}/jobs")
        except GitHubCIError:
            return []
        lines: list[str] = []
        for job in data.get("jobs") or []:
            if not isinstance(job, dict):
                continue
            jname = str(job.get("name") or "job")
            for st in job.get("steps") or []:
                if isinstance(st, dict) and st.get("conclusion") == "failure":
                    lines.append(f"{jname} / step #{st.get('number')}: {st.get('name')}")
        return lines

    def list_run_artifacts(self, run_id: int) -> list[dict[str, Any]]:
        _, data = self.request_json("GET", f"/repos/{self.repo}/actions/runs/{run_id}/artifacts")
        arts = data.get("artifacts") if isinstance(data, dict) else []
        return [a for a in arts if isinstance(a, dict) and not a.get("expired")]


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _parse_iso_utc(text: str) -> datetime:
    cleaned = text.strip().replace("Z", "+00:00")
    return datetime.fromisoformat(cleaned)


def _select_ipa_artifact(artifacts: list[dict[str, Any]], version: str) -> dict[str, Any] | None:
    expected = f"LWSMF-{version}-ios-arm64"
    for art in artifacts:
        if art.get("name") == expected:
            return art
    for art in artifacts:
        name = str(art.get("name") or "")
        if "ios" in name.lower():
            return art
    return artifacts[0] if artifacts else None


def extract_ipa_from_artifact_zip(artifact_zip: Path, out_dir: Path) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    extracted_ipa: Path | None = None
    with zipfile.ZipFile(artifact_zip, "r") as zf:
        bad = zf.testzip()
        if bad is not None:
            raise GitHubCIError(f"Downloaded artifact ZIP failed CRC check at {bad}")
        for info in zf.infolist():
            if info.is_dir():
                continue
            name = Path(info.filename).name
            if not name or name in (".", ".."):
                continue
            if name.lower().endswith(".ipa") or name == "SHA256SUMS.txt":
                dest = out_dir / name
                tmp = dest.with_name(dest.name + ".tmp")
                with zf.open(info, "r") as src, tmp.open("wb") as dst:
                    shutil.copyfileobj(src, dst, length=1 << 20)
                os.replace(tmp, dest)
                if name.lower().endswith(".ipa"):
                    extracted_ipa = dest
    if extracted_ipa is None or not extracted_ipa.is_file():
        raise GitHubCIError("Artifact ZIP did not contain an .ipa file")
    with zipfile.ZipFile(extracted_ipa, "r") as zf:
        if zf.testzip() is not None:
            raise GitHubCIError(f"Extracted IPA failed ZIP CRC check: {extracted_ipa}")
    return extracted_ipa


def download_run_ipa(
    client: GitHubActionsClient,
    run_id: int,
    version: str,
    out_dir: Path,
) -> Path:
    artifacts = client.list_run_artifacts(run_id)
    chosen = _select_ipa_artifact(artifacts, version)
    if chosen is None:
        raise GitHubCIError(f"Workflow run {run_id} has no downloadable (unexpired) iOS IPA artifact")

    dl_url = str(chosen.get("archive_download_url") or "")
    if not dl_url:
        raise GitHubCIError(f"Artifact {chosen.get('name')} has no archive_download_url")

    size_mb = int(chosen.get("size_in_bytes") or 0) / (1024 * 1024)
    print(f"Downloading artifact {chosen.get('name')} ({size_mb:.2f} MiB) from run #{run_id}...", flush=True)
    with tempfile.TemporaryDirectory(prefix="lwsmf-gh-art-") as tmp:
        art_zip = Path(tmp) / "artifact.zip"
        client.download_file(dl_url, art_zip)
        return extract_ipa_from_artifact_zip(art_zip, out_dir)


def fetch_latest_ios_ipa(
    client: GitHubActionsClient,
    version: str,
    out_dir: Path,
    workflow: str = DEFAULT_WORKFLOW,
) -> tuple[Path, int]:
    client.canonicalize_repo()
    runs = client.list_workflow_runs(workflow, status="completed", per_page=20)
    expected = f"LWSMF-{version}-ios-arm64"
    fallback_run_id: int | None = None
    fallback_art_name = ""

    for run in runs:
        if run.get("conclusion") != "success":
            continue
        run_id = int(run["id"])
        arts = client.list_run_artifacts(run_id)
        if not arts:
            continue
        if any(a.get("name") == expected for a in arts):
            ipa = download_run_ipa(client, run_id, version, out_dir)
            return ipa, run_id
        if fallback_run_id is None:
            fallback_run_id = run_id
            chosen = _select_ipa_artifact(arts, version)
            fallback_art_name = str(chosen.get("name") or "") if chosen else ""

    if fallback_run_id is not None:
        print(
            f"WARN: no artifact matched expected version {expected!r}; "
            f"falling back to run #{fallback_run_id} ({fallback_art_name or 'latest artifact'})",
            file=sys.stderr,
            flush=True,
        )
        ipa = download_run_ipa(client, fallback_run_id, version, out_dir)
        return ipa, fallback_run_id

    raise GitHubCIError(
        f"No completed successful {workflow} run with an active artifact found in {client.repo}. "
        "Trigger a new build first (--trigger)."
    )


def trigger_and_download_ios_ipa(
    client: GitHubActionsClient,
    ref: str,
    version: str,
    out_dir: Path,
    workflow: str = DEFAULT_WORKFLOW,
    poll_interval: int = 15,
    timeout_seconds: int = 3600,
) -> tuple[Path, int]:
    client.canonicalize_repo()
    existing_ids = {int(r["id"]) for r in client.list_workflow_runs(workflow, event="workflow_dispatch", per_page=10)}
    started_utc = datetime.now(timezone.utc)
    print(f"Dispatching {workflow} on {client.repo}@{ref} (version={version})...", flush=True)
    client.dispatch_workflow(workflow, ref, version)

    run_id: int | None = None
    run_url = ""
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout_seconds:
        time.sleep(min(poll_interval, 5) if run_id is None else poll_interval)
        runs = client.list_workflow_runs(workflow, event="workflow_dispatch", per_page=10)
        if run_id is None:
            for r in runs:
                rid = int(r["id"])
                if rid in existing_ids:
                    continue
                created = _parse_iso_utc(str(r.get("created_at") or ""))
                if (created - started_utc).total_seconds() >= -30:
                    run_id = rid
                    run_url = str(r.get("html_url") or "")
                    print(f"Workflow run started: #{run_id}  {run_url}", flush=True)
                    break
            if run_id is None:
                elapsed = int(time.monotonic() - t0)
                print(f"  waiting for GitHub Actions run to register... ({elapsed}s)", flush=True)
                continue

        current = next((r for r in runs if int(r["id"]) == run_id), None)
        if current is None:
            _, current = client.request_json("GET", f"/repos/{client.repo}/actions/runs/{run_id}")
        status = str(current.get("status") or "unknown")
        conclusion = str(current.get("conclusion") or "")
        elapsed = int(time.monotonic() - t0)
        print(f"  run #{run_id}: status={status} conclusion={conclusion or '-'} ({elapsed}s)", flush=True)
        if status == "completed":
            if conclusion != "success":
                failed_steps = client.describe_failed_steps(run_id)
                step_note = f" | failed step(s): {'; '.join(failed_steps)}" if failed_steps else ""
                raise GitHubCIError(f"Workflow run #{run_id} finished with conclusion={conclusion}{step_note}: {run_url}")
            ipa = download_run_ipa(client, run_id, version, out_dir)
            return ipa, run_id

    raise GitHubCIError(f"Timed out after {timeout_seconds}s waiting for {workflow} run #{run_id}: {run_url}")


def main(argv: list[str] | None = None) -> int:
    repo_root = Path(__file__).resolve().parents[2]
    detected_repo, detected_branch = detect_git_repo_and_branch(repo_root)

    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--fetch-latest", action="store_true", help="Download the latest successful ios-unsigned.yml IPA artifact")
    mode.add_argument("--trigger", action="store_true", help="Trigger ios-unsigned.yml on GitHub Actions, wait, and download IPA")
    mode.add_argument("--status", action="store_true", help="Show detected repo/branch/token source and recent workflow runs")
    parser.add_argument("--repo", default=detected_repo or "", help="GitHub repository owner/name (default: git origin)")
    parser.add_argument("--ref", default=detected_branch, help="Branch or tag ref for --trigger")
    parser.add_argument("--version", default="0.1.0", help="Package version input / expected artifact version")
    parser.add_argument("--workflow", default=DEFAULT_WORKFLOW, help="Workflow file name")
    parser.add_argument("--out-dir", type=Path, default=None, help="Output directory (default: work/out/ios_ci/<version>)")
    parser.add_argument("--token", default="", help="Optional GitHub token (otherwise auto-detected)")
    parser.add_argument("--poll-interval", type=int, default=15, help="Polling interval in seconds for --trigger")
    parser.add_argument("--timeout", type=int, default=3600, help="Max wait seconds for --trigger")
    args = parser.parse_args(argv)

    if not args.repo:
        print("ERROR: unable to detect GitHub repository from git origin; pass --repo owner/name", file=sys.stderr)
        return 2

    out_dir = args.out_dir or (repo_root / "work/out/ios_ci" / args.version)

    try:
        client, source = select_working_client(args.repo, repo_root, args.token)
        if args.status:
            print(f"GITHUB_CI_STATUS repo={client.repo} ref={args.ref} token_source={source}")
            runs = client.list_workflow_runs(args.workflow, per_page=5)
            for r in runs:
                print(
                    f"  run=#{r.get('id')} status={r.get('status')} conclusion={r.get('conclusion') or '-'} "
                    f"branch={r.get('head_branch')} created={r.get('created_at')}"
                )
            return 0

        if args.fetch_latest:
            ipa, run_id = fetch_latest_ios_ipa(client, args.version, out_dir, workflow=args.workflow)
        else:
            ipa, run_id = trigger_and_download_ios_ipa(
                client,
                args.ref,
                args.version,
                out_dir,
                workflow=args.workflow,
                poll_interval=max(5, args.poll_interval),
                timeout_seconds=max(60, args.timeout),
            )
    except GitHubCIError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    sha = _sha256(ipa)
    print(f"IOS_CI_IPA_OK path={ipa} run_id={run_id} bytes={ipa.stat().st_size} sha256={sha}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
