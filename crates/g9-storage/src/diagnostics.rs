//! Read-only diagnostics for the performance tooling (`docs/release/path-to-10.md`
//! PERF-1 and PERF-3). Nothing here is reachable from `g9_app::dispatch`, and
//! nothing here writes: [`explain_query_plan`] refuses every text that is not
//! one query or data change ([`is_plannable`]), because `EXPLAIN QUERY PLAN`
//! still prepares the statement and some `PRAGMA`s act while they are
//! prepared (`PRAGMA foreign_keys=OFF` would switch enforcement off).
//!
//! | Item | Use |
//! |---|---|
//! | [`explain_query_plan`] | the plan SQLite chooses for one query or data change (`EXPLAIN QUERY PLAN`, parameters unbound) |
//! | [`user_tables`], [`row_count`] | the tables of the schema and their sizes |
//! | [`has_planner_statistics`] | whether `sqlite_stat*` tables exist (plans then depend on row counts) |
//! | [`dump_table`] | every row of one table in `rowid` order, for a canonical checksum of a fixture |
//! | [`foreign_keys`], [`lookup_indexes`] | the schema's foreign keys and the column lists a lookup can use (for the check that a parent-row change never scans a child table) |
//! | `StatementCapture` (feature `statement-trace`) | the SQL text of every statement a connection runs |
//!
//! Every statement this module runs starts with the comment [`MARKER`], so
//! that a capture of what the application ran leaves the tooling's own reads
//! out by that marker alone ([`is_own_statement`]), never by the shape of a
//! statement (the backup runs `SELECT COUNT(*) FROM "<table>"` too).
//!
//! Table names are never taken from a caller verbatim: [`row_count`] and
//! [`dump_table`] accept only a name that [`user_tables`] lists, and quote it.
//! The text given to [`explain_query_plan`] is SQL of this crate (captured
//! from a workload, or a string literal of its source), never operator input.

use crate::error::{StorageError, StorageResult};
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// `concat!` of [`MARKER`] and a statement.
macro_rules! marked {
    ($sql:literal) => {
        concat!("/* g9-storage diagnostics */ ", $sql)
    };
}

/// The comment (and the space after it) every statement of this module
/// starts with. No other statement of the workspace carries it (the
/// query-plan gate, `apps/cli/tests/query_plans.rs`, checks the sources).
pub const MARKER: &str = marked!("");

const USER_TABLES_SQL: &str = marked!("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY name");
const STATISTICS_SQL: &str = marked!("SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name LIKE 'sqlite\\_stat%' ESCAPE '\\'");
const FOREIGN_KEYS_SQL: &str = marked!(
    "SELECT id, seq, \"table\", \"from\" FROM pragma_foreign_key_list(?1) ORDER BY id, seq"
);
const INDEX_LIST_SQL: &str =
    marked!("SELECT name, partial FROM pragma_index_list(?1) ORDER BY name");
const INDEX_INFO_SQL: &str = marked!("SELECT name FROM pragma_index_info(?1) ORDER BY seqno");
const PRIMARY_KEY_SQL: &str =
    marked!("SELECT name FROM pragma_table_info(?1) WHERE pk > 0 ORDER BY pk");

/// Whether `sql` is one of this module's own statements: it starts with
/// [`MARKER`] (leading whitespace aside). Nothing else is recognised, so a
/// statement of the application is never left out of a capture because it
/// looks like one of the tooling's reads.
pub fn is_own_statement(sql: &str) -> bool {
    sql.trim_start().starts_with(MARKER)
}

/// Whether `sql` is one query or data change: its first word is `SELECT`,
/// `INSERT`, `UPDATE`, `DELETE`, `WITH` or `REPLACE` (case ignored). `BEGIN`,
/// `COMMIT`, `PRAGMA`, `ATTACH`, DDL, `SAVEPOINT`, `EXPLAIN` and text that
/// starts with a comment are not.
pub fn is_plannable(sql: &str) -> bool {
    let first = sql
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(
        first.as_str(),
        "SELECT" | "INSERT" | "UPDATE" | "DELETE" | "WITH" | "REPLACE"
    )
}

