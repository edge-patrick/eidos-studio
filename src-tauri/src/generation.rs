use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{OwnedSemaphorePermit, TryAcquireError};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::db::NewAsset;
use crate::error::{AppError, ErrorKind};
use crate::images::{create_history_thumbnail, create_reference_thumbnail, inspect_raster};
use crate::models::{
    AppResult, CancelResult, GenerateRequest, GenerationJobEvent, GenerationJobStatus,
    GenerationResult, GenerationSettings, IMAGE_MODEL_ID, JobAccepted, MAX_PROMPT_CHARS,
    MAX_REFERENCE_BYTES, MAX_REFERENCE_TOTAL_BYTES, MAX_REFERENCES, SelectedReference,
    validate_reference_total_bytes,
};
use crate::state::{AppState, locked};

pub const GENERATION_JOB_EVENT: &str = "generation-job-updated";
pub const MAX_CONCURRENT_GENERATIONS: usize = 2;

struct PreparedReference {
    mime_type: String,
    bytes: Vec<u8>,
}

struct DatabaseReference {
    asset_id: String,
    content_hash: String,
    storage_key: PathBuf,
    thumbnail_storage_key: Option<PathBuf>,
    mime_type: String,
    width: u32,
    height: u32,
    sort_order: i64,
}

struct GenerationTask {
    request_id: String,
    prompt: String,
    settings: GenerationSettings,
    references: Vec<PreparedReference>,
    api_key: String,
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
    // Credentials are required before an attempt becomes durable. Offline use
    // of the local Library must never create authentication-failure history.
    let api_key = crate::credentials::get_api_key()?;
    let permit = acquire_generation_slot(state.generation_slots.clone())?;

    if request.reference_tokens.len() > MAX_REFERENCES {
        return Err(AppError::new(
            ErrorKind::Validation,
            format!("Choose no more than {MAX_REFERENCES} reference images."),
            false,
        ));
    }
    let unique_tokens = request.reference_tokens.iter().collect::<HashSet<_>>();
    if unique_tokens.len() != request.reference_tokens.len() {
        return Err(AppError::new(
            ErrorKind::Validation,
            "Each reference image can only be included once.",
            false,
        ));
    }
    let selected_references = {
        let references = locked(&state.references, "reference")?;
        request
            .reference_tokens
            .iter()
            .map(|token| {
                references.get(token).cloned().ok_or_else(|| {
                    AppError::new(
                        ErrorKind::Validation,
                        "Choose the reference images again before generating.",
                        false,
                    )
                })
            })
            .collect::<AppResult<Vec<_>>>()?
    };

    let cancellation = CancellationToken::new();
    {
        let mut jobs = locked(&state.jobs, "generation job")?;
        if jobs.contains_key(&request_id) {
            return Err(AppError::new(
                ErrorKind::Validation,
                "That generation request is already running.",
                false,
            ));
        }
        jobs.insert(request_id.clone(), cancellation.clone());
    }

    let (prepared_references, database_references) =
        match prepare_references(selected_references, &cancellation, state).await {
            Ok(prepared) => prepared,
            Err(error) => {
                remove_job(&request_id, state);
                return Err(error);
            }
        };

    if cancellation.is_cancelled() {
        remove_job(&request_id, state);
        return Err(cancelled_error());
    }

    let database_request_id = request_id.clone();
    let database_prompt = prompt.clone();
    let database_result = state
        .database
        .execute(move |database| {
            let references = database_references
                .iter()
                .map(|reference| NewAsset {
                    id: &reference.asset_id,
                    content_hash: &reference.content_hash,
                    role: "reference",
                    local_path: &reference.storage_key,
                    thumbnail_path: reference.thumbnail_storage_key.as_deref(),
                    mime_type: &reference.mime_type,
                    width: reference.width,
                    height: reference.height,
                    sort_order: reference.sort_order,
                })
                .collect::<Vec<_>>();
            database.start_attempt(
                &database_request_id,
                &database_prompt,
                IMAGE_MODEL_ID,
                i64::try_from(references.len()).map_err(AppError::internal)?,
                &settings_json,
                references,
            )
        })
        .await;
    if let Err(error) = database_result {
        remove_job(&request_id, state);
        return Err(error);
    }

