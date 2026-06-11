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
#[command(name = "cloche")]
#[command(about = "Open-source desktop capture CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Capture(CaptureArgs),
    Polish(PolishArgs),
    Doctor(DoctorArgs),
    ListWindows(ListWindowsArgs),
    Gallery(GalleryArgs),
    Latest(LatestArgs),
    #[command(alias = "open")]
    Preview(PreviewArgs),
    Schema(SchemaArgs),
    CodexPayload(crate::codex::CodexPayloadArgs),
    Mcp(crate::mcp::McpArgs),
}

/// Style an existing image into a Cloche presentation card: rounded window,
/// layered shadows, and a vibrant gradient backdrop.
#[derive(Debug, Args)]
pub struct PolishArgs {
    /// Image to style (PNG, JPEG, or WebP).
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,
    /// Output card path; defaults to `<input>-card.png` next to the input.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Gradient palette; random when omitted.
    #[arg(long, value_parser = palette_name_parser())]
    pub palette: Option<String>,
    /// Seed for deterministic styling.
    #[arg(long)]
    pub style_seed: Option<u64>,
    #[arg(long, default_value = "json")]
    pub format: OutputFormat,
}

fn palette_name_parser() -> clap::builder::PossibleValuesParser {
    clap::builder::PossibleValuesParser::new(polish::palette_names())
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
    /// Write a self-contained HTML gallery to this path.
    #[arg(long)]
    pub html: Option<PathBuf>,
    /// HTML page title (used with --html).
    #[arg(long, default_value = "Cloche Shots")]
    pub title: String,
    /// Open the exported HTML gallery after writing it (requires --html).
    #[arg(long)]
    pub open: bool,
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

pub fn polish(args: PolishArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let result = run_polish(args);
    print_json(&result)?;
    Ok(if result.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_polish(args: PolishArgs) -> crate::contract::PolishResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut input_info = None;
    let mut card_info = None;
    let mut style_info = None;

    let card_path = args
        .out
        .clone()
        .unwrap_or_else(|| default_card_path(&args.input));
    if card_path.extension().and_then(|ext| ext.to_str()) != Some("png") {
        errors.push(format!(
            "output path {} must end in .png; cards are always PNG",
            card_path.display()
        ));
    } else {
        let seed = args.style_seed.unwrap_or_else(polish::random_seed);
        // The palette value is pre-validated by clap, so a miss here is a bug.
        let style = match args.palette.as_deref() {
            Some(name) => polish::style_with_palette(seed, name)
                .ok_or_else(|| format!("unknown palette: {name}")),
            None => Ok(polish::style_from_seed(seed)),
        };
        match style {
            Ok(style) => {
                let parent_ready = card_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map_or(Ok(()), util::create_dir_all);
                match parent_ready.map_err(|err| err.to_string()).and_then(|()| {
                    polish::render_codex_card(&args.input, &card_path, None, &style)
                        .map_err(|err| err.to_string())
                }) {
                    Ok(()) => {
                        style_info = Some(style.info());
                        match image_info(&args.input, ImageDetail::Original) {
                            Ok(info) => input_info = Some(info),
                            Err(err) => warnings.push(err.to_string()),
                        }
                        match image_info(&card_path, ImageDetail::Original) {
                            Ok(info) => card_info = Some(info),
                            Err(err) => errors.push(err.to_string()),
                        }
                    }
                    Err(err) => errors.push(err),
                }
            }
            Err(err) => errors.push(err),
        }
    }

    crate::contract::PolishResult {
        ok: card_info.is_some() && errors.is_empty(),
        version: VERSION.to_string(),
        created_at: Utc::now(),
        input: input_info,
        card: card_info,
        presentation_style: style_info,
        warnings,
        errors,
    }
}

/// Sibling path with a `-card.png` suffix: `shot.png` -> `shot-card.png`.
fn default_card_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("shot");
    input.with_file_name(format!("{stem}-card.png"))
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
    let mut html_path = None;
    if let Some(path) = args.html.as_ref() {
        let html = render_gallery_html(&args.title, &captures);
        util::write(path, html)?;
        let written = util::canonical_or_original(path);
        if args.open {
            open_path(&written)?;
        }
        html_path = Some(written);
    }
    print_json(&GalleryOutput {
        ok: true,
        html_path,
        captures,
    })?;
    Ok(ExitCode::SUCCESS)
}

fn render_gallery_html(title: &str, captures: &[CaptureSummary]) -> String {
    let items: Vec<crate::html::GalleryItem> = captures
        .iter()
        .map(|capture| {
            let image = capture
                .presentation_image
                .as_ref()
                .or(capture.image.as_ref());
            let window_title = capture
                .window
                .as_ref()
                .and_then(|window| window.title.clone().or_else(|| window.app_name.clone()))
                .unwrap_or_else(|| "Untitled capture".to_string());
            crate::html::GalleryItem {
                title: window_title,
                app: capture
                    .window
                    .as_ref()
                    .and_then(|window| window.app_name.clone()),
                target: format!("{:?}", capture.target).to_lowercase(),
                width: image.and_then(|info| info.width),
                height: image.and_then(|info| info.height),
                created_at: capture.created_at.to_rfc3339(),
                output_dir: capture.output_dir.display().to_string(),
                image_path: image.map(|info| info.path.as_path()),
            }
        })
        .collect();
    crate::html::render(title, &items)
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
            .ok_or("no Cloche captures found")?,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    html_path: Option<PathBuf>,
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
    captures.sort_by_key(|capture| std::cmp::Reverse(capture.created_at));
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
        if !name.starts_with("cloche-shot") && !name.starts_with("appshot") {
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
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cloche-polish-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_test_image(path: &Path, width: u32, height: u32) {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([180, 180, 180, 255]));
        image.save(path).expect("write test image");
    }

    #[test]
    fn default_card_path_appends_card_suffix() {
        assert_eq!(
            default_card_path(Path::new("/tmp/example/shot.png")),
            PathBuf::from("/tmp/example/shot-card.png")
        );
        assert_eq!(
            default_card_path(Path::new("diff.jpg")),
            PathBuf::from("diff-card.png")
        );
        assert_eq!(
            default_card_path(Path::new("/tmp/noext")),
            PathBuf::from("/tmp/noext-card.png")
        );
    }

    #[test]
    fn polish_writes_card_next_to_input() {
        let dir = temp_dir("default-out");
        let input = dir.join("shot.png");
        write_test_image(&input, 320, 240);
        let result = run_polish(PolishArgs {
            input: input.clone(),
            out: None,
            palette: None,
            style_seed: Some(7),
            format: OutputFormat::Json,
        });
        assert!(result.ok, "errors: {:?}", result.errors);
        let card = result.card.expect("card info");
        assert_eq!(
            card.path,
            util::canonical_or_original(&dir.join("shot-card.png"))
        );
        assert!(card.path.exists());
        // The card adds padding around the input pixels.
        assert!(card.width.expect("width") > 320);
        assert!(card.height.expect("height") > 240);
        let style = result.presentation_style.expect("style info");
        assert_eq!(style.seed, 7);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn polish_honors_palette_and_out_path() {
        let dir = temp_dir("palette-out");
        let input = dir.join("input.png");
        write_test_image(&input, 200, 160);
        let out = dir.join("nested").join("styled.png");
        let result = run_polish(PolishArgs {
            input,
            out: Some(out.clone()),
            palette: Some("aurora-teal".to_string()),
            style_seed: Some(11),
            format: OutputFormat::Json,
        });
        assert!(result.ok, "errors: {:?}", result.errors);
        assert_eq!(
            result.presentation_style.expect("style").palette,
            "aurora-teal"
        );
        assert!(out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn polish_rejects_non_png_output() {
        let dir = temp_dir("bad-out");
        let input = dir.join("input.png");
        write_test_image(&input, 64, 64);
        let result = run_polish(PolishArgs {
            input,
            out: Some(dir.join("card.jpg")),
            palette: None,
            style_seed: Some(3),
            format: OutputFormat::Json,
        });
        assert!(!result.ok);
        assert!(result.errors.iter().any(|err| err.contains(".png")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn polish_reports_missing_input() {
        let dir = temp_dir("missing-input");
        let result = run_polish(PolishArgs {
            input: dir.join("does-not-exist.png"),
            out: None,
            palette: None,
            style_seed: Some(5),
            format: OutputFormat::Json,
        });
        assert!(!result.ok);
        assert!(!result.errors.is_empty());
        assert!(result.card.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
