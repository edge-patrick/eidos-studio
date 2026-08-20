use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::{ImageFormat, ImageReader};
use reqwest::{Client, Response, StatusCode, Url};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, ErrorKind};
use crate::models::{
    AppResult, GeneratedImage, GenerationSettings, IMAGE_MODEL_PRO_ID, ImageModel,
    fallback_image_models,
};

const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1/";
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_RESPONSE_BYTES: usize = MAX_IMAGE_BYTES.div_ceil(3) * 4 + 1024 * 1024;
const MAX_METADATA_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
struct ImageResponse<'a> {
    #[serde(borrow)]
    data: Vec<ImageData<'a>>,
    #[serde(default)]
    usage: Option<Value>,
    #[serde(default)]
    provider_name: Option<Cow<'a, str>>,
}

#[derive(Debug, Deserialize)]
struct ImageData<'a> {
    #[serde(borrow)]
    b64_json: Cow<'a, str>,
    #[serde(default)]
    media_type: Option<Cow<'a, str>>,
}

#[derive(Debug, Deserialize)]
struct ImageModelsResponse {
    data: Vec<RemoteImageModel>,
}

#[derive(Debug, Deserialize)]
struct RemoteImageModel {
    id: String,
    supported_parameters: Option<RemoteSupportedParameters>,
}

#[derive(Debug, Deserialize)]
struct RemoteSupportedParameters {
    aspect_ratio: Option<EnumCapability>,
    resolution: Option<EnumCapability>,
    input_references: Option<RangeCapability>,
}

#[derive(Debug, Deserialize)]
struct EnumCapability {
    #[serde(default)]
    values: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RangeCapability {
    max: usize,
}

pub struct OpenRouterClient {
    http: Client,
    base_url: Url,
}

impl OpenRouterClient {
    pub fn new(http: Client, base_url: Url) -> Self {
        Self { http, base_url }
    }

