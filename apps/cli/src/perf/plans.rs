//! Reading query plans for the PERF-3 gate (`tests/query_plans.rs`) and the
//! static SQL inventory of `g9-storage` for its coverage report. String work
//! over what `g9_storage::diagnostics::explain_query_plan` returns; the only
//! SQL run here is that function, called by [`plan_all`].
//!
//! A plan step is a finding when it reads a whole table:
//!
//! | `EXPLAIN QUERY PLAN` detail | Kind |
//! |---|---|
//! | `SCAN <t>` | [`ScanKind::Table`] |
//! | `SCAN <t> USING [COVERING] INDEX <i>` | [`ScanKind::FullIndex`]: every entry of the index is visited (in index order) |
//! | `SEARCH <t> USING AUTOMATIC [PARTIAL] COVERING INDEX ...` | [`ScanKind::AutomaticIndex`]: SQLite builds a transient index from a full scan on every run |
//! | `SEARCH <t> USING [COVERING] INDEX <i>` with no `(...)` constraint | [`ScanKind::FullIndex`]: the MIN/MAX optimisation calls the step a search, but nothing narrows it; with a filter it cannot apply through the index it walks the whole index |
//! | `SEARCH <t>` or `SEARCH <t> USING INTEGER PRIMARY KEY` with no constraint | [`ScanKind::Table`]: the same walk in `rowid` order |
//!
//! A `SEARCH` step with a `(...)` constraint (`(id=?)`, `(a>? AND a<?)`),
//! `SCAN CONSTANT ROW`, sub-query and virtual-table (`json_each`) steps are
//! not findings. A genuine MIN/MAX seek without a filter (`SELECT MAX(x) FROM
//! t`, one index entry) is reported too and needs an allowance with a reason.
//! A step names a table by its alias when the statement gives one;
//! [`table_refs`] maps aliases back to tables.

use g9_domain::G9Error;
use g9_storage::{diagnostics, Database};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// How a step reads a whole table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ScanKind {
    /// `SCAN t`.
    Table,
    /// `SCAN t USING [COVERING] INDEX i`.
    FullIndex,
    /// `SEARCH t USING AUTOMATIC ... INDEX`.
    AutomaticIndex,
}

/// One whole-table read in a plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Scan {
    /// Table (alias resolved); the name as written when it could not be resolved.
    pub table: String,
    /// Kind.
    pub kind: ScanKind,
    /// The plan detail line.
    pub detail: String,
}

