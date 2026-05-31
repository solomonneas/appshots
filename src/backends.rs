use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use crate::capture::windows;
use crate::contract::BackendInfo;
#[cfg(not(target_os = "windows"))]
use crate::contract::CapabilityStatus;
use crate::contract::CaptureTarget;
use crate::contract::DoctorReport;
#[cfg(not(target_os = "windows"))]
use crate::contract::Geometry;
#[cfg(not(target_os = "windows"))]
use crate::contract::HelperStatus;
#[cfg(not(target_os = "windows"))]
use crate::contract::SessionInfo;
use crate::contract::WindowInfo;
use crate::contract::WindowList;
#[cfg(not(target_os = "windows"))]
use crate::util;
use crate::util::AppError;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct CaptureRequest<'a> {
    pub target: CaptureTarget,
    pub output_path: &'a Path,
    pub window_id: Option<&'a str>,
    pub title: Option<&'a str>,
    pub app: Option<&'a str>,
}

pub struct CaptureSuccess {
    pub backend: BackendInfo,
    pub window: Option<WindowInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameExtents {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

pub fn doctor_report() -> DoctorReport {
    #[cfg(target_os = "windows")]
    {
        return windows::doctor_report(VERSION);
    }
    #[cfg(not(target_os = "windows"))]
    {
        linux_doctor_report()
    }
}

#[cfg(not(target_os = "windows"))]
fn linux_doctor_report() -> DoctorReport {
    let session = session_info();
    let helper_names = [
        "xdotool",
        "wmctrl",
        "grim",
        "gnome-screenshot",
        "import",
        "scrot",
        "spectacle",
        "python3",
        "gdbus",
        "timeout",
    ];
    let helpers = helper_names
        .iter()
        .map(|name| HelperStatus {
            name: (*name).to_string(),
            available: util::has_command(name),
            path: util::command_path(name),
        })
        .collect::<Vec<_>>();

    let has_gui = session.display.is_some() || session.wayland_display.is_some();
    let capabilities = vec![
        CapabilityStatus {
            name: "x11-active-window".to_string(),
            available: session.display.is_some()
                && util::has_command("xdotool")
                && capture_helper_available(),
            detail: "Requires DISPLAY, xdotool, and import/gnome-screenshot/scrot.".to_string(),
        },
        CapabilityStatus {
            name: "wayland-wlroots-screen".to_string(),
            available: session.wayland_display.is_some() && util::has_command("grim"),
            detail: "Requires WAYLAND_DISPLAY and grim. Active-window capture depends on compositor metadata and is not generic.".to_string(),
        },
        CapabilityStatus {
            name: "accessibility-text".to_string(),
            available: util::has_command("python3"),
            detail: "Best-effort AT-SPI text extraction via Python GI.".to_string(),
        },
    ];

    let mut warnings = Vec::new();
    if !has_gui {
        warnings.push("No DISPLAY or WAYLAND_DISPLAY is present in this process. Capture commands need a real desktop session.".to_string());
    }
    if session.wayland_display.is_some() {
        warnings.push("Wayland may require compositor-specific support or a portal prompt for non-screen captures.".to_string());
    }

    DoctorReport {
        ok: has_gui,
        version: VERSION.to_string(),
        session,
        helpers,
        capabilities,
        warnings,
    }
}

pub fn list_windows() -> WindowList {
    #[cfg(target_os = "windows")]
    {
        return windows::list_windows();
    }
    #[cfg(not(target_os = "windows"))]
    {
        linux_list_windows()
    }
}

#[cfg(not(target_os = "windows"))]
fn linux_list_windows() -> WindowList {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    if util::env_var("DISPLAY").is_none() {
        warnings.push("Window listing currently requires X11 DISPLAY and wmctrl.".to_string());
    }
    if !util::has_command("wmctrl") {
        errors.push("wmctrl is not installed or not on PATH.".to_string());
        return WindowList {
            ok: false,
            backend: None,
            windows: Vec::new(),
            warnings,
            errors,
        };
    }

    match parse_wmctrl_windows() {
        Ok(windows) => WindowList {
            ok: true,
            backend: Some(BackendInfo {
                name: "x11".to_string(),
                strategy: "wmctrl -lp".to_string(),
            }),
            windows,
            warnings,
            errors,
        },
        Err(err) => WindowList {
            ok: false,
            backend: None,
            windows: Vec::new(),
            warnings,
            errors: vec![err.to_string()],
        },
    }
}

pub fn capture(request: CaptureRequest<'_>) -> Result<CaptureSuccess, AppError> {
    #[cfg(target_os = "windows")]
    {
        return windows::capture(request);
    }
    #[cfg(not(target_os = "windows"))]
    {
        linux_capture(request)
    }
}

#[cfg(not(target_os = "windows"))]
fn linux_capture(request: CaptureRequest<'_>) -> Result<CaptureSuccess, AppError> {
    match request.target {
        CaptureTarget::Screen => capture_screen(request.output_path),
        CaptureTarget::Active => capture_active(request.output_path),
        CaptureTarget::Window => capture_window(request),
    }
}

#[cfg(not(target_os = "windows"))]
fn capture_active(output_path: &Path) -> Result<CaptureSuccess, AppError> {
    if util::env_var("DISPLAY").is_some() {
        let active_window = if util::has_command("xdotool") {
            active_x11_window().ok()
        } else {
            None
        };

        if let Some(window) = active_window.clone() {
            if capture_x11_window(window.id.as_deref(), output_path).is_ok() {
                return Ok(CaptureSuccess {
                    backend: BackendInfo {
                        name: "x11".to_string(),
                        strategy: "xdotool active window + ImageMagick import".to_string(),
                    },
                    window: Some(window),
                });
            }
        }

        if util::has_command("gnome-screenshot") {
            let output = output_path.display().to_string();
            util::run_status("gnome-screenshot", &["-w", "-f", &output])?;
            return Ok(CaptureSuccess {
                backend: BackendInfo {
                    name: "gnome-screenshot".to_string(),
                    strategy: "active window".to_string(),
                },
                window: active_window,
            });
        }

        if util::has_command("scrot") {
            let output = output_path.display().to_string();
            util::run_status("scrot", &["-u", &output])?;
            return Ok(CaptureSuccess {
                backend: BackendInfo {
                    name: "scrot".to_string(),
                    strategy: "active window".to_string(),
                },
                window: active_window,
            });
        }
    }

    if util::env_var("WAYLAND_DISPLAY").is_some() {
        return Err(AppError::Message(
            "active-window capture is not generic on Wayland; use --target screen or an X11 session".to_string(),
        ));
    }

    Err(AppError::Message(
        "no active-window backend is available; run `appshots doctor --format json`".to_string(),
    ))
}

#[cfg(not(target_os = "windows"))]
fn capture_screen(output_path: &Path) -> Result<CaptureSuccess, AppError> {
    let output = output_path.display().to_string();
    if util::env_var("WAYLAND_DISPLAY").is_some() && util::has_command("grim") {
        util::run_status("grim", &[&output])?;
        return Ok(CaptureSuccess {
            backend: BackendInfo {
                name: "wayland".to_string(),
                strategy: "grim screen capture".to_string(),
            },
            window: None,
        });
    }

    if util::has_command("gnome-screenshot") {
        util::run_status("gnome-screenshot", &["-f", &output])?;
        return Ok(CaptureSuccess {
            backend: BackendInfo {
                name: "gnome-screenshot".to_string(),
                strategy: "screen capture".to_string(),
            },
            window: None,
        });
    }

    if util::env_var("DISPLAY").is_some() && util::has_command("import") {
        util::run_status("import", &["-window", "root", &output])?;
        return Ok(CaptureSuccess {
            backend: BackendInfo {
                name: "x11".to_string(),
                strategy: "ImageMagick import root".to_string(),
            },
            window: None,
        });
    }

    if util::env_var("DISPLAY").is_some() && util::has_command("scrot") {
        util::run_status("scrot", &[&output])?;
        return Ok(CaptureSuccess {
            backend: BackendInfo {
                name: "scrot".to_string(),
                strategy: "screen capture".to_string(),
            },
            window: None,
        });
    }

    Err(AppError::Message(
        "no screen capture backend is available; install grim, gnome-screenshot, ImageMagick, or scrot".to_string(),
    ))
}

#[cfg(not(target_os = "windows"))]
fn capture_window(request: CaptureRequest<'_>) -> Result<CaptureSuccess, AppError> {
    let window = if let Some(id) = request.window_id {
        WindowInfo {
            id: Some(id.to_string()),
            ..WindowInfo::default()
        }
    } else {
        find_window(request.title, request.app)?
    };
    capture_x11_window(window.id.as_deref(), request.output_path)?;
    Ok(CaptureSuccess {
        backend: BackendInfo {
            name: "x11".to_string(),
            strategy: "wmctrl lookup + ImageMagick import".to_string(),
        },
        window: Some(window),
    })
}

#[cfg(not(target_os = "windows"))]
fn capture_x11_window(window_id: Option<&str>, output_path: &Path) -> Result<(), AppError> {
    let Some(window_id) = window_id else {
        return Err(AppError::Message("window id is required".to_string()));
    };
    if !util::has_command("import") {
        return Err(AppError::Message(
            "ImageMagick `import` is required for X11 window capture".to_string(),
        ));
    }
    let output = output_path.display().to_string();
    util::run_status("import", &["-window", window_id, &output])
}

pub fn frame_extents_for_window(window: &WindowInfo) -> Option<FrameExtents> {
    #[cfg(target_os = "windows")]
    {
        let _ = window;
        return None;
    }
    #[cfg(not(target_os = "windows"))]
    {
        linux_frame_extents_for_window(window)
    }
}

#[cfg(not(target_os = "windows"))]
fn linux_frame_extents_for_window(window: &WindowInfo) -> Option<FrameExtents> {
    let id = window.id.as_deref()?;
    if !util::has_command("xprop") {
        return None;
    }
    let output = util::run_output("xprop", &["-id", id, "_GTK_FRAME_EXTENTS"]).ok()?;
    parse_frame_extents(&output).or_else(|| {
        util::run_output("xprop", &["-id", id, "_NET_FRAME_EXTENTS"])
            .ok()
            .and_then(|output| parse_frame_extents(&output))
    })
}

#[cfg(not(target_os = "windows"))]
fn active_x11_window() -> Result<WindowInfo, AppError> {
    let id = util::run_output("xdotool", &["getactivewindow"])?;
    let title = util::run_output("xdotool", &["getwindowname", &id]).ok();
    let pid = util::run_output("xdotool", &["getwindowpid", &id])
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    let geometry = util::run_output("xdotool", &["getwindowgeometry", "--shell", &id])
        .ok()
        .and_then(|value| parse_xdotool_geometry(&value));
    let app_name = pid.and_then(util::proc_comm);
    Ok(WindowInfo {
        id: Some(id),
        title,
        app_name,
        pid,
        geometry,
    })
}

#[cfg(not(target_os = "windows"))]
fn find_window(title: Option<&str>, app: Option<&str>) -> Result<WindowInfo, AppError> {
    let windows = parse_wmctrl_windows()?;
    windows
        .into_iter()
        .find(|window| {
            let title_matches = title.is_none_or(|needle| {
                window
                    .title
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&needle.to_lowercase()))
            });
            let app_matches = app.is_none_or(|needle| {
                window
                    .app_name
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&needle.to_lowercase()))
            });
            title_matches && app_matches
        })
        .ok_or_else(|| AppError::Message("no matching window found".to_string()))
}

