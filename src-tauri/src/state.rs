use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::asset_store::AssetStore;
use crate::db::DatabaseHandle;
use crate::error::AppError;
use crate::models::{AppResult, ImageModel, SelectedReference};
use crate::openrouter::OpenRouterClient;

pub struct AppState {
    pub database: DatabaseHandle,
    pub openrouter: OpenRouterClient,
    pub references: Mutex<HashMap<String, SelectedReference>>,
    pub image_models: Mutex<Vec<ImageModel>>,
    pub jobs: Mutex<HashMap<String, CancellationToken>>,
    pub generation_slots: Arc<Semaphore>,
    pub assets: AssetStore,
}

pub fn locked<'a, T>(mutex: &'a Mutex<T>, label: &str) -> AppResult<MutexGuard<'a, T>> {
    mutex
        .lock()
        .map_err(|_| AppError::internal(format!("{label} lock was poisoned")))
}