/// Whitespace runs collapsed to one space, trimmed.
pub fn normalize(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `sql` is one query or data change (the statements
/// [`diagnostics::explain_query_plan`] accepts).
pub use g9_storage::diagnostics::is_plannable;

fn tokens(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            // String literal: skipped as one token.
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            while let Some(d) = chars.next() {
                if d == '\'' {
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                        continue;
                    }
                    break;
                }
            }
            out.push("'…'".to_string());
        } else if c.is_alphanumeric() || c == '_' || c == '"' {
            if c != '"' {
                current.push(c);
            }
        } else {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

const NOT_AN_ALIAS: &[&str] = &[
    "WHERE",
    "ON",
    "JOIN",
    "LEFT",
    "RIGHT",
    "FULL",
    "INNER",
    "OUTER",
    "CROSS",
    "NATURAL",
    "ORDER",
    "GROUP",
    "LIMIT",
    "USING",
    "SET",
    "VALUES",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "HAVING",
    "WINDOW",
    "INDEXED",
    "NOT",
    "RETURNING",
    "DEFAULT",
    "SELECT",
];

fn is_identifier(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// What [`table_refs`] binds an alias to when the statement uses one alias for
/// two different tables: never a table of the schema, so a step that names it
/// is reported as unresolved instead of being attributed to either table.
pub const AMBIGUOUS: &str = "?ambiguous";

/// The names a statement's `FROM` clauses introduce.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    /// `alias → table` (a table without an alias maps to itself; an alias
    /// bound to two different tables maps to [`AMBIGUOUS`]).
    pub tables: BTreeMap<String, String>,
    /// Aliases of `FROM (SELECT ...) alias` sub-queries: intermediate results,
    /// not tables (the reads of their tables are steps of their own).
    pub derived: BTreeSet<String>,
}

impl References {
    fn bind(&mut self, alias: &str, table: &str) {
        match self.tables.get(alias) {
            Some(bound) if bound != table => {
                self.tables.insert(alias.to_string(), AMBIGUOUS.to_string());
            }
            Some(_) => {}
            None => {
                self.tables.insert(alias.to_string(), table.to_string());
            }
        }
    }
}

fn alias_at(toks: &[String], mut j: usize) -> (Option<String>, usize) {
    if toks.get(j).is_some_and(|t| t.eq_ignore_ascii_case("AS")) {
        j += 1;
    }
    match toks
        .get(j)
        .filter(|t| is_identifier(t) && !NOT_AN_ALIAS.iter().any(|k| t.eq_ignore_ascii_case(k)))
    {
        Some(alias) => (Some(alias.clone()), j + 1),
        None => (None, j),
    }
}

/// Keywords that end a `FROM` clause at its own parenthesis depth.
const FROM_ENDS: &[&str] = &[
    "WHERE",
    "GROUP",
    "ORDER",
    "LIMIT",
    "HAVING",
    "WINDOW",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "RETURNING",
    "SELECT",
    "VALUES",
    "SET",
];

/// Keywords that can follow `UPDATE`/`INTO` without naming a table
/// (`ON CONFLICT ... DO UPDATE SET`).
const NOT_A_TABLE: &[&str] = &["SET", "SELECT", "VALUES", "DEFAULT", "OR"];

/// Every name after `FROM`, `JOIN`, `INTO` and `UPDATE`, and after a comma of
/// a `FROM` list (at the list's own parenthesis depth, also after a join
/// constraint), with its alias; the aliases of `FROM (SELECT ...) alias`
/// sub-queries are recorded as derived. Sub-queries are read too: their
/// `FROM` clauses bind their own names.
pub fn references(sql: &str) -> References {
    let toks = tokens(sql);
    let mut refs = References::default();
    // One flag per open parenthesis: inside a FROM clause at that depth.
    let mut in_from: Vec<bool> = vec![false];
    let mut i = 0;
    while i < toks.len() {
        let upper = toks[i].to_ascii_uppercase();
        let depth = in_from.len() - 1;
        let introduces = match upper.as_str() {
            "(" => {
                in_from.push(false);
                false
            }
            ")" => {
                if in_from.len() > 1 {
                    in_from.pop();
                }
                false
            }
            "FROM" => {
                in_from[depth] = true;
                true
            }
            "JOIN" | "INTO" | "UPDATE" => true,
            "," => in_from[depth],
            k if FROM_ENDS.contains(&k) => {
                in_from[depth] = false;
                false
            }
            _ => false,
        };
        if !introduces {
            i += 1;
            continue;
        }
        let j = i + 1;
        if toks.get(j).is_some_and(|t| t == "(") {
            // A sub-query in the FROM clause: its alias is derived, and its
            // tokens are read next (the parenthesis is pushed there).
            let (alias, _) = alias_at(&toks, skip_group(&toks, j));
            if let Some(alias) = alias {
                refs.derived.insert(alias);
            }
            i = j;
            continue;
        }
        match toks
            .get(j)
            .filter(|t| is_identifier(t) && !NOT_A_TABLE.iter().any(|k| t.eq_ignore_ascii_case(k)))
        {
            Some(table) => {
                let table = table.clone();
                let (alias, next) = alias_at(&toks, j + 1);
                if let Some(alias) = alias {
                    refs.bind(&alias, &table);
                }
                refs.bind(&table, &table);
                i = next;
            }
            None => i += 1,
        }
    }
    refs
}

/// Table references of `sql` as `alias → table` ([`References::tables`]).
pub fn table_refs(sql: &str) -> BTreeMap<String, String> {
    references(sql).tables
}

/// Index after the parenthesised group that opens at `toks[open]`.
fn skip_group(toks: &[String], open: usize) -> usize {
    let mut depth = 0usize;
    let mut j = open;
    while j < toks.len() {
        match toks[j].as_str() {
            "(" => depth += 1,
            ")" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    j
}

/// Names of the common table expressions of `sql` (`WITH [RECURSIVE] a(x) AS
/// (...), b AS (...)`). A step that scans one reads an intermediate result,
/// not a table; the reads of its tables are steps of their own.
pub fn cte_names(sql: &str) -> Vec<String> {
    let toks = tokens(sql);
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if !toks[i].eq_ignore_ascii_case("WITH") {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        if toks
            .get(j)
            .is_some_and(|t| t.eq_ignore_ascii_case("RECURSIVE"))
        {
            j += 1;
        }
        while let Some(name) = toks.get(j).filter(|t| is_identifier(t)) {
            out.push(name.clone());
            j += 1;
            if toks.get(j).is_some_and(|t| t == "(") {
                j = skip_group(&toks, j);
            }
            if !toks.get(j).is_some_and(|t| t.eq_ignore_ascii_case("AS")) {
                break;
            }
            j += 1;
            while toks.get(j).is_some_and(|t| {
                t.eq_ignore_ascii_case("NOT") || t.eq_ignore_ascii_case("MATERIALIZED")
            }) {
                j += 1;
            }
            if toks.get(j).is_some_and(|t| t == "(") {
                j = skip_group(&toks, j);
            }
            if toks.get(j).is_some_and(|t| t == ",") {
                j += 1;
                continue;
            }
            break;
        }
        i = j.max(i + 1);
    }
    out
}

/// What a name in a plan step stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Named {
    /// A table (alias resolved).
    Table(String),
    /// A common table expression or a `FROM` sub-query.
    Intermediate,
    /// An alias the statement binds to more than one thing.
    Unresolved(String),
}

fn named(refs: &References, ctes: &[String], name: &str) -> Named {
    let is_cte = |n: &str| ctes.iter().any(|c| c == n);
    let derived = refs.derived.contains(name) || is_cte(name);
    match refs.tables.get(name) {
        Some(t) if t == AMBIGUOUS => Named::Unresolved(format!("{name} {AMBIGUOUS}")),
        Some(t) if is_cte(t) => Named::Intermediate,
        Some(_) if derived => Named::Unresolved(format!("{name} {AMBIGUOUS}")),
        Some(t) => Named::Table(t.clone()),
        None if derived => Named::Intermediate,
        None => Named::Table(name.to_string()),
    }
}

/// Whole-table reads among `details` (the plan of `sql`). A step that names
/// an alias the statement binds to two different things is reported with the
/// table `"<alias> ?ambiguous"` (never a schema table, so the gate lists it
/// as unresolved).
pub fn scans<'a>(sql: &str, details: impl IntoIterator<Item = &'a str>) -> Vec<Scan> {
    let refs = references(sql);
    let ctes = cte_names(sql);
    let mut out = Vec::new();
    for detail in details {
        let (rest, kind) = if let Some(rest) = detail.strip_prefix("SCAN ") {
            if rest.split(' ').next() == Some("CONSTANT") {
                continue;
            }
            let kind = if rest.contains(" USING ") && rest.contains("INDEX") {
                ScanKind::FullIndex
            } else {
                ScanKind::Table
            };
            (rest, kind)
        } else if let Some(rest) = detail.strip_prefix("SEARCH ") {
            if rest.contains(" USING AUTOMATIC ") {
                (rest, ScanKind::AutomaticIndex)
            } else if rest.contains(" (") {
                // A constraint (`(id=?)`, `(a>? AND a<?)`) narrows the read.
                continue;
            } else if rest.contains(" INDEX ") {
                (rest, ScanKind::FullIndex)
            } else {
                (rest, ScanKind::Table)
            }
        } else {
            continue;
        };
        let name = rest.split(' ').next().unwrap_or("");
        if name.is_empty() || name.starts_with('(') || rest.contains(" VIRTUAL TABLE") {
            continue;
        }
        let table = match named(&refs, &ctes, name) {
            Named::Intermediate => continue,
            Named::Table(t) | Named::Unresolved(t) => t,
        };
        out.push(Scan {
            table,
            kind,
            detail: detail.to_string(),
        });
    }
    out
}

/// [`Allowed::table`] of an allowance that covers a whole-table read of any
/// table by the statements of one template (with a [`Shape`] that bounds
/// what those statements may be).
pub const ANY_TABLE: &str = "*";

/// What the statements a template allowance covers must look like as a
/// whole: the template's placeholders match any text, the shape says which
/// text. A template allowance without a shape covers nothing (the gate then
/// reports it as unused).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `SELECT * FROM <table> WHERE <clause> ORDER BY rowid`, `<clause>` built
    /// only from the filter forms of `g9_storage::export` (`filter`): `1 = 1`;
    /// `data_domain IN ('…', …) AND classification IN ('…', …)`;
    /// `<column> IN (SELECT <key> FROM <parent> WHERE <clause>)`; each
    /// optionally followed by ` AND erased = 0`; then any number of
    /// ` AND (<column> IS NULL OR <column> IN (SELECT id FROM <parent> WHERE
    /// <clause>))` and ` AND <column> IS NULL`.
    ExportSelect,
    /// `SELECT COUNT(*) FROM "<table>"`, one quoted plain identifier and
    /// nothing else (`g9_storage::migrations::table_counts`).
    QuotedTableCount,
}

impl Shape {
    /// Whether `sql` (normalized) has this shape.
    pub fn accepts(self, sql: &str) -> bool {
        match self {
            Shape::ExportSelect => export_select(sql),
            Shape::QuotedTableCount => sql
                .strip_prefix("SELECT COUNT(*) FROM \"")
                .and_then(|rest| rest.strip_suffix('"'))
                .is_some_and(|name| {
                    plain_identifier(name).is_some_and(|(_, rest)| rest.is_empty())
                }),
        }
    }
}

/// A lower-case SQL identifier at the start of `s` and the text after it.
fn plain_identifier(s: &str) -> Option<(&str, &str)> {
    let end = s
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        .unwrap_or(s.len());
    let name = s.get(..end)?;
    let rest = s.get(end..)?;
    name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
        .then_some((name, rest))
}

fn eat(s: &mut &str, text: &str) -> bool {
    match s.strip_prefix(text) {
        Some(rest) => {
            *s = rest;
            true
        }
        None => false,
    }
}

fn identifier(s: &mut &str) -> bool {
    match plain_identifier(s) {
        Some((_, rest)) => {
            *s = rest;
            true
        }
        None => false,
    }
}

/// `'A', 'B_C'`: upper-case quoted words separated by `, `.
fn quoted_words(s: &mut &str) -> bool {
    loop {
        if !eat(s, "'") {
            return false;
        }
        let end = s
            .find(|c: char| !(c.is_ascii_uppercase() || c == '_'))
            .unwrap_or(s.len());
        if end == 0 {
            return false;
        }
        *s = s.get(end..).unwrap_or("");
        if !eat(s, "'") {
            return false;
        }
        if !eat(s, ", ") {
            return true;
        }
    }
}

fn export_select(sql: &str) -> bool {
    let mut s = sql;
    eat(&mut s, "SELECT * FROM ")
        && identifier(&mut s)
        && eat(&mut s, " WHERE ")
        && export_clause(&mut s, 0)
        && s == " ORDER BY rowid"
}

/// One export filter (see [`Shape::ExportSelect`]); `depth` bounds the
/// parent chain as `g9_storage::export` does.
fn export_clause(s: &mut &str, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    let mut t = *s;
    let own = |t: &mut &str| {
        eat(t, "data_domain IN (")
            && quoted_words(t)
            && eat(t, ") AND classification IN (")
            && quoted_words(t)
            && eat(t, ")")
    };
    let child = |t: &mut &str| {
        identifier(t)
            && eat(t, " IN (SELECT ")
            && identifier(t)
            && eat(t, " FROM ")
            && identifier(t)
            && eat(t, " WHERE ")
            && export_clause(t, depth + 1)
            && eat(t, ")")
    };
    let base = eat(&mut t, "1 = 1") || {
        let mut u = t;
        let matched = own(&mut u) || {
            u = t;
            child(&mut u)
        };
        if matched {
            t = u;
            eat(&mut t, " AND erased = 0");
        }
        matched
    };
    if !base {
        return false;
    }
    loop {
        let mut u = t;
        if eat(&mut u, " AND (")
            && identifier(&mut u)
            && eat(&mut u, " IS NULL OR ")
            && identifier(&mut u)
            && eat(&mut u, " IN (SELECT id FROM ")
            && identifier(&mut u)
            && eat(&mut u, " WHERE ")
            && export_clause(&mut u, depth + 1)
            && eat(&mut u, "))")
        {
            t = u;
            continue;
        }
        let mut u = t;
        if eat(&mut u, " AND ") && identifier(&mut u) && eat(&mut u, " IS NULL") {
            t = u;
            continue;
        }
        break;
    }
    *s = t;
    true
}

/// A whole-table read that is accepted, with the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowed {
    /// Table read, or [`ANY_TABLE`].
    pub table: &'static str,
    /// The statement, as [`normalize`] writes it; or a template with `{NAME}`
    /// placeholders (the source literal of a statement built at run time),
    /// matched with [`literal_matches`].
    pub sql: &'static str,
    /// Why the read is acceptable.
    pub reason: &'static str,
    /// For a template: what its statements must look like ([`Shape`]).
    /// `None` for an exact statement; a template without one covers nothing.
    pub shape: Option<Shape>,
}