    let task = GenerationTask {
        request_id: request_id.clone(),
        prompt,
        settings,
        references: prepared_references,
        api_key,
        _permit: permit,
    };
    tauri::async_runtime::spawn(run(task, cancellation, app));

    Ok(JobAccepted { request_id })
}

async fn prepare_references(
    selected_references: Vec<SelectedReference>,
    cancellation: &CancellationToken,
    state: &AppState,
) -> AppResult<(Vec<PreparedReference>, Vec<DatabaseReference>)> {
    let mut prepared_references = Vec::with_capacity(selected_references.len());
    let mut database_references = Vec::with_capacity(selected_references.len());
    let mut total_bytes = 0_u64;

    for (index, reference) in selected_references.into_iter().enumerate() {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }

        let bytes = tokio::fs::read(&reference.path)
            .await
            .map_err(AppError::file)?;
        if bytes.len() as u64 > MAX_REFERENCE_BYTES {
            return Err(AppError::new(
                ErrorKind::File,
                "A selected reference image is now larger than 12 MB.",
                false,
            ));
        }
        total_bytes = total_bytes
            .checked_add(u64::try_from(bytes.len()).map_err(AppError::internal)?)
            .ok_or_else(|| AppError::file("Reference image sizes overflowed."))?;
        validate_reference_total_bytes(total_bytes, MAX_REFERENCE_TOTAL_BYTES)?;

        let (mime_type, extension, width, height) = inspect_raster(&bytes)?;
        if mime_type != reference.mime_type
            || extension != reference.extension
            || width != reference.width
            || height != reference.height
        {
            return Err(AppError::new(
                ErrorKind::File,
                "A managed reference image changed unexpectedly. Choose it again.",
                false,
            ));
        }

        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let stored = state.assets.store(extension, &bytes).await?;
        let thumbnail_bytes = match tokio::fs::read(&reference.thumbnail_path).await {
            Ok(thumbnail) => Some(thumbnail),
            Err(_) => create_reference_thumbnail(&bytes).ok(),
        };
        let thumbnail_storage_key = match thumbnail_bytes {
            Some(thumbnail) => state
                .assets
                .store_reference_thumbnail(&stored.content_hash, &thumbnail)
                .await
                .ok(),
            None => None,
        };
        database_references.push(DatabaseReference {
            asset_id: Uuid::new_v4().to_string(),
            content_hash: stored.content_hash,
            storage_key: stored.storage_key,
            thumbnail_storage_key,
            mime_type: mime_type.to_owned(),
            width,
            height,
            sort_order: i64::try_from(index).map_err(AppError::internal)?,
        });
        prepared_references.push(PreparedReference {
            mime_type: mime_type.to_owned(),
            bytes,
        });
    }

    Ok((prepared_references, database_references))
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
        Ok((image, stored, thumbnail_storage_key)) => {
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
                            thumbnail_path: thumbnail_storage_key.as_deref(),
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

    remove_job(&task.request_id, &state);
    let _ = app.emit(GENERATION_JOB_EVENT, event);
}

fn remove_job(request_id: &str, state: &AppState) {
    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.remove(request_id);
    }
}

async fn generate_and_store(
    task: &GenerationTask,
    cancellation: &CancellationToken,
    state: &AppState,
) -> AppResult<(
    crate::models::GeneratedImage,
    crate::asset_store::StoredAsset,
    Option<std::path::PathBuf>,
)> {
    if cancellation.is_cancelled() {
        return Err(cancelled_error());
    }

    let references = task
        .references
        .iter()
        .map(|reference| (reference.mime_type.as_str(), reference.bytes.as_slice()))
        .collect::<Vec<_>>();
    let image = state
        .openrouter
        .generate_image(
            &task.api_key,
            &task.prompt,
            &task.settings,
            &references,
            cancellation,
        )
        .await?;

    let stored = state.assets.store(&image.extension, &image.bytes).await?;
    let thumbnail_storage_key = match create_history_thumbnail(&image.bytes) {
        Ok(bytes) => state
            .assets
            .store_thumbnail(&stored.content_hash, &bytes)
            .await
            .ok(),
        Err(_) => None,
    };
    Ok((image, stored, thumbnail_storage_key))
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