#[cfg(not(target_os = "windows"))]
fn parse_wmctrl_windows() -> Result<Vec<WindowInfo>, AppError> {
    let output = util::run_output("wmctrl", &["-lp"])?;
    let mut windows = Vec::new();
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let id = parts.next().map(str::to_string);
        let _desktop = parts.next();
        let pid = parts.next().and_then(|value| value.parse::<u32>().ok());
        let _host = parts.next();
        let title = parts.collect::<Vec<_>>().join(" ");
        let app_name = pid.and_then(util::proc_comm);
        windows.push(WindowInfo {
            id,
            title: (!title.is_empty()).then_some(title),
            app_name,
            pid,
            geometry: None,
        });
    }
    Ok(windows)
}

#[cfg(not(target_os = "windows"))]
fn parse_xdotool_geometry(output: &str) -> Option<Geometry> {
    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;
    let mut screen = None;
    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "X" => x = value.parse::<i32>().ok(),
            "Y" => y = value.parse::<i32>().ok(),
            "WIDTH" => width = value.parse::<u32>().ok(),
            "HEIGHT" => height = value.parse::<u32>().ok(),
            "SCREEN" => screen = value.parse::<i32>().ok(),
            _ => {}
        }
    }
    Some(Geometry {
        x: x?,
        y: y?,
        width: width?,
        height: height?,
        screen,
    })
}