    pub fn production() -> AppResult<Self> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(300))
            .user_agent("Eidos Studio/0.1.0")
            .build()
            .map_err(AppError::internal)?;
        let base_url = Url::parse(OPENROUTER_BASE_URL).map_err(AppError::internal)?;
        Ok(Self::new(http, base_url))
    }

    pub async fn validate_api_key(&self, api_key: &str) -> AppResult<()> {
        let response = self
            .http
            .get(self.endpoint("key")?)
            .bearer_auth(api_key)
            .header("X-Title", "Eidos Studio")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let bytes = read_limited(response, MAX_METADATA_RESPONSE_BYTES).await?;
        if !status.is_success() {
            return Err(parse_api_error(status, &bytes));
        }

        let body: Value = serde_json::from_slice(&bytes).map_err(|error| {
            AppError::new(
                ErrorKind::InvalidResponse,
                "OpenRouter returned unreadable API-key metadata.",
                true,
            )
            .with_details(error.to_string())
        })?;

        if !body.get("data").is_some_and(Value::is_object) {
            return Err(AppError::new(
                ErrorKind::InvalidResponse,
                "OpenRouter returned incomplete API-key metadata.",
                true,
            ));
        }

        Ok(())
    }

    pub async fn list_image_models(&self, api_key: &str) -> AppResult<Vec<ImageModel>> {
        let response = self
            .http
            .get(self.endpoint("images/models")?)
            .bearer_auth(api_key)
            .header("X-Title", "Eidos Studio")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let bytes = read_limited(response, MAX_METADATA_RESPONSE_BYTES).await?;
        if !status.is_success() {
            return Err(parse_api_error(status, &bytes));
        }

        let response: ImageModelsResponse = serde_json::from_slice(&bytes).map_err(|error| {
            AppError::new(
                ErrorKind::InvalidResponse,
                "OpenRouter returned an unreadable image-model catalog.",
                true,
            )
            .with_details(error.to_string())
        })?;
        let remote = response
            .data
            .into_iter()
            .map(|model| (model.id.clone(), model))
            .collect::<HashMap<_, _>>();

        fallback_image_models()
            .into_iter()
            .map(|mut model| {
                let Some(discovered) = remote.get(&model.id) else {
                    model.available = false;
                    model.unavailable_reason =
                        Some("Unavailable on OpenRouter. Try again later.".to_owned());
                    model.supported_aspect_ratios.clear();
                    model.supported_resolutions.clear();
                    model.max_references = 0;
                    return Ok(model);
                };
                let supported = discovered.supported_parameters.as_ref().ok_or_else(|| {
                    AppError::new(
                        ErrorKind::InvalidResponse,
                        "OpenRouter returned incomplete image-model capabilities.",
                        true,
                    )
                    .with_details(format!(
                        "Model {} omitted supported_parameters.",
                        discovered.id
                    ))
                })?;
                model.supported_aspect_ratios = intersect_values(
                    model.supported_aspect_ratios,
                    supported.aspect_ratio.as_ref(),
                );
                model.supported_resolutions =
                    intersect_values(model.supported_resolutions, supported.resolution.as_ref());
                if let Some(references) = supported.input_references.as_ref() {
                    model.max_references = model.max_references.min(references.max);
                } else {
                    model.max_references = 0;
                }
                Ok(model)
            })
            .collect()
    }

    pub async fn generate_image(
        &self,
        api_key: &str,
        model_id: &str,
        prompt: &str,
        settings: &GenerationSettings,
        references: &[(&str, &[u8])],
        cancellation: &CancellationToken,
    ) -> AppResult<GeneratedImage> {
        let mut payload = generation_payload(model_id, prompt, settings);

        if !references.is_empty() {
            payload["input_references"] = Value::Array(reference_payload(references));
        }

        let send = self
            .http
            .post(self.endpoint("images")?)
            .bearer_auth(api_key)
            .header("X-Title", "Eidos Studio")
            .json(&payload)
            .send();

        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(cancelled_error()),
            response = send => response.map_err(map_reqwest_error)?,
        };

        let status = response.status();
        let response_limit = if status.is_success() {
            MAX_IMAGE_RESPONSE_BYTES
        } else {
            MAX_METADATA_RESPONSE_BYTES
        };
        let response_bytes = tokio::select! {
            _ = cancellation.cancelled() => return Err(cancelled_error()),
            bytes = read_limited(response, response_limit) => bytes?,
        };

        if !status.is_success() {
            return Err(parse_api_error(status, &response_bytes));
        }

        let response: ImageResponse<'_> =
            serde_json::from_slice(&response_bytes).map_err(|error| {
                AppError::new(
                    ErrorKind::InvalidResponse,
                    "OpenRouter returned an unreadable image response.",
                    true,
                )
                .with_details(error.to_string())
            })?;

        let image = response.data.into_iter().next().ok_or_else(|| {
            AppError::new(
                ErrorKind::InvalidResponse,
                "OpenRouter completed the request without returning an image.",
                true,
            )
        })?;

        let max_encoded_bytes = MAX_IMAGE_BYTES.div_ceil(3) * 4;
        if image.b64_json.len() > max_encoded_bytes {
            return Err(AppError::new(
                ErrorKind::InvalidResponse,
                "The generated image was too large to process safely.",
                false,
            ));
        }

        let (data_url_mime, encoded) = split_data_url(&image.b64_json);
        let bytes = BASE64.decode(encoded).map_err(|error| {
            AppError::new(
                ErrorKind::InvalidResponse,
                "OpenRouter returned invalid image data.",
                true,
            )
            .with_details(error.to_string())
        })?;

        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(AppError::new(
                ErrorKind::InvalidResponse,
                "The generated image was too large to process safely.",
                false,
            ));
        }

        let format = image::guess_format(&bytes).map_err(|error| {
            AppError::new(
                ErrorKind::InvalidResponse,
                "OpenRouter returned an unsupported image format.",
                false,
            )
            .with_details(error.to_string())
        })?;
        let (mime_type, extension) = raster_format(format)?;
        let declared_mime = image.media_type.map(Cow::into_owned).or(data_url_mime);
        if declared_mime
            .as_deref()
            .is_some_and(|value| value != mime_type)
        {
            return Err(AppError::new(
                ErrorKind::InvalidResponse,
                "The generated image format did not match its declared type.",
                true,
            ));
        }

        let (width, height) = ImageReader::new(Cursor::new(bytes.as_slice()))
            .with_guessed_format()
            .map_err(AppError::file)?
            .into_dimensions()
            .map_err(AppError::file)?;

        let cost_usd = response.usage.as_ref().and_then(|usage| {
            usage
                .get("cost")
                .and_then(|cost| cost.as_f64().or_else(|| cost.as_str()?.parse().ok()))
        });

        Ok(GeneratedImage {
            bytes,
            mime_type: mime_type.to_owned(),
            extension: extension.to_owned(),
            width,
            height,
            cost_usd,
            provider_name: response.provider_name.map(Cow::into_owned),
        })
    }

    fn endpoint(&self, path: &str) -> AppResult<Url> {
        self.base_url.join(path).map_err(AppError::internal)
    }
}

