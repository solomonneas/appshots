$ErrorActionPreference = 'Stop'

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $RepoRoot

cargo install --path . --force

@'
Installed appshots.

Try:
  appshots doctor --format json
  appshots capture --target active --presentation both --format json
  appshots latest
  appshots preview
'@ | Write-Host
