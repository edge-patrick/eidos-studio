#[cfg(not(all(debug_assertions, target_os = "macos")))]
use keyring::{Entry, Error as KeyringError};
use tauri::AppHandle;

#[cfg(not(all(debug_assertions, target_os = "macos")))]
use crate::error::{AppError, ErrorKind};
use crate::models::AppResult;

#[cfg(not(all(debug_assertions, target_os = "macos")))]
const SERVICE: &str = "studio.eidos.desktop";
#[cfg(not(all(debug_assertions, target_os = "macos")))]
const ACCOUNT: &str = "openrouter-api-key";

pub fn mask_api_key(value: &str) -> String {
    let value = value.trim();
    let prefix = value.strip_prefix("sk-or-v1-").map_or_else(
        || value.chars().take(4).collect::<String>(),
        |_| "sk-or-v1-".to_owned(),
    );
    let suffix = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{prefix}••••••••••••{suffix}")
}

#[cfg(not(all(debug_assertions, target_os = "macos")))]
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

#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn has_api_key(app: &AppHandle) -> AppResult<bool> {
    macos_debug::has_api_key(app)
}

#[cfg(not(all(debug_assertions, target_os = "macos")))]
pub fn has_api_key(_app: &AppHandle) -> AppResult<bool> {
    match entry()?.get_password() {
        Ok(value) => Ok(!value.trim().is_empty()),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(error) => Err(AppError::internal(error)),
    }
}

#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn get_api_key(app: &AppHandle) -> AppResult<String> {
    macos_debug::get_api_key(app)
}

#[cfg(not(all(debug_assertions, target_os = "macos")))]
pub fn get_api_key(_app: &AppHandle) -> AppResult<String> {
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

#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn save_api_key(app: &AppHandle, value: &str) -> AppResult<()> {
    macos_debug::save_api_key(app, value)
}

#[cfg(not(all(debug_assertions, target_os = "macos")))]
pub fn save_api_key(_app: &AppHandle, value: &str) -> AppResult<()> {
    entry()?.set_password(value).map_err(AppError::internal)
}

#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn remove_api_key(app: &AppHandle) -> AppResult<()> {
    macos_debug::remove_api_key(app)
}

#[cfg(not(all(debug_assertions, target_os = "macos")))]
pub fn remove_api_key(_app: &AppHandle) -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(AppError::internal(error)),
    }
}

// Tauri's macOS development binary is ad-hoc signed with a designated requirement
// based on its code hash. That hash changes after each Rust rebuild, so a Keychain
// item approved for the previous binary prompts again. Release builds remain on the
// operating-system credential store; only local macOS debug builds use this 0600 file.
#[cfg(all(debug_assertions, target_os = "macos"))]
mod macos_debug {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::PathBuf;

    use tauri::{AppHandle, Manager};

    use crate::error::{AppError, ErrorKind};
    use crate::models::AppResult;

    const FILE_NAME: &str = "openrouter-api-key.dev";

    fn path(app: &AppHandle) -> AppResult<PathBuf> {
        app.path()
            .app_data_dir()
            .map(|path| path.join(FILE_NAME))
            .map_err(|error| {
                AppError::new(
                    ErrorKind::Internal,
                    "Eidos could not locate its development credential store.",
                    false,
                )
                .with_details(error.to_string())
            })
    }

    pub fn has_api_key(app: &AppHandle) -> AppResult<bool> {
        match fs::read_to_string(path(app)?) {
            Ok(value) => Ok(!value.trim().is_empty()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(AppError::file(error)),
        }
    }

    pub fn get_api_key(app: &AppHandle) -> AppResult<String> {
        match fs::read_to_string(path(app)?) {
            Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()),
            Ok(_) => Err(missing_key()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(missing_key()),
            Err(error) => Err(AppError::file(error)),
        }
    }

    pub fn save_api_key(app: &AppHandle, value: &str) -> AppResult<()> {
        let path = path(app)?;
        let parent = path.parent().ok_or_else(|| {
            AppError::new(
                ErrorKind::Internal,
                "Eidos could not locate its development credential store.",
                false,
            )
        })?;
        fs::create_dir_all(parent).map_err(AppError::file)?;

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(AppError::file)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(AppError::file)?;
        file.write_all(value.as_bytes()).map_err(AppError::file)?;
        file.sync_all().map_err(AppError::file)
    }

    pub fn remove_api_key(app: &AppHandle) -> AppResult<()> {
        match fs::remove_file(path(app)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::file(error)),
        }
    }

    fn missing_key() -> AppError {
        AppError::new(
            ErrorKind::Authentication,
            "Connect an OpenRouter API key before generating.",
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::mask_api_key;

    #[test]
    fn masks_the_middle_of_an_openrouter_key() {
        assert_eq!(
            mask_api_key("sk-or-v1-abcdefghijklmnopqrstuvwxyz"),
            "sk-or-v1-••••••••••••wxyz"
        );
    }
}
