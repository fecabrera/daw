#Requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$Profile = 'debug',
    # Accept --local as well as the standard PowerShell -Local spelling.
    [Alias('-local')]
    [switch]$Local,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArguments
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# -File binds the double-hyphen alias; direct PowerShell invocation passes
# --local as a positional string. Normalize both without losing -Profile/-Local.
$profileSpecified = $PSBoundParameters.ContainsKey('Profile')
if ($Profile -eq '--local') {
    $Local = $true
    $Profile = 'debug'
    $profileSpecified = $false
}
foreach ($argument in $RemainingArguments) {
    if ($argument -eq '--local') {
        $Local = $true
    } elseif ($argument -in @('debug', 'release') -and -not $profileSpecified) {
        $Profile = $argument
        $profileSpecified = $true
    } else {
        throw 'Usage: tools/package_windows.ps1 [debug|release] [--local]'
    }
}
if ($Profile -notin @('debug', 'release')) {
    throw 'Usage: tools/package_windows.ps1 [debug|release] [--local]'
}

if ($env:OS -ne 'Windows_NT') {
    throw 'Run this packager on Windows with the MSVC Rust toolchain.'
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$stagingDirectory = $null
Push-Location -LiteralPath $repositoryRoot
try {
    if (-not $Local) {
        $sourceRevision = & python tools/package_sources.py --revision-only
        if ($LASTEXITCODE -ne 0) { throw 'Cannot identify a clean corresponding-source revision.' }
    }
    $cargoArguments = @('build', '--locked', '-p', 'daw-desktop', '--message-format=json-render-diagnostics')
    if ($Profile -eq 'release') {
        $cargoArguments += '--release'
    }
    $buildOutput = & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed (exit $LASTEXITCODE). The existing ZIP was not changed."
    }

    # Cargo reports the actual artifact path, including configured target directories.
    $executable = $null
    foreach ($line in $buildOutput) {
        $message = $line | ConvertFrom-Json
        if ($message.reason -eq 'compiler-artifact' -and
            $message.target.name -eq 'daw-desktop' -and $message.executable) {
            $executable = [string]$message.executable
        }
    }
    if (-not $executable -or [System.IO.Path]::GetExtension($executable) -ne '.exe') {
        throw 'Cargo did not produce a Windows daw-desktop.exe. Check the configured build target.'
    }
    $executable = (Resolve-Path -LiteralPath $executable).ProviderPath
    $artifactDirectory = Split-Path -Parent $executable
    $archivePath = Join-Path $artifactDirectory 'DAW.zip'
    $stagingDirectory = Join-Path $artifactDirectory ('.daw-package-' + [guid]::NewGuid().ToString('N'))
    $packageDirectory = Join-Path $stagingDirectory 'DAW'
    $licenseDirectory = Join-Path $packageDirectory 'licenses'
    New-Item -ItemType Directory -Path $licenseDirectory | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $packageDirectory 'daw-desktop.exe')
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $packageDirectory 'LICENSE.txt')

    foreach ($entry in Get-Content -LiteralPath 'tools/desktop_notices.txt') {
        $source, $name = $entry -split '\|', 2
        if ($name) {
            Copy-Item -LiteralPath $source -Destination (Join-Path $licenseDirectory $name)
        } else {
            $notices = @(Get-ChildItem -LiteralPath $source -Filter '*.txt' -File)
            if ($notices.Count -eq 0) {
                throw "No dependency notices found in $source."
            }
            foreach ($notice in $notices) {
                Copy-Item -LiteralPath $notice.FullName -Destination $licenseDirectory -Force
            }
        }
    }

    if ($Local) {
        @'
Local development build from the current working tree, including uncommitted changes.
No corresponding-source archive is included. This is not a distribution package.
Keep LICENSE.txt and the licenses directory with this app.
'@ | Set-Content -LiteralPath (Join-Path $packageDirectory 'LOCAL-BUILD.txt') -Encoding UTF8
    } else {
        & python tools/package_sources.py --revision $sourceRevision --profile $Profile `
            --destination $packageDirectory --binary (Join-Path $packageDirectory 'daw-desktop.exe')
        if ($LASTEXITCODE -ne 0) { throw 'Corresponding-source packaging failed. The existing ZIP was not changed.' }
    }

    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $temporaryArchive = Join-Path $stagingDirectory 'DAW.zip'
    $archive = [System.IO.Compression.ZipFile]::Open(
        $temporaryArchive, [System.IO.Compression.ZipArchiveMode]::Create
    )
    try {
        # Build names from the tree, not absolute-path offsets. Provider/short-path
        # spellings can differ from FileInfo.FullName on Windows.
        function Add-PackageDirectory {
            param(
                [System.IO.Compression.ZipArchive]$Archive,
                [string]$Directory,
                [string]$Prefix
            )
            foreach ($item in Get-ChildItem -LiteralPath $Directory) {
                $entryName = $Prefix + $item.Name
                if ($item.PSIsContainer) {
                    Add-PackageDirectory -Archive $Archive -Directory $item.FullName -Prefix ($entryName + '/')
                } else {
                    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                        $Archive, $item.FullName, $entryName, [System.IO.Compression.CompressionLevel]::Optimal
                    )
                }
            }
        }
        Add-PackageDirectory -Archive $archive -Directory $packageDirectory -Prefix 'DAW/'

    } finally {
        $archive.Dispose()
    }
    # Publish only the complete archive. Preserve the previous ZIP on packaging failure.
    if (Test-Path -LiteralPath $archivePath) {
        $previousArchive = Join-Path $stagingDirectory 'previous.zip'
        [System.IO.File]::Replace($temporaryArchive, $archivePath, $previousArchive)
    } else {
        [System.IO.File]::Move($temporaryArchive, $archivePath)
    }
    Write-Output $archivePath
} finally {
    if ($stagingDirectory -and (Test-Path -LiteralPath $stagingDirectory)) {
        $stagingPath = [System.IO.Path]::GetFullPath($stagingDirectory)
        $separator = [System.IO.Path]::DirectorySeparatorChar
        $artifactPrefix = [System.IO.Path]::GetFullPath($artifactDirectory).TrimEnd($separator) + $separator
        if (-not $stagingPath.StartsWith($artifactPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing to remove a staging directory outside the build output directory.'
        }
        Remove-Item -LiteralPath $stagingPath -Recurse -Force
    }
    Pop-Location
}
