use std::sync::Arc;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{OwnedSemaphorePermit, TryAcquireError};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::db::NewAsset;
use crate::error::{AppError, ErrorKind};
use crate::images::inspect_raster;
use crate::models::{
    AppResult, CancelResult, GenerateRequest, GenerationJobEvent, GenerationJobStatus,
    GenerationResult, GenerationSettings, IMAGE_MODEL_ID, JobAccepted, MAX_PROMPT_CHARS,
    MAX_REFERENCE_BYTES,
};
use crate::state::{AppState, locked};

pub const GENERATION_JOB_EVENT: &str = "generation-job-updated";
pub const MAX_CONCURRENT_GENERATIONS: usize = 2;

struct PreparedReference {
    mime_type: String,
    bytes: Vec<u8>,
}

struct GenerationTask {
    request_id: String,
    prompt: String,
    settings: GenerationSettings,
    reference: Option<PreparedReference>,
    _permit: OwnedSemaphorePermit,
}

pub async fn start(
    request: GenerateRequest,
    app: AppHandle,
    state: &AppState,
) -> AppResult<JobAccepted> {
    let request_id = validate_request_id(&request.request_id)?;
    let prompt = validate_prompt(&request.prompt)?;
    let settings = request.validated_settings()?;
    let settings_json = serde_json::to_string(&settings).map_err(AppError::internal)?;
    let permit = acquire_generation_slot(state.generation_slots.clone())?;

    let selected_reference = match request.reference_token.as_deref() {
        Some(token) => Some(
            locked(&state.references, "reference")?
                .get(token)
                .cloned()
                .ok_or_else(|| {
                    AppError::new(
                        ErrorKind::Validation,
                        "Choose the reference image again before generating.",
                        false,
                    )
                })?,
        ),
        None => None,
    };

    let prepared_reference = if let Some(reference) = selected_reference {
        let bytes = tokio::fs::read(&reference.path)
            .await
            .map_err(AppError::file)?;
        if bytes.len() as u64 > MAX_REFERENCE_BYTES {
            return Err(AppError::new(
                ErrorKind::File,
                "The selected reference image is now larger than 12 MB.",
                false,
            ));
        }

        let (mime_type, extension, width, height) = inspect_raster(&bytes)?;
        if mime_type != reference.mime_type
            || extension != reference.extension
            || width != reference.width
            || height != reference.height
        {
            return Err(AppError::new(
                ErrorKind::File,
                "The managed reference image changed unexpectedly. Choose it again.",
                false,
            ));
        }

        let stored = state.assets.store(extension, &bytes).await?;
        let asset_id = Uuid::new_v4().to_string();
        let database_request_id = request_id.clone();
        let database_prompt = prompt.clone();
        let database_settings = settings_json.clone();
        let content_hash = stored.content_hash;
        let storage_key = stored.storage_key;
        let database_mime_type = mime_type.to_owned();
        state
            .database
            .execute(move |database| {
                database.start_attempt(
                    &database_request_id,
                    &database_prompt,
                    IMAGE_MODEL_ID,
                    1,
                    &database_settings,
                    Some(NewAsset {
                        id: &asset_id,
                        content_hash: &content_hash,
                        role: "reference",
                        local_path: &storage_key,
                        mime_type: &database_mime_type,
                        width,
                        height,
                        sort_order: 0,
                    }),
                )
            })
            .await?;

        Some(PreparedReference {
            mime_type: mime_type.to_owned(),
            bytes,
        })
    } else {
        let database_request_id = request_id.clone();
        let database_prompt = prompt.clone();
        state
            .database
            .execute(move |database| {
                database.start_attempt(
                    &database_request_id,
                    &database_prompt,
                    IMAGE_MODEL_ID,
                    0,
                    &settings_json,
                    None,
                )
            })
            .await?;
        None
    };

    let cancellation = CancellationToken::new();
    locked(&state.jobs, "generation job")?.insert(request_id.clone(), cancellation.clone());

    let task = GenerationTask {
        request_id: request_id.clone(),
        prompt,
        settings,
        reference: prepared_reference,
        _permit: permit,
    };
    tauri::async_runtime::spawn(run(task, cancellation, app));

    Ok(JobAccepted { request_id })
}

pub fn cancel(request_id: String, state: &AppState) -> AppResult<CancelResult> {
    let cancellation = locked(&state.jobs, "generation job")?
        .get(&request_id)
        .cloned();
    if let Some(cancellation) = cancellation {
        cancellation.cancel();
        Ok(CancelResult { cancelled: true })
    } else {
        Ok(CancelResult { cancelled: false })
    }
}