async fn read_limited(mut response: Response, limit: usize) -> AppResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(response_too_large_error());
    }

    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(limit),
    );

    while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(response_too_large_error());
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}

fn response_too_large_error() -> AppError {
    AppError::new(
        ErrorKind::InvalidResponse,
        "OpenRouter returned a response that was too large to process safely.",
        false,
    )
}

fn intersect_values(configured: Vec<String>, discovered: Option<&EnumCapability>) -> Vec<String> {
    let Some(discovered) = discovered else {
        return Vec::new();
    };
    configured
        .into_iter()
        .filter(|value| discovered.values.contains(value))
        .collect()
}

fn generation_payload(model_id: &str, prompt: &str, settings: &GenerationSettings) -> Value {
    let mut payload = json!({
        "model": model_id,
        "prompt": prompt,
        "n": 1
    });

    if let Some(aspect_ratio) = settings.aspect_ratio.as_deref() {
        payload["aspect_ratio"] = json!(aspect_ratio);
    }
    if let Some(resolution) = settings.resolution.as_deref() {
        payload["resolution"] = json!(resolution);
    }
    if model_id == IMAGE_MODEL_PRO_ID && settings.resolution.as_deref() == Some("4K") {
        payload["provider"] = json!({
            "only": ["google-ai-studio/global"],
            "allow_fallbacks": false
        });
    }

    payload
}

fn reference_payload(references: &[(&str, &[u8])]) -> Vec<Value> {
    references
        .iter()
        .map(|(mime_type, bytes)| {
            json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{mime_type};base64,{}", BASE64.encode(bytes))
                }
            })
        })
        .collect()
}

fn split_data_url(value: &str) -> (Option<String>, &str) {
    if let Some(rest) = value.strip_prefix("data:")
        && let Some((metadata, encoded)) = rest.split_once(',')
    {
        let mime_type = metadata.split(';').next().map(ToOwned::to_owned);
        return (mime_type, encoded);
    }
    (None, value)
}

fn raster_format(format: ImageFormat) -> AppResult<(&'static str, &'static str)> {
    match format {
        ImageFormat::Png => Ok(("image/png", "png")),
        ImageFormat::Jpeg => Ok(("image/jpeg", "jpg")),
        ImageFormat::WebP => Ok(("image/webp", "webp")),
        _ => Err(AppError::new(
            ErrorKind::InvalidResponse,
            "The selected model returned an unsupported image format.",
            false,
        )),
    }
}

