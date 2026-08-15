use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::AppResult;

#[derive(Debug, Clone)]
pub struct AssetStore {
    root: PathBuf,
    objects_dir: PathBuf,
    orphaned_dir: PathBuf,
    selections_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredAsset {
    pub content_hash: String,
    pub storage_key: PathBuf,
    pub path: PathBuf,
}

impl AssetStore {
    pub fn initialize(root: &Path) -> AppResult<Self> {
        let objects_dir = root.join("objects");
        let orphaned_dir = root.join("orphaned");
        let selections_dir = root.join("selections");

        std::fs::create_dir_all(&objects_dir).map_err(AppError::file)?;
        std::fs::create_dir_all(&orphaned_dir).map_err(AppError::file)?;

        // Selections are session-scoped previews. Any files left here came from
        // a previous process that can no longer hold their selection tokens.
        if selections_dir.exists() {
            std::fs::remove_dir_all(&selections_dir).map_err(AppError::file)?;
        }
        std::fs::create_dir_all(&selections_dir).map_err(AppError::file)?;

        Ok(Self {
            root: root.to_path_buf(),
            objects_dir,
            orphaned_dir,
            selections_dir,
        })
    }

    pub async fn stage_selection(
        &self,
        token: &str,
        extension: &str,
        bytes: &[u8],
    ) -> AppResult<PathBuf> {
        let path = self.selections_dir.join(format!("{token}.{extension}"));
        write_atomically(&path, bytes).await?;
        Ok(path)
    }

    pub async fn discard_selection(&self, path: &Path) -> AppResult<()> {
        if !path.starts_with(&self.selections_dir) {
            return Err(AppError::internal(
                "selection path escaped the managed selection directory",
            ));
        }

        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::file(error)),
        }
    }

    pub async fn store(&self, extension: &str, bytes: &[u8]) -> AppResult<StoredAsset> {
        let content_hash = sha256(bytes);
        let storage_key = PathBuf::from("objects").join(format!("{content_hash}.{extension}"));
        let path = self.root.join(&storage_key);

        if !tokio::fs::try_exists(&path).await.map_err(AppError::file)? {
            write_atomically(&path, bytes).await?;
        }

        Ok(StoredAsset {
            content_hash,
            storage_key,
            path,
        })
    }

    pub fn resolve_persisted_path(&self, stored_path: &Path) -> AppResult<PathBuf> {
        // Absolute paths are read-only compatibility for assets recorded by
        // schema versions 1 and 2. All newly managed objects use relative keys.
        if stored_path.is_absolute() {
            return Ok(stored_path.to_path_buf());
        }

        if stored_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(AppError::internal(
                "persisted asset path escaped the managed asset directory",
            ));
        }

        Ok(self.root.join(stored_path))
    }

    /// Moves unreferenced content-addressed objects into a recoverable folder.
    /// Reconciliation uses the content hash, never an absolute path.
    pub fn quarantine_unreferenced_objects(
        &self,
        persisted_hashes: &HashSet<String>,
    ) -> AppResult<usize> {
        let mut quarantined = 0;
        for entry in std::fs::read_dir(&self.objects_dir).map_err(AppError::file)? {
            let entry = entry.map_err(AppError::file)?;
            let file_type = entry.file_type().map_err(AppError::file)?;
            if !file_type.is_file() {
                continue;
            }

            let entry_path = entry.path();
            let Some(content_hash) = object_hash(&entry_path) else {
                // Unknown files are left untouched. Startup cleanup must favor
                // recoverability over reclaiming disk space.
                continue;
            };
            if !persisted_hashes.contains(content_hash) {
                let file_name = entry.file_name();
                let mut destination = self.orphaned_dir.join(&file_name);
                if destination.exists() {
                    destination = self.orphaned_dir.join(format!(
                        "{}-{}",
                        Uuid::new_v4(),
                        file_name.to_string_lossy()
                    ));
                }
                std::fs::rename(entry_path, destination).map_err(AppError::file)?;
                quarantined += 1;
            }
        }
        Ok(quarantined)
    }
}

fn object_hash(path: &Path) -> Option<&str> {
    let hash = path.file_stem()?.to_str()?;
    (hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(hash)
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

async fn write_atomically(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let partial = path.with_extension(format!(
        "{}.{}.partial",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("asset"),
        Uuid::new_v4()
    ));

    tokio::fs::write(&partial, bytes)
        .await
        .map_err(AppError::file)?;

    match tokio::fs::rename(&partial, path).await {
        Ok(()) => Ok(()),
        Err(_) if tokio::fs::try_exists(path).await.unwrap_or(false) => {
            let _ = tokio::fs::remove_file(&partial).await;
            Ok(())
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&partial).await;
            Err(AppError::file(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_identical_content_consistently() {
        assert_eq!(sha256(b"eidos"), sha256(b"eidos"));
        assert_ne!(sha256(b"eidos"), sha256(b"studio"));
    }

    #[tokio::test]
    async fn quarantines_only_unreferenced_object_files() {
        let root = std::env::temp_dir().join(format!("eidos-asset-test-{}", Uuid::new_v4()));
        let store = AssetStore::initialize(&root).expect("asset store");
        let retained = store.store("png", b"retained").await.expect("retained");
        let orphaned = store.store("png", b"orphaned").await.expect("orphaned");

        let persisted = HashSet::from([retained.content_hash.clone()]);
        assert_eq!(
            store
                .quarantine_unreferenced_objects(&persisted)
                .expect("cleanup"),
            1
        );
        assert!(retained.path.exists());
        assert!(!orphaned.path.exists());
        assert!(
            store
                .orphaned_dir
                .join(orphaned.path.file_name().unwrap())
                .exists()
        );

        std::fs::remove_dir_all(root).expect("remove test directory");
    }

    #[tokio::test]
    async fn relative_storage_keys_survive_moving_the_asset_root() {
        let original_root =
            std::env::temp_dir().join(format!("eidos-asset-original-{}", Uuid::new_v4()));
        let moved_root = std::env::temp_dir().join(format!("eidos-asset-moved-{}", Uuid::new_v4()));
        let original_store = AssetStore::initialize(&original_root).expect("asset store");
        let asset = original_store
            .store("png", b"portable")
            .await
            .expect("stored asset");

        assert!(asset.storage_key.is_relative());
        std::fs::rename(&original_root, &moved_root).expect("move asset root");

        let moved_store = AssetStore::initialize(&moved_root).expect("moved asset store");
        let persisted = HashSet::from([asset.content_hash]);
        assert_eq!(
            moved_store
                .quarantine_unreferenced_objects(&persisted)
                .expect("reconcile moved library"),
            0
        );
        assert!(
            moved_store
                .resolve_persisted_path(&asset.storage_key)
                .expect("resolved moved asset")
                .exists()
        );

        std::fs::remove_dir_all(moved_root).expect("remove test directory");
    }
}
