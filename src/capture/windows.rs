#![cfg(target_os = "windows")]

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::backends::CaptureRequest;
use crate::backends::CaptureSuccess;
use crate::contract::BackendInfo;
use crate::contract::CapabilityStatus;
use crate::contract::CaptureTarget;
use crate::contract::DoctorReport;
use crate::contract::HelperStatus;
use crate::contract::SessionInfo;
use crate::contract::WindowInfo;
use crate::contract::WindowList;
use crate::util;
use crate::util::AppError;

const WINDOW_CAPTURE_SCRIPT: &str = r#"
param(
    [string]$Target,
    [string]$OutputPath,
    [string]$WindowId,
    [string]$Title,
    [string]$App
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
}

public static class NativeMethods {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll", SetLastError=true)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern int GetWindowTextLength(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr extraData);
}
'@

function Convert-Handle([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return [IntPtr]::Zero }
    $trimmed = $Value.Trim()
    if ($trimmed.StartsWith('0x', [StringComparison]::OrdinalIgnoreCase)) {
        return [IntPtr]([Convert]::ToInt64($trimmed.Substring(2), 16))
    }
    return [IntPtr]([Convert]::ToInt64($trimmed, 10))
}

function Get-WindowInfo([IntPtr]$Handle) {
    if ($Handle -eq [IntPtr]::Zero) { return $null }
    if (-not [NativeMethods]::IsWindowVisible($Handle)) { return $null }

    $titleBuilder = New-Object System.Text.StringBuilder 1024
    [void][NativeMethods]::GetWindowText($Handle, $titleBuilder, $titleBuilder.Capacity)
    $titleText = $titleBuilder.ToString()
    $titleValue = if ([string]::IsNullOrWhiteSpace($titleText)) { $null } else { $titleText }

    $pidValue = [uint32]0
    [void][NativeMethods]::GetWindowThreadProcessId($Handle, [ref]$pidValue)
    $process = if ($pidValue -gt 0) { Get-Process -Id $pidValue -ErrorAction SilentlyContinue } else { $null }

    $rect = New-Object RECT
    [void][NativeMethods]::GetWindowRect($Handle, [ref]$rect)
    $width = [Math]::Max(0, $rect.Right - $rect.Left)
    $height = [Math]::Max(0, $rect.Bottom - $rect.Top)

    [ordered]@{
        id = ('0x{0:X}' -f $Handle.ToInt64())
        title = $titleValue
        appName = if ($process) { $process.ProcessName } else { $null }
        pid = if ($pidValue -gt 0) { [uint32]$pidValue } else { $null }
        geometry = [ordered]@{
            x = [int]$rect.Left
            y = [int]$rect.Top
            width = [uint32]$width
            height = [uint32]$height
            screen = $null
        }
    }
}

function Get-TopLevelWindows {
    $windows = New-Object 'System.Collections.Generic.List[object]'
    $callback = [NativeMethods+EnumWindowsProc]{
        param([IntPtr]$Handle, [IntPtr]$ExtraData)
        if ([NativeMethods]::IsWindowVisible($Handle) -and -not [NativeMethods]::IsIconic($Handle)) {
            $info = Get-WindowInfo $Handle
            if ($info -and $info.title) {
                [void]$windows.Add($info)
            }
        }
        return $true
    }
    [void][NativeMethods]::EnumWindows($callback, [IntPtr]::Zero)
    $windows
}

function Find-Window {
    if (-not [string]::IsNullOrWhiteSpace($WindowId)) {
        $handle = Convert-Handle $WindowId
        $info = Get-WindowInfo $handle
        if (-not $info) { throw "window id was not found or is not visible: $WindowId" }
        return $info
    }

    $titleNeedle = if ([string]::IsNullOrWhiteSpace($Title)) { $null } else { $Title.ToLowerInvariant() }
    $appNeedle = if ([string]::IsNullOrWhiteSpace($App)) { $null } else { $App.ToLowerInvariant() }
    foreach ($window in Get-TopLevelWindows) {
        $titleMatches = -not $titleNeedle -or (($window.title + '').ToLowerInvariant().Contains($titleNeedle))
        $appMatches = -not $appNeedle -or (($window.appName + '').ToLowerInvariant().Contains($appNeedle))
        if ($titleMatches -and $appMatches) { return $window }
    }
    throw "no matching window found"
}

