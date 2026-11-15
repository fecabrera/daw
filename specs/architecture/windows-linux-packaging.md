# Windows and Linux desktop packaging

Distribution mode remains the default. It checks the clean committed tracked
tree before building, then includes the source archive, manifest and access
notice through `tools/package_sources.py`.

Local mode builds the current working tree without checking Git state or
vendoring source dependencies. It adds `DAW/LOCAL-BUILD.txt` and omits
`DAW-source.zip`, `source-manifest.json` and `SOURCE-ACCESS.txt`. It retains the
executable, project license, dependency notices and platform resources. It does
not commit, modify the index or collect user data.

Linux accepts `tools/package_linux.sh [debug|release] [--local]`. The profile
defaults to debug and the flag can precede or follow it. Reject unknown
arguments and duplicate profiles with exit code 2 before building. The native
host check remains required. The build helper overrides Cargo target-directory
and target settings, validates the native executable and retains the existing
architecture-specific ZIP name. Both modes include runtime requirements,
desktop entry and hicolor icons.

Windows accepts `tools/package_windows.ps1 [debug|release] --local` and the
standard `-Local` spelling. The switch's `-local` alias handles `powershell.exe
-File`; positional normalization also handles direct PowerShell calls with
`--local`. Preserve debug/release validation and reject extra profiles or unknown
arguments before building. Keep PowerShell 5.1 compatibility. Local mode does
not invoke the Python source-packaging helper.
Cargo still reports the executable path; the archive is placed beside it,
including configured target-directory paths.

Both modes build fresh staging directories and publish only complete ZIP files.
Failures preserve the previous archive. Fresh staging excludes stale distribution
metadata from local packages and excludes the local marker from distributions.
Linux uses its existing archive move; Windows uses the existing file replace or
move operation. Temporary directories are removed after success or failure.

POSIX fixtures cover both Linux modes, contents, native-target selection and
failure preservation. PowerShell fixtures exercise the Windows script with fake
Cargo output, a source helper that fails if called locally, argument spellings,
profiles, notices, omitted source metadata and failure preservation. PowerShell
fixtures on macOS do not replace native Windows PowerShell 5.1/MSVC checks.