impl Allowed {
    /// Whether this allowance covers a read of `table` by `sql` (normalized).
    pub fn covers(&self, table: &str, sql: &str) -> bool {
        (self.table == ANY_TABLE || self.table == table)
            && literal_matches(&normalize(self.sql), sql)
            && self
                .shape
                .map_or(!self.is_template(), |shape| shape.accepts(sql))
    }

    /// Whether [`Allowed::sql`] is a template.
    pub fn is_template(&self) -> bool {
        self.sql.contains('{')
    }
}

/// The plan of one statement that ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatementPlan {
    /// The statement, normalized.
    pub sql: String,
    /// How often it ran during the workload.
    pub runs: u64,
    /// `EXPLAIN QUERY PLAN` detail lines.
    pub details: Vec<String>,
    /// Whole-table reads among them.
    pub scans: Vec<Scan>,
}

/// Plan every plannable statement of `statements` (`sql → runs`) on `db`.
pub fn plan_all(
    db: &Database,
    statements: &BTreeMap<String, u64>,
) -> Result<Vec<StatementPlan>, G9Error> {
    let mut out = Vec::new();
    for (sql, runs) in statements {
        if !is_plannable(sql) {
            continue;
        }
        let details: Vec<String> = diagnostics::explain_query_plan(db.conn(), sql)?
            .into_iter()
            .map(|s| s.detail)
            .collect();
        let scans = scans(sql, details.iter().map(String::as_str));
        out.push(StatementPlan {
            sql: normalize(sql),
            runs: *runs,
            details,
            scans,
        });
    }
    Ok(out)
}

