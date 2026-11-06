//! Reconciliation diff between the operator-supplied original approved plan and
//! a candidate manifest (handoff §8 "Provenance gate"; correction C01).
//!
//! The importer "compares the full original approved plan (when provided) to
//! seed by ID, week/day, objective text, time allocation, resource link,
//! deliverable and competencies" and "prints a human-readable diff,
//! checksum/version IDs, missing/duplicate/changed sections and explicit
//! approval request". [`reconcile`] does exactly that and nothing more: it never
//! mutates either manifest and never decides the promotion itself — the
//! decision is a command-level approval in `g9-app` (see [`crate::provenance`]).
//!
//! Which manifest is the *original* is the caller's responsibility: it must be
//! the operator-supplied approved plan (TrustClass::Operator provenance), never
//! a model- or skill-produced document.

use crate::checksum::{json_checksum, manifest_checksum, Checksum};
use crate::error::CurriculumError;
use crate::manifest::{Day, Manifest, Week};
use crate::provenance::provenance_label;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Per-day comparison outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DayDiffStatus {
    /// Identical in every compared field.
    Unchanged,
    /// Present in both; the listed fields differ (`objectives[1].text`, `blocks`, ...).
    Changed {
        /// Differing fields, e.g. `objectives[1].text`, `blocks`, `week_number`.
        fields: Vec<String>,
    },
    /// Present in the original only.
    MissingInCandidate,
    /// Present in the candidate only.
    AddedInCandidate,
}

/// One row of the per-day diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayDiff {
    /// Day id as written in the manifest(s).
    pub day_id: String,
    /// Outcome.
    pub status: DayDiffStatus,
}

/// Summary counts and non-day differences.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReconciliationSummary {
    /// Days identical in both manifests.
    pub unchanged: u32,
    /// Days present in both with differing fields.
    pub changed: u32,
    /// Days present in the original only.
    pub missing_in_candidate: u32,
    /// Days present in the candidate only.
    pub added_in_candidate: u32,
    /// Top-level content fields that differ (`name`, `structure`, `weekday_blocks`,
    /// `saturday_blocks`, `weeks.count`, `weeks[i].title`, `weeks[i].week_number`,
    /// `curriculum_id`). Provenance metadata (`provenance`, `provenance_note`,
    /// `version`, `reconciliation`, `manifest_schema_version`) is deliberately
    /// excluded: it is expected to differ between an original and its candidate.
    pub metadata_changes: Vec<String>,
    /// Day ids that occur more than once in the original (first occurrence compared).
    pub duplicate_day_ids_original: Vec<String>,
    /// Day ids that occur more than once in the candidate (first occurrence compared).
    pub duplicate_day_ids_candidate: Vec<String>,
}

impl ReconciliationSummary {
    /// Whether any curriculum content differs (days or top-level content fields).
    pub fn has_content_diffs(&self) -> bool {
        self.changed > 0
            || self.missing_in_candidate > 0
            || self.added_in_candidate > 0
            || !self.metadata_changes.is_empty()
    }
}

/// How the candidate's `reconciliation` block relates to this comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconciliationBinding {
    /// The candidate carries no reconciliation block.
    Absent,
    /// The block references this original (by checksum) and this exact diff.
    Matched,
    /// The block references a different original.
    OriginalChecksumMismatch {
        /// Checksum written in the candidate's block.
        found: String,
    },
    /// The block references a diff other than the one computed now (the
    /// candidate changed after the diff was reviewed).
    DiffChecksumMismatch {
        /// Checksum written in the candidate's block.
        found: String,
    },
}

/// Output of [`reconcile`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationReport {
    /// Checksum of the original manifest.
    pub original_checksum: Checksum,
    /// Checksum of the candidate manifest.
    pub candidate_checksum: Checksum,
    /// Checksum of the content diff (`per_day` + `summary`), independent of the
    /// candidate's provenance metadata; the value a reconciliation block must
    /// carry as `diff_checksum`.
    pub diff_checksum: Checksum,
    /// Content version of the original, for the header and audit metadata.
    pub original_version: String,
    /// Content version of the candidate.
    pub candidate_version: String,
    /// Whether both manifests describe the same curriculum id.
    pub curriculum_id_matches: bool,
    /// Per-day rows: original order first, then days only in the candidate.
    pub per_day: Vec<DayDiff>,
    /// Counts and non-day differences.
    pub summary: ReconciliationSummary,
    /// Relation of the candidate's reconciliation block to this diff.
    pub binding: ReconciliationBinding,
    /// True only when there are zero content diffs, or the candidate carries a
    /// reconciliation block bound to this original and this diff. Never true
    /// when the curriculum ids differ. This is a *precondition* for promotion;
    /// the operator approval is still required (command level).
    pub promotion_allowed: bool,
    /// Human-readable diff with checksums, counts, per-day lines and the explicit
    /// approval request.
    pub human_readable: String,
}

