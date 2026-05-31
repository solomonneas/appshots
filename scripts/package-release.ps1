$ErrorActionPreference = 'Stop'

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $RepoRoot

$CargoToml = Get-Content Cargo.toml -Raw
$VersionMatch = [regex]::Match($CargoToml, '(?m)^version = "([^"]+)"')
if (-not $VersionMatch.Success) {
    throw 'Could not read package version from Cargo.toml'
}
$Version = $VersionMatch.Groups[1].Value

$Target = switch -Regex ($env:PROCESSOR_ARCHITECTURE) {
    'ARM64|AARCH64' { 'aarch64-pc-windows-msvc'; break }
    default { 'x86_64-pc-windows-msvc' }
}

cargo build --release --bin appshots

$Stage = Join-Path 'dist' "appshots-$Version-$Target"
$Archive = "$Stage.zip"
Remove-Item -Recurse -Force $Stage, $Archive -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item 'target/release/appshots.exe' (Join-Path $Stage 'appshots.exe')
Copy-Item README.md, ROADMAP.md $Stage

Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Archive -Force
Write-Output $Archive
