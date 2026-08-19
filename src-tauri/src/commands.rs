use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::error::{AppError, ErrorKind};
use crate::generation;
use crate::images::{create_history_thumbnail, create_reference_thumbnail, inspect_raster};
use crate::models::{
    AppResult, AppStatus, CancelResult, DeleteHistoryResult, GenerateRequest, HistoryCursor,
    HistoryPage, IMAGE_MODEL_ID, IMAGE_MODEL_NAME, ImageModel, JobAccepted, MAX_REFERENCE_BYTES,
    MAX_REFERENCE_TOTAL_BYTES, MAX_REFERENCES, ReferenceSelection, SUPPORTED_ASPECT_RATIOS,
    SUPPORTED_RESOLUTIONS, SaveResult, SelectedReference, validate_reference_total_bytes,
};
use crate::state::{AppState, locked};

const DEFAULT_HISTORY_PAGE_SIZE: usize = 60;
pub const HISTORY_THUMBNAILS_UPDATED_EVENT: &str = "history-thumbnails-updated";

struct ReferenceCandidate {
    file_name: String,
    mime_type: String,
    extension: String,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

#[tauri::command]
pub fn get_app_status() -> AppResult<AppStatus> {
    Ok(AppStatus {
        has_api_key: crate::credentials::has_api_key()?,
        model_id: IMAGE_MODEL_ID,
        model_name: IMAGE_MODEL_NAME,
        supported_aspect_ratios: SUPPORTED_ASPECT_RATIOS,
        supported_resolutions: SUPPORTED_RESOLUTIONS,
        max_references: MAX_REFERENCES,
        max_reference_total_bytes: MAX_REFERENCE_TOTAL_BYTES,
    })
}

#[tauri::command]
pub async fn list_image_models(state: State<'_, AppState>) -> AppResult<Vec<ImageModel>> {
    let cached = locked(&state.image_models, "image model")?.clone();
    let Ok(api_key) = crate::credentials::get_api_key() else {
        return Ok(cached);
    };

    let discovered = match state.openrouter.list_image_models(&api_key).await {
        Ok(discovered) => discovered,
        Err(error) => {
            eprintln!(
                "Image model catalog refresh failed; using cached models ({}): {}",
                error.kind.as_str(),
                error
            );
            return Ok(cached);
        }
    };
    if discovered.is_empty() {
        return Ok(cached);
    }

    *locked(&state.image_models, "image model")? = discovered.clone();
    Ok(discovered)
}

#[tauri::command]
pub async fn save_api_key(api_key: String, state: State<'_, AppState>) -> AppResult<AppStatus> {
    let api_key = api_key.trim().to_owned();
    if api_key.len() < 20 {
        return Err(AppError::new(
            ErrorKind::Validation,
            "Enter a complete OpenRouter API key.",
            false,
        ));
    }

    state.openrouter.validate_api_key(&api_key).await?;
    crate::credentials::save_api_key(&api_key)?;

    Ok(AppStatus {
        has_api_key: true,
        model_id: IMAGE_MODEL_ID,
        model_name: IMAGE_MODEL_NAME,
        supported_aspect_ratios: SUPPORTED_ASPECT_RATIOS,
        supported_resolutions: SUPPORTED_RESOLUTIONS,
        max_references: MAX_REFERENCES,
        max_reference_total_bytes: MAX_REFERENCE_TOTAL_BYTES,
    })
}

#[tauri::command]
pub fn remove_api_key() -> AppResult<()> {
    crate::credentials::remove_api_key()
}

#[tauri::command]
pub async fn select_reference_image(
    selected_bytes: u64,
    state: State<'_, AppState>,
) -> AppResult<Vec<ReferenceSelection>> {
    let selection = rfd::AsyncFileDialog::new()
        .set_title("Choose reference images")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_files()
        .await;

    let Some(selections) = selection else {
        return Ok(Vec::new());
    };
    if selections.len() > MAX_REFERENCES {
        return Err(AppError::new(
            ErrorKind::Validation,
            format!("Choose no more than {MAX_REFERENCES} reference images at once."),
            false,
        ));
    }

    let selected_bytes = selected_bytes.min(MAX_REFERENCE_TOTAL_BYTES);
    let mut source_paths = Vec::with_capacity(selections.len());
    let mut total_bytes = 0_u64;
    for selection in selections {
        let source_path = selection.path().canonicalize().map_err(AppError::file)?;
        let metadata = tokio::fs::metadata(&source_path)
            .await
            .map_err(AppError::file)?;
        if !metadata.is_file() {
            return Err(AppError::new(
                ErrorKind::File,
                "Choose image files, not folders.",
                false,
            ));
        }
        if metadata.len() > MAX_REFERENCE_BYTES {
            return Err(AppError::new(
                ErrorKind::File,
                "Each reference image must be smaller than 12 MB.",
                false,
            ));
        }

        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| AppError::file("Reference image sizes overflowed."))?;
        source_paths.push(source_path);
    }
    let combined_bytes = selected_bytes
        .checked_add(total_bytes)
        .ok_or_else(|| AppError::file("Reference image sizes overflowed."))?;
    validate_reference_total_bytes(combined_bytes, MAX_REFERENCE_TOTAL_BYTES)?;

