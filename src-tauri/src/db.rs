use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use tokio::sync::oneshot;

use crate::error::AppError;
use crate::models::AppResult;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial",
        r#"
        CREATE TABLE generation_attempts (
            id TEXT PRIMARY KEY,
            prompt TEXT NOT NULL,
            model_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled')),
            reference_count INTEGER NOT NULL DEFAULT 0,
            output_count INTEGER NOT NULL DEFAULT 0,
            provider_name TEXT,
            settings_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            completed_at TEXT,
            duration_ms INTEGER,
            cost_usd REAL,
            error_kind TEXT,
            error_message TEXT
        );

        CREATE TABLE assets (
            id TEXT PRIMARY KEY,
            generation_id TEXT NOT NULL REFERENCES generation_attempts(id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK (role IN ('reference', 'output')),
            local_path TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE INDEX assets_generation_id_idx ON assets(generation_id);
        CREATE INDEX generation_attempts_created_at_idx ON generation_attempts(created_at DESC);
    "#,
    ),
    (
        "002_content_addressed_assets",
        r#"
        ALTER TABLE assets RENAME TO assets_legacy;
        DROP INDEX assets_generation_id_idx;

        CREATE TABLE assets (
            id TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL UNIQUE,
            local_path TEXT NOT NULL UNIQUE,
            mime_type TEXT NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE attempt_assets (
            generation_id TEXT NOT NULL REFERENCES generation_attempts(id) ON DELETE CASCADE,
            asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
            role TEXT NOT NULL CHECK (role IN ('reference', 'output')),
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            PRIMARY KEY (generation_id, role, sort_order)
        );

        INSERT INTO assets(id, content_hash, local_path, mime_type, width, height, created_at)
        SELECT id, 'legacy:' || id, local_path, mime_type, width, height, created_at
        FROM assets_legacy;

        INSERT INTO attempt_assets(generation_id, asset_id, role, sort_order, created_at)
        SELECT generation_id, id, role, sort_order, created_at
        FROM assets_legacy;

        DROP TABLE assets_legacy;
        CREATE INDEX attempt_assets_generation_id_idx ON attempt_assets(generation_id);
        CREATE INDEX assets_content_hash_idx ON assets(content_hash);
    "#,
    ),
    (
        "003_portable_asset_paths",
        r#"
        UPDATE assets
        SET local_path = 'objects/' || content_hash ||
            CASE mime_type
                WHEN 'image/png' THEN '.png'
                WHEN 'image/jpeg' THEN '.jpg'
                WHEN 'image/webp' THEN '.webp'
            END
        WHERE length(content_hash) = 64
          AND content_hash NOT GLOB '*[^0-9a-f]*'
          AND mime_type IN ('image/png', 'image/jpeg', 'image/webp');
    "#,
    ),
];

pub struct Database {
    connection: Connection,
}

type DatabaseOperation = Box<dyn FnOnce(&mut Database) + Send + 'static>;

/// Serializes SQLite work on one dedicated thread. Async generation tasks send
/// short operations here instead of blocking a runtime worker on a mutex.
#[derive(Clone)]
pub struct DatabaseHandle {
    sender: Sender<DatabaseOperation>,
}

impl DatabaseHandle {
    pub fn start(mut database: Database) -> AppResult<Self> {
        let (sender, receiver) = mpsc::channel::<DatabaseOperation>();
        std::thread::Builder::new()
            .name("eidos-database".to_owned())
            .spawn(move || {
                while let Ok(operation) = receiver.recv() {
                    operation(&mut database);
                }
            })
            .map_err(AppError::storage)?;
        Ok(Self { sender })
    }

    pub async fn execute<T, F>(&self, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Database) -> AppResult<T> + Send + 'static,
    {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Box::new(move |database| {
                let _ = result_sender.send(operation(database));
            }))
            .map_err(|_| AppError::storage("The database worker stopped unexpectedly."))?;

        result_receiver
            .await
            .map_err(|_| AppError::storage("The database worker stopped unexpectedly."))?
    }
}

