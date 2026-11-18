"""Shared workspace and command handling for local Drive checks."""
from __future__ import annotations

import subprocess
from pathlib import Path


def owned_scratch(path: Path, root: Path, marker_name: str, signature: str, *,
                  outside: str, unrecognized: str) -> Path:
    base = root / "work"
    path = path.resolve()
    if path == base or not path.is_relative_to(base):
        raise ValueError(outside)
    path.mkdir(parents=True, exist_ok=True)
    marker = path / marker_name
    if marker.exists():
        if marker.read_text() != signature:
            raise ValueError(unrecognized)
    elif any(path.iterdir()):
        raise ValueError("Retaining unmarked scratch contents; choose a fresh --work-dir")
    else:
        marker.write_text(signature)
    return path


def run_logged(command, log, env, *, cwd, allowed=(0,), status=False, create_parent=False,
               preview_count=4):
    if create_parent:
        log.parent.mkdir(parents=True, exist_ok=True)
    print("Running " + " ".join(map(str, command[:preview_count])), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    log.write_text(result.stdout)
    if status:
        log.with_suffix(log.suffix + ".status").write_text(str(result.returncode))
    log.with_suffix(log.suffix + ".stderr").write_text(result.stderr)
    if result.returncode not in allowed:
        raise RuntimeError(f"Command failed ({result.returncode}); inspect {log} and its .stderr file")
    return result.stdout
