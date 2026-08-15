use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const IMAGE_MODEL_ID: &str = "google/gemini-3.1-flash-image";
pub const IMAGE_MODEL_NAME: &str = "Nano Banana";
pub const MAX_PROMPT_CHARS: usize = 8_000;
pub const MAX_REFERENCE_BYTES: u64 = 12 * 1024 * 1024;
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSelection {
    pub token: String,
    pub file_name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub asset_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub request_id: String,
    pub prompt: String,
    pub reference_token: Option<String>,
    pub aspect_ratio: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
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
            reference_token: None,
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
}
