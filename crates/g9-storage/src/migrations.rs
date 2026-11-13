//! Embedded, checksummed, transactional schema migrations (ARCHITECTURE.md §6
//! "Migrations"; runbook `docs/runbooks/backup-restore.md` § Migration).
//!
//! * Migrations are numbered SQL files embedded with `include_str!`.
//! * `schema_migrations(version, name, sha256, applied_at, app_version)` is
//!   bootstrapped before the first migration runs (it must exist to record it).
//! * Each migration runs inside one `BEGIN IMMEDIATE` transaction together with
//!   its bookkeeping row, so a failure leaves no partial schema.
//! * On every open the recorded checksums are compared with the embedded SQL;
//!   a mismatch is `DATA_CORRUPTION` (someone edited an applied migration or the
//!   file is not ours).
//! * A database whose recorded version exceeds the embedded maximum was written
//!   by a newer build and is refused (`PRECONDITION_FAILED`, forward-only rule C12).
//! * [`Migrator::dry_run_on_copy`] rehearses pending migrations on a
//!   `VACUUM INTO` copy and reports integrity, foreign-key and count results
//!   without touching the live file.
//! * [`verify_schema`] compares the schema itself (every table, index, trigger
//!   and view in `sqlite_master`) with what the embedded migrations produce at
//!   the version the file records. Checksums alone only prove that the
//!   `schema_migrations` rows are genuine: they are data inside the file, and a
//!   copied genuine database keeps them while its triggers are dropped or
//!   replaced (security audit 2026-11-12). Backup verification, restore and
//!   open-time recovery all call it.

use crate::error::{StorageError, StorageResult};
use g9_domain::hashing::sha256_hex;
use g9_domain::UtcMillis;
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// One embedded migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Monotonic version number (1-based, contiguous).
    pub version: u32,
    /// File stem, e.g. `0001_governance`.
    pub name: &'static str,
    /// SQL text as embedded.
    pub sql: &'static str,
}

impl Migration {
    /// Hex SHA-256 of the embedded SQL bytes.
    pub fn sha256(&self) -> String {
        sha256_hex(self.sql.as_bytes())
    }
}

/// All migrations in application order.
pub const EMBEDDED_MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_governance",
        sql: include_str!("migrations/0001_governance.sql"),
    },
    Migration {
        version: 2,
        name: "0002_curriculum",
        sql: include_str!("migrations/0002_curriculum.sql"),
    },
    Migration {
        version: 3,
        name: "0003_competency",
        sql: include_str!("migrations/0003_competency.sql"),
    },
    Migration {
        version: 4,
        name: "0004_calendar",
        sql: include_str!("migrations/0004_calendar.sql"),
    },
    Migration {
        version: 5,
        name: "0005_evidence",
        sql: include_str!("migrations/0005_evidence.sql"),
    },
    Migration {
        version: 6,
        name: "0006_assessment",
        sql: include_str!("migrations/0006_assessment.sql"),
    },
    Migration {
        version: 7,
        name: "0007_retention",
        sql: include_str!("migrations/0007_retention.sql"),
    },
    Migration {
        version: 8,
        name: "0008_knowledge_ai",
        sql: include_str!("migrations/0008_knowledge_ai.sql"),
    },
    Migration {
        version: 9,
        name: "0009_business",
        sql: include_str!("migrations/0009_business.sql"),
    },
    Migration {
        version: 10,
        name: "0010_intervention_review_and_reliability",
        sql: include_str!("migrations/0010_intervention_review_and_reliability.sql"),
    },
    Migration {
        version: 11,
        name: "0011_time_interval_immutability",
        sql: include_str!("migrations/0011_time_interval_immutability.sql"),
    },
    Migration {
        version: 12,
        name: "0012_rubric_anchors",
        sql: include_str!("migrations/0012_rubric_anchors.sql"),
    },
    Migration {
        version: 13,
        name: "0013_grade_observations",
        sql: include_str!("migrations/0013_grade_observations.sql"),
    },
];

/// Highest embedded migration version.
pub const CURRENT_SCHEMA_VERSION: u32 = 13;

