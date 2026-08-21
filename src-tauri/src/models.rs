use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};

pub const IMAGE_MODEL_ID: &str = "google/gemini-3.1-flash-image";
pub const IMAGE_MODEL_NAME: &str = "Nano Banana 2";
pub const IMAGE_MODEL_LITE_ID: &str = "google/gemini-3.1-flash-lite-image";
pub const IMAGE_MODEL_PRO_ID: &str = "google/gemini-3-pro-image";
pub const GPT_IMAGE_2_ID: &str = "openai/gpt-image-2";
pub const FLUX_2_KLEIN_ID: &str = "black-forest-labs/flux.2-klein-4b";
pub const FLUX_2_PRO_ID: &str = "black-forest-labs/flux.2-pro";
pub const FLUX_2_FLEX_ID: &str = "black-forest-labs/flux.2-flex";
pub const FLUX_2_MAX_ID: &str = "black-forest-labs/flux.2-max";
pub const MAX_PROMPT_CHARS: usize = 8_000;
pub const MAX_REFERENCE_BYTES: u64 = 12 * 1024 * 1024;
pub const MAX_REFERENCE_TOTAL_BYTES: u64 = 48 * 1024 * 1024;
pub const MAX_REFERENCES: usize = 14;
pub const SUPPORTED_ASPECT_RATIOS: &[&str] = &["1:1", "2:3", "3:2", "16:9"];
pub const SUPPORTED_RESOLUTIONS: &[&str] = &["1K", "2K", "4K"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageModel {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub description: String,
    pub available: bool,
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub supported_aspect_ratios: Vec<String>,
    pub supported_resolutions: Vec<String>,
    pub max_references: usize,
}

pub fn fallback_image_models() -> Vec<ImageModel> {
    vec![
        ImageModel {
            id: IMAGE_MODEL_LITE_ID.to_owned(),
            name: "Nano Banana 2 Lite".to_owned(),
            provider: "Google".to_owned(),
            description: "Fast, low-cost 1K drafts and quick iterations.".to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: vec!["1K".to_owned()],
            max_references: MAX_REFERENCES,
        },
        ImageModel {
            id: IMAGE_MODEL_ID.to_owned(),
            name: IMAGE_MODEL_NAME.to_owned(),
            provider: "Google".to_owned(),
            description: "Best everyday balance of speed and quality.".to_owned(),
            available: true,
            is_default: true,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: owned_values(SUPPORTED_RESOLUTIONS),
            max_references: MAX_REFERENCES,
        },
        ImageModel {
            id: IMAGE_MODEL_PRO_ID.to_owned(),
            name: "Nano Banana Pro".to_owned(),
            provider: "Google".to_owned(),
            description: "Higher detail for polished, precision-sensitive work.".to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: owned_values(SUPPORTED_RESOLUTIONS),
            max_references: MAX_REFERENCES,
        },
        ImageModel {
            id: GPT_IMAGE_2_ID.to_owned(),
            name: "GPT Image 2".to_owned(),
            provider: "OpenAI".to_owned(),
            description: "Strong at faithful edits, references, and complex instructions."
                .to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: Vec::new(),
            max_references: MAX_REFERENCES,
        },
        ImageModel {
            id: FLUX_2_KLEIN_ID.to_owned(),
            name: "FLUX.2 Klein 4B".to_owned(),
            provider: "Black Forest Labs".to_owned(),
            description: "Fast, affordable exploration and many variations.".to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: Vec::new(),
            max_references: 4,
        },
        ImageModel {
            id: FLUX_2_PRO_ID.to_owned(),
            name: "FLUX.2 Pro".to_owned(),
            provider: "Black Forest Labs".to_owned(),
            description: "Balanced production quality without the cost of Max.".to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: Vec::new(),
            max_references: 8,
        },
        ImageModel {
            id: FLUX_2_FLEX_ID.to_owned(),
            name: "FLUX.2 Flex".to_owned(),
            provider: "Black Forest Labs".to_owned(),
            description: "Typography, fine detail, and greater creative control.".to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: Vec::new(),
            max_references: 8,
        },
        ImageModel {
            id: FLUX_2_MAX_ID.to_owned(),
            name: "FLUX.2 Max".to_owned(),
            provider: "Black Forest Labs".to_owned(),
            description: "Highest FLUX quality and consistency for final work.".to_owned(),
            available: true,
            is_default: false,
            unavailable_reason: None,
            supported_aspect_ratios: owned_values(SUPPORTED_ASPECT_RATIOS),
            supported_resolutions: Vec::new(),
            max_references: 8,
        },
    ]
}

fn owned_values(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

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
    pub model_id: String,
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
    pub fn validated_settings(&self, model: &ImageModel) -> AppResult<GenerationSettings> {
        Ok(GenerationSettings {
            aspect_ratio: validate_setting(
                self.aspect_ratio.as_deref(),
                &model.supported_aspect_ratios,
                "aspect ratio",
            )?,
            resolution: validate_setting(
                self.resolution.as_deref(),
                &model.supported_resolutions,
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
    supported: &[String],
    setting_name: &str,
) -> AppResult<Option<String>> {
    match requested {
        None => Ok(None),
        Some(value) if supported.iter().any(|candidate| candidate == value) => {
            Ok(Some(value.to_owned()))
        }
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
    pub model_id: String,
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

    fn request(
        model_id: &str,
        aspect_ratio: Option<&str>,
        resolution: Option<&str>,
    ) -> GenerateRequest {
        GenerateRequest {
            request_id: "request-id".to_owned(),
            model_id: model_id.to_owned(),
            prompt: "A test image".to_owned(),
            reference_tokens: Vec::new(),
            aspect_ratio: aspect_ratio.map(ToOwned::to_owned),
            resolution: resolution.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn validates_supported_generation_settings() {
        let model = fallback_image_models()
            .into_iter()
            .find(|model| model.id == IMAGE_MODEL_ID)
            .expect("default model");
        let settings = request(IMAGE_MODEL_ID, Some("16:9"), Some("2K"))
            .validated_settings(&model)
            .expect("settings");

        assert_eq!(settings.aspect_ratio.as_deref(), Some("16:9"));
        assert_eq!(settings.resolution.as_deref(), Some("2K"));
    }

    #[test]
    fn rejects_unknown_generation_settings() {
        let model = fallback_image_models()
            .into_iter()
            .find(|model| model.id == IMAGE_MODEL_ID)
            .expect("default model");
        let error = request(IMAGE_MODEL_ID, Some("5:7"), None)
            .validated_settings(&model)
            .expect_err("unsupported aspect ratio should fail");

        assert_eq!(error.kind, ErrorKind::Validation);
    }

    #[test]
    fn lite_model_rejects_higher_resolutions() {
        let model = fallback_image_models()
            .into_iter()
            .find(|model| model.id == IMAGE_MODEL_LITE_ID)
            .expect("lite model");
        let error = request(IMAGE_MODEL_LITE_ID, None, Some("2K"))
            .validated_settings(&model)
            .expect_err("lite must remain 1K-only");

        assert_eq!(error.kind, ErrorKind::Validation);
    }

    #[test]
    fn includes_gpt_image_as_a_non_default_openai_model() {
        let models = fallback_image_models();
        let model = models
            .iter()
            .find(|model| model.id == GPT_IMAGE_2_ID)
            .expect("GPT Image 2 model");

        assert_eq!(model.name, "GPT Image 2");
        assert_eq!(model.provider, "OpenAI");
        assert!(!model.is_default);
        assert!(model.supported_resolutions.is_empty());
        assert_eq!(model.max_references, MAX_REFERENCES);
        assert_eq!(models.iter().filter(|model| model.is_default).count(), 1);
    }

    #[test]
    fn includes_the_complete_flux_2_family() {
        let models = fallback_image_models();
        let flux_models = models
            .iter()
            .filter(|model| model.provider == "Black Forest Labs")
            .collect::<Vec<_>>();

        assert_eq!(flux_models.len(), 4);
        assert_eq!(
            flux_models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            [
                FLUX_2_KLEIN_ID,
                FLUX_2_PRO_ID,
                FLUX_2_FLEX_ID,
                FLUX_2_MAX_ID,
            ]
        );
        assert_eq!(flux_models[0].max_references, 4);
        assert!(
            flux_models[1..]
                .iter()
                .all(|model| model.max_references == 8)
        );
        assert!(
            flux_models
                .iter()
                .all(|model| model.supported_resolutions.is_empty())
        );
        assert!(flux_models.iter().all(|model| !model.is_default));
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