fn map_reqwest_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        return AppError::new(
            ErrorKind::Timeout,
            "The generation timed out before an image was returned.",
            true,
        );
    }

    AppError::new(
        ErrorKind::Network,
        "Eidos could not reach OpenRouter.",
        true,
    )
    .with_details(error.to_string())
}

fn cancelled_error() -> AppError {
    AppError::new(ErrorKind::Cancelled, "Generation cancelled.", true)
}

fn parse_api_error(status: StatusCode, bytes: &[u8]) -> AppError {
    let value = serde_json::from_slice::<Value>(bytes).unwrap_or(Value::Null);
    let provider = find_string(&value, &["provider_name", "provider"]);
    let reason = find_string(
        &value,
        &["finish_reason", "block_reason", "error_type", "code"],
    );
    let flagged_input = find_string(&value, &["flagged_input"]);
    let moderation_reasons = find_string_values(&value, "reasons");
    let upstream_message = find_string(&value, &["message"]);

    let mut details = vec![format!("HTTP {}", status.as_u16())];
    if let Some(provider) = provider.as_deref() {
        details.push(format!("Provider: {provider}"));
    }
    if let Some(reason) = reason.as_deref() {
        details.push(format!("Reason: {reason}"));
    }
    if !moderation_reasons.is_empty() {
        details.push(format!("Flagged: {}", moderation_reasons.join(", ")));
    }

    let reason_upper = reason.as_deref().unwrap_or_default().to_ascii_uppercase();
    let message_lower = upstream_message
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let error = if status == StatusCode::UNAUTHORIZED {
        AppError::new(
            ErrorKind::Authentication,
            "OpenRouter rejected this API key.",
            false,
        )
    } else if status == StatusCode::PAYMENT_REQUIRED
        || message_lower.contains("credit")
        || message_lower.contains("balance")
    {
        AppError::new(
            ErrorKind::InsufficientCredits,
            "This OpenRouter account does not have enough credit for the request.",
            false,
        )
    } else if reason_upper == "IMAGE_PROHIBITED_CONTENT" {
        AppError::new(
            ErrorKind::GeneratedCandidateBlocked,
            "The provider blocked the generated image candidate. Retrying may produce a different result.",
            true,
        )
    // A provider's raw error may surface this string code. Without a captured moderation stage,
    // input and output blocks cannot be distinguished here.
    } else if reason_upper == "MODERATION_BLOCKED" {
        AppError::new(
            ErrorKind::PromptBlocked,
            "The provider declined the prompt or reference images before generation.",
            false,
        )
    } else if message_lower.contains("rejected by the safety system") {
        AppError::new(
            ErrorKind::PromptBlocked,
            "The provider's safety system rejected this prompt.",
            false,
        )
    } else if reason_upper.contains("PROMPT") && reason_upper.contains("BLOCK") {
        AppError::new(
            ErrorKind::PromptBlocked,
            "The provider declined the prompt before generating an image.",
            false,
        )
    } else if reason_upper == "CONTENT_POLICY_VIOLATION"
        || reason_upper == "REFUSAL"
        || reason_upper.contains("SAFETY")
        || reason_upper.contains("RECITATION")
    {
        AppError::new(
            ErrorKind::ProviderSafetyRefusal,
            "The provider declined this generation request.",
            false,
        )
    } else if flagged_input.is_some() || !moderation_reasons.is_empty() {
        AppError::new(
            ErrorKind::PromptBlocked,
            "OpenRouter's moderation flagged this prompt or its reference images.",
            false,
        )
    } else if status == StatusCode::FORBIDDEN {
        AppError::new(
            ErrorKind::Authentication,
            "OpenRouter denied this request. Check the API key's permissions and guardrail settings.",
            false,
        )
    } else if status == StatusCode::BAD_REQUEST {
        AppError::new(
            ErrorKind::UnsupportedParameter,
            "OpenRouter could not accept this generation request.",
            false,
        )
    } else if status == StatusCode::REQUEST_TIMEOUT || status == StatusCode::GATEWAY_TIMEOUT {
        AppError::new(
            ErrorKind::Timeout,
            "The provider timed out before returning an image.",
            true,
        )
    } else if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        AppError::new(
            ErrorKind::ProviderUnavailable,
            "The image provider is temporarily unavailable.",
            true,
        )
    } else {
        AppError::new(
            ErrorKind::ProviderUnavailable,
            "OpenRouter could not complete this generation.",
            true,
        )
    };

    error.with_details(details.join(" · "))
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(Value::String(value)) = map.get(*key) {
                    return Some(value.clone());
                }
            }
            map.values().find_map(|value| find_string(value, keys))
        }
        Value::Array(values) => values.iter().find_map(|value| find_string(value, keys)),
        Value::String(raw) if raw.trim_start().starts_with('{') => {
            serde_json::from_str::<Value>(raw)
                .ok()
                .and_then(|value| find_string(&value, keys))
        }
        _ => None,
    }
}

