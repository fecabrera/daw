#Requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('debug', 'release')]
    [string]$Profile = 'debug'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($env:OS -ne 'Windows_NT') {
    throw 'Run this packager on Windows with the MSVC Rust toolchain.'
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$stagingDirectory = $null
Push-Location -LiteralPath $repositoryRoot
try {
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
    $executable = (Resolve-Path -LiteralPath $executable).Path
    $artifactDirectory = Split-Path -Parent $executable
    $archivePath = Join-Path $artifactDirectory 'DAW.zip'
    $stagingDirectory = Join-Path $artifactDirectory ('.daw-package-' + [guid]::NewGuid().ToString('N'))
    $packageDirectory = Join-Path $stagingDirectory 'DAW'
    $licenseDirectory = Join-Path $packageDirectory 'licenses'
    New-Item -ItemType Directory -Path $licenseDirectory | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $packageDirectory 'daw-desktop.exe')
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $packageDirectory 'LICENSE.txt')

    $namedNotices = @{
        'crates/ui/assets/fonts/OFL.txt' = 'Outfit-OFL.txt'
        'crates/ui/assets/fonts/Hack-LICENSE.txt' = 'Hack-LICENSE.txt'
        'crates/ui/assets/icons/LUCIDE-LICENSE.txt' = 'LUCIDE-LICENSE.txt'
        'crates/ui/assets/icons/EGUI-LUCIDE-LICENSE.txt' = 'EGUI-LUCIDE-LICENSE.txt'
    }
    foreach ($source in $namedNotices.Keys) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $licenseDirectory $namedNotices[$source])
    }
    foreach ($directory in @(
        'crates/ui/assets/licenses',
        'crates/media/assets/licenses',
        'crates/oscillator/assets/licenses',
        'crates/equalizer/assets/licenses',
        'crates/timestretch/licenses',
        'crates/separation/assets/licenses'
    )) {
        $notices = @(Get-ChildItem -LiteralPath $directory -Filter '*.txt' -File)
        if ($notices.Count -eq 0) {
            throw "No dependency notices found in $directory."
        }
        foreach ($notice in $notices) {
            Copy-Item -LiteralPath $notice.FullName -Destination $licenseDirectory -Force
        }
    }

    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $temporaryArchive = Join-Path $stagingDirectory 'DAW.zip'
    $archive = [System.IO.Compression.ZipFile]::Open(
        $temporaryArchive, [System.IO.Compression.ZipArchiveMode]::Create
    )
    try {
        foreach ($file in Get-ChildItem -LiteralPath $packageDirectory -File -Recurse) {
            # ZIP entry names use forward slashes, including on Windows PowerShell 5.1.
            $entryName = $file.FullName.Substring($stagingDirectory.Length + 1).Replace('\', '/')
            [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $archive, $file.FullName, $entryName, [System.IO.Compression.CompressionLevel]::Optimal
            )
        }
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
        $artifactPrefix = [System.IO.Path]::GetFullPath($artifactDirectory).TrimEnd('\') + '\'
        if (-not $stagingPath.StartsWith($artifactPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing to remove a staging directory outside the build output directory.'
        }
        Remove-Item -LiteralPath $stagingPath -Recurse -Force
    }
    Pop-Location
}