/// One line of `EXPLAIN QUERY PLAN` output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    /// Node id.
    pub id: i64,
    /// Parent node id (0 for a top-level node).
    pub parent: i64,
    /// SQLite's description, e.g. `SCAN audit_metadata` or
    /// `SEARCH notes USING INDEX idx_notes_session (session_id=?)`.
    pub detail: String,
}

/// The query plan of `sql`, one query or data change ([`is_plannable`];
/// anything else is refused with [`StorageError::InvalidInput`] before SQLite
/// sees it). Parameters stay unbound: the plan does not depend on their values
/// while the database holds no `sqlite_stat*` statistics (the schema creates
/// none and nothing runs `ANALYZE`). The statement is prepared under
/// `EXPLAIN QUERY PLAN` and never stepped, and text with a second statement is
/// refused by the prepare (rusqlite `MultipleStatement`; SQLite compiles only
/// the first).
pub fn explain_query_plan(conn: &Connection, sql: &str) -> StorageResult<Vec<PlanStep>> {
    if !is_plannable(sql) {
        return Err(StorageError::InvalidInput(
            "explain_query_plan accepts one SELECT, INSERT, UPDATE, DELETE, WITH or REPLACE statement"
                .to_string(),
        ));
    }
    let mut stmt = conn.prepare(&format!("{MARKER}EXPLAIN QUERY PLAN {sql}"))?;
    let mut rows = stmt.raw_query();
    let mut steps = Vec::new();
    while let Some(row) = rows.next()? {
        steps.push(PlanStep {
            id: row.get(0)?,
            parent: row.get(1)?,
            detail: row.get(3)?,
        });
    }
    Ok(steps)
}

/// Names of the schema's own tables (`sqlite_*` internals excluded), sorted.
pub fn user_tables(conn: &Connection) -> StorageResult<Vec<String>> {
    let mut stmt = conn.prepare(USER_TABLES_SQL)?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

/// Whether the database holds planner statistics (`sqlite_stat1`, written by
/// `ANALYZE` or `PRAGMA optimize`). Without them SQLite plans every statement
/// from the schema alone, so a plan does not depend on how many rows a table
/// holds.
pub fn has_planner_statistics(conn: &Connection) -> StorageResult<bool> {
    let n: i64 = conn.query_row(STATISTICS_SQL, [], |r| r.get(0))?;
    Ok(n > 0)
}

/// `"name"` with embedded quotes doubled, after checking that `table` is one of
/// [`user_tables`].
fn quoted_table(conn: &Connection, table: &str) -> StorageResult<String> {
    if !user_tables(conn)?.iter().any(|t| t == table) {
        return Err(StorageError::NotFound("table"));
    }
    Ok(format!("\"{}\"", table.replace('"', "\"\"")))
}

/// Number of rows of `table` (one of [`user_tables`]).
pub fn row_count(conn: &Connection, table: &str) -> StorageResult<u64> {
    let quoted = quoted_table(conn, table)?;
    let n: i64 = conn.query_row(&format!("{MARKER}SELECT COUNT(*) FROM {quoted}"), [], |r| {
        r.get(0)
    })?;
    u64::try_from(n).map_err(|_| StorageError::Internal("diagnostics.row_count"))
}

/// One stored value, as SQLite holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cell {
    /// `NULL`.
    Null,
    /// `INTEGER`.
    Integer(i64),
    /// `REAL`.
    Real(f64),
    /// `TEXT` (invalid UTF-8 is replaced, never dropped).
    Text(String),
    /// `BLOB`.
    Blob(Vec<u8>),
}

/// Every row of one table, in `rowid` order (the schema has no `WITHOUT ROWID`
/// table; a table without a `rowid` is refused).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableDump {
    /// Table name.
    pub table: String,
    /// Column names in declaration order.
    pub columns: Vec<String>,
    /// Rows, each with one cell per column.
    pub rows: Vec<Vec<Cell>>,
}