/// A whole-table read the gate reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Table.
    pub table: String,
    /// Kind of read.
    pub kind: ScanKind,
    /// Plan detail line.
    pub detail: String,
    /// Statement (normalized).
    pub sql: String,
}

/// What the gate decided.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GateOutcome {
    /// Whole-table reads of a large table that no allowance covers.
    pub violations: Vec<Finding>,
    /// Scans whose table is not a table of the schema (an alias the parser
    /// could not resolve): reported, never assumed small.
    pub unresolved: Vec<Finding>,
    /// Whole-table reads of a large table that an allowance covers.
    pub allowed: Vec<Finding>,
    /// Whole-table reads of a table that is not large.
    pub small: Vec<Finding>,
    /// Allowances that matched nothing (stale: the statement changed or is
    /// no longer reached).
    pub unused_allowances: Vec<String>,
}

/// Apply the gate: `tables` are the schema's tables, `large` the tables that
/// are large in the PERF-1 fixture.
pub fn gate(
    plans: &[StatementPlan],
    tables: &[String],
    large: &[&str],
    allowed: &[Allowed],
) -> GateOutcome {
    let mut outcome = GateOutcome::default();
    let mut used = vec![false; allowed.len()];
    for plan in plans {
        for scan in &plan.scans {
            let finding = Finding {
                table: scan.table.clone(),
                kind: scan.kind,
                detail: scan.detail.clone(),
                sql: plan.sql.clone(),
            };
            let catalog = scan.table.starts_with("sqlite_");
            if !catalog && !tables.contains(&scan.table) {
                outcome.unresolved.push(finding);
            } else if catalog || !large.contains(&scan.table.as_str()) {
                outcome.small.push(finding);
            } else if let Some(k) = allowed
                .iter()
                .position(|a| a.covers(&scan.table, &plan.sql))
            {
                used[k] = true;
                outcome.allowed.push(finding);
            } else {
                outcome.violations.push(finding);
            }
        }
    }
    outcome.unused_allowances = allowed
        .iter()
        .zip(used)
        .filter(|(_, u)| !u)
        .map(|(a, _)| format!("{}: {}", a.table, a.sql))
        .collect();
    outcome
}

/// Which SQL literals of the source the workload reached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// SQL literals found.
    pub literals: usize,
    /// Literals with at least one matching statement that ran.
    pub reached: usize,
    /// `file:line: sql` of every literal no statement matched.
    pub unreached: Vec<String>,
}

/// Coverage of `files` (`(name, source)`) by the statements that `ran`.
pub fn coverage(files: &[(String, String)], ran: &[String]) -> Coverage {
    let ran: Vec<String> = ran.iter().map(|s| normalize(s)).collect();
    let mut out = Coverage::default();
    for (name, source) in files {
        for literal in sql_literals(source) {
            out.literals += 1;
            if ran.iter().any(|r| literal_matches(&literal.sql, r)) {
                out.reached += 1;
            } else {
                out.unreached
                    .push(format!("{name}:{}: {}", literal.line, literal.sql));
            }
        }
    }
    out
}

