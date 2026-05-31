param(
    [string]$AppshotsExe = "appshots.exe",
    [string]$OutRoot = "$env:TEMP\appshots-live-test",
    [ValidateSet("active", "screen", "window")]
    [string]$Target = "active",
    [int]$StyleSeed = 424242,
    [switch]$LaunchNotepad
)

$ErrorActionPreference = 'Continue'

Remove-Item -Recurse -Force $OutRoot -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null

$process = $null
if ($LaunchNotepad) {
    $process = Start-Process notepad.exe -PassThru
    Start-Sleep -Seconds 2
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class FocusNative {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
"@
    try {
        $process.Refresh()
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) {
            [void][FocusNative]::ShowWindow($process.MainWindowHandle, 9)
            [void][FocusNative]::SetForegroundWindow($process.MainWindowHandle)
        }
    } catch {
        $_ | Out-String | Set-Content -Path (Join-Path $OutRoot "focus-error.txt")
    }
    Start-Sleep -Seconds 1
}

$captureDir = Join-Path $OutRoot "capture"
$captureArgs = @(
    "capture",
    "--target",
    $Target,
    "--presentation",
    "both",
    "--style-seed",
    "$StyleSeed",
    "--out-dir",
    $captureDir,
    "--format",
    "json"
)
if ($LaunchNotepad -and $Target -eq "window") {
    $captureArgs += @("--app", "notepad")
}
& $AppshotsExe @captureArgs *> (Join-Path $OutRoot "combined.log")
$exitCode = $LASTEXITCODE
$exitCode | Set-Content -Path (Join-Path $OutRoot "exit.txt")

if ($process -and -not $process.HasExited) {
    try {
        [void]$process.CloseMainWindow()
        Start-Sleep -Seconds 1
    } catch {}
    try {
        if (-not $process.HasExited) {
            $process.Kill()
        }
    } catch {}
}

exit $exitCode