#[cfg(not(target_os = "windows"))]
fn capture_helper_available() -> bool {
    util::has_command("import")
        || util::has_command("gnome-screenshot")
        || util::has_command("scrot")
}

#[cfg(not(target_os = "windows"))]
fn parse_frame_extents(output: &str) -> Option<FrameExtents> {
    let (_name, values) = output.split_once('=')?;
    let values = values
        .split(',')
        .map(str::trim)
        .filter_map(|value| value.parse::<u32>().ok())
        .collect::<Vec<_>>();
    let [left, right, top, bottom] = values.as_slice() else {
        return None;
    };
    Some(FrameExtents {
        left: *left,
        right: *right,
        top: *top,
        bottom: *bottom,
    })
}

#[cfg(not(target_os = "windows"))]
fn session_info() -> SessionInfo {
    SessionInfo {
        xdg_session_type: util::env_var("XDG_SESSION_TYPE"),
        wayland_display: util::env_var("WAYLAND_DISPLAY"),
        display: util::env_var("DISPLAY"),
        current_desktop: util::env_var("XDG_CURRENT_DESKTOP"),
        desktop_session: util::env_var("DESKTOP_SESSION"),
    }
}

pub fn default_output_dir() -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    PathBuf::from(format!("appshot-{stamp}"))
}

#[cfg(test)]
#[cfg(not(target_os = "windows"))]
mod tests {
    use super::FrameExtents;
    use super::parse_frame_extents;
    use super::parse_xdotool_geometry;
    use crate::contract::Geometry;

    #[test]
    fn parses_xdotool_shell_geometry() {
        let output = "WINDOW=6291459\nX=20\nY=30\nWIDTH=800\nHEIGHT=600\nSCREEN=0\n";

        let geometry = parse_xdotool_geometry(output);

        assert_eq!(
            geometry,
            Some(Geometry {
                x: 20,
                y: 30,
                width: 800,
                height: 600,
                screen: Some(0),
            })
        );
    }

    #[test]
    fn rejects_incomplete_xdotool_geometry() {
        let output = "WINDOW=6291459\nX=20\nWIDTH=800\nHEIGHT=600\n";

        assert_eq!(parse_xdotool_geometry(output), None);
    }

    #[test]
    fn parses_gtk_frame_extents() {
        let output = "_GTK_FRAME_EXTENTS(CARDINAL) = 61, 61, 55, 67";

        assert_eq!(
            parse_frame_extents(output),
            Some(FrameExtents {
                left: 61,
                right: 61,
                top: 55,
                bottom: 67,
            })
        );
    }
}
