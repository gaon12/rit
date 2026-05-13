use crate::{ObjectId, ObjectKind, Repository, Result, RitError, parse_commit};
use rusqlite::{Connection, OpenFlags, params};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Current SQLite auxiliary index schema version.
pub const INDEXDB_SCHEMA_VERSION: i64 = 1;

/// Storage paths used by the optional SQLite auxiliary index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexDbStorage {
    /// Directory reserved for rit metadata under the Git common directory.
    pub directory: PathBuf,
    /// SQLite database path.
    pub database_path: PathBuf,
    /// Future lock-file path documented for external tooling and repair plans.
    pub lock_path: PathBuf,
}

/// Status of the optional SQLite auxiliary index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexDbStatus {
    /// Storage paths for the index database.
    pub storage: IndexDbStorage,
    /// Whether the SQLite database file exists.
    pub exists: bool,
    /// Schema version read from `PRAGMA user_version`.
    pub schema_version: Option<i64>,
    /// Whether the file could be opened and has the expected schema version.
    pub healthy: bool,
    /// Whether the database is older than canonical Git repository state.
    pub stale: bool,
    /// Reasons the database needs build, rebuild, or repair work.
    pub stale_reasons: Vec<String>,
}

/// Result of `ensure`, `build`, `update`, `repair`, or `rebuild`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexDbEnsureResult {
    /// Final status after the operation.
    pub status: IndexDbStatus,
    /// Whether a new database file was created.
    pub created: bool,
    /// Whether an existing database was updated in place.
    pub updated: bool,
    /// Number of commit rows inserted or refreshed.
    pub commits_indexed: usize,
}

/// Repository-scoped manager for the optional SQLite auxiliary index.
pub struct IndexDb<'repo> {
    repository: &'repo Repository,
}

impl Repository {
    /// Returns the optional SQLite auxiliary index manager.
    pub fn indexdb(&self) -> IndexDb<'_> {
        IndexDb { repository: self }
    }
}

impl<'repo> IndexDb<'repo> {
    /// Returns the storage layout for this repository.
    pub fn storage(&self) -> IndexDbStorage {
        let directory = self.repository.common_dir().join("rit");
        IndexDbStorage {
            database_path: directory.join("indexdb.sqlite"),
            lock_path: directory.join("indexdb.lock"),
            directory,
        }
    }

    /// Reports whether the database exists and matches the current schema.
    pub fn status(&self) -> Result<IndexDbStatus> {
        let storage = self.storage();
        if !storage.database_path.exists() {
            return Ok(IndexDbStatus {
                storage,
                exists: false,
                schema_version: None,
                healthy: false,
                stale: true,
                stale_reasons: vec!["indexdb is missing".to_owned()],
            });
        }

        let connection = match Connection::open_with_flags(
            &storage.database_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(connection) => connection,
            Err(error) => {
                return Ok(IndexDbStatus {
                    storage,
                    exists: true,
                    schema_version: None,
                    healthy: false,
                    stale: true,
                    stale_reasons: vec![format!("indexdb could not be opened: {error}")],
                });
            }
        };
        let schema_version = match read_schema_version(&connection) {
            Ok(version) => Some(version),
            Err(error) => {
                return Ok(IndexDbStatus {
                    storage,
                    exists: true,
                    schema_version: None,
                    healthy: false,
                    stale: true,
                    stale_reasons: vec![format!("schema version could not be read: {error}")],
                });
            }
        };
        let healthy = schema_version == Some(INDEXDB_SCHEMA_VERSION);
        let mut stale_reasons = Vec::new();
        if !healthy {
            stale_reasons.push(format!(
                "schema version is {}, expected {INDEXDB_SCHEMA_VERSION}",
                schema_version.unwrap_or_default()
            ));
        } else if refs_snapshot_is_stale(&connection, self.repository)? {
            stale_reasons.push("refs snapshot is stale".to_owned());
        }
        let stale = !stale_reasons.is_empty();

        Ok(IndexDbStatus {
            storage,
            exists: true,
            schema_version,
            healthy,
            stale,
            stale_reasons,
        })
    }