    let mut candidates = Vec::with_capacity(source_paths.len());
    for source_path in source_paths {
        let bytes = tokio::fs::read(&source_path)
            .await
            .map_err(AppError::file)?;
        let (mime_type, extension, width, height) = inspect_raster(&bytes)?;
        let file_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("reference")
            .to_owned();

        candidates.push(ReferenceCandidate {
            file_name,
            mime_type: mime_type.to_owned(),
            extension: extension.to_owned(),
            width,
            height,
            bytes,
        });
    }

    stage_reference_candidates(candidates, &state).await
}

#[tauri::command]
pub async fn discard_reference(token: String, state: State<'_, AppState>) -> AppResult<()> {
    let reference = locked(&state.references, "reference")?.remove(&token);
    if let Some(reference) = reference {
        discard_selected_reference(&state, &reference).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_generation(
    request: GenerateRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<JobAccepted> {
    generation::start(request, app, &state).await
}

#[tauri::command]
pub fn cancel_generation(
    request_id: String,
    state: State<'_, AppState>,
) -> AppResult<CancelResult> {
    generation::cancel(request_id, &state)
}

#[tauri::command]
pub async fn save_output(
    attempt_id: String,
    state: State<'_, AppState>,
) -> AppResult<Option<SaveResult>> {
    let database_attempt_id = attempt_id.clone();
    let stored_path = state
        .database
        .execute(move |database| database.output_for_attempt(&database_attempt_id))
        .await?
        .ok_or_else(|| {
            AppError::new(
                ErrorKind::File,
                "The generated image could not be found in the local library.",
                false,
            )
        })?;
    let source = state.assets.resolve_persisted_path(&stored_path)?;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    let short_id = attempt_id.chars().take(8).collect::<String>();
    let selection = rfd::AsyncFileDialog::new()
        .set_title("Save generated image")
        .set_file_name(format!("eidos-{short_id}.{extension}"))
        .save_file()
        .await;

    let Some(selection) = selection else {
        return Ok(None);
    };
    let destination = selection.path();
    tokio::fs::copy(&source, destination)
        .await
        .map_err(AppError::file)?;

    Ok(Some(SaveResult {
        path: destination.to_string_lossy().into_owned(),
    }))
}

#[tauri::command]
pub async fn list_history(
    cursor: Option<HistoryCursor>,
    limit: Option<usize>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<HistoryPage> {
    let page_limit = limit.unwrap_or(DEFAULT_HISTORY_PAGE_SIZE).clamp(1, 100);
    let database_cursor = cursor;
    let mut page = state
        .database
        .execute(move |database| database.list_history(database_cursor.as_ref(), page_limit))
        .await?;

    let mut thumbnail_backfills = Vec::new();
    for attempt in &mut page.attempts {
        if let Some(output) = &attempt.output
            && output.thumbnail_path.is_none()
        {
            thumbnail_backfills.push((
                output.content_hash.clone(),
                PathBuf::from(&output.asset_path),
                false,
            ));
        }
        for reference in &attempt.references {
            if reference.thumbnail_path.is_none() {
                thumbnail_backfills.push((
                    reference.content_hash.clone(),
                    PathBuf::from(&reference.asset_path),
                    true,
                ));
            }
        }
        for asset in attempt
            .output
            .iter_mut()
            .chain(attempt.references.iter_mut())
        {
            let persisted_path = std::path::Path::new(&asset.asset_path);
            asset.asset_path = state
                .assets
                .resolve_persisted_path(persisted_path)?
                .to_string_lossy()
                .into_owned();
            if let Some(thumbnail_path) = &asset.thumbnail_path {
                asset.thumbnail_path = Some(
                    state
                        .assets
                        .resolve_persisted_path(std::path::Path::new(thumbnail_path))?
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }

    if !thumbnail_backfills.is_empty() {
        spawn_thumbnail_backfill(
            thumbnail_backfills,
            state.database.clone(),
            state.assets.clone(),
            app,
        );
    }

    Ok(page)
}

#[tauri::command]
pub async fn restore_history_reference(
    attempt_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<ReferenceSelection>> {
    let reference_attempt_id = attempt_id;
    let saved_references = state
        .database
        .execute(move |database| database.references_for_attempt(&reference_attempt_id))
        .await?;
    let mut saved_sources = Vec::with_capacity(saved_references.len());
    let mut total_bytes = 0_u64;
    for (index, reference) in saved_references.into_iter().enumerate() {
        let persisted_path = std::path::Path::new(&reference.asset_path);
        let source_path = state.assets.resolve_persisted_path(persisted_path)?;
        let metadata = tokio::fs::metadata(&source_path)
            .await
            .map_err(AppError::file)?;
        if metadata.len() > MAX_REFERENCE_BYTES {
            return Err(AppError::new(
                ErrorKind::File,
                "A saved reference is too large to reuse.",
                false,
            ));
        }

        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| AppError::file("Reference image sizes overflowed."))?;
        saved_sources.push((index, reference, source_path));
    }
    validate_reference_total_bytes(total_bytes, MAX_REFERENCE_TOTAL_BYTES)?;

    let mut candidates = Vec::with_capacity(saved_sources.len());
    for (index, reference, source_path) in saved_sources {
        let bytes = tokio::fs::read(&source_path)
            .await
            .map_err(AppError::file)?;
        let (mime_type, extension, width, height) = inspect_raster(&bytes)?;
        if mime_type != reference.mime_type
            || width != reference.width
            || height != reference.height
        {
            return Err(AppError::new(
                ErrorKind::File,
                "A saved reference image changed unexpectedly.",
                false,
            ));
        }

        candidates.push(ReferenceCandidate {
            file_name: format!("Saved reference {}", index + 1),
            mime_type: mime_type.to_owned(),
            extension: extension.to_owned(),
            width,
            height,
            bytes,
        });
    }

    stage_reference_candidates(candidates, &state).await
}

async fn stage_reference_candidates(
    candidates: Vec<ReferenceCandidate>,
    state: &AppState,
) -> AppResult<Vec<ReferenceSelection>> {
    let mut staged = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let thumbnail_bytes = match create_reference_thumbnail(&candidate.bytes) {
            Ok(bytes) => bytes,
            Err(error) => {
                discard_staged_references(state, &staged).await;
                return Err(error);
            }
        };
        let token = Uuid::new_v4().to_string();
        let managed_path = match state
            .assets
            .stage_selection(&token, &candidate.extension, &candidate.bytes)
            .await
        {
            Ok(path) => path,
            Err(error) => {
                discard_staged_references(state, &staged).await;
                return Err(error);
            }
        };
        let thumbnail_path = match state
            .assets
            .stage_selection_thumbnail(&token, &thumbnail_bytes)
            .await
        {
            Ok(path) => path,
            Err(error) => {
                let _ = state.assets.discard_selection(&managed_path).await;
                discard_staged_references(state, &staged).await;
                return Err(error);
            }
        };
        staged.push((
            ReferenceSelection {
                token: token.clone(),
                file_name: candidate.file_name,
                mime_type: candidate.mime_type.clone(),
                width: candidate.width,
                height: candidate.height,
                size_bytes: u64::try_from(candidate.bytes.len()).map_err(AppError::internal)?,
                asset_path: managed_path.to_string_lossy().into_owned(),
                thumbnail_path: thumbnail_path.to_string_lossy().into_owned(),
            },
            SelectedReference {
                path: managed_path,
                thumbnail_path,
                mime_type: candidate.mime_type,
                extension: candidate.extension,
                width: candidate.width,
                height: candidate.height,
            },
        ));
    }

    if let Err(error) = register_staged_references(state, &staged) {
        discard_staged_references(state, &staged).await;
        return Err(error);
    }
    Ok(staged.into_iter().map(|(selection, _)| selection).collect())
}

fn register_staged_references(
    state: &AppState,
    staged: &[(ReferenceSelection, SelectedReference)],
) -> AppResult<()> {
    let mut references = locked(&state.references, "reference")?;
    for (selection, reference) in staged {
        references.insert(selection.token.clone(), reference.clone());
    }
    Ok(())
}

async fn discard_staged_references(
    state: &AppState,
    staged: &[(ReferenceSelection, SelectedReference)],
) {
    for (_, reference) in staged {
        let _ = discard_selected_reference(state, reference).await;
    }
}

async fn discard_selected_reference(
    state: &AppState,
    reference: &SelectedReference,
) -> AppResult<()> {
    let asset_error = state.assets.discard_selection(&reference.path).await.err();
    let thumbnail_error = state
        .assets
        .discard_selection(&reference.thumbnail_path)
        .await
        .err();
    match asset_error.or(thumbnail_error) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[tauri::command]
pub async fn delete_history_attempt(
    attempt_id: String,
    state: State<'_, AppState>,
) -> AppResult<DeleteHistoryResult> {
    let (deleted, unreferenced_paths) = state
        .database
        .execute(move |database| database.delete_history_attempt(&attempt_id))
        .await?;
    let completed_paths = state.assets.delete_pending_paths(&unreferenced_paths);
    if !completed_paths.is_empty()
        && let Err(error) = state
            .database
            .execute(move |database| database.clear_pending_file_deletions(&completed_paths))
            .await
    {
        eprintln!("could not clear completed history file deletions: {error:?}");
    }
    Ok(DeleteHistoryResult { deleted })
}

fn spawn_thumbnail_backfill(
    jobs: Vec<(String, PathBuf, bool)>,
    database: crate::db::DatabaseHandle,
    assets: crate::asset_store::AssetStore,
    app: AppHandle,
) {
    tauri::async_runtime::spawn(async move {
        let mut updated = false;
        for (content_hash, stored_path, reference_preview) in jobs {
            let Ok(source) = assets.resolve_persisted_path(&stored_path) else {
                continue;
            };
            let Ok(bytes) = tokio::fs::read(source).await else {
                continue;
            };
            let thumbnail = if reference_preview {
                create_reference_thumbnail(&bytes)
            } else {
                create_history_thumbnail(&bytes)
            };
            let Ok(thumbnail_bytes) = thumbnail else {
                continue;
            };
            let stored_thumbnail = if reference_preview {
                assets
                    .store_reference_thumbnail(&content_hash, &thumbnail_bytes)
                    .await
            } else {
                assets
                    .store_thumbnail(&content_hash, &thumbnail_bytes)
                    .await
            };
            let Ok(thumbnail_path) = stored_thumbnail else {
                continue;
            };
            let database_hash = content_hash.clone();
            let database_path = thumbnail_path.clone();
            let asset_exists = database
                .execute(move |database| {
                    database.set_asset_thumbnail(&database_hash, &database_path, reference_preview)
                })
                .await
                .unwrap_or(false);
            if asset_exists {
                updated = true;
            } else {
                let _ = assets.delete_persisted_path(&thumbnail_path);
            }
        }
        if updated {
            let _ = app.emit(HISTORY_THUMBNAILS_UPDATED_EVENT, ());
        }
    });
}