pub struct NewAsset<'a> {
    pub id: &'a str,
    pub content_hash: &'a str,
    pub role: &'a str,
    pub local_path: &'a Path,
    pub mime_type: &'a str,
    pub width: u32,
    pub height: u32,
    pub sort_order: i64,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        let mut connection = Connection::open(path).map_err(AppError::storage)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
            )
            .map_err(AppError::storage)?;

        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (version TEXT PRIMARY KEY, applied_at TEXT NOT NULL);",
            )
            .map_err(AppError::storage)?;

        for (version, sql) in MIGRATIONS {
            let applied = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                    [version],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(AppError::storage)?;

            if !applied {
                let transaction = connection.transaction().map_err(AppError::storage)?;
                transaction.execute_batch(sql).map_err(AppError::storage)?;
                transaction
                    .execute(
                        "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                        params![version, Utc::now().to_rfc3339()],
                    )
                    .map_err(AppError::storage)?;
                transaction.commit().map_err(AppError::storage)?;
            }
        }

        Ok(Self { connection })
    }

    pub fn start_attempt(
        &mut self,
        id: &str,
        prompt: &str,
        model_id: &str,
        reference_count: i64,
        settings_json: &str,
        reference: Option<NewAsset<'_>>,
    ) -> AppResult<()> {
        let transaction = self.connection.transaction().map_err(AppError::storage)?;
        transaction
            .execute(
                "INSERT INTO generation_attempts(id, prompt, model_id, status, reference_count, settings_json, created_at) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6)",
                params![id, prompt, model_id, reference_count, settings_json, Utc::now().to_rfc3339()],
            )
            .map_err(AppError::storage)?;
        if let Some(reference) = reference {
            upsert_and_link_asset(&transaction, id, reference)?;
        }
        transaction.commit().map_err(AppError::storage)?;
        Ok(())
    }

    pub fn complete_attempt(
        &mut self,
        id: &str,
        duration_ms: i64,
        cost_usd: Option<f64>,
        provider_name: Option<&str>,
        output: NewAsset<'_>,
    ) -> AppResult<()> {
        let transaction = self.connection.transaction().map_err(AppError::storage)?;
        let updated = transaction
            .execute(
                "UPDATE generation_attempts SET status = 'succeeded', output_count = 1, completed_at = ?2, duration_ms = ?3, cost_usd = ?4, provider_name = ?5 WHERE id = ?1 AND status = 'running'",
                params![id, Utc::now().to_rfc3339(), duration_ms, cost_usd, provider_name],
            )
            .map_err(AppError::storage)?;
        if updated != 1 {
            return Err(AppError::internal(
                "generation attempt was not running during completion",
            ));
        }
        upsert_and_link_asset(&transaction, id, output)?;
        transaction.commit().map_err(AppError::storage)?;
        Ok(())
    }

    pub fn mark_failed(&self, id: &str, duration_ms: i64, error: &AppError) -> AppResult<()> {
        self.connection
            .execute(
                "UPDATE generation_attempts SET status = 'failed', completed_at = ?2, duration_ms = ?3, error_kind = ?4, error_message = ?5 WHERE id = ?1 AND status = 'running'",
                params![id, Utc::now().to_rfc3339(), duration_ms, error.kind.as_str(), error.message],
            )
            .map_err(AppError::storage)?;
        Ok(())
    }

    pub fn mark_cancelled(&self, id: &str, duration_ms: i64) -> AppResult<()> {
        self.connection
            .execute(
                "UPDATE generation_attempts SET status = 'cancelled', completed_at = ?2, duration_ms = ?3, error_kind = 'cancelled', error_message = 'Generation cancelled by the user.' WHERE id = ?1 AND status = 'running'",
                params![id, Utc::now().to_rfc3339(), duration_ms],
            )
            .map_err(AppError::storage)?;
        Ok(())
    }

    pub fn recover_interrupted_attempts(&self) -> AppResult<usize> {
        self.connection
            .execute(
                "UPDATE generation_attempts SET status = 'failed', completed_at = ?1, error_kind = 'interrupted', error_message = 'Generation was interrupted when Eidos closed.' WHERE status = 'running'",
                [Utc::now().to_rfc3339()],
            )
            .map_err(AppError::storage)
    }

    pub fn output_for_attempt(&self, id: &str) -> AppResult<Option<PathBuf>> {
        self.connection
            .query_row(
                "SELECT assets.local_path FROM attempt_assets JOIN assets ON assets.id = attempt_assets.asset_id WHERE attempt_assets.generation_id = ?1 AND attempt_assets.role = 'output' ORDER BY attempt_assets.sort_order LIMIT 1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|path| path.map(PathBuf::from))
            .map_err(AppError::storage)
    }

    pub fn asset_hashes(&self) -> AppResult<HashSet<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT content_hash FROM assets")
            .map_err(AppError::storage)?;
        let hashes = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(AppError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::storage)?;
        Ok(hashes.into_iter().collect())
    }
}