/// Compare `original` (the operator-supplied approved plan) with `candidate`.
///
/// Days are matched by `day_id`. Compared fields per day: week number, day of
/// week, planned minutes, objectives (id, seq, text, competency ids),
/// deliverable, blocks, resources and the assessment flag. Returns an error only
/// if a manifest cannot be serialized for hashing.
pub fn reconcile(
    original: &Manifest,
    candidate: &Manifest,
) -> Result<ReconciliationReport, CurriculumError> {
    let original_checksum = manifest_checksum(original)?;
    let candidate_checksum = manifest_checksum(candidate)?;

    let (orig_index, dup_orig) = index_days(original);
    let (cand_index, dup_cand) = index_days(candidate);

    let mut per_day = Vec::new();
    let mut summary = ReconciliationSummary {
        duplicate_day_ids_original: dup_orig,
        duplicate_day_ids_candidate: dup_cand,
        ..ReconciliationSummary::default()
    };

    let mut emitted: BTreeSet<&str> = BTreeSet::new();
    for (ow, od) in original.days() {
        if !emitted.insert(od.day_id.as_str()) {
            continue; // duplicate in original: already compared once
        }
        let status = match cand_index.get(od.day_id.as_str()) {
            None => {
                summary.missing_in_candidate += 1;
                DayDiffStatus::MissingInCandidate
            }
            Some((cw, cd)) => {
                let fields = day_fields_changed((ow, od), (cw, cd));
                if fields.is_empty() {
                    summary.unchanged += 1;
                    DayDiffStatus::Unchanged
                } else {
                    summary.changed += 1;
                    DayDiffStatus::Changed { fields }
                }
            }
        };
        per_day.push(DayDiff {
            day_id: od.day_id.clone(),
            status,
        });
    }
    for (_, cd) in candidate.days() {
        if orig_index.contains_key(cd.day_id.as_str()) {
            continue;
        }
        if !emitted.insert(cd.day_id.as_str()) {
            continue;
        }
        summary.added_in_candidate += 1;
        per_day.push(DayDiff {
            day_id: cd.day_id.clone(),
            status: DayDiffStatus::AddedInCandidate,
        });
    }

    summary.metadata_changes = metadata_changes(original, candidate);
    let curriculum_id_matches = original.curriculum_id == candidate.curriculum_id;

    let diff_value = serde_json::to_value(serde_json::json!({
        "per_day": &per_day,
        "summary": &summary,
    }))
    .map_err(|_| CurriculumError::Serialization)?;
    let diff_checksum = json_checksum(&diff_value);

    let binding = match &candidate.reconciliation {
        None => ReconciliationBinding::Absent,
        Some(rec) if !original_checksum.matches(&rec.original_checksum) => {
            ReconciliationBinding::OriginalChecksumMismatch {
                found: rec.original_checksum.clone(),
            }
        }
        Some(rec) if !diff_checksum.matches(&rec.diff_checksum) => {
            ReconciliationBinding::DiffChecksumMismatch {
                found: rec.diff_checksum.clone(),
            }
        }
        Some(_) => ReconciliationBinding::Matched,
    };

    let promotion_allowed = curriculum_id_matches
        && (!summary.has_content_diffs() || binding == ReconciliationBinding::Matched);

    let mut report = ReconciliationReport {
        original_checksum,
        candidate_checksum,
        diff_checksum,
        original_version: original.version.clone(),
        candidate_version: candidate.version.clone(),
        curriculum_id_matches,
        per_day,
        summary,
        binding,
        promotion_allowed,
        human_readable: String::new(),
    };
    report.human_readable = render(original, candidate, &report);
    Ok(report)
}

type DayIndex<'a> = BTreeMap<&'a str, (&'a Week, &'a Day)>;

fn index_days(m: &Manifest) -> (DayIndex<'_>, Vec<String>) {
    let mut index: DayIndex<'_> = BTreeMap::new();
    let mut duplicates = Vec::new();
    for (w, d) in m.days() {
        if index.contains_key(d.day_id.as_str()) {
            if !duplicates.contains(&d.day_id) {
                duplicates.push(d.day_id.clone());
            }
        } else {
            index.insert(d.day_id.as_str(), (w, d));
        }
    }
    (index, duplicates)
}

fn day_fields_changed(o: (&Week, &Day), c: (&Week, &Day)) -> Vec<String> {
    let (ow, od) = o;
    let (cw, cd) = c;
    let mut fields = Vec::new();
    if ow.week_number != cw.week_number {
        fields.push("week_number".to_string());
    }
    if od.day_of_week != cd.day_of_week {
        fields.push("day_of_week".to_string());
    }
    if od.planned_minutes != cd.planned_minutes {
        fields.push("planned_minutes".to_string());
    }
    if od.objectives.len() != cd.objectives.len() {
        fields.push("objectives.count".to_string());
    } else {
        for (i, (oo, co)) in od.objectives.iter().zip(cd.objectives.iter()).enumerate() {
            if oo.seq != co.seq {
                fields.push(format!("objectives[{i}].seq"));
            }
            if oo.objective_id != co.objective_id {
                fields.push(format!("objectives[{i}].objective_id"));
            }
            if oo.text != co.text {
                fields.push(format!("objectives[{i}].text"));
            }
            if oo.competency_ids != co.competency_ids {
                fields.push(format!("objectives[{i}].competency_ids"));
            }
        }
    }
    if od.evidence_artifact != cd.evidence_artifact {
        fields.push("evidence_artifact".to_string());
    }
    if od.blocks != cd.blocks {
        fields.push("blocks".to_string());
    }
    if od.is_assessment_day != cd.is_assessment_day {
        fields.push("is_assessment_day".to_string());
    }
    if od.resources != cd.resources {
        fields.push("resources".to_string());
    }
    fields
}

