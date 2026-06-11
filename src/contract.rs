use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use clap::ValueEnum;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaptureTarget {
    Active,
    Screen,
    Window,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

impl std::fmt::Display for ImageDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            ImageDetail::Auto => "auto",
            ImageDetail::Low => "low",
            ImageDetail::High => "high",
            ImageDetail::Original => "original",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppshotResult {
    pub ok: bool,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub target: CaptureTarget,
    pub backend: Option<BackendInfo>,
    pub output_dir: PathBuf,
    pub image: Option<ImageInfo>,
    pub presentation_image: Option<ImageInfo>,
    pub presentation_style: Option<PresentationStyleInfo>,
    pub window: Option<WindowInfo>,
    pub text: TextInfo,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Result of styling an existing image into a presentation card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolishResult {
    pub ok: bool,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub input: Option<ImageInfo>,
    pub card: Option<ImageInfo>,
    pub presentation_style: Option<PresentationStyleInfo>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfo {
    pub name: String,
    pub strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: PathBuf,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: u64,
    pub mime: String,
    pub detail: ImageDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresentationStyleInfo {
    pub seed: u64,
    pub palette: String,
    pub padding: u32,
    pub corner_radius: u32,
    pub shadow_blur: f32,
    pub shadow_offset_y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: Option<String>,
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub pid: Option<u32>,
    pub geometry: Option<Geometry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub screen: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextInfo {
    pub available: bool,
    pub path: Option<PathBuf>,
    pub bytes: u64,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WindowList {
    pub ok: bool,
    pub backend: Option<BackendInfo>,
    pub windows: Vec<WindowInfo>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub ok: bool,
    pub version: String,
    pub session: SessionInfo,
    pub helpers: Vec<HelperStatus>,
    pub capabilities: Vec<CapabilityStatus>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub xdg_session_type: Option<String>,
    pub wayland_display: Option<String>,
    pub display: Option<String>,
    pub current_desktop: Option<String>,
    pub desktop_session: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HelperStatus {
    pub name: String,
    pub available: bool,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityStatus {
    pub name: String,
    pub available: bool,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::AppshotResult;
    use super::CaptureTarget;
    use super::TextInfo;

    #[test]
    fn appshot_result_uses_camel_case_wire_keys() {
        let result = AppshotResult {
            ok: false,
            version: "0.1.0".to_string(),
            created_at: Utc::now(),
            target: CaptureTarget::Active,
            backend: None,
            output_dir: "/tmp/appshot".into(),
            image: None,
            presentation_image: None,
            presentation_style: None,
            window: None,
            text: TextInfo::default(),
            warnings: Vec::new(),
            errors: vec!["no display".to_string()],
        };

        let value = serde_json::to_value(result).expect("serialize appshot result");

        assert!(value["createdAt"].is_string());
        assert_eq!(value["outputDir"], json!("/tmp/appshot"));
        assert_eq!(value["target"], json!("active"));
    }
}
