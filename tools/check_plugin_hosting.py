#!/usr/bin/env python3
"""Run owned synthetic hosting fixtures without installing a user plugin."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

from plugin_bundle_layout import library_filename, stage_test_bundle

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile",default="debug",choices=["debug","release"])
    args=parser.parse_args()
    system=platform.system()
    root=Path("target")/args.profile
    library=(root / library_filename(system, "daw_plugin_host_fixtures")).resolve(strict=True)
    check=(root/"examples"/("native_fixture_check.exe" if system=="Windows" else "native_fixture_check")).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="daw-host-fixtures-") as directory:
        for kind in ["clap","vst3"]:
            bundle=Path(directory)/f"HostFixtures.{kind}"
            stage_test_bundle(bundle, library, "HostFixtures", kind, system, platform.machine(),
                              "TestPayload", f"org.daw.host-fixtures.{kind}")
            subprocess.run([str(check),kind,str(bundle)],check=True,timeout=300)
            if kind=="clap":
                preset=(root/"examples"/("native_preset_check.exe" if system=="Windows" else "native_preset_check")).resolve(strict=True)
                subprocess.run([str(preset),str(bundle)],check=True,timeout=30)
if __name__=="__main__":main()