/// Dump `table` (one of [`user_tables`]) in `rowid` order.
pub fn dump_table(conn: &Connection, table: &str) -> StorageResult<TableDump> {
    let quoted = quoted_table(conn, table)?;
    let mut stmt = conn.prepare(&format!("{MARKER}SELECT * FROM {quoted} ORDER BY rowid"))?;
    let columns: Vec<String> = stmt.column_names().map(str::to_string).collect();
    let width = columns.len();
    let mut rows = Vec::new();
    let mut cursor = stmt.query([])?;
    while let Some(row) = cursor.next()? {
        let mut cells = Vec::with_capacity(width);
        for i in 0..width {
            cells.push(match row.get_ref(i)? {
                ValueRef::Null => Cell::Null,
                ValueRef::Integer(v) => Cell::Integer(v),
                ValueRef::Real(v) => Cell::Real(v),
                ValueRef::Text(t) => Cell::Text(String::from_utf8_lossy(t).into_owned()),
                ValueRef::Blob(b) => Cell::Blob(b.to_vec()),
            });
        }
        rows.push(cells);
    }
    Ok(TableDump {
        table: table.to_string(),
        columns,
        rows,
    })
}

/// One foreign key, seen from the referencing (child) table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKey {
    /// Referencing table.
    pub table: String,
    /// Referencing columns, in key order.
    pub columns: Vec<String>,
    /// Referenced (parent) table.
    pub parent: String,
}