fn upsert_and_link_asset(
    transaction: &Transaction<'_>,
    generation_id: &str,
    asset: NewAsset<'_>,
) -> AppResult<()> {
    transaction
        .execute(
            "INSERT INTO assets(id, content_hash, local_path, mime_type, width, height, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(content_hash) DO NOTHING",
            params![
                asset.id,
                asset.content_hash,
                asset.local_path.to_string_lossy(),
                asset.mime_type,
                asset.width,
                asset.height,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(AppError::storage)?;

    let asset_id = transaction
        .query_row(
            "SELECT id FROM assets WHERE content_hash = ?1",
            [asset.content_hash],
            |row| row.get::<_, String>(0),
        )
        .map_err(AppError::storage)?;

    transaction
        .execute(
            "INSERT INTO attempt_assets(generation_id, asset_id, role, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation_id,
                asset_id,
                asset.role,
                asset.sort_order,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(AppError::storage)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn migrates_and_records_an_attempt() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        database
            .start_attempt(
                "attempt-1",
                "A red chair",
                "test/model",
                0,
                r#"{"aspectRatio":"1:1","resolution":"1K"}"#,
                None,
            )
            .expect("attempt");
        let output_path = Path::new("/tmp/eidos-test-output.png");
        database
            .complete_attempt(
                "attempt-1",
                1200,
                Some(0.04),
                Some("Test"),
                NewAsset {
                    id: "asset-1",
                    content_hash: "hash-1",
                    role: "output",
                    local_path: output_path,
                    mime_type: "image/png",
                    width: 100,
                    height: 100,
                    sort_order: 0,
                },
            )
            .expect("success");

        let settings: String = database
            .connection
            .query_row(
                "SELECT settings_json FROM generation_attempts WHERE id = 'attempt-1'",
                [],
                |row| row.get(0),
            )
            .expect("saved settings");
        assert_eq!(settings, r#"{"aspectRatio":"1:1","resolution":"1K"}"#);
        assert_eq!(
            database
                .output_for_attempt("attempt-1")
                .expect("output path"),
            Some(output_path.to_path_buf())
        );
    }

    #[test]
    fn recovers_attempts_interrupted_by_process_exit() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        database
            .start_attempt("attempt-1", "Prompt", "test/model", 0, "{}", None)
            .expect("attempt");

        assert_eq!(
            database.recover_interrupted_attempts().expect("recovery"),
            1
        );

        let status: String = database
            .connection
            .query_row(
                "SELECT status FROM generation_attempts WHERE id = 'attempt-1'",
                [],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(status, "failed");
    }

    #[test]
    fn completing_a_non_running_attempt_does_not_link_an_asset() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        database
            .start_attempt("attempt-1", "Prompt", "test/model", 0, "{}", None)
            .expect("attempt");
        database
            .mark_cancelled("attempt-1", 10)
            .expect("cancel attempt");

        let error = database
            .complete_attempt(
                "attempt-1",
                20,
                None,
                None,
                NewAsset {
                    id: "asset-1",
                    content_hash: "hash-1",
                    role: "output",
                    local_path: Path::new("/tmp/should-not-be-linked.png"),
                    mime_type: "image/png",
                    width: 1,
                    height: 1,
                    sort_order: 0,
                },
            )
            .expect_err("terminal attempts cannot complete again");

        assert_eq!(error.kind, crate::error::ErrorKind::Internal);
        let asset_count: i64 = database
            .connection
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .expect("asset count");
        assert_eq!(asset_count, 0);
    }

    #[test]
    fn migrates_content_addressed_assets_to_relative_paths() {
        let root = std::env::temp_dir().join(format!("eidos-db-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("test directory");
        let path = root.join("eidos.sqlite3");
        let hash = "a".repeat(64);

        {
            let connection = Connection::open(&path).expect("legacy database");
            connection
                .execute_batch(
                    "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at TEXT NOT NULL);
                     INSERT INTO schema_migrations VALUES ('001_initial', 'now');
                     INSERT INTO schema_migrations VALUES ('002_content_addressed_assets', 'now');
                     CREATE TABLE assets (
                         id TEXT PRIMARY KEY,
                         content_hash TEXT NOT NULL UNIQUE,
                         local_path TEXT NOT NULL UNIQUE,
                         mime_type TEXT NOT NULL,
                         width INTEGER NOT NULL,
                         height INTEGER NOT NULL,
                         created_at TEXT NOT NULL
                     );",
                )
                .expect("legacy schema");
            connection
                .execute(
                    "INSERT INTO assets VALUES ('asset-1', ?1, '/Users/old/Library/Application Support/studio.eidos.desktop/assets/objects/old.png', 'image/png', 1, 1, 'now')",
                    [&hash],
                )
                .expect("legacy asset");
        }

        {
            let database = Database::open(&path).expect("migrated database");
            let local_path: String = database
                .connection
                .query_row("SELECT local_path FROM assets", [], |row| row.get(0))
                .expect("portable path");
            assert_eq!(local_path, format!("objects/{hash}.png"));
        }

        std::fs::remove_dir_all(root).expect("remove test directory");
    }

    #[tokio::test]
    async fn database_handle_runs_operations_on_its_worker() {
        let database = Database::open(Path::new(":memory:")).expect("database");
        let handle = DatabaseHandle::start(database).expect("database worker");

        handle
            .execute(|database| {
                database.start_attempt("attempt-1", "Prompt", "test/model", 0, "{}", None)
            })
            .await
            .expect("worker operation");

        let recovered = handle
            .execute(|database| database.recover_interrupted_attempts())
            .await
            .expect("worker recovery");
        assert_eq!(recovered, 1);
    }
}
