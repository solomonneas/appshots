param(
    [string]$LiveScript = "$env:TEMP\windows-live-capture-test.ps1",
    [string]$AppshotsExe = "appshots.exe",
    [string]$OutRoot = "$env:TEMP\appshots-live-test",
    [ValidateSet("active", "screen", "window")]
    [string]$Target = "active",
    [string]$TaskName = "AppShotsLiveTest"
)

$ErrorActionPreference = 'Continue'

schtasks.exe /Delete /TN $TaskName /F 2>$null | Out-Null

$start = (Get-Date).AddMinutes(1).ToString("HH:mm")
$wrapper = Join-Path $env:TEMP "appshots-live-task.ps1"
@"
& '$LiveScript' -AppshotsExe '$AppshotsExe' -OutRoot '$OutRoot' -Target '$Target' -StyleSeed 424242 -LaunchNotepad
exit `$LASTEXITCODE
"@ | Set-Content -Path $wrapper -Encoding UTF8

$taskRun = 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' + $wrapper + '"'

schtasks.exe /Create /TN $TaskName /SC ONCE /ST $start /TR $taskRun /F /IT
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create scheduled task $TaskName"
}
schtasks.exe /Run /TN $TaskName
if ($LASTEXITCODE -ne 0) {
    throw "Failed to run scheduled task $TaskName"
}
