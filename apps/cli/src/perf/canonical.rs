//! The fixture checksum (PERF-1): SHA-256 over a canonical projection of every
//! table, so that two builds into two fresh directories can be compared.
//!
//! What the backend generates is not reproducible and is projected out:
//!
//! | Generated value | Why it differs between runs | Projection |
//! |---|---|---|
//! | uuid v7 text (`8-4-4-4-12` lowercase hex), alone or inside a value (`session-<uuid>`, JSON details) | `g9_domain` ids and `new_id` read the host clock and a random source (production id generation is not changed for the fixture) | each distinct uuid becomes `{uuid#N}`, `N` = order of first appearance |
//! | 64-character lowercase hex tokens | digests over rows or requests that contain those ids (audit chain hashes, keyed request hashes, approval MACs) | each distinct token becomes `{sha256#N}` |
//!
//! Everything else is kept as stored: timestamps come from the fixture's
//! stepping clock, request/idempotency/trace ids are sequential, every
//! count, state, text and number is compared. First appearance is taken over
//! the tables in name order, rows in `rowid` order (insertion order, which the
//! fixture's single-threaded command sequence fixes), cells in column order,
//! so the numbering is itself deterministic and equal values stay equal after
//! the projection (a foreign key still points at the same row). A digest of
//! deterministic content (a note body hash) is projected too: the checksum
//! answers "same rows, same values", not "digests computed correctly".
//!
//! The checksum is a function of the build (schema, handlers) and the fixture
//! parameters; a code change that changes what is stored changes it.

use g9_domain::G9Error;
use g9_storage::diagnostics::{self, Cell};
use g9_storage::Database;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// Kinds of projected tokens.
#[derive(Debug, Default)]
struct Projector {
    uuids: HashMap<String, usize>,
    digests: HashMap<String, usize>,
}

fn is_hex_lower(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

/// Length of a uuid (`8-4-4-4-12`) starting at `i`, if one starts there.
fn uuid_at(bytes: &[u8], i: usize) -> Option<usize> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut j = i;
    for (g, len) in GROUPS.iter().enumerate() {
        for _ in 0..*len {
            if !bytes.get(j).copied().is_some_and(is_hex_lower) {
                return None;
            }
            j += 1;
        }
        if g < 4 {
            if bytes.get(j) != Some(&b'-') {
                return None;
            }
            j += 1;
        }
    }
    // Not the middle of a longer hex run.
    if bytes.get(j).copied().is_some_and(is_hex_lower) {
        return None;
    }
    Some(j - i)
}

impl Projector {
    fn ordinal(map: &mut HashMap<String, usize>, token: &str) -> usize {
        let next = map.len() + 1;
        *map.entry(token.to_string()).or_insert(next)
    }

    /// `text` with every uuid and every maximal run of exactly 64 lowercase
    /// hex characters replaced by its ordinal.
    fn project(&mut self, text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        let mut copied = 0;
        while i < bytes.len() {
            let starts_token = i == 0 || !is_hex_lower(bytes[i - 1]);
            if starts_token && is_hex_lower(bytes[i]) {
                if let Some(len) = uuid_at(bytes, i) {
                    out.push_str(&text[copied..i]);
                    let n = Self::ordinal(&mut self.uuids, &text[i..i + len]);
                    out.push_str(&format!("{{uuid#{n}}}"));
                    i += len;
                    copied = i;
                    continue;
                }
                let run = bytes[i..].iter().take_while(|b| is_hex_lower(**b)).count();
                if run == 64 {
                    out.push_str(&text[copied..i]);
                    let n = Self::ordinal(&mut self.digests, &text[i..i + run]);
                    out.push_str(&format!("{{sha256#{n}}}"));
                    i += run;
                    copied = i;
                    continue;
                }
                i += run.max(1);
                continue;
            }
            i += 1;
        }
        out.push_str(&text[copied..]);
        out
    }

    fn cell(&mut self, cell: &Cell) -> serde_json::Value {
        match cell {
            Cell::Null => serde_json::Value::Null,
            Cell::Integer(v) => serde_json::json!(v),
            // Bit pattern: no float formatting question.
            Cell::Real(v) => serde_json::json!({ "real": v.to_bits() }),
            Cell::Text(t) => serde_json::Value::String(self.project(t)),
            Cell::Blob(b) => {
                serde_json::json!({ "blob_sha256": g9_domain::hashing::sha256_hex(b) })
            }
        }
    }
}