const BOOTSTRAP_SQL: &str = "CREATE TABLE IF NOT EXISTS schema_migrations (
  version     INTEGER PRIMARY KEY NOT NULL CHECK (version >= 1),
  name        TEXT    NOT NULL,
  sha256      TEXT    NOT NULL CHECK (length(sha256) = 64),
  applied_at  INTEGER NOT NULL,
  app_version TEXT    NOT NULL
) STRICT;";

/// A `schema_migrations` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedMigration {
    /// Version number.
    pub version: u32,
    /// Migration name.
    pub name: String,
    /// Recorded checksum.
    pub sha256: String,
    /// When it was applied.
    pub applied_at: UtcMillis,
    /// Application version that applied it.
    pub app_version: String,
}

/// Result of [`Migrator::migrate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    /// Schema version before the run.
    pub schema_version_before: u32,
    /// Schema version after the run.
    pub schema_version_after: u32,
    /// Migrations applied by this run, in order.
    pub applied: Vec<AppliedMigration>,
    /// Embedded migrations that were already recorded.
    pub already_applied: u32,
}

/// One checksum mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecksumMismatch {
    /// Migration version.
    pub version: u32,
    /// Checksum recorded in the database.
    pub recorded: String,
    /// Checksum of the embedded SQL (`None` when the version is unknown to this build).
    pub embedded: Option<String>,
}

/// Result of [`Migrator::verify_checksums`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecksumReport {
    /// Versions whose checksums matched.
    pub verified: Vec<u32>,
    /// Mismatches (empty when `ok`).
    pub mismatches: Vec<ChecksumMismatch>,
    /// Highest recorded version.
    pub db_schema_version: u32,
    /// `true` when there are no mismatches.
    pub ok: bool,
}

/// Result of [`Migrator::dry_run_on_copy`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunReport {
    /// Path of the migrated copy (left in place for inspection; the caller deletes it).
    pub copy_path: PathBuf,
    /// Migration report produced on the copy.
    pub report: MigrationReport,
    /// `PRAGMA integrity_check` returned `ok` on the migrated copy.
    pub integrity_ok: bool,
    /// Number of `PRAGMA foreign_key_check` violations on the migrated copy.
    pub foreign_key_violations: u64,
    /// Row counts per application table on the migrated copy.
    pub table_counts: BTreeMap<String, u64>,
}

/// Applies and verifies embedded migrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migrator {
    app_version: String,
    now: UtcMillis,
}

impl Migrator {
    /// A migrator that records `app_version` and `now` on every row it applies.
    pub fn new(app_version: impl Into<String>, now: UtcMillis) -> Self {
        Self {
            app_version: app_version.into(),
            now,
        }
    }

    /// Create `schema_migrations` if it does not exist.
    pub fn bootstrap(conn: &Connection) -> StorageResult<()> {
        conn.execute_batch(BOOTSTRAP_SQL)?;
        Ok(())
    }