function Save-Bounds([int]$X, [int]$Y, [int]$Width, [int]$Height) {
    if ($Width -le 0 -or $Height -le 0) { throw "capture bounds are empty" }
    $bitmap = New-Object System.Drawing.Bitmap $Width, $Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($X, $Y, 0, 0, $bitmap.Size)
        $bitmap.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

$selectedWindow = $null
switch ($Target) {
    'screen' {
        $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
        Save-Bounds $bounds.X $bounds.Y $bounds.Width $bounds.Height
    }
    'active' {
        $handle = [NativeMethods]::GetForegroundWindow()
        $selectedWindow = Get-WindowInfo $handle
        if (-not $selectedWindow) { throw "no visible foreground window was found" }
        $g = $selectedWindow.geometry
        Save-Bounds $g.x $g.y $g.width $g.height
    }
    'window' {
        $selectedWindow = Find-Window
        $g = $selectedWindow.geometry
        Save-Bounds $g.x $g.y $g.width $g.height
    }
    default {
        throw "unsupported capture target: $Target"
    }
}

[ordered]@{ window = $selectedWindow } | ConvertTo-Json -Depth 8 -Compress
"#;

const LIST_WINDOWS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
}

public static class NativeMethods {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr extraData);
}
'@

function Get-WindowInfo([IntPtr]$Handle) {
    if ($Handle -eq [IntPtr]::Zero) { return $null }
    if (-not [NativeMethods]::IsWindowVisible($Handle) -or [NativeMethods]::IsIconic($Handle)) { return $null }

    $titleBuilder = New-Object System.Text.StringBuilder 1024
    [void][NativeMethods]::GetWindowText($Handle, $titleBuilder, $titleBuilder.Capacity)
    $titleText = $titleBuilder.ToString()
    if ([string]::IsNullOrWhiteSpace($titleText)) { return $null }

    $pidValue = [uint32]0
    [void][NativeMethods]::GetWindowThreadProcessId($Handle, [ref]$pidValue)
    $process = if ($pidValue -gt 0) { Get-Process -Id $pidValue -ErrorAction SilentlyContinue } else { $null }
    $rect = New-Object RECT
    [void][NativeMethods]::GetWindowRect($Handle, [ref]$rect)

    [ordered]@{
        id = ('0x{0:X}' -f $Handle.ToInt64())
        title = $titleText
        appName = if ($process) { $process.ProcessName } else { $null }
        pid = if ($pidValue -gt 0) { [uint32]$pidValue } else { $null }
        geometry = [ordered]@{
            x = [int]$rect.Left
            y = [int]$rect.Top
            width = [uint32]([Math]::Max(0, $rect.Right - $rect.Left))
            height = [uint32]([Math]::Max(0, $rect.Bottom - $rect.Top))
            screen = $null
        }
    }
}

