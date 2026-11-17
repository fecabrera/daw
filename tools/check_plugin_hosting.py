#!/usr/bin/env python3
"""Run owned synthetic hosting fixtures without installing a user plugin."""
import argparse
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tempfile

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile",default="debug",choices=["debug","release"])
    args=parser.parse_args()
    system=platform.system()
    prefix,suffix=("",".dll") if system=="Windows" else ("lib",".dylib" if system=="Darwin" else ".so")
    root=Path("target")/args.profile
    library=(root/f"{prefix}daw_plugin_host_fixtures{suffix}").resolve(strict=True)
    check=(root/"examples"/("native_fixture_check.exe" if system=="Windows" else "native_fixture_check")).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="daw-host-fixtures-") as directory:
        for kind in ["clap","vst3"]:
            bundle=Path(directory)/f"HostFixtures.{kind}"
            if system=="Darwin":binary=bundle/"Contents/MacOS/TestPayload"
            elif system=="Windows":binary=bundle/f"Contents/{'arm64' if platform.machine().lower() in ('arm64','aarch64') else 'x86_64'}-win/HostFixtures.{kind}"
            else:binary=bundle/f"Contents/{platform.machine()}-linux/HostFixtures.so"
            binary.parent.mkdir(parents=True)
            shutil.copy2(library,binary)
            if system=="Darwin":
                with (bundle/"Contents/Info.plist").open("wb") as output:plistlib.dump({"CFBundleExecutable":"TestPayload","CFBundleIdentifier":f"org.daw.host-fixtures.{kind}","CFBundlePackageType":"BNDL"},output)
            subprocess.run([str(check),kind,str(bundle)],check=True,timeout=300)
            if kind=="clap":
                preset=(root/"examples"/("native_preset_check.exe" if system=="Windows" else "native_preset_check")).resolve(strict=True)
                subprocess.run([str(preset),str(bundle)],check=True,timeout=30)
if __name__=="__main__":main()