fn metadata_changes(o: &Manifest, c: &Manifest) -> Vec<String> {
    let mut v = Vec::new();
    if o.curriculum_id != c.curriculum_id {
        v.push("curriculum_id".to_string());
    }
    if o.name != c.name {
        v.push("name".to_string());
    }
    if o.structure != c.structure {
        v.push("structure".to_string());
    }
    if o.weekday_blocks != c.weekday_blocks {
        v.push("weekday_blocks".to_string());
    }
    if o.saturday_blocks != c.saturday_blocks {
        v.push("saturday_blocks".to_string());
    }
    if o.weeks.len() != c.weeks.len() {
        v.push("weeks.count".to_string());
    }
    for (i, (ow, cw)) in o.weeks.iter().zip(c.weeks.iter()).enumerate() {
        if ow.week_number != cw.week_number {
            v.push(format!("weeks[{i}].week_number"));
        }
        if ow.title != cw.title {
            v.push(format!("weeks[{i}].title"));
        }
    }
    v
}

fn render(original: &Manifest, candidate: &Manifest, r: &ReconciliationReport) -> String {
    let mut out = String::new();
    out.push_str("G9 curriculum reconciliation report\n");
    out.push_str(&format!(
        "original : {} v{} provenance={} checksum={}\n",
        original.curriculum_id,
        original.version,
        provenance_label(original.provenance),
        r.original_checksum
    ));
    out.push_str(&format!(
        "candidate: {} v{} provenance={} checksum={}\n",
        candidate.curriculum_id,
        candidate.version,
        provenance_label(candidate.provenance),
        r.candidate_checksum
    ));
    out.push_str(&format!("diff checksum: {}\n", r.diff_checksum));
    if !r.curriculum_id_matches {
        out.push_str("CURRICULUM ID MISMATCH: the candidate describes a different curriculum\n");
    }
    let s = &r.summary;
    out.push_str(&format!(
        "days: {} unchanged, {} changed, {} missing in candidate, {} added in candidate\n",
        s.unchanged, s.changed, s.missing_in_candidate, s.added_in_candidate
    ));
    if !s.metadata_changes.is_empty() {
        out.push_str(&format!(
            "metadata changes: {}\n",
            s.metadata_changes.join(", ")
        ));
    }
    if !s.duplicate_day_ids_original.is_empty() {
        out.push_str(&format!(
            "DUPLICATE day ids in original: {}\n",
            s.duplicate_day_ids_original.join(", ")
        ));
    }
    if !s.duplicate_day_ids_candidate.is_empty() {
        out.push_str(&format!(
            "DUPLICATE day ids in candidate: {}\n",
            s.duplicate_day_ids_candidate.join(", ")
        ));
    }
    for d in &r.per_day {
        match &d.status {
            DayDiffStatus::Unchanged => {}
            DayDiffStatus::Changed { fields } => {
                out.push_str(&format!("{} CHANGED: {}\n", d.day_id, fields.join(", ")));
            }
            DayDiffStatus::MissingInCandidate => {
                out.push_str(&format!("{} MISSING IN CANDIDATE\n", d.day_id));
            }
            DayDiffStatus::AddedInCandidate => {
                out.push_str(&format!("{} ADDED IN CANDIDATE\n", d.day_id));
            }
        }
    }
    out.push_str(&format!(
        "reconciliation binding: {}\n",
        match &r.binding {
            ReconciliationBinding::Absent => "absent".to_string(),
            ReconciliationBinding::Matched =>
                "matched (original checksum and diff checksum)".to_string(),
            ReconciliationBinding::OriginalChecksumMismatch { found } =>
                format!("ORIGINAL CHECKSUM MISMATCH (block references {found})"),
            ReconciliationBinding::DiffChecksumMismatch { found } =>
                format!("DIFF CHECKSUM MISMATCH (block references {found})"),
        }
    ));
    if r.promotion_allowed {
        out.push_str(&format!(
            "promotion: PRECONDITIONS MET. APPROVAL REQUEST: an operator approval (Capability::CurriculumPromote) bound to candidate checksum {} is required before this manifest becomes APPROVED_CANONICAL.\n",
            r.candidate_checksum
        ));
    } else {
        out.push_str("promotion: BLOCKED. Resolve the differences above, or review the diff and attach a reconciliation block {original_checksum, diff_checksum, approved_by, approved_at} to the candidate; then re-run reconciliation.\n");
    }
    out
}