/// One SQL string literal of a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SqlLiteral {
    /// 1-based line of the opening quote.
    pub line: usize,
    /// The literal's value (escapes resolved), normalized.
    pub sql: String,
}

/// Decode a normal Rust string literal body starting after the opening quote;
/// returns the value and the index after the closing quote.
fn normal_literal(src: &[char], mut i: usize) -> (String, usize) {
    let mut value = String::new();
    while i < src.len() {
        match src[i] {
            '"' => return (value, i + 1),
            '\\' => {
                match src.get(i + 1) {
                    Some('\n') => {
                        // Line continuation: the newline and the next line's
                        // leading whitespace are dropped.
                        i += 2;
                        while src.get(i).is_some_and(|c| c.is_whitespace()) {
                            i += 1;
                        }
                        continue;
                    }
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some(other) => value.push(*other),
                    None => {}
                }
                i += 2;
            }
            c => {
                value.push(c);
                i += 1;
            }
        }
    }
    (value, i)
}

/// SQL statements written as string literals in `source` (a `.rs` file), up to
/// its first `#[cfg(test)]` line (unit tests write SQL of their own that the
/// application never runs). A literal is SQL when [`is_plannable`] says so.
/// Line comments are skipped; character literals and lifetimes are not
/// confused with strings.
pub fn sql_literals(source: &str) -> Vec<SqlLiteral> {
    // `split('\n')`, not `lines()`: the offsets must count every byte (a
    // `\r` included) so that the cut is a char boundary of `source`.
    let cut = source
        .split('\n')
        .scan(0usize, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line))
        })
        .find(|(_, line)| line.trim_start().starts_with("#[cfg(test)]"))
        .map_or(source.len(), |(start, _)| start);
    let src: Vec<char> = source.get(..cut).unwrap_or(source).chars().collect();
    let mut out = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '/' && src.get(i + 1) == Some(&'/') {
            while i < src.len() && src[i] != '\n' {
                i += 1;
            }
        } else if c == '\'' {
            // char literal ('x', '\n', '\'') or a lifetime ('a): skip the
            // shortest form and never enter a string.
            if src.get(i + 1) == Some(&'\\') {
                i += 4;
            } else if src.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if c == '"'
            || (c == 'r'
                && matches!(src.get(i + 1), Some('"') | Some('#'))
                && !(i > 0 && (src[i - 1].is_alphanumeric() || src[i - 1] == '_')))
        {
            let start_line = line;
            let (value, end) = if c == '"' {
                normal_literal(&src, i + 1)
            } else {
                let mut j = i + 1;
                let mut hashes = 0;
                while src.get(j) == Some(&'#') {
                    hashes += 1;
                    j += 1;
                }
                if src.get(j) != Some(&'"') {
                    i += 1;
                    continue;
                }
                j += 1;
                let mut value = String::new();
                loop {
                    if j >= src.len() {
                        break;
                    }
                    if src[j] == '"' && (0..hashes).all(|h| src.get(j + 1 + h) == Some(&'#')) {
                        j += 1 + hashes;
                        break;
                    }
                    value.push(src[j]);
                    j += 1;
                }
                (value, j)
            };
            line += src[i..end.min(src.len())]
                .iter()
                .filter(|c| **c == '\n')
                .count();
            if is_plannable(&value) {
                out.push(SqlLiteral {
                    line: start_line,
                    sql: normalize(&value),
                });
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// Whether the statement `run` (normalized) is an instance of `literal`
/// (normalized); a `{NAME}` placeholder in the literal (a `format!` argument
/// such as a column list) matches any text.
pub fn literal_matches(literal: &str, run: &str) -> bool {
    let mut parts = Vec::new();
    let mut rest = literal;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        parts.push(&rest[..open]);
        rest = &rest[open + close + 1..];
    }
    parts.push(rest);
    if parts.len() == 1 {
        return literal == run;
    }
    let mut position = 0;
    for (k, part) in parts.iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if k == 0 {
            if !run.starts_with(part) {
                return false;
            }
            position = part.len();
        } else if k == parts.len() - 1 {
            return run.len() >= position + part.len() && run[position..].ends_with(part);
        } else {
            match run[position..].find(part) {
                Some(at) => position += at + part.len(),
                None => return false,
            }
        }
    }
    true
}

/// One `CREATE TRIGGER` of a migration: its name and whether its body names
/// a table to read (`FROM`). SQLite reports a trigger program to a statement
/// trace as one `-- TRIGGER <name>` line, so the statements inside it are not
/// planned by the gate; a body that reads no table needs no plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TriggerBody {
    /// Trigger name.
    pub name: String,
    /// Whether the body (`WHEN` condition included) contains `FROM`.
    pub reads_a_table: bool,
}

