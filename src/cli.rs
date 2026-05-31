use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use schemars::schema_for;
use serde::Serialize;

use crate::backends;
use crate::contract::AppshotResult;
use crate::contract::CaptureTarget;
use crate::contract::ImageDetail;
use crate::contract::ImageInfo;
use crate::polish;
use crate::text;
use crate::util;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[command(name = "appshots")]
#[command(about = "Agent-neutral app screenshot capture CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Capture(CaptureArgs),
    Doctor(DoctorArgs),
    ListWindows(ListWindowsArgs),
    Gallery(GalleryArgs),
    Latest(LatestArgs),
    #[command(alias = "open")]
    Preview(PreviewArgs),
    Schema(SchemaArgs),
    CodexPayload(crate::codex::CodexPayloadArgs),
}

#[derive(Debug, Args)]
pub struct CaptureArgs {
    #[arg(long, value_enum, default_value = "active")]
    pub target: CaptureTarget,
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    #[arg(long)]
    pub window_id: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long, value_enum, default_value = "high")]
    pub detail: ImageDetail,
    #[arg(long, value_enum, default_value = "both")]
    pub presentation: PresentationMode,
    #[arg(long)]
    pub style_seed: Option<u64>,
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct ListWindowsArgs {
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct GalleryArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    #[arg(long)]
    pub root: Vec<PathBuf>,
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct LatestArgs {
    #[arg(long)]
    pub root: Vec<PathBuf>,
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct PreviewArgs {
    #[arg(value_name = "CAPTURE_DIR")]
    pub capture_dir: Option<PathBuf>,
    #[arg(long)]
    pub raw: bool,
    #[arg(long)]
    pub root: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[arg(long)]
    pub compact: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum PresentationMode {
    Raw,
    Card,
    Both,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "json" => Ok(Self::Json),
            _ => Err("only `json` is supported".to_string()),
        }
    }
}

pub fn capture(args: CaptureArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let output_dir = args.out_dir.unwrap_or_else(backends::default_output_dir);
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut backend = None;
    let mut window = None;
    let mut image = None;
    let mut presentation_image = None;
    let mut presentation_style = None;

    if let Err(err) = util::create_dir_all(&output_dir) {
        errors.push(err.to_string());
    } else {
        let image_path = output_dir.join("shot.png");
        match backends::capture(backends::CaptureRequest {
            target: args.target,
            output_path: &image_path,
            window_id: args.window_id.as_deref(),
            title: args.title.as_deref(),
            app: args.app.as_deref(),
        }) {
            Ok(success) => {
                backend = Some(success.backend);
                let frame_extents = success
                    .window
                    .as_ref()
                    .and_then(backends::frame_extents_for_window);
                window = success.window;
                match image_info(&image_path, args.detail) {
                    Ok(info) => {
                        if matches!(
                            args.presentation,
                            PresentationMode::Card | PresentationMode::Both
                        ) {
                            let card_path = output_dir.join("shot-card.png");
                            let style = args
                                .style_seed
                                .map(polish::style_from_seed)
                                .unwrap_or_else(polish::random_style);
                            match polish::render_codex_card(
                                &image_path,
                                &card_path,
                                frame_extents,
                                &style,
                            )
                            .and_then(|()| image_info(&card_path, args.detail))
                            {
                                Ok(card_info) => {
                                    presentation_style = Some(style.info());
                                    presentation_image = Some(card_info);
                                }
                                Err(err) => warnings.push(format!(
                                    "Codex-style presentation image could not be created: {err}"
                                )),
                            }
                        }
                        image = Some(info);
                    }
                    Err(err) => errors.push(err.to_string()),
                }
            }
            Err(err) => errors.push(err.to_string()),
        }
    }

    let text = if image.is_some() {
        text::extract(&output_dir, &mut warnings)
    } else {
        Default::default()
    };

    let result = AppshotResult {
        ok: image.is_some() && errors.is_empty(),
        version: VERSION.to_string(),
        created_at: Utc::now(),
        target: args.target,
        backend,
        output_dir: util::canonical_or_original(&output_dir),
        image,
        presentation_image,
        presentation_style,
        window,
        text,
        warnings,
        errors,
    };

    if let Ok(metadata) = serde_json::to_vec_pretty(&result) {
        let _ = util::write(&output_dir.join("metadata.json"), metadata);
    }
    print_json(&result)?;
    Ok(if result.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

pub fn doctor(_args: DoctorArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let report = backends::doctor_report();
    print_json(&report)?;
    Ok(if report.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

pub fn list_windows(_args: ListWindowsArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let result = backends::list_windows();
    print_json(&result)?;
    Ok(if result.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

pub fn gallery(args: GalleryArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let captures = find_captures(args.root, args.limit);
    print_json(&GalleryOutput { ok: true, captures })?;
    Ok(ExitCode::SUCCESS)
}

pub fn latest(args: LatestArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let capture = find_captures(args.root, 1).into_iter().next();
    let ok = capture.is_some();
    print_json(&LatestOutput { ok, capture })?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

pub fn preview(args: PreviewArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let capture_dir = match args.capture_dir {
        Some(path) => path,
        None => find_captures(args.root, 1)
            .into_iter()
            .next()
            .map(|capture| capture.output_dir)
            .ok_or("no appshot captures found")?,
    };
    let metadata = read_metadata(&capture_dir)?;
    let path = if args.raw {
        metadata
            .image
            .as_ref()
            .map(|image| image.path.clone())
            .ok_or("capture does not include a raw image")?
    } else {
        metadata
            .presentation_image
            .as_ref()
            .or(metadata.image.as_ref())
            .map(|image| image.path.clone())
            .ok_or("capture does not include an image")?
    };
    open_path(&path)?;
    Ok(ExitCode::SUCCESS)
}

pub fn schema(args: SchemaArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let schema = schema_for!(AppshotResult);
    if args.compact {
        println!("{}", serde_json::to_string(&schema)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&schema)?);
    }
    Ok(ExitCode::SUCCESS)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureSummary {
    output_dir: PathBuf,
    created_at: chrono::DateTime<Utc>,
    target: CaptureTarget,
    image: Option<ImageInfo>,
    presentation_image: Option<ImageInfo>,
    presentation_style: Option<crate::contract::PresentationStyleInfo>,
    window: Option<crate::contract::WindowInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GalleryOutput {
    ok: bool,
    captures: Vec<CaptureSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LatestOutput {
    ok: bool,
    capture: Option<CaptureSummary>,
}

fn find_captures(roots: Vec<PathBuf>, limit: usize) -> Vec<CaptureSummary> {
    let roots = if roots.is_empty() {
        vec![PathBuf::from("."), PathBuf::from("/tmp")]
    } else {
        roots
    };
    let mut captures = Vec::new();
    for root in roots {
        collect_captures(&root, &mut captures);
    }
    captures.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    captures.truncate(limit);
    captures
}

fn collect_captures(root: &Path, captures: &mut Vec<CaptureSummary>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("appshot") {
            continue;
        }
        if let Ok(metadata) = read_metadata(&path) {
            captures.push(CaptureSummary {
                output_dir: metadata.output_dir,
                created_at: metadata.created_at,
                target: metadata.target,
                image: metadata.image,
                presentation_image: metadata.presentation_image,
                presentation_style: metadata.presentation_style,
                window: metadata.window,
            });
        }
    }
}

fn read_metadata(capture_dir: &Path) -> Result<AppshotResult, Box<dyn std::error::Error>> {
    let bytes = util::read(&capture_dir.join("metadata.json"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn open_path(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.display().to_string();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()?;
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        if util::has_command("xdg-open") {
            util::desktop_command("xdg-open").arg(&path).spawn()?;
        } else if util::has_command("gio") {
            util::desktop_command("gio").args(["open", &path]).spawn()?;
        } else {
            return Err("no opener found: install xdg-open or gio".into());
        }
        Ok(())
    }
}

fn image_info(path: &std::path::Path, detail: ImageDetail) -> Result<ImageInfo, util::AppError> {
    let bytes = util::file_size(path)?;
    let (width, height) = util::png_dimensions(path)
        .map(|(width, height)| (Some(width), Some(height)))
        .unwrap_or((None, None));
    Ok(ImageInfo {
        path: util::canonical_or_original(path),
        width,
        height,
        bytes,
        mime: "image/png".to_string(),
        detail,
    })
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