    /// Ensures the database exists. Existing healthy databases receive a light
    /// metadata refresh; missing databases are built.
    pub fn ensure(&self) -> Result<IndexDbEnsureResult> {
        let status = self.status()?;
        if !status.exists {
            return self.build();
        }
        if !status.healthy {
            return Err(RitError::invalid_input(
                "indexdb schema is unsupported; run `rit indexdb rebuild`",
            ));
        }
        self.update()
    }

    /// Builds a new database. Existing databases require `rebuild`.
    pub fn build(&self) -> Result<IndexDbEnsureResult> {
        let storage = self.storage();
        if storage.database_path.exists() {
            return Err(RitError::invalid_input(
                "indexdb already exists; use `rit indexdb rebuild`",
            ));
        }
        fs::create_dir_all(&storage.directory)
            .map_err(|source| RitError::io(&storage.directory, source))?;
        let mut connection = open_read_write(&storage.database_path)?;
        let commits_indexed = initialize_and_refresh(&mut connection, self.repository)?;
        Ok(IndexDbEnsureResult {
            status: self.status()?,
            created: true,
            updated: false,
            commits_indexed,
        })
    }

    /// Updates reproducible metadata from canonical Git data.
    pub fn update(&self) -> Result<IndexDbEnsureResult> {
        let storage = self.storage();
        if !storage.database_path.exists() {
            return Err(RitError::invalid_input(
                "indexdb is missing; run `rit indexdb build`",
            ));
        }
        let mut connection = open_read_write(&storage.database_path)?;
        ensure_supported_schema(&connection)?;
        let commits_indexed = refresh_indexdb(&mut connection, self.repository)?;
        Ok(IndexDbEnsureResult {
            status: self.status()?,
            created: false,
            updated: true,
            commits_indexed,
        })
    }

    /// Rebuilds the database from canonical Git data.
    pub fn rebuild(&self) -> Result<IndexDbEnsureResult> {
        self.drop()?;
        self.build()
    }

    /// Repairs the database conservatively by rebuilding it.
    pub fn repair(&self) -> Result<IndexDbEnsureResult> {
        self.rebuild()
    }

    /// Deletes the auxiliary database without touching Git objects, refs, index,
    /// or working tree files.
    pub fn drop(&self) -> Result<()> {
        let storage = self.storage();
        if storage.database_path.exists() {
            fs::remove_file(&storage.database_path)
                .map_err(|source| RitError::io(&storage.database_path, source))?;
        }
        Ok(())
    }

    /// Runs SQLite VACUUM when the database exists.
    pub fn vacuum(&self) -> Result<IndexDbStatus> {
        let storage = self.storage();
        if !storage.database_path.exists() {
            return Err(RitError::invalid_input(
                "indexdb is missing; run `rit indexdb build`",
            ));
        }
        let connection = open_read_write(&storage.database_path)?;
        ensure_supported_schema(&connection)?;
        connection.execute_batch("VACUUM;").map_err(sqlite_error)?;
        self.status()
    }
}

fn open_read_write(path: &PathBuf) -> Result<Connection> {
    Connection::open(path).map_err(sqlite_error)
}

fn initialize_and_refresh(connection: &mut Connection, repository: &Repository) -> Result<usize> {
    connection
        .execute_batch(INDEXDB_SCHEMA)
        .map_err(sqlite_error)?;
    refresh_indexdb(connection, repository)
}

