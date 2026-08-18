use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const IMAGE_MODEL_ID: &str = "google/gemini-3.1-flash-image";
pub const IMAGE_MODEL_NAME: &str = "Nano Banana";
pub const MAX_PROMPT_CHARS: usize = 8_000;
pub const MAX_REFERENCE_BYTES: u64 = 12 * 1024 * 1024;
pub const MAX_REFERENCE_TOTAL_BYTES: u64 = 48 * 1024 * 1024;
pub const MAX_REFERENCES: usize = 14;
pub const SUPPORTED_ASPECT_RATIOS: &[&str] = &["1:1", "2:3", "3:2", "16:9"];
pub const SUPPORTED_RESOLUTIONS: &[&str] = &["1K", "2K", "4K"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub has_api_key: bool,
    pub model_id: &'static str,
    pub model_name: &'static str,
    pub supported_aspect_ratios: &'static [&'static str],
    pub supported_resolutions: &'static [&'static str],
    pub max_references: usize,
    pub max_reference_total_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSelection {
    pub token: String,
    pub file_name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub asset_path: String,
    pub thumbnail_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub request_id: String,
    pub prompt: String,
    #[serde(default)]
    pub reference_tokens: Vec<String>,
    pub aspect_ratio: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAsset {
    pub asset_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_path: Option<String>,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    #[serde(skip)]
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAttempt {
    pub id: String,
    pub prompt: String,
    pub model_id: String,
    pub status: String,
    pub settings: GenerationSettings,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub provider_name: Option<String>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub output: Option<HistoryAsset>,
    pub references: Vec<HistoryAsset>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCursor {
    pub created_at: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub attempts: Vec<HistoryAttempt>,
    pub next_cursor: Option<HistoryCursor>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteHistoryResult {
    pub deleted: bool,
}

impl GenerateRequest {
    pub fn validated_settings(&self) -> AppResult<GenerationSettings> {
        Ok(GenerationSettings {
            aspect_ratio: validate_setting(
                self.aspect_ratio.as_deref(),
                SUPPORTED_ASPECT_RATIOS,
                "aspect ratio",
            )?,
            resolution: validate_setting(
                self.resolution.as_deref(),
                SUPPORTED_RESOLUTIONS,
                "resolution",
            )?,
        })
    }
}

pub fn validate_reference_total_bytes(total_bytes: u64, maximum_bytes: u64) -> AppResult<()> {
    if total_bytes <= maximum_bytes {
        return Ok(());
    }

    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    let maximum_mb = maximum_bytes as f64 / (1024.0 * 1024.0);
    Err(AppError::new(
        ErrorKind::File,
        format!("Reference images total {total_mb:.1} MB; the maximum is {maximum_mb:.1} MB."),
        false,
    ))
}

fn validate_setting(
    requested: Option<&str>,
    supported: &[&str],
    setting_name: &str,
) -> AppResult<Option<String>> {
    match requested {
        None => Ok(None),
        Some(value) if supported.contains(&value) => Ok(Some(value.to_owned())),
        Some(_) => Err(AppError::new(
            ErrorKind::Validation,
            format!("Choose a supported {setting_name}."),
            false,
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationResult {
    pub attempt_id: String,
    pub asset_path: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub cost_usd: Option<f64>,
    pub duration_ms: i64,
    pub model_id: &'static str,
    pub provider_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelResult {
    pub cancelled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobAccepted {
    pub request_id: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationJobStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationJobEvent {
    pub request_id: String,
    pub status: GenerationJobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<GenerationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

#[derive(Debug, Clone)]
pub struct SelectedReference {
    pub path: std::path::PathBuf,
    pub thumbnail_path: std::path::PathBuf,
    pub mime_type: String,
    pub extension: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub cost_usd: Option<f64>,
    pub provider_name: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn request(aspect_ratio: Option<&str>, resolution: Option<&str>) -> GenerateRequest {
        GenerateRequest {
            request_id: "request-id".to_owned(),
            prompt: "A test image".to_owned(),
            reference_tokens: Vec::new(),
            aspect_ratio: aspect_ratio.map(ToOwned::to_owned),
            resolution: resolution.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn validates_supported_generation_settings() {
        let settings = request(Some("16:9"), Some("2K"))
            .validated_settings()
            .expect("settings");

        assert_eq!(settings.aspect_ratio.as_deref(), Some("16:9"));
        assert_eq!(settings.resolution.as_deref(), Some("2K"));
    }

    #[test]
    fn rejects_unknown_generation_settings() {
        let error = request(Some("5:7"), None)
            .validated_settings()
            .expect_err("unsupported aspect ratio should fail");

        assert_eq!(error.kind, ErrorKind::Validation);
    }

    #[test]
    fn rejects_reference_totals_above_the_available_budget() {
        validate_reference_total_bytes(MAX_REFERENCE_TOTAL_BYTES, MAX_REFERENCE_TOTAL_BYTES)
            .expect("the exact limit should be accepted");

        let error = validate_reference_total_bytes(
            MAX_REFERENCE_TOTAL_BYTES + 1,
            MAX_REFERENCE_TOTAL_BYTES,
        )
        .expect_err("a reference total above the limit should fail");

        assert_eq!(error.kind, ErrorKind::File);
        assert!(error.message.contains("48.0 MB"));
    }
}
