"""Check CLI exit codes and deterministic export using the generated demo."""
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import uuid
import wave

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug" / ("daw-cli.exe" if os.name == "nt" else "daw-cli")


def run(expected, *args):
    env = dict(os.environ, DISPLAY="", WAYLAND_DISPLAY="")
    result = subprocess.run([str(BINARY), *map(str, args)], capture_output=True, text=True, env=env)
    assert result.returncode == expected, (args, result.returncode, result.stderr)
    return result


def main():
    assert BINARY.exists(), "Build daw-cli first"
    manifest = json.loads((ROOT / "examples/demo/project.json").read_text())
    for asset in manifest["assets"]:
        asset["source"]["path"] = str((ROOT / "examples/demo" / asset["source"]["path"]).resolve())
        asset["source"]["path_kind"] = "absolute"
    with tempfile.TemporaryDirectory(prefix="daw-cli-check-") as temp:
        folder = Path(temp)
        project = folder / "project.json"
        project.write_text(json.dumps(manifest))
        run(0, "validate", "--project", folder)
        first, second = folder / "a.wav", folder / "b.wav"
        run(0, "render", "--project", folder, "--output", first)
        run(0, "render", "--project", folder, "--output", second)
        assert first.read_bytes() == second.read_bytes()
        with wave.open(str(first)) as output:
            assert (output.getnchannels(), output.getsampwidth(), output.getframerate(), output.getnframes()) == (2, 3, 48000, 576000)
        run(1, "render", "--project", folder, "--output", first)
        run(0, "render", "--project", folder, "--output", first, "--overwrite")
        missing = copy.deepcopy(manifest)
        missing["assets"][0]["source"]["path"] = str(folder / "missing.wav")
        project.write_text(json.dumps(missing))
        assert "Missing source" in run(0, "validate", "--project", folder).stderr
        assert "Missing source" in run(0, "render", "--project", folder, "--output", folder / "missing-mix.wav").stderr
        expanded = copy.deepcopy(manifest)
        for _ in range(4):
            track = copy.deepcopy(manifest["tracks"][0])
            track["id"] = str(uuid.uuid4())
            for clip in track["clips"]:
                clip["id"] = str(uuid.uuid4())
            expanded["tracks"].append(track)
        assert len(expanded["tracks"]) == 8
        project.write_text(json.dumps(expanded))
        run(0, "validate", "--project", folder)
        run(0, "render", "--project", folder, "--output", folder / "eight-tracks.wav")
        project.write_text("invalid json")
        run(1, "validate", "--project", folder)
        run(2, "render")
    print("CLI smoke check passed: WAV format, repeatability, missing sources, eight-track projects, overwrite, and exit codes")


if __name__ == "__main__":
    main()