fn refresh_indexdb(connection: &mut Connection, repository: &Repository) -> Result<usize> {
    let transaction = connection.transaction().map_err(sqlite_error)?;
    let refs_snapshot = current_refs_snapshot(repository)?;
    transaction
        .execute(
            "INSERT OR REPLACE INTO cache_state(key, value) VALUES (?1, ?2)",
            params!["schema_version", INDEXDB_SCHEMA_VERSION.to_string()],
        )
        .map_err(sqlite_error)?;
    transaction
        .execute(
            "INSERT OR REPLACE INTO cache_state(key, value) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params!["last_update_utc"],
        )
        .map_err(sqlite_error)?;
    refresh_refs_snapshot(&transaction, &refs_snapshot)?;
    let commits_indexed = refresh_commits(&transaction, repository, &refs_snapshot)?;
    transaction.commit().map_err(sqlite_error)?;
    Ok(commits_indexed)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RefSnapshotRow {
    name: String,
    hash_kind: Option<String>,
    object_id: Option<Vec<u8>>,
    target: Option<String>,
}

fn current_refs_snapshot(repository: &Repository) -> Result<Vec<RefSnapshotRow>> {
    let mut rows = Vec::new();
    if let Some(head) = repository.resolve_head()? {
        let target = repository
            .current_branch_name()?
            .map(|branch| format!("refs/heads/{branch}"));
        rows.push(RefSnapshotRow {
            name: "HEAD".to_owned(),
            hash_kind: Some("sha1".to_owned()),
            object_id: Some(head.as_bytes().to_vec()),
            target,
        });
    }
    for branch in repository.list_branches()? {
        rows.push(RefSnapshotRow {
            name: format!("refs/heads/{}", branch.name),
            hash_kind: Some("sha1".to_owned()),
            object_id: Some(branch.target.as_bytes().to_vec()),
            target: None,
        });
    }
    for tag in repository.list_tags()? {
        rows.push(RefSnapshotRow {
            name: format!("refs/tags/{}", tag.name),
            hash_kind: Some("sha1".to_owned()),
            object_id: Some(tag.target.as_bytes().to_vec()),
            target: None,
        });
    }
    rows.sort();
    Ok(rows)
}

fn stored_refs_snapshot(connection: &Connection) -> Result<Vec<RefSnapshotRow>> {
    let mut statement = connection
        .prepare("SELECT name, hash_kind, object_id, target FROM refs_snapshot ORDER BY name")
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(RefSnapshotRow {
                name: row.get(0)?,
                hash_kind: row.get(1)?,
                object_id: row.get(2)?,
                target: row.get(3)?,
            })
        })
        .map_err(sqlite_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sqlite_error)
}

fn refs_snapshot_is_stale(connection: &Connection, repository: &Repository) -> Result<bool> {
    Ok(stored_refs_snapshot(connection)? != current_refs_snapshot(repository)?)
}

fn refresh_refs_snapshot(connection: &Connection, refs_snapshot: &[RefSnapshotRow]) -> Result<()> {
    connection
        .execute("DELETE FROM refs_snapshot", [])
        .map_err(sqlite_error)?;
    for row in refs_snapshot {
        insert_ref_snapshot(connection, row)?;
    }
    Ok(())
}