/// The triggers `CREATE TRIGGER <name> ... END;` of a migration's SQL text
/// (`--` comments skipped).
pub fn trigger_bodies(sql: &str) -> Vec<TriggerBody> {
    let text: String = sql
        .lines()
        .map(|l| l.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let toks = tokens(&text);
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 < toks.len() {
        if !(toks[i].eq_ignore_ascii_case("CREATE") && toks[i + 1].eq_ignore_ascii_case("TRIGGER"))
        {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        // `IF NOT EXISTS`
        while toks.get(j).is_some_and(|t| {
            ["IF", "NOT", "EXISTS"]
                .iter()
                .any(|k| t.eq_ignore_ascii_case(k))
        }) {
            j += 1;
        }
        let name = toks.get(j).cloned().unwrap_or_default();
        let mut reads = false;
        let mut k = j + 1;
        while k < toks.len()
            && !(toks[k].eq_ignore_ascii_case("END") && toks.get(k + 1).is_some_and(|t| t == ";"))
        {
            reads |= toks[k].eq_ignore_ascii_case("FROM");
            k += 1;
        }
        out.push(TriggerBody {
            name,
            reads_a_table: reads,
        });
        i = k + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_bodies_are_found_and_reads_detected() {
        let sql = "-- CREATE TRIGGER commented BEGIN SELECT 1 FROM t; END;\n\
            CREATE TRIGGER trg_a BEFORE UPDATE ON a\n\
            WHEN OLD.state = 'FINAL'\n\
            BEGIN SELECT RAISE(ABORT, 'a is final: read FROM nowhere'); END;\n\
            CREATE TABLE b (x);\n\
            CREATE TRIGGER IF NOT EXISTS trg_b AFTER INSERT ON b\n\
            WHEN EXISTS (SELECT 1 FROM c WHERE c.x = NEW.x)\n\
            BEGIN SELECT RAISE(ABORT, 'from c'); END;\n";
        assert_eq!(
            trigger_bodies(sql),
            vec![
                TriggerBody {
                    name: "trg_a".to_string(),
                    reads_a_table: false
                },
                TriggerBody {
                    name: "trg_b".to_string(),
                    reads_a_table: true
                },
            ]
        );
    }

    #[test]
    fn aliases_resolve_to_tables() {
        let refs = table_refs(
            "SELECT a.x FROM notes n JOIN attachments AS a ON a.note_id = n.id, weeks WHERE n.title = 'FROM x y' ORDER BY 1",
        );
        assert_eq!(refs.get("n").map(String::as_str), Some("notes"));
        assert_eq!(refs.get("a").map(String::as_str), Some("attachments"));
        assert_eq!(refs.get("weeks").map(String::as_str), Some("weeks"));
        assert!(!refs.contains_key("x"), "{refs:?}");
        let refs = table_refs("UPDATE study_sessions SET state = ?1 WHERE id = ?2");
        assert_eq!(
            refs.get("study_sessions").map(String::as_str),
            Some("study_sessions")
        );
        let refs = table_refs("INSERT INTO audit_metadata (a) VALUES (?1)");
        assert!(refs.contains_key("audit_metadata"));
    }

    #[test]
    fn an_alias_bound_twice_is_ambiguous_and_sub_queries_are_derived() {
        let sql = "SELECT * FROM notes x JOIN (SELECT id FROM weeks) AS w ON 1 WHERE x.id IN (SELECT x.id FROM attachments x)";
        let refs = references(sql);
        assert_eq!(refs.tables.get("x").map(String::as_str), Some(AMBIGUOUS));
        assert_eq!(refs.tables.get("notes").map(String::as_str), Some("notes"));
        assert_eq!(refs.tables.get("weeks").map(String::as_str), Some("weeks"));
        assert!(refs.derived.contains("w"), "{refs:?}");
        let found = scans(
            sql,
            [
                "SCAN x",
                "SCAN w",
                "SCAN weeks",
                "SEARCH w USING AUTOMATIC COVERING INDEX (id=?)",
            ],
        );
        let got: Vec<&str> = found.iter().map(|s| s.table.as_str()).collect();
        // `x` could be either table: never attributed to one of them.
        assert_eq!(got, vec!["x ?ambiguous", "weeks"]);
        // An alias that is both a table alias and a sub-query is ambiguous too.
        let sql = "SELECT * FROM notes i JOIN (SELECT 1 AS id) i ON 1";
        let found = scans(sql, ["SCAN i"]);
        assert_eq!(found[0].table, "i ?ambiguous");
        // The same table under the same alias twice is not ambiguous.
        let refs =
            table_refs("SELECT 1 FROM notes n WHERE EXISTS (SELECT 1 FROM notes n WHERE n.id = 1)");
        assert_eq!(refs.get("n").map(String::as_str), Some("notes"));
    }

    #[test]
    fn template_allowances_cover_only_statements_of_their_shape() {
        let a = Allowed {
            table: ANY_TABLE,
            sql: "SELECT * FROM {name} WHERE {clause} ORDER BY rowid",
            reason: "a template allowance",
            shape: Some(Shape::ExportSelect),
        };
        assert!(a.is_template());
        // Statements `g9_storage::export::collect` builds.
        for sql in [
            "SELECT * FROM audit_metadata WHERE 1 = 1 ORDER BY rowid",
            "SELECT * FROM notes WHERE data_domain IN ('LEARNING', 'SECURITY_SENSITIVE') AND classification IN ('PUBLIC', 'INTERNAL') AND erased = 0 ORDER BY rowid",
            "SELECT * FROM note_versions WHERE note_id IN (SELECT id FROM notes WHERE data_domain IN ('LEARNING') AND classification IN ('PUBLIC', 'INTERNAL', 'PRIVATE') AND erased = 0) AND erased = 0 ORDER BY rowid",
            "SELECT * FROM model_evals WHERE model_id IN (SELECT model_id FROM model_registry WHERE 1 = 1) ORDER BY rowid",
            "SELECT * FROM exposure_log WHERE data_domain IN ('LEARNING') AND classification IN ('PUBLIC', 'INTERNAL') AND (attempt_id IS NULL OR attempt_id IN (SELECT id FROM attempts WHERE data_domain IN ('LEARNING') AND classification IN ('PUBLIC', 'INTERNAL'))) ORDER BY rowid",
            "SELECT * FROM claim_versions WHERE 1 = 1 AND chunk_id IS NULL ORDER BY rowid",
        ] {
            assert!(a.covers("notes", sql), "{sql}");
        }
        // Repository-like statements of the same outline are not covered.
        for sql in [
            "SELECT * FROM notes WHERE erased = 0 ORDER BY rowid",
            "SELECT * FROM notes WHERE session_id = ?1 ORDER BY rowid",
            "SELECT * FROM notes WHERE data_domain = ?1 ORDER BY rowid",
            "SELECT * FROM x WHERE id IN (SELECT id FROM attempts) ORDER BY rowid",
            "SELECT * FROM notes WHERE 1 = 1 AND title LIKE ?1 ORDER BY rowid",
            "SELECT * FROM notes WHERE 1 = 1 ORDER BY rowid LIMIT 1",
            "SELECT * FROM notes ORDER BY rowid",
            "SELECT id FROM notes WHERE 1 = 1 ORDER BY rowid",
            "SELECT * FROM notes WHERE data_domain IN ('learning') AND classification IN ('PUBLIC') ORDER BY rowid",
        ] {
            assert!(!a.covers("notes", sql), "{sql}");
        }
        let count = Allowed {
            table: ANY_TABLE,
            sql: "SELECT COUNT(*) FROM \"{quoted}\"",
            reason: "a template allowance",
            shape: Some(Shape::QuotedTableCount),
        };
        assert!(count.covers("audit_metadata", "SELECT COUNT(*) FROM \"audit_metadata\""));
        for sql in [
            "SELECT COUNT(*) FROM \"notes\" n JOIN \"x\"",
            "SELECT COUNT(*) FROM \"notes\" WHERE \"",
            "SELECT COUNT(*) FROM \"\"",
            "SELECT COUNT(*) FROM notes",
        ] {
            assert!(!count.covers("notes", sql), "{sql}");
        }
        // A template without a shape covers nothing.
        let unbounded = Allowed {
            shape: None,
            ..count
        };
        assert!(!unbounded.covers("audit_metadata", "SELECT COUNT(*) FROM \"audit_metadata\""));
        let exact = Allowed {
            table: "notes",
            sql: "SELECT id FROM notes   WHERE erased = 0",
            reason: "an exact allowance",
            shape: None,
        };
        assert!(!exact.is_template());
        assert!(exact.covers("notes", "SELECT id FROM notes WHERE erased = 0"));
        assert!(!exact.covers("attempts", "SELECT id FROM notes WHERE erased = 0"));
        assert!(!exact.covers("notes", "SELECT id FROM notes WHERE erased = 1"));
    }

    #[test]
    fn scans_are_classified_and_searches_are_not() {
        let sql = "SELECT * FROM notes n JOIN attachments a ON a.note_id = n.id";
        let found = scans(
            sql,
            [
                "SCAN n",
                "SEARCH a USING INDEX idx_attachments_note (note_id=?)",
                "SCAN a USING COVERING INDEX idx_attachments_note",
                "SEARCH a USING AUTOMATIC COVERING INDEX (note_id=?)",
                "SCAN CONSTANT ROW",
                "SCAN (subquery-1)",
                "SCAN json_each VIRTUAL TABLE INDEX 1:",
                "USE TEMP B-TREE FOR ORDER BY",
                "SEARCH n USING INTEGER PRIMARY KEY (rowid=?)",
            ],
        );
        let got: Vec<(&str, ScanKind)> = found.iter().map(|s| (s.table.as_str(), s.kind)).collect();
        assert_eq!(
            got,
            vec![
                ("notes", ScanKind::Table),
                ("attachments", ScanKind::FullIndex),
                ("attachments", ScanKind::AutomaticIndex),
            ]
        );
    }

    #[test]
    fn a_search_without_a_constraint_reads_the_whole_index_or_table() {
        // `SELECT COALESCE(MAX(version_no), 0) FROM note_versions WHERE
        // +note_id = ?1`: the MIN/MAX optimisation makes SQLite call the step
        // a SEARCH, but no term narrows it, so it walks the whole index until
        // a row passes the filter.
        let sql = "SELECT COALESCE(MAX(version_no), 0) FROM note_versions WHERE +note_id = ?1";
        let found = scans(
            sql,
            ["SEARCH note_versions USING COVERING INDEX sqlite_autoindex_note_versions_2"],
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            (found[0].table.as_str(), found[0].kind),
            ("note_versions", ScanKind::FullIndex)
        );
        let sql = "SELECT MAX(seq) FROM audit_metadata a WHERE +a.operation = ?1";
        let found = scans(
            sql,
            [
                "SEARCH a",
                "SEARCH a USING INTEGER PRIMARY KEY",
                "SEARCH a USING INDEX idx_x",
                "SEARCH a USING INTEGER PRIMARY KEY (rowid=?)",
                "SEARCH a USING INDEX idx_x (operation=?)",
                "SEARCH a USING COVERING INDEX idx_x (operation>? AND operation<?)",
            ],
        );
        let got: Vec<(&str, ScanKind)> = found.iter().map(|s| (s.table.as_str(), s.kind)).collect();
        assert_eq!(
            got,
            vec![
                ("audit_metadata", ScanKind::Table),
                ("audit_metadata", ScanKind::Table),
                ("audit_metadata", ScanKind::FullIndex),
            ]
        );
    }

    #[test]
    fn literals_are_found_with_escapes_raw_strings_and_continuations() {
        let source = r####"
const COLS: &str = "id, title";
fn a() {
    // "SELECT commented FROM out"
    let c = 'x';
    let q = "SELECT id FROM notes \
             WHERE id = ?1";
    let r = r#"UPDATE notes SET title = "x" WHERE id = ?1"#;
    let f = format!("SELECT {COLS} FROM notes WHERE id = ?1");
    let lifetime: &'static str = "INSERT INTO t (a) VALUES (?1)";
}
#[cfg(test)]
mod tests { const T: &str = "DELETE FROM notes"; }
"####;
        let found = sql_literals(source);
        let sqls: Vec<&str> = found.iter().map(|l| l.sql.as_str()).collect();
        assert_eq!(
            sqls,
            vec![
                "SELECT id FROM notes WHERE id = ?1",
                "UPDATE notes SET title = \"x\" WHERE id = ?1",
                "SELECT {COLS} FROM notes WHERE id = ?1",
                "INSERT INTO t (a) VALUES (?1)",
            ]
        );
        assert_eq!(found[0].line, 6);
        assert_eq!(found[1].line, 8);
    }

    #[test]
    fn crlf_sources_and_multibyte_text_are_cut_at_the_test_module() {
        let source = "// é ü\r\nfn a() { q(\"SELECT 1 FROM t\"); }\r\n#[cfg(test)]\r\nmod tests { const T: &str = \"DELETE FROM t\"; }\r\n";
        let found = sql_literals(source);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].sql, "SELECT 1 FROM t");
        assert_eq!(found[0].line, 2);
    }

    #[test]
    fn common_table_expressions_are_not_tables() {
        let sql = "WITH RECURSIVE a(id) AS (SELECT x FROM t WHERE y = (1)), b AS MATERIALIZED (SELECT 1) SELECT * FROM a JOIN b";
        assert_eq!(cte_names(sql), vec!["a".to_string(), "b".to_string()]);
        let found = scans(
            sql,
            [
                "SCAN a",
                "SCAN t",
                "SEARCH b USING AUTOMATIC COVERING INDEX (x=?)",
            ],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].table, "t");
        assert!(cte_names("SELECT 1").is_empty());
    }

    fn plan(sql: &str, details: &[&str]) -> StatementPlan {
        StatementPlan {
            sql: normalize(sql),
            runs: 1,
            details: details.iter().map(|d| d.to_string()).collect(),
            scans: scans(sql, details.iter().copied()),
        }
    }

    #[test]
    fn the_gate_sorts_findings_and_reports_stale_allowances() {
        let tables: Vec<String> = ["big", "small"].iter().map(|t| t.to_string()).collect();
        let plans = vec![
            plan("SELECT * FROM big", &["SCAN big"]),
            plan("SELECT * FROM big WHERE  k = 1", &["SCAN big"]),
            plan("SELECT * FROM small", &["SCAN small"]),
            plan("SELECT * FROM ghost g", &["SCAN g"]),
            plan("SELECT * FROM sqlite_schema", &["SCAN sqlite_schema"]),
            plan(
                "SELECT * FROM big WHERE id = ?1",
                &["SEARCH big USING INDEX i (id=?)"],
            ),
        ];
        let allowed = [
            Allowed {
                table: "big",
                sql: "SELECT * FROM big WHERE k = 1",
                reason: "a reason that is long enough",
                shape: None,
            },
            Allowed {
                table: "big",
                sql: "SELECT nothing FROM big",
                reason: "a stale allowance that matches nothing",
                shape: None,
            },
        ];
        let outcome = gate(&plans, &tables, &["big"], &allowed);
        assert_eq!(outcome.violations.len(), 1);
        assert_eq!(outcome.violations[0].sql, "SELECT * FROM big");
        assert_eq!(outcome.allowed.len(), 1);
        assert_eq!(outcome.small.len(), 2, "{:?}", outcome.small);
        assert_eq!(outcome.unresolved.len(), 1);
        assert_eq!(outcome.unresolved[0].table, "ghost");
        assert_eq!(
            outcome.unused_allowances,
            vec!["big: SELECT nothing FROM big".to_string()]
        );
    }

    #[test]
    fn coverage_counts_reached_and_unreached_literals() {
        let files = vec![(
            "a.rs".to_string(),
            "fn f() { q(\"SELECT {C} FROM t WHERE id = ?1\"); q(\"DELETE FROM t\"); }\n"
                .to_string(),
        )];
        let c = coverage(&files, &["SELECT  a, b FROM t WHERE id = ?1".to_string()]);
        assert_eq!((c.literals, c.reached), (2, 1));
        assert_eq!(c.unreached, vec!["a.rs:1: DELETE FROM t".to_string()]);
    }

    #[test]
    fn placeholders_match_any_text() {
        assert!(literal_matches(
            "SELECT {COLS} FROM notes WHERE id = ?1",
            "SELECT id, title, body FROM notes WHERE id = ?1"
        ));
        assert!(!literal_matches(
            "SELECT {COLS} FROM notes WHERE id = ?1",
            "SELECT id FROM attachments WHERE id = ?1"
        ));
        assert!(literal_matches("SELECT 1", "SELECT 1"));
        assert!(!literal_matches("SELECT 1", "SELECT 2"));
        assert!(literal_matches(
            "INSERT INTO t ({C}) VALUES ({P})",
            "INSERT INTO t (a, b) VALUES (?1, ?2)"
        ));
    }
}