fn find_string_values(value: &Value, key: &str) -> Vec<String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::Array(values)) = map.get(key) {
                let collected = values
                    .iter()
                    .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>();
                if !collected.is_empty() {
                    return collected;
                }
            }
            map.values()
                .map(|value| find_string_values(value, key))
                .find(|values| !values.is_empty())
                .unwrap_or_default()
        }
        Value::Array(values) => values
            .iter()
            .map(|value| find_string_values(value, key))
            .find(|values| !values.is_empty())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn mock_server(status: &str, body: &str) -> (Url, thread::JoinHandle<String>) {
        mock_server_with_length(status, body, body.len())
    }

    fn mock_server_with_length(
        status: &str,
        body: &str,
        declared_length: usize,
    ) -> (Url, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let address = listener.local_addr().expect("mock server address");
        let status = status.to_owned();
        let body = body.to_owned();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                declared_length
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            String::from_utf8(request).expect("request text")
        });

        (
            Url::parse(&format!("http://{address}/")).expect("mock base URL"),
            handle,
        )
    }

    #[test]
    fn includes_requested_size_settings_in_payload() {
        let payload = generation_payload(
            crate::models::IMAGE_MODEL_ID,
            "A wide editorial photograph",
            &GenerationSettings {
                aspect_ratio: Some("16:9".to_owned()),
                resolution: Some("2K".to_owned()),
            },
        );

        assert_eq!(payload["aspect_ratio"], "16:9");
        assert_eq!(payload["resolution"], "2K");
    }

    #[test]
    fn omits_provider_default_size_settings() {
        let payload = generation_payload(
            crate::models::IMAGE_MODEL_ID,
            "A provider-default image",
            &GenerationSettings {
                aspect_ratio: None,
                resolution: None,
            },
        );

        assert!(payload.get("aspect_ratio").is_none());
        assert!(payload.get("resolution").is_none());
    }

    #[test]
    fn pins_pro_4k_requests_to_the_compatible_endpoint() {
        let payload = generation_payload(
            IMAGE_MODEL_PRO_ID,
            "A detailed campaign image",
            &GenerationSettings {
                aspect_ratio: Some("16:9".to_owned()),
                resolution: Some("4K".to_owned()),
            },
        );

        assert_eq!(payload["provider"]["only"][0], "google-ai-studio/global");
        assert_eq!(payload["provider"]["allow_fallbacks"], false);
    }

    #[test]
    fn preserves_reference_order_in_payload() {
        let references = reference_payload(&[("image/png", b"first"), ("image/jpeg", b"second")]);

        assert_eq!(references.len(), 2);
        assert_eq!(
            references[0]["image_url"]["url"],
            "data:image/png;base64,Zmlyc3Q="
        );
        assert_eq!(
            references[1]["image_url"]["url"],
            "data:image/jpeg;base64,c2Vjb25k"
        );
    }

    #[test]
    fn identifies_generated_candidate_blocks() {
        let body = br#"{
            "error": {
                "metadata": {
                    "provider_name": "Google",
                    "finish_reason": "IMAGE_PROHIBITED_CONTENT"
                }
            }
        }"#;
        let error = parse_api_error(StatusCode::BAD_REQUEST, body);
        assert_eq!(error.kind, ErrorKind::GeneratedCandidateBlocked);
        assert!(error.retryable);
    }

    #[test]
    fn identifies_string_moderation_block_codes_as_prompt_blocks() {
        let body = br#"{
            "error": {
                "code": "moderation_blocked"
            }
        }"#;
        let error = parse_api_error(StatusCode::BAD_REQUEST, body);

        assert_eq!(error.kind, ErrorKind::PromptBlocked);
        assert!(!error.retryable);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("Reason: moderation_blocked"))
        );
    }

    #[test]
    fn identifies_observed_gpt_image_safety_refusals() {
        let body = br#"{
            "error": {
                "message": "Your request was rejected by the safety system. If you believe this is an error, contact us at help.openai.com and include the request ID req_ce759ec13e9f4e1fb9d2e229314b71c6.",
                "code": 400,
                "metadata": {"provider_name": "OpenAI"}
            }
        }"#;
        let error = parse_api_error(StatusCode::BAD_REQUEST, body);

        assert_eq!(error.kind, ErrorKind::PromptBlocked);
        assert!(!error.retryable);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("Provider: OpenAI"))
        );
    }

    #[test]
    fn identifies_openrouter_content_policy_error_types() {
        let body = br#"{
            "error": {
                "message": "The request was refused.",
                "code": 400,
                "metadata": {
                    "provider_name": "OpenAI",
                    "error_type": "content_policy_violation"
                }
            }
        }"#;
        let error = parse_api_error(StatusCode::BAD_REQUEST, body);

        assert_eq!(error.kind, ErrorKind::ProviderSafetyRefusal);
        assert!(!error.retryable);
    }

    #[test]
    fn identifies_openrouter_moderation_flags() {
        let body = br#"{
            "error": {
                "code": 403,
                "message": "Your input was flagged.",
                "metadata": {
                    "reasons": ["sexual"],
                    "flagged_input": "a prompt fragment",
                    "provider_name": "OpenAI",
                    "model_slug": "openai/gpt-image-2"
                }
            }
        }"#;
        let error = parse_api_error(StatusCode::FORBIDDEN, body);

        assert_eq!(error.kind, ErrorKind::PromptBlocked);
        assert!(!error.retryable);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("Flagged: sexual"))
        );
    }

    #[test]
    fn keeps_unrecognized_forbidden_errors_as_non_retryable_access_failures() {
        let body = br#"{
            "error": {
                "message": "Forbidden",
                "code": 403
            }
        }"#;
        let error = parse_api_error(StatusCode::FORBIDDEN, body);

        assert_eq!(error.kind, ErrorKind::Authentication);
        assert!(!error.retryable);
        assert!(error.message.contains("denied this request"));
    }

    #[test]
    fn parses_data_urls() {
        let (mime_type, encoded) = split_data_url("data:image/png;base64,abcd");
        assert_eq!(mime_type.as_deref(), Some("image/png"));
        assert_eq!(encoded, "abcd");
    }

    #[tokio::test]
    async fn validates_keys_with_the_dedicated_key_endpoint() {
        let (base_url, request) = mock_server("200 OK", r#"{"data":{"label":"test"}}"#);
        let client = OpenRouterClient::new(Client::new(), base_url);

        client
            .validate_api_key("sk-or-v1-test-key")
            .await
            .expect("valid key");

        let request = request.join().expect("mock request");
        assert!(request.starts_with("GET /key HTTP/1.1"));
        assert!(request.contains("authorization: Bearer sk-or-v1-test-key"));
    }

    #[tokio::test]
    async fn refreshes_curated_capabilities_from_the_image_catalog() {
        let body = r#"{"data":[
            {"id":"google/gemini-3.1-flash-lite-image","supported_parameters":{"resolution":{"type":"enum","values":["1K"]},"aspect_ratio":{"type":"enum","values":["1:1","16:9"]},"input_references":{"type":"range","min":0,"max":10}}},
            {"id":"google/gemini-3.1-flash-image","supported_parameters":{"resolution":{"type":"enum","values":["1K","2K","4K"]},"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"input_references":{"type":"range","min":0,"max":14}}},
            {"id":"google/gemini-3-pro-image","supported_parameters":{"resolution":{"type":"enum","values":["1K","2K","4K"]},"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"input_references":{"type":"range","min":0,"max":14}}},
            {"id":"openai/gpt-image-2","supported_parameters":{"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"input_references":{"type":"range","min":0,"max":16}}},
            {"id":"black-forest-labs/flux.2-klein-4b","supported_parameters":{"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"output_format":{"type":"enum","values":["png","jpeg"]},"input_references":{"type":"range","min":0,"max":4},"seed":{"type":"boolean"}}},
            {"id":"black-forest-labs/flux.2-pro","supported_parameters":{"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"output_format":{"type":"enum","values":["png","jpeg"]},"input_references":{"type":"range","min":0,"max":8},"seed":{"type":"boolean"}}},
            {"id":"black-forest-labs/flux.2-flex","supported_parameters":{"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"output_format":{"type":"enum","values":["png","jpeg"]},"input_references":{"type":"range","min":0,"max":8},"seed":{"type":"boolean"}}},
            {"id":"black-forest-labs/flux.2-max","supported_parameters":{"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"output_format":{"type":"enum","values":["png","jpeg"]},"input_references":{"type":"range","min":0,"max":8},"seed":{"type":"boolean"}}}
        ]}"#;
        let (base_url, request) = mock_server("200 OK", body);
        let client = OpenRouterClient::new(Client::new(), base_url);

        let models = client
            .list_image_models("sk-or-v1-test-key")
            .await
            .expect("catalog");

        assert_eq!(models.len(), 8);
        assert!(models.iter().all(|model| model.available));
        assert_eq!(models[0].supported_aspect_ratios, ["1:1", "16:9"]);
        assert_eq!(models[0].max_references, 10);
        assert!(models[3].supported_resolutions.is_empty());
        assert_eq!(models[3].max_references, crate::models::MAX_REFERENCES);
        assert_eq!(models[4].max_references, 4);
        assert!(models[5..].iter().all(|model| model.max_references == 8));
        assert!(
            models[4..]
                .iter()
                .all(|model| model.supported_resolutions.is_empty())
        );
        let request = request.join().expect("mock request");
        assert!(request.starts_with("GET /images/models HTTP/1.1"));
    }

    #[tokio::test]
    async fn marks_curated_models_missing_from_the_catalog_unavailable() {
        let body = r#"{"data":[
            {"id":"google/gemini-3.1-flash-image","supported_parameters":{"resolution":{"type":"enum","values":["1K","2K","4K"]},"aspect_ratio":{"type":"enum","values":["1:1","2:3","3:2","16:9"]},"input_references":{"type":"range","min":0,"max":14}}}
        ]}"#;
        let (base_url, request) = mock_server("200 OK", body);
        let client = OpenRouterClient::new(Client::new(), base_url);

        let models = client
            .list_image_models("sk-or-v1-test-key")
            .await
            .expect("catalog");

        assert_eq!(models.len(), 8);
        assert!(!models[0].available);
        assert_eq!(
            models[0].unavailable_reason.as_deref(),
            Some("Unavailable on OpenRouter. Try again later.")
        );
        assert!(models[0].supported_resolutions.is_empty());
        assert!(models[1].available);
        assert_eq!(models[1].supported_resolutions, ["1K", "2K", "4K"]);
        assert_eq!(models[1].max_references, 14);
        assert!(!models[2].available);
        assert!(!models[3].available);
        assert!(models[4..].iter().all(|model| !model.available));
        request.join().expect("mock request");
    }

    #[tokio::test]
    async fn clears_capabilities_the_catalog_does_not_report() {
        let body = r#"{"data":[
            {"id":"google/gemini-3.1-flash-lite-image","supported_parameters":{"resolution":{"type":"enum","values":["1K"]},"aspect_ratio":{"type":"enum","values":["1:1"]},"input_references":{"type":"range","min":0,"max":14}}},
            {"id":"google/gemini-3.1-flash-image","supported_parameters":{}},
            {"id":"google/gemini-3-pro-image","supported_parameters":{"resolution":{"type":"enum","values":["1K","2K","4K"]},"aspect_ratio":{"type":"enum","values":["1:1"]},"input_references":{"type":"range","min":0,"max":14}}},
            {"id":"openai/gpt-image-2","supported_parameters":{}}
        ]}"#;
        let (base_url, request) = mock_server("200 OK", body);
        let client = OpenRouterClient::new(Client::new(), base_url);

        let models = client
            .list_image_models("sk-or-v1-test-key")
            .await
            .expect("catalog");

        assert!(models[1].available);
        assert!(models[1].supported_aspect_ratios.is_empty());
        assert!(models[1].supported_resolutions.is_empty());
        assert_eq!(models[1].max_references, 0);
        assert!(models[3].available);
        assert!(models[3].supported_aspect_ratios.is_empty());
        assert!(models[3].supported_resolutions.is_empty());
        assert_eq!(models[3].max_references, 0);
        request.join().expect("mock request");
    }

    #[tokio::test]
    async fn rejects_curated_models_without_capability_metadata() {
        let body = r#"{"data":[
            {"id":"google/gemini-3.1-flash-image"}
        ]}"#;
        let (base_url, request) = mock_server("200 OK", body);
        let client = OpenRouterClient::new(Client::new(), base_url);

        let error = client
            .list_image_models("sk-or-v1-test-key")
            .await
            .expect_err("missing supported_parameters");

        assert_eq!(error.kind, ErrorKind::InvalidResponse);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("gemini-3.1-flash-image"))
        );
        request.join().expect("mock request");
    }

    #[tokio::test]
    async fn rejects_oversized_responses_before_buffering_them() {
        let (base_url, request) =
            mock_server_with_length("200 OK", r#"{"data":{}}"#, MAX_METADATA_RESPONSE_BYTES + 1);
        let client = OpenRouterClient::new(Client::new(), base_url);

        let error = client
            .validate_api_key("sk-or-v1-test-key")
            .await
            .expect_err("oversized response");

        assert_eq!(error.kind, ErrorKind::InvalidResponse);
        request.join().expect("mock request");
    }

    #[tokio::test]
    async fn decodes_a_generated_image_from_an_injected_server() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
        let body = format!(
            r#"{{"data":[{{"b64_json":"{png}","media_type":"image/png"}}],"usage":{{"cost":0.04}},"provider_name":"Test"}}"#
        );
        let (base_url, request) = mock_server("200 OK", &body);
        let client = OpenRouterClient::new(Client::new(), base_url);

        let image = client
            .generate_image(
                "sk-or-v1-test-key",
                crate::models::IMAGE_MODEL_ID,
                "A one-pixel image",
                &GenerationSettings {
                    aspect_ratio: Some("1:1".to_owned()),
                    resolution: Some("1K".to_owned()),
                },
                &[],
                &CancellationToken::new(),
            )
            .await
            .expect("generated image");

        assert_eq!((image.width, image.height), (1, 1));
        assert_eq!(image.mime_type, "image/png");
        assert_eq!(image.provider_name.as_deref(), Some("Test"));
        let request = request.join().expect("mock request");
        assert!(request.starts_with("POST /images HTTP/1.1"));
    }
}