fn insert_ref_snapshot(connection: &Connection, row: &RefSnapshotRow) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO refs_snapshot(name, hash_kind, object_id, target) VALUES (?1, ?2, ?3, ?4)",
            params![
                row.name.as_str(),
                row.hash_kind.as_deref(),
                row.object_id.as_deref(),
                row.target.as_deref()
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn refresh_commits(
    connection: &Connection,
    repository: &Repository,
    refs_snapshot: &[RefSnapshotRow],
) -> Result<usize> {
    let mut indexed = 0;
    let mut seen = HashSet::new();
    let mut stack = refs_snapshot
        .iter()
        .filter_map(|row| object_id_from_snapshot_bytes(row.object_id.as_deref()))
        .collect::<Vec<_>>();
    while let Some(object_id) = stack.pop() {
        if !seen.insert(object_id) {
            continue;
        }
        let object = repository.read_object(object_id)?;
        if object.kind != ObjectKind::Commit {
            continue;
        }
        let commit = parse_commit(&object.data)?;
        insert_commit(connection, object_id, &commit)?;
        for (parent_order, parent) in commit.parents.iter().copied().enumerate() {
            insert_commit_parent(connection, object_id, parent_order, parent)?;
            stack.push(parent);
        }
        indexed += 1;
    }
    Ok(indexed)
}

fn object_id_from_snapshot_bytes(bytes: Option<&[u8]>) -> Option<ObjectId> {
    let bytes = bytes?;
    let object_id: [u8; 20] = bytes.try_into().ok()?;
    Some(ObjectId::from_bytes(object_id))
}

fn insert_commit(
    connection: &Connection,
    object_id: ObjectId,
    commit: &crate::Commit,
) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO commits(
                hash_kind, object_id, tree_hash_kind, tree_object_id,
                author_name, author_email, author_timestamp, author_offset,
                committer_name, committer_email, committer_timestamp, committer_offset,
                message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                "sha1",
                object_id.as_bytes().to_vec(),
                "sha1",
                commit.tree.as_bytes().to_vec(),
                commit.author.name,
                commit.author.email,
                commit.author.timestamp,
                commit.author.offset,
                commit.committer.name,
                commit.committer.email,
                commit.committer.timestamp,
                commit.committer.offset,
                commit.message,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn insert_commit_parent(
    connection: &Connection,
    object_id: ObjectId,
    parent_order: usize,
    parent: ObjectId,
) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO commit_parents(
                hash_kind, object_id, parent_order, parent_hash_kind, parent_object_id
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "sha1",
                object_id.as_bytes().to_vec(),
                parent_order as i64,
                "sha1",
                parent.as_bytes().to_vec(),
            ],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn read_schema_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn ensure_supported_schema(connection: &Connection) -> Result<()> {
    let schema_version = read_schema_version(connection).map_err(sqlite_error)?;
    if schema_version != INDEXDB_SCHEMA_VERSION {
        return Err(RitError::invalid_input(format!(
            "indexdb schema version is {schema_version}, expected {INDEXDB_SCHEMA_VERSION}; run `rit indexdb rebuild`"
        )));
    }
    Ok(())
}

fn sqlite_error(error: rusqlite::Error) -> RitError {
    RitError::invalid_input(format!("SQLite indexdb error: {error}"))
}

const INDEXDB_SCHEMA: &str = "
PRAGMA user_version = 1;

CREATE TABLE IF NOT EXISTS cache_state (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS commits (
    hash_kind TEXT NOT NULL,
    object_id BLOB NOT NULL,
    tree_hash_kind TEXT NOT NULL,
    tree_object_id BLOB NOT NULL,
    author_name TEXT NOT NULL,
    author_email TEXT NOT NULL,
    author_timestamp INTEGER NOT NULL,
    author_offset TEXT NOT NULL,
    committer_name TEXT NOT NULL,
    committer_email TEXT NOT NULL,
    committer_timestamp INTEGER NOT NULL,
    committer_offset TEXT NOT NULL,
    message TEXT NOT NULL,
    PRIMARY KEY (hash_kind, object_id)
);

CREATE TABLE IF NOT EXISTS commit_parents (
    hash_kind TEXT NOT NULL,
    object_id BLOB NOT NULL,
    parent_order INTEGER NOT NULL,
    parent_hash_kind TEXT NOT NULL,
    parent_object_id BLOB NOT NULL,
    PRIMARY KEY (hash_kind, object_id, parent_order)
);

CREATE TABLE IF NOT EXISTS file_changes (
    hash_kind TEXT NOT NULL,
    object_id BLOB NOT NULL,
    path TEXT NOT NULL,
    change_kind TEXT NOT NULL,
    path_hash_kind TEXT,
    path_object_id BLOB,
    mode INTEGER,
    PRIMARY KEY (hash_kind, object_id, path)
);

CREATE TABLE IF NOT EXISTS refs_snapshot (
    name TEXT PRIMARY KEY NOT NULL,
    hash_kind TEXT,
    object_id BLOB,
    target TEXT
);
";