    /// Recorded migrations in version order.
    pub fn applied(conn: &Connection) -> StorageResult<Vec<AppliedMigration>> {
        let mut stmt =
            conn.prepare("SELECT version, name, sha256, applied_at, app_version FROM schema_migrations ORDER BY version")?;
        let rows = stmt.query_map([], |r| {
            Ok(AppliedMigration {
                version: r.get::<_, i64>(0)? as u32,
                name: r.get(1)?,
                sha256: r.get(2)?,
                applied_at: UtcMillis::from_millis(r.get(3)?),
                app_version: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Embedded versions not yet recorded in `schema_migrations`, ascending.
    pub fn pending(conn: &Connection) -> StorageResult<Vec<u32>> {
        let applied: std::collections::BTreeSet<u32> = Self::applied(conn)?
            .into_iter()
            .map(|m| m.version)
            .collect();
        Ok(EMBEDDED_MIGRATIONS
            .iter()
            .map(|m| m.version)
            .filter(|v| !applied.contains(v))
            .collect())
    }

    /// Highest recorded version (0 when none).
    pub fn schema_version(conn: &Connection) -> StorageResult<u32> {
        let v: Option<i64> =
            conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })?;
        Ok(v.unwrap_or(0).max(0) as u32)
    }

    /// Compare every recorded checksum with the embedded SQL.
    pub fn verify_checksums(conn: &Connection) -> StorageResult<ChecksumReport> {
        let applied = Self::applied(conn)?;
        let mut verified = Vec::new();
        let mut mismatches = Vec::new();
        let mut db_schema_version = 0;
        for row in applied {
            db_schema_version = db_schema_version.max(row.version);
            match EMBEDDED_MIGRATIONS
                .iter()
                .find(|m| m.version == row.version)
            {
                Some(m) if m.sha256() == row.sha256 => verified.push(row.version),
                Some(m) => mismatches.push(ChecksumMismatch {
                    version: row.version,
                    recorded: row.sha256,
                    embedded: Some(m.sha256()),
                }),
                None => mismatches.push(ChecksumMismatch {
                    version: row.version,
                    recorded: row.sha256,
                    embedded: None,
                }),
            }
        }
        let ok = mismatches.is_empty();
        Ok(ChecksumReport {
            verified,
            mismatches,
            db_schema_version,
            ok,
        })
    }

    /// Verify checksums and refuse to proceed on mismatch or on a newer schema.
    pub fn ensure_checksums(&self, conn: &Connection) -> StorageResult<ChecksumReport> {
        let report = Self::verify_checksums(conn)?;
        if report.db_schema_version > CURRENT_SCHEMA_VERSION {
            return Err(StorageError::SchemaNewerThanApp {
                db_version: report.db_schema_version,
                app_max: CURRENT_SCHEMA_VERSION,
            });
        }
        if let Some(bad) = report.mismatches.first() {
            return Err(StorageError::ChecksumMismatch {
                version: bad.version,
                recorded: bad.recorded.clone(),
                embedded: bad
                    .embedded
                    .clone()
                    .unwrap_or_else(|| "<unknown version>".to_string()),
            });
        }
        Ok(report)
    }

    /// Apply every pending migration (each in its own `BEGIN IMMEDIATE` transaction).
    pub fn migrate(&self, conn: &mut Connection) -> StorageResult<MigrationReport> {
        self.migrate_up_to(conn, CURRENT_SCHEMA_VERSION)
    }

    /// Apply pending migrations up to and including `target` (used by dry runs
    /// and by tests that need a database at an older schema version).
    pub fn migrate_up_to(
        &self,
        conn: &mut Connection,
        target: u32,
    ) -> StorageResult<MigrationReport> {
        Self::bootstrap(conn)?;
        self.ensure_checksums(conn)?;
        let before = Self::schema_version(conn)?;
        let mut applied = Vec::new();
        let mut already_applied = 0;
        for m in EMBEDDED_MIGRATIONS.iter().filter(|m| m.version <= target) {
            if m.version <= before {
                already_applied += 1;
                continue;
            }
            let sha = m.sha256();
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(m.sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, name, sha256, applied_at, app_version) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![i64::from(m.version), m.name, sha, self.now.as_millis(), self.app_version],
            )?;
            tx.commit()?;
            tracing::info!(version = m.version, name = m.name, "migration applied");
            applied.push(AppliedMigration {
                version: m.version,
                name: m.name.to_string(),
                sha256: sha,
                applied_at: self.now,
                app_version: self.app_version.clone(),
            });
        }
        let after = Self::schema_version(conn)?;
        Ok(MigrationReport {
            schema_version_before: before,
            schema_version_after: after,
            applied,
            already_applied,
        })
    }

    /// Rehearse pending migrations on a `VACUUM INTO` copy of `db_path` placed in
    /// `temp_dir`. The live database is opened read-only and never modified; the
    /// copy is left in `temp_dir` for inspection and the caller removes it.
    pub fn dry_run_on_copy(&self, db_path: &Path, temp_dir: &Path) -> StorageResult<DryRunReport> {
        let src = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let file_name = db_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("database");
        let copy_path = temp_dir.join(format!("{file_name}.dry-run.sqlite"));
        if copy_path.exists() {
            return Err(StorageError::Conflict(
                "dry-run copy already exists; remove it before rehearsing again".to_string(),
            ));
        }
        let copy_str = copy_path.to_str().ok_or_else(|| {
            StorageError::InvalidInput("temp_dir path is not valid UTF-8".to_string())
        })?;
        src.execute("VACUUM INTO ?1", params![copy_str])?;
        drop(src);

        let mut copy = Connection::open_with_flags(
            &copy_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        copy.pragma_update(None, "foreign_keys", true)?;
        copy.pragma_update(None, "trusted_schema", false)?;
        let report = self.migrate(&mut copy)?;
        let integrity = integrity_check(&copy)?;
        let foreign_key_violations = foreign_key_check(&copy)?;
        let table_counts = table_counts(&copy)?;
        Ok(DryRunReport {
            copy_path,
            report,
            integrity_ok: integrity.len() == 1 && integrity[0] == "ok",
            foreign_key_violations,
            table_counts,
        })
    }
}

/// `PRAGMA integrity_check` messages (`["ok"]` when healthy).
pub fn integrity_check(conn: &Connection) -> StorageResult<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Number of `PRAGMA foreign_key_check` violations.
pub fn foreign_key_check(conn: &Connection) -> StorageResult<u64> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let rows = stmt.query_map([], |_| Ok(()))?;
    let mut n = 0;
    for row in rows {
        row?;
        n += 1;
    }
    Ok(n)
}

/// Names of application tables (excluding SQLite internals), sorted.
pub fn app_tables(conn: &Connection) -> StorageResult<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Row count per application table.
pub fn table_counts(conn: &Connection) -> StorageResult<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::new();
    for table in app_tables(conn)? {
        // Names come from the file's own sqlite_master, which may be a foreign
        // snapshot under verification: quote them as identifiers.
        let quoted = table.replace('"', "\"\"");
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM \"{quoted}\""), [], |r| {
            r.get(0)
        })?;
        counts.insert(table, n.max(0) as u64);
    }
    Ok(counts)
}

