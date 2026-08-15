use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::error::{AppError, ErrorKind};
use crate::generation;
use crate::images::inspect_raster;
use crate::models::{
    AppResult, AppStatus, CancelResult, GenerateRequest, IMAGE_MODEL_ID, IMAGE_MODEL_NAME,
    JobAccepted, MAX_REFERENCE_BYTES, ReferenceSelection, SUPPORTED_ASPECT_RATIOS,
    SUPPORTED_RESOLUTIONS, SaveResult, SelectedReference,
};
use crate::state::{AppState, locked};

#[tauri::command]
pub fn get_app_status() -> AppResult<AppStatus> {
    Ok(AppStatus {
        has_api_key: crate::credentials::has_api_key()?,
        model_id: IMAGE_MODEL_ID,
        model_name: IMAGE_MODEL_NAME,
        supported_aspect_ratios: SUPPORTED_ASPECT_RATIOS,
        supported_resolutions: SUPPORTED_RESOLUTIONS,
    })
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
    })
}

#[tauri::command]
pub fn remove_api_key() -> AppResult<()> {
    crate::credentials::remove_api_key()
}

#[tauri::command]
pub async fn select_reference_image(
    state: State<'_, AppState>,
) -> AppResult<Option<ReferenceSelection>> {
    let selection = rfd::AsyncFileDialog::new()
        .set_title("Choose a reference image")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
        .await;

    let Some(selection) = selection else {
        return Ok(None);
    };

    let source_path = selection.path().canonicalize().map_err(AppError::file)?;
    let metadata = tokio::fs::metadata(&source_path)
        .await
        .map_err(AppError::file)?;
    if !metadata.is_file() {
        return Err(AppError::new(
            ErrorKind::File,
            "Choose an image file, not a folder.",
            false,
        ));
    }
    if metadata.len() > MAX_REFERENCE_BYTES {
        return Err(AppError::new(
            ErrorKind::File,
            "Reference images must be smaller than 12 MB.",
            false,
        ));
    }

    let bytes = tokio::fs::read(&source_path)
        .await
        .map_err(AppError::file)?;
    let (mime_type, extension, width, height) = inspect_raster(&bytes)?;
    let token = Uuid::new_v4().to_string();
    let managed_path = state
        .assets
        .stage_selection(&token, extension, &bytes)
        .await?;
    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("reference")
        .to_owned();

    locked(&state.references, "reference")?.insert(
        token.clone(),
        SelectedReference {
            path: managed_path.clone(),
            mime_type: mime_type.to_owned(),
            extension: extension.to_owned(),
            width,
            height,
        },
    );

    Ok(Some(ReferenceSelection {
        token,
        file_name,
        mime_type: mime_type.to_owned(),
        width,
        height,
        asset_path: managed_path.to_string_lossy().into_owned(),
    }))
}

#[tauri::command]
pub async fn discard_reference(token: String, state: State<'_, AppState>) -> AppResult<()> {
    let reference = locked(&state.references, "reference")?.remove(&token);
    if let Some(reference) = reference {
        state.assets.discard_selection(&reference.path).await?;
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
