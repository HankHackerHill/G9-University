//! What a record is, in words the operator recognises, read from storage.
//!
//! The native confirmation of an approval (ADR-0014) is the one surface a
//! compromised webview cannot draw. It showed an operation name and an opaque
//! id, so the operator could not tell the record the page displayed from the one
//! the approval covered (security audit 2026-11-13). [`describe_resource`] gives
//! the confirmation a line that only the backend writes: the stored title of a
//! note, the time of a calendar event, the grade an attempt would make final.
//!
//! Every value passes [`g9_domain::text::one_line`]: stored text is data, and a title with line
//! breaks or bidirectional controls must not be able to forge a second line of
//! the confirmation or reorder the first.

use crate::error::StorageResult;
use g9_domain::text::one_line;
use g9_domain::ResourceKind;
use rusqlite::{params, Connection, OptionalExtension};

/// Longest stored text shown in a description.
const MAX_TEXT: usize = 96;

fn quoted(text: &str) -> String {
    format!("“{}”", one_line(text, MAX_TEXT))
}

fn lower(word: &str) -> String {
    one_line(&word.replace('_', " ").to_lowercase(), 40)
}

/// A line that says what `kind` `id` is, or `None` when no such row is stored
/// (an approval for a record that does not exist yet) or the kind has no
/// describable row. Read-only.
pub fn describe_resource(
    conn: &Connection,
    kind: ResourceKind,
    id: &str,
) -> StorageResult<Option<String>> {
    Ok(match kind {
        ResourceKind::Note => {
            let row = conn
                .query_row(
                    "SELECT title, classification, data_domain, erased FROM notes WHERE id = ?1",
                    params![id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()?;
            match row {
                None => None,
                Some((_, _, _, 1)) => Some("a note that is already erased".to_string()),
                Some((title, classification, domain, _)) => {
                    // An erasure of a note takes its files with it (ADR-0017): say how many.
                    let files: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM attachments WHERE note_id = ?1 AND erased = 0",
                        params![id],
                        |r| r.get(0),
                    )?;
                    let attached = match files {
                        0 => String::new(),
                        1 => ", with 1 attached file".to_string(),
                        n => format!(", with {n} attached files"),
                    };
                    Some(format!(
                        "the note {} ({}, {}{attached})",
                        quoted(&title),
                        lower(&classification),
                        lower(&domain)
                    ))
                }
            }
        }
        ResourceKind::Attachment => conn
            .query_row(
                "SELECT a.original_filename, a.size_bytes, a.classification, a.erased, n.title
                   FROM attachments a LEFT JOIN notes n ON n.id = a.note_id WHERE a.id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()?
            .map(|(name, size, classification, erased, note)| {
                if erased == 1 {
                    "a file that is already erased".to_string()
                } else {
                    let of_note = note.map_or_else(String::new, |t| format!(" of the note {}", quoted(&t)));
                    format!(
                        "the file {} ({size} bytes, {}){of_note}",
                        quoted(&name),
                        lower(&classification)
                    )
                }
            }),
        ResourceKind::CalendarEvent => conn
            .query_row(
                "SELECT title, kind, civil_start, civil_end, zone FROM calendar_events WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?
            .map(|(title, kind, start, end, zone)| {
                format!(
                    "the calendar event {} ({}), {} to {} {}",
                    quoted(&title),
                    lower(&kind),
                    one_line(&start, 32),
                    one_line(&end, 32),
                    one_line(&zone, 48)
                )
            }),
        ResourceKind::Attempt => {
            let attempt = conn
                .query_row(
                    "SELECT d.title, d.code, a.state, a.mode
                       FROM attempts a
                       JOIN assessment_versions v ON v.id = a.assessment_version_id
                       JOIN assessment_definitions d ON d.id = v.definition_id
                      WHERE a.id = ?1",
                    params![id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()?;
            match attempt {
                None => None,
                Some((title, code, state, mode)) => {
                    let grade = conn
                        .query_row(
                            "SELECT total_score, passed, is_final FROM grade_decisions
                              WHERE attempt_id = ?1 ORDER BY decided_at DESC, id DESC LIMIT 1",
                            params![id],
                            |r| Ok((r.get::<_, f64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
                        )
                        .optional()?;
                    let graded = grade.map_or_else(
                        || "no grade recorded".to_string(),
                        |(total, passed, is_final)| {
                            format!(
                                "latest grade {total}, {}, {}",
                                if passed == 1 { "pass" } else { "fail" },
                                if is_final == 1 { "final" } else { "not final" }
                            )
                        },
                    );
                    Some(format!(
                        "an attempt at the assessment {} ({}), {} mode, {}; {graded}",
                        quoted(&title),
                        one_line(&code, 48),
                        lower(&mode),
                        lower(&state)
                    ))
                }
            }
        }
        ResourceKind::CurriculumVersion => conn
            .query_row(
                "SELECT version_label, provenance, is_current FROM curriculum_versions WHERE id = ?1",
                params![id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
            )
            .optional()?
            .map(|(label, provenance, current)| {
                format!(
                    "the stored curriculum version {} ({}{})",
                    quoted(&label),
                    lower(&provenance),
                    if current == 1 { ", in use" } else { "" }
                )
            }),
        ResourceKind::ScheduleProposal => conn
            .query_row(
                "SELECT feasible, state, json_array_length(json_extract(proposal_json, '$.changes'))
                   FROM schedule_proposals WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .optional()?
            .map(|(feasible, state, changes)| {
                let n = changes.map_or_else(|| "its".to_string(), |c| c.to_string());
                format!(
                    "a schedule proposal with {n} calendar changes ({}, {})",
                    if feasible == 1 { "feasible" } else { "not feasible" },
                    lower(&state)
                )
            }),
        ResourceKind::Export => {
            // The id is written by the backend: `export:<AREAS joined by .>:<ceiling>:<format>`.
            let parts: Vec<&str> = id.split(':').collect();
            match parts.as_slice() {
                ["export", areas, ceiling, format] => Some(format!(
                    "an export of the areas {} as {}; {}",
                    one_line(&areas.replace('.', ", ").replace('_', " ").to_lowercase(), 120),
                    one_line(format, 8),
                    if *ceiling == "PRIVATE" {
                        "private records included, secret records never"
                    } else {
                        "private and secret records left out"
                    }
                )),
                _ => None,
            }
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_export_id_reads_as_areas_ceiling_and_format() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(
            describe_resource(&conn, ResourceKind::Export, "export:LEARNING.SECURITY_SENSITIVE:PRIVATE:CSV").unwrap(),
            Some("an export of the areas learning, security sensitive as CSV; private records included, secret records never".to_string())
        );
        assert_eq!(
            describe_resource(&conn, ResourceKind::Export, "export:LEARNING:INTERNAL:JSON")
                .unwrap(),
            Some(
                "an export of the areas learning as JSON; private and secret records left out"
                    .to_string()
            )
        );
        assert_eq!(
            describe_resource(&conn, ResourceKind::Export, "something else").unwrap(),
            None
        );
    }
}