/// At most this many differences are listed; the rest are counted.
const MAX_REPORTED_DIFFERENCES: usize = 20;

/// Result of [`verify_schema`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaCheck {
    /// The schema version the file records (`MAX(schema_migrations.version)`, 0 when unreadable).
    pub version: u32,
    /// What differs from the schema the embedded migrations produce at that version,
    /// for example `unexpected trigger "zz"`, `missing trigger "trg_x"` or
    /// `changed table "notes"`. Names are printed escaped (`{:?}`): they come from
    /// the file. Empty when the schema matches.
    pub differences: Vec<String>,
}

impl SchemaCheck {
    /// `true` when the schema equals the expected one.
    pub fn ok(&self) -> bool {
        self.differences.is_empty()
    }
}

/// `(type, name)` -> `(tbl_name, sql)`.
type SchemaMap = BTreeMap<(String, String), (String, Option<String>)>;

/// Objects SQLite creates and maintains itself. Everything else in
/// `sqlite_master`, whatever its name, must come from the embedded migrations:
/// a `sqlite_`-prefixed trigger written through `writable_schema` is loaded and
/// fires like any other, so names alone never exempt an object.
fn is_internal(kind: &str, name: &str, sql: Option<&str>) -> bool {
    match kind {
        "table" => matches!(
            name,
            "sqlite_sequence" | "sqlite_stat1" | "sqlite_stat2" | "sqlite_stat3" | "sqlite_stat4"
        ),
        // Indexes behind PRIMARY KEY / UNIQUE constraints carry no SQL of their own.
        "index" => sql.is_none() && name.starts_with("sqlite_autoindex_"),
        _ => false,
    }
}

/// Whitespace runs collapse to one space, so formatting cannot cause a mismatch.
fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn schema_map(conn: &Connection) -> StorageResult<SchemaMap> {
    let mut stmt = conn.prepare("SELECT type, name, tbl_name, sql FROM sqlite_master")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut map = SchemaMap::new();
    for row in rows {
        let (kind, name, table, sql) = row?;
        if !is_internal(&kind, &name, sql.as_deref()) {
            map.insert((kind, name), (table, sql.as_deref().map(normalize_sql)));
        }
    }
    Ok(map)
}