async fn run(task: GenerationTask, cancellation: CancellationToken, app: AppHandle) {
    let started = Instant::now();
    let state = app.state::<AppState>();
    let result = generate_and_store(&task, &cancellation, &state).await;
    let duration_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);

    let event = match result {
        Ok((image, stored)) => {
            let output_id = Uuid::new_v4().to_string();
            let database_request_id = task.request_id.clone();
            let content_hash = stored.content_hash.clone();
            let storage_key = stored.storage_key.clone();
            let mime_type = image.mime_type.clone();
            let provider_name = image.provider_name.clone();
            let cost_usd = image.cost_usd;
            let width = image.width;
            let height = image.height;
            let completed = state
                .database
                .execute(move |database| {
                    database.complete_attempt(
                        &database_request_id,
                        duration_ms,
                        cost_usd,
                        provider_name.as_deref(),
                        NewAsset {
                            id: &output_id,
                            content_hash: &content_hash,
                            role: "output",
                            local_path: &storage_key,
                            mime_type: &mime_type,
                            width,
                            height,
                            sort_order: 0,
                        },
                    )
                })
                .await;

            match completed {
                Ok(()) => GenerationJobEvent {
                    request_id: task.request_id.clone(),
                    status: GenerationJobStatus::Succeeded,
                    result: Some(GenerationResult {
                        attempt_id: task.request_id.clone(),
                        asset_path: stored.path.to_string_lossy().into_owned(),
                        mime_type: image.mime_type,
                        width: image.width,
                        height: image.height,
                        cost_usd: image.cost_usd,
                        duration_ms,
                        model_id: IMAGE_MODEL_ID,
                        provider_name: image.provider_name,
                    }),
                    error: None,
                },
                Err(error) => failed_event(&task.request_id, duration_ms, error, &state).await,
            }
        }
        Err(error) if error.kind == ErrorKind::Cancelled => {
            let database_request_id = task.request_id.clone();
            let recording_error = state
                .database
                .execute(move |database| database.mark_cancelled(&database_request_id, duration_ms))
                .await
                .err();
            let error = preserve_primary_error(error, recording_error);
            GenerationJobEvent {
                request_id: task.request_id.clone(),
                status: GenerationJobStatus::Cancelled,
                result: None,
                error: Some(error),
            }
        }
        Err(error) => failed_event(&task.request_id, duration_ms, error, &state).await,
    };

    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.remove(&task.request_id);
    }
    let _ = app.emit(GENERATION_JOB_EVENT, event);
}

async fn generate_and_store(
    task: &GenerationTask,
    cancellation: &CancellationToken,
    state: &AppState,
) -> AppResult<(
    crate::models::GeneratedImage,
    crate::asset_store::StoredAsset,
)> {
    if cancellation.is_cancelled() {
        return Err(cancelled_error());
    }

    let api_key = crate::credentials::get_api_key()?;
    let reference = task
        .reference
        .as_ref()
        .map(|reference| (reference.mime_type.as_str(), reference.bytes.as_slice()));
    let image = state
        .openrouter
        .generate_image(
            &api_key,
            &task.prompt,
            &task.settings,
            reference,
            cancellation,
        )
        .await?;

    let stored = state.assets.store(&image.extension, &image.bytes).await?;
    Ok((image, stored))
}

async fn failed_event(
    request_id: &str,
    duration_ms: i64,
    error: AppError,
    state: &AppState,
) -> GenerationJobEvent {
    let database_request_id = request_id.to_owned();
    let database_error = error.clone();
    let recording_error = state
        .database
        .execute(move |database| {
            database.mark_failed(&database_request_id, duration_ms, &database_error)
        })
        .await
        .err();
    let error = preserve_primary_error(error, recording_error);
    GenerationJobEvent {
        request_id: request_id.to_owned(),
        status: GenerationJobStatus::Failed,
        result: None,
        error: Some(error),
    }
}

fn preserve_primary_error(mut primary: AppError, recording_error: Option<AppError>) -> AppError {
    let Some(recording_error) = recording_error else {
        return primary;
    };

    let secondary = match recording_error.details {
        Some(details) => format!("{} {details}", recording_error.message),
        None => recording_error.message,
    };
    let note = format!("Local history update also failed: {secondary}");
    primary.details = Some(match primary.details {
        Some(details) => format!("{details} · {note}"),
        None => note,
    });
    primary
}

fn acquire_generation_slot(slots: Arc<tokio::sync::Semaphore>) -> AppResult<OwnedSemaphorePermit> {
    slots.try_acquire_owned().map_err(|error| match error {
        TryAcquireError::NoPermits => AppError::new(
            ErrorKind::Busy,
            "Eidos is already running the maximum number of generations.",
            true,
        ),
        TryAcquireError::Closed => AppError::internal("generation limit was closed"),
    })
}

fn validate_request_id(value: &str) -> AppResult<String> {
    Uuid::parse_str(value)
        .map(|id| id.to_string())
        .map_err(|_| {
            AppError::new(
                ErrorKind::Validation,
                "The generation request ID was invalid.",
                false,
            )
        })
}

fn validate_prompt(value: &str) -> AppResult<String> {
    let prompt = value.trim().to_owned();
    if prompt.is_empty() {
        return Err(AppError::new(
            ErrorKind::Validation,
            "Write a prompt before generating.",
            false,
        ));
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(AppError::new(
            ErrorKind::Validation,
            "The prompt is longer than 8,000 characters.",
            false,
        ));
    }
    Ok(prompt)
}

fn cancelled_error() -> AppError {
    AppError::new(ErrorKind::Cancelled, "Generation cancelled.", true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_failures_do_not_replace_provider_errors() {
        let provider_error = AppError::new(
            ErrorKind::GeneratedCandidateBlocked,
            "The generated candidate was blocked.",
            true,
        )
        .with_details("Provider: Google");
        let database_error = AppError::storage("database is locked");

        let merged = preserve_primary_error(provider_error, Some(database_error));

        assert_eq!(merged.kind, ErrorKind::GeneratedCandidateBlocked);
        assert_eq!(merged.message, "The generated candidate was blocked.");
        assert!(merged.details.as_deref().is_some_and(|details| {
            details.contains("Provider: Google") && details.contains("database is locked")
        }));
    }

    #[test]
    fn backend_rejects_jobs_beyond_its_concurrency_limit() {
        let slots = Arc::new(tokio::sync::Semaphore::new(2));
        let first = acquire_generation_slot(slots.clone()).expect("first slot");
        let _second = acquire_generation_slot(slots.clone()).expect("second slot");

        let error = acquire_generation_slot(slots.clone()).expect_err("capacity reached");
        assert_eq!(error.kind, ErrorKind::Busy);

        drop(first);
        let _replacement = acquire_generation_slot(slots).expect("released slot");
    }
}
