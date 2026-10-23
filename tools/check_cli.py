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
        pcm16 = folder / "pcm16.wav"
        run(0, "render", "--project", folder, "--output", pcm16, "--codec", "pcm16")
        with wave.open(str(pcm16)) as output:
            assert (output.getnchannels(), output.getsampwidth(), output.getframerate(), output.getnframes()) == (2, 2, 48000, 576000)
        float32 = folder / "float32.wav"
        run(0, "render", "--project", folder, "--output", float32, "--codec", "float32")
        assert float32.read_bytes()[:4] == b"RIFF"
        for bitrate, index in [(128, 9), (192, 11), (256, 13), (320, 14)]:
            mp3 = folder / f"{bitrate}.mp3"
            run(0, "render", "--project", folder, "--output", mp3, "--bitrate", bitrate)
            data = mp3.read_bytes()
            assert data[0] == 0xff and data[1] & 0xfe == 0xfa
            assert data[2] >> 4 == index and data[2] & 0x0c == 4
            run(1, "render", "--project", folder, "--output", mp3)
            run(0, "render", "--project", folder, "--output", mp3, "--format", "mp3", "--bitrate", bitrate, "--overwrite")
            assert mp3.read_bytes() == data
        for options in [("--format", "mp3"), ("--bitrate", "320"), ("--codec", "invalid")]:
            run(2, "render", "--project", folder, "--output", first, *options)
        run(2, "render", "--project", folder, "--output", folder / "invalid.mp3", "--bitrate", "7")
        run(2, "render", "--project", folder, "--output", folder / "invalid.mp3", "--codec", "pcm16")
        # Keep codec smoke coverage short; quality/rate coverage lives in media tests.
        aac_project = copy.deepcopy(manifest)
        aac_project["tracks"] = aac_project["tracks"][:1]
        aac_project["tracks"][0]["clips"] = aac_project["tracks"][0]["clips"][:1]
        aac_project["tracks"][0]["clips"][0].update(start_frame=0, length_frames=4800)
        project.write_text(json.dumps(aac_project))
        for bitrate in (128, 192, 256, 320):
            aac = folder / f"{bitrate}.aac"
            run(0, "render", "--project", folder, "--output", aac, "--bitrate", bitrate)
            data = aac.read_bytes()
            offset, packets = 0, 0
            while offset < len(data):
                assert len(data) - offset >= 7
                header = data[offset:offset + 7]
                assert header[0] == 0xff and header[1] & 0xf6 == 0xf0
                assert header[2] >> 6 == 1  # AAC-LC
                assert header[2] >> 2 & 15 == 3  # 48 kHz
                assert ((header[2] & 1) << 2) | (header[3] >> 6) == 2
                length = ((header[3] & 3) << 11) | (header[4] << 3) | (header[5] >> 5)
                assert length >= 7 and offset + length <= len(data)
                offset += length
                packets += 1
            assert packets == 6  # ceil(4800 / 1024) plus codec overlap flush
            run(1, "render", "--project", folder, "--output", aac)
            run(0, "render", "--project", folder, "--output", aac, "--format", "aac", "--bitrate", bitrate, "--overwrite")
            assert aac.read_bytes() == data
        for options in [("--codec", "pcm16"), ("--bitrate", "7"), ("--format", "wav")]:
            run(2, "render", "--project", folder, "--output", folder / "invalid.aac", *options)
        run(2, "render", "--project", folder, "--output", folder / "invalid.m4a", "--format", "aac")
        project.write_text(json.dumps(manifest))
        # A unity group is a real processing bus and preserves the flat mix.
        grouped = copy.deepcopy(manifest)
        group = copy.deepcopy(grouped["tracks"][0])
        group.update(id=str(uuid.uuid4()), name="Group bus", kind="group",
                     clips=[], effects=[], gain_db=0.0, pan=0.0,
                     muted=False, soloed=False, group_collapsed=True)
        group.pop("instrument", None)
        for track in grouped["tracks"]:
            track["group_id"] = group["id"]
        grouped["tracks"].insert(0, group)
        project.write_text(json.dumps(grouped))
        run(0, "validate", "--project", folder)
        group_mix = folder / "group-mix.wav"
        run(0, "render", "--project", folder, "--output", group_mix)
        assert group_mix.read_bytes() == first.read_bytes()
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
    print("CLI smoke check passed: WAV/MP3/AAC codecs and bitrates, repeatability, missing sources, eight-track projects, group buses, overwrite, and exit codes")


if __name__ == "__main__":
    main()