/// Every foreign key of the schema's tables ([`user_tables`] order).
pub fn foreign_keys(conn: &Connection) -> StorageResult<Vec<ForeignKey>> {
    let mut out = Vec::new();
    for table in user_tables(conn)? {
        let mut stmt = conn.prepare(FOREIGN_KEYS_SQL)?;
        let rows = stmt
            .query_map([&table], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut current: Option<(i64, ForeignKey)> = None;
        for (id, parent, column) in rows {
            match &mut current {
                Some((key, fk)) if *key == id => fk.columns.push(column),
                _ => {
                    if let Some((_, fk)) = current.take() {
                        out.push(fk);
                    }
                    current = Some((
                        id,
                        ForeignKey {
                            table: table.clone(),
                            columns: vec![column],
                            parent,
                        },
                    ));
                }
            }
        }
        if let Some((_, fk)) = current {
            out.push(fk);
        }
    }
    Ok(out)
}

/// The column lists SQLite can search `table` by (one of [`user_tables`]):
/// the primary key and every index that is not partial (a partial index
/// serves only queries that imply its `WHERE`), each in index order.
pub fn lookup_indexes(conn: &Connection, table: &str) -> StorageResult<Vec<Vec<String>>> {
    if !user_tables(conn)?.iter().any(|t| t == table) {
        return Err(StorageError::NotFound("table"));
    }
    let mut out = Vec::new();
    let primary: Vec<String> = conn
        .prepare(PRIMARY_KEY_SQL)?
        .query_map([table], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if !primary.is_empty() {
        out.push(primary);
    }
    let indexes: Vec<(String, bool)> = conn
        .prepare(INDEX_LIST_SQL)?
        .query_map([table], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (name, partial) in indexes {
        if partial {
            continue;
        }
        let columns: Vec<String> = conn
            .prepare(INDEX_INFO_SQL)?
            .query_map([&name], |r| r.get::<_, Option<String>>(0))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            // An expression column has no name; the index cannot serve a
            // plain column lookup from that position on.
            .map_while(|c| c)
            .collect();
        if !columns.is_empty() {
            out.push(columns);
        }
    }
    Ok(out)
}

#[cfg(feature = "statement-trace")]
pub use capture::StatementCapture;

#[cfg(feature = "statement-trace")]
mod capture {
    use crate::error::StorageResult;
    use rusqlite::trace::{TraceEvent, TraceEventCodes};
    use rusqlite::Connection;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    /// The SQL text of every statement a connection runs, with how often each
    /// started, recorded through SQLite's `sqlite3_trace_v2` statement event
    /// (feature `statement-trace`, which enables rusqlite's `trace`; only the
    /// performance tooling turns it on). The text is the event's unexpanded
    /// SQL, without bound values, so nothing a parameter carries is recorded.
    /// When a trigger program starts, SQLite reports a `-- TRIGGER <name>`
    /// comment instead of a statement; those are kept apart
    /// ([`StatementCapture::triggers`]).
    #[derive(Debug, Clone, Default)]
    pub struct StatementCapture {
        seen: Arc<Mutex<BTreeMap<String, u64>>>,
    }

    impl StatementCapture {
        /// Start recording on `conn` (replaces any trace callback set before).
        pub fn install(conn: &mut Connection) -> StorageResult<Self> {
            let capture = Self::default();
            let seen = capture.seen.clone();
            conn.trace_v2(
                TraceEventCodes::SQLITE_TRACE_STMT,
                Some(move |event: TraceEvent<'_>| {
                    // The second field is SQLite's `X` argument: the
                    // statement's unexpanded text, or `-- TRIGGER <name>`
                    // when a trigger program starts (the statement pointer
                    // then still names the outer statement).
                    if let TraceEvent::Stmt(_, sql) = event {
                        if let Ok(mut map) = seen.lock() {
                            *map.entry(sql.trim().to_string()).or_insert(0) += 1;
                        }
                    }
                }),
            )?;
            Ok(capture)
        }

        /// Stop recording on `conn` (what was recorded stays readable).
        pub fn uninstall(conn: &mut Connection) -> StorageResult<()> {
            conn.trace_v2(TraceEventCodes::empty(), None::<fn(TraceEvent<'_>)>)?;
            Ok(())
        }

        /// Distinct statements with how often each started (an approximation
        /// of runs: SQLite may report a statement again at points of its
        /// execution), trigger markers and this module's own statements
        /// ([`super::is_own_statement`]) excluded.
        pub fn statements(&self) -> BTreeMap<String, u64> {
            self.entries(|sql| !sql.starts_with("--") && !super::is_own_statement(sql))
        }

        /// Trigger programs that ran (`-- TRIGGER <name>` markers) with their counts.
        pub fn triggers(&self) -> BTreeMap<String, u64> {
            self.entries(|sql| sql.starts_with("--"))
        }

        fn entries(&self, keep: impl Fn(&str) -> bool) -> BTreeMap<String, u64> {
            self.seen
                .lock()
                .map(|map| {
                    map.iter()
                        .filter(|(sql, _)| keep(sql))
                        .map(|(sql, n)| (sql.clone(), *n))
                        .collect()
                })
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, OpenOptions};
    use g9_domain::UtcMillis;

    fn db() -> Database {
        Database::open_in_memory(OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)))
            .unwrap()
    }

    #[test]
    fn plans_name_the_index_or_the_scan() {
        let db = db();
        let indexed = explain_query_plan(
            db.conn(),
            "SELECT id FROM study_sessions WHERE training_day_id = ?1",
        )
        .unwrap();
        assert!(
            indexed
                .iter()
                .any(|s| s.detail.starts_with("SEARCH study_sessions USING")),
            "{indexed:?}"
        );
        let scan =
            explain_query_plan(db.conn(), "SELECT id FROM study_sessions WHERE mode = ?1").unwrap();
        assert!(
            scan.iter().any(|s| s.detail == "SCAN study_sessions"),
            "{scan:?}"
        );
        assert!(explain_query_plan(db.conn(), "SELECT nope FROM nowhere").is_err());
        // A plan never runs the statement, and a second statement is refused.
        assert!(explain_query_plan(db.conn(), "DELETE FROM schema_migrations").is_ok());
        assert!(explain_query_plan(db.conn(), "SELECT 1; DROP TABLE notes").is_err());
        assert!(row_count(db.conn(), "schema_migrations").unwrap() > 0);
        assert!(user_tables(db.conn())
            .unwrap()
            .contains(&"notes".to_string()));
    }

    #[test]
    fn tables_are_listed_counted_and_dumped_in_rowid_order() {
        let db = db();
        let tables = user_tables(db.conn()).unwrap();
        assert!(tables.contains(&"audit_metadata".to_string()));
        assert!(tables.iter().all(|t| !t.starts_with("sqlite_")));
        let mut sorted = tables.clone();
        sorted.sort();
        assert_eq!(tables, sorted);
        assert!(!has_planner_statistics(db.conn()).unwrap());
        let migrations = row_count(db.conn(), "schema_migrations").unwrap();
        assert!(migrations > 0);
        let dump = dump_table(db.conn(), "schema_migrations").unwrap();
        assert_eq!(u64::try_from(dump.rows.len()).unwrap(), migrations);
        assert_eq!(dump.columns.first().map(String::as_str), Some("version"));
        assert_eq!(dump.rows[0][0], Cell::Integer(1));
        assert!(dump.rows.iter().all(|r| r.len() == dump.columns.len()));
    }

    #[test]
    fn foreign_keys_and_lookup_indexes_are_read() {
        let db = db();
        let fks = foreign_keys(db.conn()).unwrap();
        assert!(fks.len() > 50, "{}", fks.len());
        let intervals = fks
            .iter()
            .find(|f| f.table == "time_intervals" && f.parent == "study_sessions")
            .unwrap();
        assert_eq!(intervals.columns, vec!["session_id".to_string()]);
        let lookups = lookup_indexes(db.conn(), "time_intervals").unwrap();
        assert_eq!(
            lookups.first(),
            Some(&vec!["id".to_string()]),
            "{lookups:?}"
        );
        // idx_time_intervals_session; idx_time_intervals_open (also on
        // session_id) is partial and is not a general lookup.
        assert_eq!(
            lookups
                .iter()
                .filter(|c| c.first().map(String::as_str) == Some("session_id"))
                .count(),
            1,
            "{lookups:?}"
        );
        assert_eq!(
            lookup_indexes(db.conn(), "no_such_table"),
            Err(StorageError::NotFound("table"))
        );
    }

    #[test]
    fn own_statements_are_recognised_by_the_marker_only() {
        for sql in [
            USER_TABLES_SQL,
            STATISTICS_SQL,
            FOREIGN_KEYS_SQL,
            INDEX_LIST_SQL,
            INDEX_INFO_SQL,
            PRIMARY_KEY_SQL,
        ] {
            assert!(is_own_statement(sql), "{sql}");
        }
        assert!(is_own_statement(&format!("  {MARKER}SELECT 1")));
        // The shapes of the tooling's reads, without the marker, are the
        // application's statements (`migrations::table_counts` runs the first).
        for sql in [
            "SELECT COUNT(*) FROM \"notes\"",
            "SELECT * FROM \"notes\" ORDER BY rowid",
            "EXPLAIN QUERY PLAN SELECT 1",
            "SELECT name FROM sqlite_schema WHERE type = 'table'",
            "SELECT 1 /* g9-storage diagnostics */",
        ] {
            assert!(!is_own_statement(sql), "{sql}");
        }
    }

    #[test]
    fn plannable_statements_are_queries_and_data_changes() {
        for sql in [
            "SELECT 1",
            " select x from t",
            "WITH a AS (SELECT 1) SELECT * FROM a",
            "INSERT INTO t VALUES (1)",
            "REPLACE INTO t VALUES (1)",
            "UPDATE t SET x = 1",
            "DELETE FROM t",
            "SELECT(1)",
        ] {
            assert!(is_plannable(sql), "{sql}");
        }
        for sql in [
            "BEGIN IMMEDIATE",
            "COMMIT",
            "PRAGMA foreign_keys",
            "CREATE TABLE t (x)",
            "ATTACH DATABASE 'x' AS y",
            "EXPLAIN SELECT 1",
            "-- TRIGGER x",
            "/* c */ SELECT 1",
            "",
        ] {
            assert!(!is_plannable(sql), "{sql}");
        }
    }

    #[test]
    fn a_pragma_is_refused_and_leaves_the_connection_unchanged() {
        let db = db();
        let read = |sql: &str| -> i64 { db.conn().query_row(sql, [], |r| r.get(0)).unwrap() };
        // `case_sensitive_like` has no read form: `'a' LIKE 'A'` shows it.
        let state = [
            "PRAGMA foreign_keys",
            "PRAGMA trusted_schema",
            "PRAGMA writable_schema",
            "SELECT 'a' LIKE 'A'",
        ];
        let before: Vec<i64> = state.iter().map(|s| read(s)).collect();
        assert_eq!(before[0], 1, "the test database enforces foreign keys");
        assert_eq!(before[2], 0);
        assert_eq!(before[3], 1);
        let accepted: Vec<&str> = [
            "PRAGMA foreign_keys=OFF",
            "PRAGMA trusted_schema=1",
            "PRAGMA case_sensitive_like=1",
            "PRAGMA writable_schema=1",
            " pragma foreign_keys = 0",
            "/* comment */ PRAGMA foreign_keys=OFF",
            "ATTACH DATABASE ':memory:' AS other",
        ]
        .into_iter()
        .filter(|sql| {
            !matches!(
                explain_query_plan(db.conn(), sql),
                Err(StorageError::InvalidInput(_))
            )
        })
        .collect();
        let after: Vec<i64> = state.iter().map(|s| read(s)).collect();
        // Several of these pragmas act while the statement is prepared, so
        // an `EXPLAIN QUERY PLAN` of them would change the connection.
        assert_eq!(before, after, "connection state changed");
        assert!(accepted.is_empty(), "not refused: {accepted:?}");
    }

    #[test]
    fn unknown_or_injected_table_names_are_refused() {
        let db = db();
        for name in ["no_such_table", "notes; DROP TABLE notes", "\"notes\""] {
            assert_eq!(
                row_count(db.conn(), name),
                Err(StorageError::NotFound("table"))
            );
            assert!(dump_table(db.conn(), name).is_err());
        }
    }

    #[cfg(feature = "statement-trace")]
    #[test]
    fn capture_records_statement_text_without_values() {
        let mut db = db();
        let capture = StatementCapture::install(db.conn_mut()).unwrap();
        db.conn()
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE title = ?1",
                ["secret title"],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        StatementCapture::uninstall(db.conn_mut()).unwrap();
        db.conn()
            .query_row("SELECT COUNT(*) FROM weeks", [], |r| r.get::<_, i64>(0))
            .unwrap();
        let seen = capture.statements();
        assert_eq!(
            seen.get("SELECT COUNT(*) FROM notes WHERE title = ?1"),
            Some(&1)
        );
        assert!(!seen.keys().any(|s| s.contains("secret title")));
        assert!(!seen.keys().any(|s| s.contains("weeks")), "{seen:?}");
        // A trigger program is reported as a marker, apart from statements.
        db.conn()
            .execute_batch(
                "CREATE TEMP TABLE perf_t (x INTEGER); \
                 CREATE TEMP TRIGGER perf_tr AFTER INSERT ON perf_t BEGIN SELECT 1; END;",
            )
            .unwrap();
        let capture = StatementCapture::install(db.conn_mut()).unwrap();
        db.conn()
            .execute("INSERT INTO perf_t (x) VALUES (?1)", [7])
            .unwrap();
        assert_eq!(
            capture.triggers().get("-- TRIGGER perf_tr"),
            Some(&1),
            "{:?}",
            capture.triggers()
        );
        assert_eq!(
            capture
                .statements()
                .get("INSERT INTO perf_t (x) VALUES (?1)"),
            Some(&1),
            "{:?}",
            capture.statements()
        );
        StatementCapture::uninstall(db.conn_mut()).unwrap();
        // The tooling's own reads are not the application's statements.
        let capture = StatementCapture::install(db.conn_mut()).unwrap();
        row_count(db.conn(), "notes").unwrap();
        dump_table(db.conn(), "notes").unwrap();
        explain_query_plan(db.conn(), "SELECT 1").unwrap();
        has_planner_statistics(db.conn()).unwrap();
        foreign_keys(db.conn()).unwrap();
        lookup_indexes(db.conn(), "notes").unwrap();
        assert!(
            capture.statements().is_empty(),
            "{:?}",
            capture.statements()
        );
        // ... but the same shape run by the application is: the backup's
        // per-table counts (`migrations::table_counts`) are kept.
        crate::migrations::table_counts(db.conn()).unwrap();
        let seen = capture.statements();
        StatementCapture::uninstall(db.conn_mut()).unwrap();
        assert_eq!(
            seen.get("SELECT COUNT(*) FROM \"notes\""),
            Some(&1),
            "{seen:?}"
        );
        assert!(seen.keys().all(|s| !is_own_statement(s)), "{seen:?}");
    }
}