/// Per-table part of the checksum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TableDigest {
    /// Rows.
    pub rows: u64,
    /// SHA-256 of the table's canonical rows (one JSON array per line).
    pub sha256: String,
}

/// The fixture checksum and what it is made of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checksum {
    /// SHA-256 over `table<TAB>rows<TAB>table digest` lines, tables in name order.
    pub sha256: String,
    /// Per table.
    pub tables: BTreeMap<String, TableDigest>,
    /// Distinct uuids projected out.
    pub projected_uuids: usize,
    /// Distinct 64-hex digests projected out.
    pub projected_digests: usize,
}

/// Compute the checksum of `db`.
pub fn checksum(db: &Database) -> Result<Checksum, G9Error> {
    let conn = db.conn();
    let mut projector = Projector::default();
    let mut tables = BTreeMap::new();
    let mut summary = String::new();
    for table in diagnostics::user_tables(conn)? {
        let dump = diagnostics::dump_table(conn, &table)?;
        let mut lines = String::new();
        for row in &dump.rows {
            let cells: Vec<serde_json::Value> = row.iter().map(|c| projector.cell(c)).collect();
            lines.push_str(&serde_json::Value::Array(cells).to_string());
            lines.push('\n');
        }
        let digest = TableDigest {
            rows: u64::try_from(dump.rows.len()).unwrap_or(u64::MAX),
            sha256: g9_domain::hashing::sha256_hex(lines.as_bytes()),
        };
        summary.push_str(&format!("{table}\t{}\t{}\n", digest.rows, digest.sha256));
        tables.insert(table, digest);
    }
    Ok(Checksum {
        sha256: g9_domain::hashing::sha256_hex(summary.as_bytes()),
        tables,
        projected_uuids: projector.uuids.len(),
        projected_digests: projector.digests.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_and_digests_become_ordinals_and_equal_values_stay_equal() {
        let mut p = Projector::default();
        let a = "019a8c5e-1b2c-7d3e-8f40-0123456789ab";
        let b = "019a8c5e-1b2c-7d3e-8f40-0123456789ac";
        let h = "ab".repeat(32);
        assert_eq!(p.project(&format!("session-{a}")), "session-{uuid#1}");
        assert_eq!(
            p.project(&format!("{{\"id\":\"{b}\",\"x\":\"{a}\"}}")),
            "{\"id\":\"{uuid#2}\",\"x\":\"{uuid#1}\"}"
        );
        assert_eq!(p.project(&h), "{sha256#1}");
        assert_eq!(p.project(&format!("{h}:{h}")), "{sha256#1}:{sha256#1}");
        // Not projected: other lengths, upper case, a uuid inside a longer hex run.
        let other = "ab".repeat(31);
        assert_eq!(p.project(&other), other);
        assert_eq!(p.project(&h.to_uppercase()), h.to_uppercase());
        assert_eq!(p.project(&format!("0{a}")), format!("0{a}"));
        assert_eq!(
            p.project("W01D1-O1 sim-w01-D01-b3"),
            "W01D1-O1 sim-w01-D01-b3"
        );
        assert_eq!(p.project(""), "");
        assert_eq!(p.uuids.len(), 2);
        assert_eq!(p.digests.len(), 1);
    }

    #[test]
    fn checksum_of_two_fresh_schemas_is_equal_and_changes_with_content() {
        let open = || {
            g9_storage::Database::open_in_memory(
                g9_storage::OpenOptions::new("t").applied_at(g9_domain::UtcMillis::from_millis(5)),
            )
            .unwrap()
        };
        let (a, b) = (open(), open());
        let ca = checksum(&a).unwrap();
        assert_eq!(ca, checksum(&b).unwrap());
        assert!(ca.tables["schema_migrations"].rows > 0);
        let c = g9_storage::Database::open_in_memory(
            g9_storage::OpenOptions::new("t").applied_at(g9_domain::UtcMillis::from_millis(6)),
        )
        .unwrap();
        assert_ne!(ca.sha256, checksum(&c).unwrap().sha256);
    }
}
