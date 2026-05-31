use std::path::Path;
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("failed to run `{program}`: {source}")]
    CommandSpawn {
        program: String,
        source: std::io::Error,
    },
    #[error("`{program}` exited with {status}: {stderr}")]
    CommandFailed {
        program: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("io error at `{path}`: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("image error at `{path}`: {source}")]
    Image {
        path: PathBuf,
        source: image::ImageError,
    },
}

pub fn command_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    #[cfg(target_os = "windows")]
    let candidates = windows_command_candidates(program);
    #[cfg(not(target_os = "windows"))]
    let candidates = vec![program.to_string()];
    std::env::split_paths(&path)
        .flat_map(|dir| candidates.iter().map(move |candidate| dir.join(candidate)))
        .find(|candidate| candidate.is_file())
}

pub fn has_command(program: &str) -> bool {
    command_path(program).is_some()
}

#[cfg(not(target_os = "windows"))]
pub fn run_output(program: &str, args: &[&str]) -> Result<String, AppError> {
    let mut command = desktop_command(program);
    let output = command
        .args(args)
        .output()
        .map_err(|source| AppError::CommandSpawn {
            program: program.to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(AppError::CommandFailed {
            program: program.to_string(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn run_status(program: &str, args: &[&str]) -> Result<(), AppError> {
    let mut command = desktop_command(program);
    let output = command
        .args(args)
        .output()
        .map_err(|source| AppError::CommandSpawn {
            program: program.to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(AppError::CommandFailed {
            program: program.to_string(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

pub fn create_dir_all(path: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), AppError> {
    std::fs::write(path, bytes).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read(path: &Path) -> Result<Vec<u8>, AppError> {
    std::fs::read(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn file_size(path: &Path) -> Result<u64, AppError> {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })
}

pub fn canonical_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn png_dimensions(path: &Path) -> Option<(u32, u32)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

#[cfg(not(target_os = "windows"))]
pub fn proc_comm(pid: u32) -> Option<String> {
    let path = PathBuf::from("/proc").join(pid.to_string()).join("comm");
    std::fs::read_to_string(path)
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(not(target_os = "windows"))]
pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| desktop_env_value(name))
}

#[cfg(not(target_os = "windows"))]
pub fn desktop_command(program: &str) -> Command {
    let mut command = Command::new(program);
    for (name, value) in desktop_env_pairs() {
        if std::env::var_os(&name).is_none() {
            command.env(name, value);
        }
    }
    command
}

#[cfg(not(target_os = "windows"))]
pub fn desktop_env_pairs() -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for name in [
        "DISPLAY",
        "XAUTHORITY",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "WAYLAND_DISPLAY",
    ] {
        if let Some(value) = std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| desktop_env_value(name))
        {
            pairs.push((name.to_string(), value));
        }
    }
    pairs
}

#[cfg(not(target_os = "windows"))]
fn desktop_env_value(name: &str) -> Option<String> {
    for pid in desktop_candidate_pids() {
        let path = PathBuf::from("/proc").join(pid).join("environ");
        let bytes = std::fs::read(path).ok()?;
        for entry in bytes.split(|byte| *byte == 0) {
            let entry = std::str::from_utf8(entry).ok()?;
            let (key, value) = entry.split_once('=')?;
            if key == name && !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn desktop_candidate_pids() -> Vec<String> {
    let uid = users_uid();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(pid) = file_name
            .to_str()
            .filter(|value| value.chars().all(|c| c.is_ascii_digit()))
        else {
            continue;
        };
        if !process_uid_matches(pid, uid.as_deref()) {
            continue;
        }
        let comm = std::fs::read_to_string(PathBuf::from("/proc").join(pid).join("comm"))
            .unwrap_or_default();
        let comm = comm.trim();
        if matches!(
            comm,
            "gnome-shell"
                | "gnome-session-b"
                | "gdm-x-session"
                | "plasmashell"
                | "kwin_wayland"
                | "Xorg"
                | "Xwayland"
        ) {
            candidates.push(pid.to_string());
        }
    }
    candidates.sort();
    candidates.reverse();
    candidates
}

#[cfg(not(target_os = "windows"))]
fn users_uid() -> Option<String> {
    std::env::var("UID")
        .ok()
        .or_else(|| run_plain_output("id", &["-u"]).ok())
}

#[cfg(not(target_os = "windows"))]
fn process_uid_matches(pid: &str, uid: Option<&str>) -> bool {
    let Some(uid) = uid else {
        return true;
    };
    let status_path = PathBuf::from("/proc").join(pid).join("status");
    let Ok(status) = std::fs::read_to_string(status_path) else {
        return false;
    };
    status.lines().any(|line| {
        line.strip_prefix("Uid:")
            .and_then(|rest| rest.split_whitespace().next())
            .is_some_and(|process_uid| process_uid == uid)
    })
}

#[cfg(not(target_os = "windows"))]
fn run_plain_output(program: &str, args: &[&str]) -> Result<String, AppError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|source| AppError::CommandSpawn {
            program: program.to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(AppError::CommandFailed {
            program: program.to_string(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "windows")]
fn windows_command_candidates(program: &str) -> Vec<String> {
    if Path::new(program).extension().is_some() {
        return vec![program.to_string()];
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    let mut candidates = vec![program.to_string()];
    candidates.extend(
        pathext
            .split(';')
            .map(str::trim)
            .filter(|ext| !ext.is_empty())
            .map(|ext| format!("{program}{ext}")),
    );
    candidates
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::png_dimensions;

    #[test]
    fn reads_png_dimensions_from_header() {
        let mut file = tempfile_file("appshots-png-dimensions");
        file.write_all(b"\x89PNG\r\n\x1a\n").unwrap();
        file.write_all(&13u32.to_be_bytes()).unwrap();
        file.write_all(b"IHDR").unwrap();
        file.write_all(&640u32.to_be_bytes()).unwrap();
        file.write_all(&480u32.to_be_bytes()).unwrap();
        file.flush().unwrap();

        assert_eq!(png_dimensions(file.path()), Some((640, 480)));
    }

    #[test]
    fn rejects_non_png_dimensions() {
        let mut file = tempfile_file("appshots-not-png");
        file.write_all(b"not a png").unwrap();
        file.flush().unwrap();

        assert_eq!(png_dimensions(file.path()), None);
    }

    fn tempfile_file(prefix: &str) -> tempfile::NamedTempFile {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempfile()
            .expect("create temp file")
    }
}
