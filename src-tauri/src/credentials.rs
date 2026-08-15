use keyring::{Entry, Error as KeyringError};

use crate::error::{AppError, ErrorKind};
use crate::models::AppResult;

const SERVICE: &str = "studio.eidos.desktop";
const ACCOUNT: &str = "openrouter-api-key";

fn entry() -> AppResult<Entry> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| {
        AppError::new(
            ErrorKind::Internal,
            "Eidos could not access the operating system credential store.",
            false,
        )
        .with_details(error.to_string())
    })
}

pub fn has_api_key() -> AppResult<bool> {
    match entry()?.get_password() {
        Ok(value) => Ok(!value.trim().is_empty()),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(error) => Err(AppError::internal(error)),
    }
}

pub fn get_api_key() -> AppResult<String> {
    match entry()?.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(KeyringError::NoEntry) => Err(AppError::new(
            ErrorKind::Authentication,
            "Connect an OpenRouter API key before generating.",
            false,
        )),
        Err(error) => Err(AppError::internal(error)),
    }
}

pub fn save_api_key(value: &str) -> AppResult<()> {
    entry()?.set_password(value).map_err(AppError::internal)
}

pub fn remove_api_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(AppError::internal(error)),
    }
}