$windows = New-Object 'System.Collections.Generic.List[object]'
$callback = [NativeMethods+EnumWindowsProc]{
    param([IntPtr]$Handle, [IntPtr]$ExtraData)
    $info = Get-WindowInfo $Handle
    if ($info) { [void]$windows.Add($info) }
    return $true
}
[void][NativeMethods]::EnumWindows($callback, [IntPtr]::Zero)
[ordered]@{ windows = @($windows.ToArray()) } | ConvertTo-Json -Depth 8 -Compress
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsCaptureOutput {
    window: Option<WindowInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsWindowListOutput {
    windows: Vec<WindowInfo>,
}

pub fn doctor_report(version: &str) -> DoctorReport {
    let powershell = util::command_path("powershell");
    let has_powershell = powershell.is_some();
    DoctorReport {
        ok: has_powershell,
        version: version.to_string(),
        session: SessionInfo {
            xdg_session_type: None,
            wayland_display: None,
            display: None,
            current_desktop: Some("Windows".to_string()),
            desktop_session: Some("windows".to_string()),
        },
        helpers: vec![HelperStatus {
            name: "powershell".to_string(),
            available: has_powershell,
            path: powershell,
        }],
        capabilities: vec![
            CapabilityStatus {
                name: "windows-active-window".to_string(),
                available: has_powershell,
                detail: "Uses Win32 foreground-window metadata and .NET screen capture."
                    .to_string(),
            },
            CapabilityStatus {
                name: "windows-screen".to_string(),
                available: has_powershell,
                detail: "Uses .NET virtual-screen capture.".to_string(),
            },
            CapabilityStatus {
                name: "accessibility-text".to_string(),
                available: has_powershell,
                detail: "Best-effort UI Automation text extraction.".to_string(),
            },
        ],
        warnings: if has_powershell {
            Vec::new()
        } else {
            vec!["PowerShell is required for the Windows backend.".to_string()]
        },
    }
}

pub fn list_windows() -> WindowList {
    match run_powershell_json::<WindowsWindowListOutput>(LIST_WINDOWS_SCRIPT, &[]) {
        Ok(result) => WindowList {
            ok: true,
            backend: Some(BackendInfo {
                name: "windows".to_string(),
                strategy: "EnumWindows + GetWindowText".to_string(),
            }),
            windows: result.windows,
            warnings: Vec::new(),
            errors: Vec::new(),
        },
        Err(err) => WindowList {
            ok: false,
            backend: None,
            windows: Vec::new(),
            warnings: Vec::new(),
            errors: vec![err.to_string()],
        },
    }
}

pub fn capture(request: CaptureRequest<'_>) -> Result<CaptureSuccess, AppError> {
    let target = match request.target {
        CaptureTarget::Active => "active",
        CaptureTarget::Screen => "screen",
        CaptureTarget::Window => "window",
    };
    let output = request.output_path.display().to_string();
    let args = vec![
        target.to_string(),
        output,
        request.window_id.unwrap_or_default().to_string(),
        request.title.unwrap_or_default().to_string(),
        request.app.unwrap_or_default().to_string(),
    ];
    let result: WindowsCaptureOutput = run_powershell_json(WINDOW_CAPTURE_SCRIPT, &args)?;
    Ok(CaptureSuccess {
        backend: BackendInfo {
            name: "windows".to_string(),
            strategy: match request.target {
                CaptureTarget::Screen => "SystemInformation virtual screen + CopyFromScreen",
                CaptureTarget::Active => "GetForegroundWindow + GetWindowRect + CopyFromScreen",
                CaptureTarget::Window => "EnumWindows lookup + GetWindowRect + CopyFromScreen",
            }
            .to_string(),
        },
        window: result.window,
    })
}

fn run_powershell_json<T>(script: &str, args: &[String]) -> Result<T, AppError>
where
    T: for<'de> Deserialize<'de>,
{
    let stdout = run_powershell(script, args)?;
    serde_json::from_str(stdout.trim()).map_err(|err| {
        AppError::Message(format!(
            "PowerShell backend returned invalid JSON: {err}: {}",
            stdout.trim()
        ))
    })
}

pub fn run_powershell(script: &str, args: &[String]) -> Result<String, AppError> {
    let script_path = temporary_script_path();
    util::write(&script_path, script.as_bytes())?;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .args(args)
        .output()
        .map_err(|source| AppError::CommandSpawn {
            program: "powershell".to_string(),
            source,
        });
    let _ = std::fs::remove_file(&script_path);
    let output = output?;
    if !output.status.success() {
        return Err(AppError::CommandFailed {
            program: "powershell".to_string(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn temporary_script_path() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("appshots-{stamp}.ps1"))
}

#[allow(dead_code)]
fn _assert_path_is_used(_: &Path) {}