/// The schema the embedded migrations produce at `version`, built the way a
/// fresh install builds it (same migrator, foreign keys on) in memory, and kept
/// for the life of the process.
fn expected_schema(version: u32) -> StorageResult<SchemaMap> {
    static CACHE: OnceLock<Mutex<BTreeMap<u32, SchemaMap>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    {
        let guard = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(map) = guard.get(&version) {
            return Ok(map.clone());
        }
    }
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Migrator::new("schema-reference", UtcMillis::from_millis(0))
        .migrate_up_to(&mut conn, version)?;
    let map = schema_map(&conn)?;
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(version, map.clone());
    Ok(map)
}

/// Compare the schema of `conn` with the schema the embedded migrations produce
/// at the version the database records. Reads only; never fails on a foreign
/// schema (the differences say what is wrong); fails only when the database
/// cannot be read at all.
pub fn verify_schema(conn: &Connection) -> StorageResult<SchemaCheck> {
    // A file without a readable schema_migrations table records no version.
    let version = Migrator::schema_version(conn).unwrap_or(0);
    if version > CURRENT_SCHEMA_VERSION {
        return Ok(SchemaCheck {
            version,
            differences: vec![format!(
                "schema version {version} is newer than this build ({CURRENT_SCHEMA_VERSION})"
            )],
        });
    }
    let expected = expected_schema(version)?;
    let actual = schema_map(conn)?;
    let mut differences = Vec::new();
    for ((kind, name), definition) in &actual {
        match expected.get(&(kind.clone(), name.clone())) {
            None => differences.push(format!("unexpected {kind} {name:?}")),
            Some(wanted) if wanted != definition => {
                differences.push(format!("changed {kind} {name:?}"))
            }
            Some(_) => {}
        }
    }
    for (kind, name) in expected.keys() {
        if !actual.contains_key(&(kind.clone(), name.clone())) {
            differences.push(format!("missing {kind} {name:?}"));
        }
    }
    if differences.len() > MAX_REPORTED_DIFFERENCES {
        let more = differences.len() - MAX_REPORTED_DIFFERENCES;
        differences.truncate(MAX_REPORTED_DIFFERENCES);
        differences.push(format!("and {more} more"));
    }
    Ok(SchemaCheck {
        version,
        differences,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_versions_are_contiguous_and_named_uniquely() {
        let mut names = std::collections::HashSet::new();
        for (i, m) in EMBEDDED_MIGRATIONS.iter().enumerate() {
            assert_eq!(m.version, i as u32 + 1);
            assert!(names.insert(m.name));
            assert!(m.name.starts_with(&format!("{:04}_", m.version)));
            assert!(!m.sql.trim().is_empty());
        }
        assert_eq!(EMBEDDED_MIGRATIONS.len() as u32, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent_and_records_checksums() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        let m = Migrator::new("0.1.0-test", UtcMillis::from_millis(1_700_000_000_000));
        let first = m.migrate(&mut conn).unwrap();
        assert_eq!(first.schema_version_before, 0);
        assert_eq!(first.schema_version_after, CURRENT_SCHEMA_VERSION);
        assert_eq!(first.applied.len() as u32, CURRENT_SCHEMA_VERSION);
        let second = m.migrate(&mut conn).unwrap();
        assert!(second.applied.is_empty());
        assert_eq!(second.already_applied, CURRENT_SCHEMA_VERSION);
        let report = Migrator::verify_checksums(&conn).unwrap();
        assert!(report.ok);
        assert_eq!(report.verified.len() as u32, CURRENT_SCHEMA_VERSION);
        assert_eq!(integrity_check(&conn).unwrap(), vec!["ok".to_string()]);
        assert_eq!(foreign_key_check(&conn).unwrap(), 0);
    }

    #[test]
    fn newer_schema_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        let m = Migrator::new("0.1.0-test", UtcMillis::from_millis(1));
        m.migrate(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, name, sha256, applied_at, app_version) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![i64::from(CURRENT_SCHEMA_VERSION + 1), "9999_future", "0".repeat(64), 1_i64, "9.9.9"],
        )
        .unwrap();
        let err = m.ensure_checksums(&conn).unwrap_err();
        assert!(
            matches!(err, StorageError::SchemaNewerThanApp { .. }),
            "{err:?}"
        );
    }
}
