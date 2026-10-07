"""Create a short four-track test project using generated audio only."""
import json
import math
from pathlib import Path
import struct
import uuid
import wave

ROOT = Path(__file__).resolve().parents[1]
RATE = 48000
SECONDS = 12
NAMES = ["Bass", "Keys", "Pulse", "Lead"]


def identity(name):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, "daw-example/" + name))


def create():
    audio = ROOT / "examples" / "audio"
    project = ROOT / "examples" / "demo"
    audio.mkdir(parents=True, exist_ok=True)
    (project / "assets").mkdir(parents=True, exist_ok=True)
    assets = []
    tracks = []
    for track, name in enumerate(NAMES):
        path = audio / (name.lower() + ".wav")
        with wave.open(str(path), "wb") as out:
            out.setparams((2, 2, RATE, 0, "NONE", "not compressed"))
            block = bytearray()
            for frame in range(RATE * SECONDS):
                t = frame / RATE
                beat = t % 0.5
                frequency = [110, 220, 880, 440][track]
                envelope = math.exp(-beat * [3, 5, 35, 7][track])
                signal = math.sin(math.tau * frequency * t) * envelope * 0.14
                signal *= min(1, t / 0.005, (SECONDS - t) / 0.005)
                left = int(signal * 32767)
                right = int(signal * (0.9 if track == 1 else 1) * 32767)
                block.extend(struct.pack("<hh", left, right))
            out.writeframes(block)
        aid = identity(name + "-asset")
        assets.append({"id": aid, "name": path.name,
                       "source": {"kind": "external", "path": "../audio/" + path.name,
                                  "path_kind": "relative"},
                       "source_metadata": {"sample_rate_hz": RATE, "channels": 2,
                                           "sample_format": "pcm_int", "bits_per_sample": 16},
                       "decoded_frame_count": RATE * SECONDS})
        tracks.append({"id": identity(name + "-track"), "name": name, "gain_db": 0,
                       "pan": [-0.1, 0.3, 0, -0.25][track], "muted": False, "soloed": False,
                       "clips": [{"id": identity(name + "-clip"), "asset_id": aid, "name": name + " 1",
                                  "start_frame": 0, "source_offset_frame": 0,
                                  "length_frames": RATE * SECONDS}]})
    manifest = {"project_id": identity("project"), "name": "Four track demo", "sample_rate_hz": RATE,
                "master": {"gain_db": 0}, "assets": assets, "tracks": tracks,
                "transport": {"playhead_frame": 0, "loop": {"enabled": False,
                              "start_frame": 0, "end_frame": RATE * SECONDS}}}
    (project / "project.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(project)


if __name__ == "__main__":
    create()
