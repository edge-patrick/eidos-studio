use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use tokio::sync::oneshot;

use crate::error::AppError;
use crate::models::{
    AppResult, GenerationSettings, HistoryAsset, HistoryAttempt, HistoryCursor, HistoryPage,
};

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
    (
        "004_history_thumbnails_and_pending_deletions",
        r#"
        ALTER TABLE assets ADD COLUMN thumbnail_path TEXT;

        CREATE TABLE pending_file_deletions (
            path TEXT PRIMARY KEY,
            created_at TEXT NOT NULL
        );
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
    pub thumbnail_path: Option<&'a Path>,
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

    pub fn list_history(
        &self,
        cursor: Option<&HistoryCursor>,
        limit: usize,
    ) -> AppResult<HistoryPage> {
        let limit = limit.clamp(1, 100);
        let total_count = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM generation_attempts WHERE status IN ('succeeded', 'failed', 'cancelled')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(AppError::storage)?;
        let total_count = usize::try_from(total_count).map_err(AppError::storage)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT
                    attempts.id,
                    attempts.prompt,
                    attempts.model_id,
                    attempts.status,
                    attempts.settings_json,
                    attempts.created_at,
                    attempts.completed_at,
                    attempts.duration_ms,
                    attempts.cost_usd,
                    attempts.provider_name,
                    attempts.error_kind,
                    attempts.error_message,
                    output.local_path,
                    output.mime_type,
                    output.width,
                    output.height,
                    output.thumbnail_path,
                    output.content_hash,
                    reference.local_path,
                    reference.mime_type,
                    reference.width,
                    reference.height,
                    reference.thumbnail_path,
                    reference.content_hash
                FROM generation_attempts attempts
                LEFT JOIN attempt_assets output_link
                    ON output_link.generation_id = attempts.id
                    AND output_link.role = 'output'
                    AND output_link.sort_order = 0
                LEFT JOIN assets output ON output.id = output_link.asset_id
                LEFT JOIN attempt_assets reference_link
                    ON reference_link.generation_id = attempts.id
                    AND reference_link.role = 'reference'
                    AND reference_link.sort_order = 0
                LEFT JOIN assets reference ON reference.id = reference_link.asset_id
                WHERE attempts.status IN ('succeeded', 'failed', 'cancelled')
                  AND (
                    ?1 IS NULL
                    OR attempts.created_at < ?1
                    OR (attempts.created_at = ?1 AND attempts.id < ?2)
                  )
                ORDER BY attempts.created_at DESC, attempts.id DESC
                LIMIT ?3",
            )
            .map_err(AppError::storage)?;

        let cursor_created_at = cursor.map(|value| value.created_at.as_str());
        let cursor_id = cursor.map(|value| value.id.as_str());
        let mut attempts = statement
            .query_map(
                params![cursor_created_at, cursor_id, (limit + 1) as i64],
                |row| {
                    let settings_json = row.get::<_, String>(4)?;
                    Ok(HistoryAttempt {
                        id: row.get(0)?,
                        prompt: row.get(1)?,
                        model_id: row.get(2)?,
                        status: row.get(3)?,
                        settings: serde_json::from_str::<GenerationSettings>(&settings_json)
                            .unwrap_or_default(),
                        created_at: row.get(5)?,
                        completed_at: row.get(6)?,
                        duration_ms: row.get(7)?,
                        cost_usd: row.get(8)?,
                        provider_name: row.get(9)?,
                        error_kind: row.get(10)?,
                        error_message: row.get(11)?,
                        output: history_asset_from_row(row, 12)?,
                        reference: history_asset_from_row(row, 18)?,
                    })
                },
            )
            .map_err(AppError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::storage)?;

        let has_more = attempts.len() > limit;
        if has_more {
            attempts.pop();
        }
        let next_cursor = has_more.then(|| {
            let last = attempts
                .last()
                .expect("a paged history result has a final item");
            HistoryCursor {
                created_at: last.created_at.clone(),
                id: last.id.clone(),
            }
        });

        Ok(HistoryPage {
            attempts,
            next_cursor,
            total_count,
        })
    }

    pub fn reference_for_attempt(&self, id: &str) -> AppResult<Option<HistoryAsset>> {
        self.connection
            .query_row(
                "SELECT assets.local_path, assets.mime_type, assets.width, assets.height,
                        assets.thumbnail_path, assets.content_hash
                 FROM attempt_assets
                 JOIN assets ON assets.id = attempt_assets.asset_id
                 WHERE attempt_assets.generation_id = ?1
                   AND attempt_assets.role = 'reference'
                 ORDER BY attempt_assets.sort_order
                 LIMIT 1",
                [id],
                |row| {
                    Ok(HistoryAsset {
                        asset_path: row.get(0)?,
                        mime_type: row.get(1)?,
                        width: row.get(2)?,
                        height: row.get(3)?,
                        thumbnail_path: row.get(4)?,
                        content_hash: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::storage)
    }

    pub fn delete_history_attempt(&mut self, id: &str) -> AppResult<(bool, Vec<PathBuf>)> {
        let transaction = self.connection.transaction().map_err(AppError::storage)?;
        let linked_assets = {
            let mut statement = transaction
                .prepare(
                    "SELECT DISTINCT assets.id, assets.local_path, assets.thumbnail_path
                     FROM attempt_assets
                     JOIN assets ON assets.id = attempt_assets.asset_id
                     WHERE attempt_assets.generation_id = ?1",
                )
                .map_err(AppError::storage)?;
            statement
                .query_map([id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })
                .map_err(AppError::storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::storage)?
        };

        let deleted = transaction
            .execute(
                "DELETE FROM generation_attempts
                 WHERE id = ?1 AND status IN ('succeeded', 'failed', 'cancelled')",
                [id],
            )
            .map_err(AppError::storage)?
            == 1;

        let mut unreferenced_paths = Vec::new();
        if deleted {
            for (asset_id, local_path, thumbnail_path) in linked_assets {
                let still_linked = transaction
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM attempt_assets WHERE asset_id = ?1)",
                        [&asset_id],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(AppError::storage)?;
                if !still_linked {
                    transaction
                        .execute("DELETE FROM assets WHERE id = ?1", [&asset_id])
                        .map_err(AppError::storage)?;
                    let paths = std::iter::once(local_path).chain(thumbnail_path);
                    for path in paths {
                        transaction
                            .execute(
                                "INSERT OR IGNORE INTO pending_file_deletions(path, created_at) VALUES (?1, ?2)",
                                params![&path, Utc::now().to_rfc3339()],
                            )
                            .map_err(AppError::storage)?;
                        unreferenced_paths.push(PathBuf::from(path));
                    }
                }
            }
        }

        transaction.commit().map_err(AppError::storage)?;
        Ok((deleted, unreferenced_paths))
    }

    pub fn pending_file_deletions(&self) -> AppResult<Vec<PathBuf>> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM pending_file_deletions ORDER BY created_at")
            .map_err(AppError::storage)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(AppError::storage)?
            .map(|path| path.map(PathBuf::from))
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::storage)
    }

    pub fn clear_pending_file_deletions(&mut self, paths: &[PathBuf]) -> AppResult<()> {
        let transaction = self.connection.transaction().map_err(AppError::storage)?;
        for path in paths {
            transaction
                .execute(
                    "DELETE FROM pending_file_deletions WHERE path = ?1",
                    [path.to_string_lossy()],
                )
                .map_err(AppError::storage)?;
        }
        transaction.commit().map_err(AppError::storage)
    }

    pub fn set_asset_thumbnail(&self, content_hash: &str, path: &Path) -> AppResult<bool> {
        self.connection
            .execute(
                "UPDATE assets
                 SET thumbnail_path = COALESCE(thumbnail_path, ?2)
                 WHERE content_hash = ?1",
                params![content_hash, path.to_string_lossy()],
            )
            .map(|updated| updated == 1)
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

fn history_asset_from_row(
    row: &rusqlite::Row<'_>,
    start_index: usize,
) -> rusqlite::Result<Option<HistoryAsset>> {
    let local_path = row.get::<_, Option<String>>(start_index)?;
    local_path
        .map(|asset_path| {
            Ok(HistoryAsset {
                asset_path,
                mime_type: row.get(start_index + 1)?,
                width: row.get(start_index + 2)?,
                height: row.get(start_index + 3)?,
                thumbnail_path: row.get(start_index + 4)?,
                content_hash: row.get(start_index + 5)?,
            })
        })
        .transpose()
}

fn upsert_and_link_asset(
    transaction: &Transaction<'_>,
    generation_id: &str,
    asset: NewAsset<'_>,
) -> AppResult<()> {
    transaction
        .execute(
            "INSERT INTO assets(id, content_hash, local_path, thumbnail_path, mime_type, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(content_hash) DO UPDATE SET
                thumbnail_path = COALESCE(assets.thumbnail_path, excluded.thumbnail_path)",
            params![
                asset.id,
                asset.content_hash,
                asset.local_path.to_string_lossy(),
                asset.thumbnail_path.map(|path| path.to_string_lossy()),
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
                    thumbnail_path: None,
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
                    thumbnail_path: None,
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

    #[test]
    fn history_includes_all_terminal_attempts() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        database
            .start_attempt("success", "A finished prompt", "test/model", 0, "{}", None)
            .expect("successful attempt");
        database
            .complete_attempt(
                "success",
                100,
                Some(0.01),
                Some("Test provider"),
                NewAsset {
                    id: "output",
                    content_hash: "output-hash",
                    role: "output",
                    local_path: Path::new("objects/output.png"),
                    thumbnail_path: None,
                    mime_type: "image/png",
                    width: 100,
                    height: 80,
                    sort_order: 0,
                },
            )
            .expect("complete attempt");
        database
            .start_attempt("failure", "A failed prompt", "test/model", 0, "{}", None)
            .expect("failed attempt");
        database
            .mark_failed(
                "failure",
                50,
                &AppError::new(
                    crate::error::ErrorKind::ProviderUnavailable,
                    "Provider rejected the prompt.",
                    false,
                ),
            )
            .expect("record failure");
        database
            .start_attempt(
                "cancelled",
                "A cancelled prompt",
                "test/model",
                0,
                "{}",
                None,
            )
            .expect("cancelled attempt");
        database
            .mark_cancelled("cancelled", 10)
            .expect("record cancellation");

        let history = database.list_history(None, 60).expect("history");
        assert_eq!(history.attempts.len(), 3);
        assert_eq!(history.total_count, 3);
        assert!(history.attempts.iter().any(|attempt| {
            attempt.id == "success" && attempt.status == "succeeded" && attempt.output.is_some()
        }));
        assert!(history.attempts.iter().any(|attempt| {
            attempt.id == "failure"
                && attempt.status == "failed"
                && attempt.error_message.as_deref() == Some("Provider rejected the prompt.")
        }));
        assert!(
            history
                .attempts
                .iter()
                .any(|attempt| { attempt.id == "cancelled" && attempt.status == "cancelled" })
        );
    }

    #[test]
    fn history_pages_with_a_stable_cursor() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        for id in ["oldest", "middle", "newest"] {
            database
                .start_attempt(id, "Prompt", "test/model", 0, "{}", None)
                .expect("attempt");
            database
                .mark_failed(id, 10, &AppError::internal("failed"))
                .expect("terminal attempt");
        }
        for (id, created_at) in [
            ("oldest", "2026-08-14T12:00:00Z"),
            ("middle", "2026-08-15T12:00:00Z"),
            ("newest", "2026-08-16T12:00:00Z"),
        ] {
            database
                .connection
                .execute(
                    "UPDATE generation_attempts SET created_at = ?2 WHERE id = ?1",
                    params![id, created_at],
                )
                .expect("set deterministic creation time");
        }

        let first = database.list_history(None, 2).expect("first page");
        assert_eq!(first.total_count, 3);
        assert_eq!(
            first
                .attempts
                .iter()
                .map(|attempt| attempt.id.as_str())
                .collect::<Vec<_>>(),
            vec!["newest", "middle"]
        );
        let cursor = first.next_cursor.expect("another page");

        let second = database
            .list_history(Some(&cursor), 2)
            .expect("second page");
        assert_eq!(
            second
                .attempts
                .iter()
                .map(|attempt| attempt.id.as_str())
                .collect::<Vec<_>>(),
            vec!["oldest"]
        );
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn deletion_removes_cancelled_attempts() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        database
            .start_attempt("cancelled", "Prompt", "test/model", 0, "{}", None)
            .expect("attempt");
        database
            .mark_cancelled("cancelled", 10)
            .expect("record cancellation");

        let (deleted, paths) = database
            .delete_history_attempt("cancelled")
            .expect("delete cancelled attempt");

        assert!(deleted);
        assert!(paths.is_empty());
        assert!(
            database
                .list_history(None, 60)
                .expect("history")
                .attempts
                .is_empty()
        );
    }

    #[test]
    fn deletion_keeps_assets_linked_to_another_attempt() {
        let mut database = Database::open(Path::new(":memory:")).expect("database");
        for (attempt_id, asset_id) in [("first", "asset-one"), ("second", "asset-two")] {
            database
                .start_attempt(attempt_id, "Prompt", "test/model", 0, "{}", None)
                .expect("attempt");
            database
                .complete_attempt(
                    attempt_id,
                    100,
                    None,
                    None,
                    NewAsset {
                        id: asset_id,
                        content_hash: "shared-hash",
                        role: "output",
                        local_path: Path::new("objects/shared.png"),
                        thumbnail_path: Some(Path::new("thumbnails/shared.png")),
                        mime_type: "image/png",
                        width: 10,
                        height: 10,
                        sort_order: 0,
                    },
                )
                .expect("complete attempt");
        }

        let (deleted, paths) = database
            .delete_history_attempt("first")
            .expect("delete first attempt");
        assert!(deleted);
        assert!(paths.is_empty());
        assert!(database.output_for_attempt("second").unwrap().is_some());

        let (deleted, paths) = database
            .delete_history_attempt("second")
            .expect("delete second attempt");
        assert!(deleted);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("objects/shared.png"),
                PathBuf::from("thumbnails/shared.png"),
            ]
        );
        assert_eq!(
            database.pending_file_deletions().expect("pending paths"),
            paths
        );
        database
            .clear_pending_file_deletions(&paths)
            .expect("clear completed paths");
        assert!(
            database
                .pending_file_deletions()
                .expect("cleared pending paths")
                .is_empty()
        );
    }
}
