use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use serde_json::json;

use crate::contract::AppshotResult;
use crate::contract::ImageDetail;
use crate::util;

#[derive(Debug, Args)]
pub struct CodexPayloadArgs {
    #[arg(long)]
    pub thread_id: String,
    #[arg(value_name = "CAPTURE_DIR")]
    pub capture_dir: PathBuf,
    #[arg(long, default_value = "Cloche shot attached.")]
    pub message: String,
    #[arg(long, value_enum, default_value = "high")]
    pub detail: ImageDetail,
    #[arg(long)]
    pub compact: bool,
}

pub fn payload(args: CodexPayloadArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let payload = build_payload(&args)?;
    if args.compact {
        println!("{}", serde_json::to_string(&payload)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    }
    Ok(ExitCode::SUCCESS)
}

/// Pure payload assembly: reads capture metadata and returns the `turn/start`
/// JSON envelope without printing. Split out so the wire contract is testable.
fn build_payload(args: &CodexPayloadArgs) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let metadata_path = args.capture_dir.join("metadata.json");
    let metadata_bytes = util::read(&metadata_path)?;
    let metadata: AppshotResult = serde_json::from_slice(&metadata_bytes)?;
    if !metadata.ok {
        return Err(
            "capture metadata is not successful; refusing to emit a Codex image payload".into(),
        );
    }
    let image_path = metadata
        .image
        .as_ref()
        .map(|image| image.path.clone())
        .ok_or("capture metadata does not include an image")?;
    let image_path = util::canonical_or_original(&image_path);
    if !image_path.is_file() {
        return Err(format!("captured image does not exist: {}", image_path.display()).into());
    }

    let mut input = vec![json!({
        "type": "text",
        "text": args.message,
        "textElements": []
    })];

    if let Some(text_path) = metadata.text.path.as_ref()
        && let Ok(text_bytes) = util::read(text_path)
    {
        let text = String::from_utf8_lossy(&text_bytes).trim().to_string();
        if !text.is_empty() {
            input.push(json!({
                "type": "text",
                "text": format!("Available app text:\n{text}"),
                "textElements": []
            }));
        }
    }

    input.push(json!({
        "type": "localImage",
        "path": image_path,
        "detail": args.detail.to_string()
    }));

    Ok(json!({
        "method": "turn/start",
        "params": {
            "threadId": args.thread_id,
            "input": input
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::AppshotResult;
    use crate::contract::CaptureTarget;
    use crate::contract::ImageInfo;
    use crate::contract::TextInfo;
    use std::path::Path;

    fn temp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cloche-codex-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn image_info(path: &Path) -> ImageInfo {
        ImageInfo {
            path: path.to_path_buf(),
            width: Some(4),
            height: Some(4),
            bytes: 1,
            mime: "image/png".to_string(),
            detail: ImageDetail::High,
        }
    }

    fn write_fixture(dir: &Path, ok: bool, with_image: bool, app_text: Option<&str>) {
        let image_path = dir.join("shot.png");
        std::fs::write(&image_path, b"png").expect("write image");
        let text = match app_text {
            Some(text) => {
                let text_path = dir.join("text.txt");
                std::fs::write(&text_path, text).expect("write text");
                TextInfo {
                    available: true,
                    path: Some(text_path),
                    bytes: text.len() as u64,
                    source: Some("test".to_string()),
                }
            }
            None => TextInfo::default(),
        };
        let metadata = AppshotResult {
            ok,
            version: "0.0.0".to_string(),
            created_at: chrono::Utc::now(),
            target: CaptureTarget::Active,
            backend: None,
            output_dir: dir.to_path_buf(),
            image: with_image.then(|| image_info(&image_path)),
            presentation_image: None,
            presentation_style: None,
            window: None,
            text,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let bytes = serde_json::to_vec_pretty(&metadata).expect("serialize metadata");
        std::fs::write(dir.join("metadata.json"), bytes).expect("write metadata");
    }

    fn args(dir: &Path) -> CodexPayloadArgs {
        CodexPayloadArgs {
            thread_id: "thread-1".to_string(),
            capture_dir: dir.to_path_buf(),
            message: "Cloche shot attached.".to_string(),
            detail: ImageDetail::High,
            compact: false,
        }
    }

    #[test]
    fn payload_wraps_message_and_local_image() {
        let dir = temp_dir("basic");
        write_fixture(&dir, true, true, None);
        let payload = build_payload(&args(&dir)).expect("payload");
        assert_eq!(payload["method"], "turn/start");
        assert_eq!(payload["params"]["threadId"], "thread-1");
        let input = payload["params"]["input"].as_array().expect("input");
        assert_eq!(input.len(), 2);
        assert_eq!(input[0]["type"], "text");
        assert_eq!(input[0]["text"], "Cloche shot attached.");
        assert_eq!(input[1]["type"], "localImage");
        assert_eq!(input[1]["detail"], "high");
        assert!(
            input[1]["path"]
                .as_str()
                .expect("path")
                .ends_with("shot.png")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn payload_appends_app_text_when_present() {
        let dir = temp_dir("with-text");
        write_fixture(&dir, true, true, Some("Visible label"));
        let payload = build_payload(&args(&dir)).expect("payload");
        let input = payload["params"]["input"].as_array().expect("input");
        assert_eq!(input.len(), 3);
        let text = input[1]["text"].as_str().expect("text");
        assert!(text.starts_with("Available app text:"));
        assert!(text.contains("Visible label"));
        assert_eq!(input[2]["type"], "localImage");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_capture_metadata_is_refused() {
        let dir = temp_dir("not-ok");
        write_fixture(&dir, false, true, None);
        let error = build_payload(&args(&dir)).expect_err("refused");
        assert!(error.to_string().contains("not successful"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn metadata_without_image_is_refused() {
        let dir = temp_dir("no-image");
        write_fixture(&dir, true, false, None);
        let error = build_payload(&args(&dir)).expect_err("refused");
        assert!(error.to_string().contains("does not include an image"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
