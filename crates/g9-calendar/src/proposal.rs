//! Deterministic schedule proposals (M03, ARCHITECTURE §7).
//!
//! `propose_schedule` is a greedy heuristic: tasks are processed in priority
//! order (immutable → safety/assessment deadlines → prerequisite readiness →
//! due reviews → operator priorities → preferred workload), each placed in the
//! earliest availability slot that fits after its prerequisites, never
//! overlapping fixed events or protected rest, never split. Every task that
//! cannot be placed is reported with a reason; `Infeasible` is a first-class
//! outcome that lists every violated constraint. Nothing is silently dropped.
//!
//! The proposal is `SystemDeterministic` output. A model may *suggest* tasks
//! or priorities, but any schedule reaching authoritative state must come from
//! this function and be applied through an approved command in `g9-app`.

use crate::civil::{to_civil, CivilDate};
use crate::conflicts::{
    check_constraints, rest_overlap_span, Constraint, ConstraintKind, ConstraintViolation,
    ResolvedEvent,
};
use crate::error::CalendarError;
use g9_domain::{DurationMs, IanaZone, UtcMillis};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Priority classes in decreasing priority (declaration order is the sort order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriorityClass {
    /// Immutable appointments / protected rest.
    Immutable,
    /// Safety items and assessment deadlines.
    SafetyOrAssessmentDeadline,
    /// Prerequisite readiness for downstream objectives.
    PrerequisiteReadiness,
    /// Due retention reviews.
    DueReview,
    /// Operator-stated priorities.
    OperatorPriority,
    /// Preferred workload / everything else.
    PreferredWorkload,
}

/// A unit of work to place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleTask {
    /// Identifier (unique within a proposal).
    pub id: String,
    /// Operator-facing title (never placed in errors or traces).
    pub title: String,
    /// Required contiguous minutes (>= 1).
    pub minutes: u32,
    /// Priority class.
    pub priority_class: PriorityClass,
    /// Earliest allowed start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earliest: Option<UtcMillis>,
    /// Latest allowed end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<UtcMillis>,
    /// Tasks that must be placed (and finished) before this one starts.
    #[serde(default)]
    pub prerequisites: Vec<String>,
}

/// A placed task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    /// Task identifier.
    pub task_id: String,
    /// Start instant.
    pub start: UtcMillis,
    /// End instant (exclusive).
    pub end: UtcMillis,
    /// Index of the availability window the placement sits in.
    pub window_index: usize,
}

/// Why a task could not be placed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnplacedReason {
    /// No free slot of sufficient length at or after the earliest start.
    NoFittingWindow,
    /// Slots exist but none ends by the deadline.
    DeadlineUnreachable {
        /// The effective deadline.
        deadline: UtcMillis,
    },
    /// Slots exist but every candidate day is at capacity.
    DailyCapacityExhausted,
    /// A prerequisite was itself unplaced.
    PrerequisiteUnplaced {
        /// The unplaced prerequisite.
        requires: String,
    },
    /// The task is part of a prerequisite cycle.
    PrerequisiteCycle,
    /// A prerequisite references a task not in the input.
    UnknownPrerequisite {
        /// The missing task id.
        requires: String,
    },
}

impl UnplacedReason {
    /// Constraint category this reason maps to.
    pub const fn constraint_kind(&self) -> ConstraintKind {
        match self {
            UnplacedReason::NoFittingWindow => ConstraintKind::Availability,
            UnplacedReason::DeadlineUnreachable { .. } => ConstraintKind::AssessmentDeadline,
            UnplacedReason::DailyCapacityExhausted => ConstraintKind::DailyCapacity,
            UnplacedReason::PrerequisiteUnplaced { .. }
            | UnplacedReason::PrerequisiteCycle
            | UnplacedReason::UnknownPrerequisite { .. } => ConstraintKind::PrerequisiteReady,
        }
    }
}

/// An unplaced task with its reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unplaced {
    /// Task identifier.
    pub task_id: String,
    /// Reason.
    pub reason: UnplacedReason,
}

/// Outcome of a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Feasibility {
    /// Every task placed, no constraint violated.
    Feasible,
    /// At least one task unplaced, or the fixed calendar already violates a
    /// hard constraint; the operator must choose an allowed trade-off.
    Infeasible {
        /// Every violation, never truncated.
        violations: Vec<ConstraintViolation>,
    },
}

/// A schedule proposal (never applied by this crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleProposal {
    /// Placed tasks in placement order.
    pub placements: Vec<Placement>,
    /// Unplaced tasks with reasons.
    pub unplaced: Vec<Unplaced>,
    /// Feasibility verdict.
    pub feasibility: Feasibility,
    /// Deterministic trace of every decision (ids and instants only).
    pub constraint_trace: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct FreeSlot {
    start: UtcMillis,
    end: UtcMillis,
    window_index: usize,
}

fn validate_tasks(tasks: &[ScheduleTask]) -> Result<(), CalendarError> {
    let mut ids = BTreeSet::new();
    for t in tasks {
        if !crate::conflicts::is_slug(&t.id) {
            // The id is not echoed: it failed the slug rule and may hold private text.
            return Err(CalendarError::InvalidTask {
                task_id: "<invalid>".to_string(),
                reason: "id must be 1..=128 chars of [A-Za-z0-9._:-]".to_string(),
            });
        }
        let refuse = |reason: &str| CalendarError::InvalidTask {
            task_id: t.id.clone(),
            reason: reason.to_string(),
        };
        if t.prerequisites
            .iter()
            .any(|p| !crate::conflicts::is_slug(p))
        {
            return Err(refuse("prerequisite ids must be slugs"));
        }
        if !ids.insert(t.id.as_str()) {
            return Err(refuse("duplicate id"));
        }
        if t.minutes == 0 {
            return Err(refuse("minutes must be >= 1"));
        }
        if let (Some(e), Some(d)) = (t.earliest, t.deadline) {
            if e >= d {
                return Err(refuse("earliest must precede deadline"));
            }
        }
        if t.prerequisites.iter().any(|p| p == &t.id) {
            return Err(refuse("task cannot be its own prerequisite"));
        }
    }
    Ok(())
}

/// Subtract `[s, e)` from every slot.
fn subtract(slots: &mut Vec<FreeSlot>, s: UtcMillis, e: UtcMillis) {
    let mut next = Vec::with_capacity(slots.len() + 1);
    for slot in slots.drain(..) {
        if e <= slot.start || s >= slot.end {
            next.push(slot);
            continue;
        }
        if s > slot.start {
            next.push(FreeSlot {
                start: slot.start,
                end: s,
                window_index: slot.window_index,
            });
        }
        if e < slot.end {
            next.push(FreeSlot {
                start: e,
                end: slot.end,
                window_index: slot.window_index,
            });
        }
    }
    *slots = next;
}

fn build_free_slots(
    windows: &[(UtcMillis, UtcMillis)],
    fixed: &[ResolvedEvent],
    constraints: &[Constraint],
    trace: &mut Vec<String>,
) -> Result<Vec<FreeSlot>, CalendarError> {
    let mut slots: Vec<FreeSlot> = Vec::with_capacity(windows.len());
    for (i, (s, e)) in windows.iter().enumerate() {
        if s >= e {
            return Err(CalendarError::InvalidWindow(i));
        }
        slots.push(FreeSlot {
            start: *s,
            end: *e,
            window_index: i,
        });
    }
    for f in fixed {
        if f.end > f.start {
            subtract(&mut slots, f.start, f.end);
        }
    }
    for c in constraints {
        if let Constraint::ProtectedRest { daily_window, zone } = c {
            // Carve the rest window out of every local day the windows touch.
            for (s, e) in windows {
                let first = CivilDate::of(&to_civil(*s, zone)?).add_days(-1)?;
                let last = CivilDate::of(&to_civil(*e, zone)?);
                let mut day = first;
                while day <= last {
                    let (rs, re) = daily_window.instants_on(day, zone)?;
                    subtract(&mut slots, rs, re);
                    day = day.add_days(1)?;
                }
            }
            trace.push(format!(
                "protected rest {:02}:{:02}-{:02}:{:02} {} removed from availability",
                daily_window.start_minute / 60,
                daily_window.start_minute % 60,
                daily_window.end_minute / 60,
                daily_window.end_minute % 60,
                zone
            ));
        }
    }
    slots.retain(|s| s.end > s.start);
    slots.sort_by_key(|s| (s.start, s.end, s.window_index));
    Ok(slots)
}

fn sort_key(t: &ScheduleTask) -> (PriorityClass, i64, i64, String) {
    (
        t.priority_class,
        t.deadline.map_or(i64::MAX, UtcMillis::as_millis),
        t.earliest.map_or(i64::MIN, UtcMillis::as_millis),
        t.id.clone(),
    )
}

/// Build a deterministic schedule proposal.
///
/// * `tasks` — work to place (validated: unique slug ids, minutes >= 1).
/// * `availability_windows` — instant ranges the operator is available.
/// * `fixed` — already-resolved events that block time (appointments, rest,
///   existing sessions). Fixed *workload* that already violates a hard
///   constraint (overlaps an immutable appointment, sits inside protected
///   rest, exceeds daily capacity) is reported as a violation, not ignored.
/// * `constraints` — see [`Constraint`].
///
/// Known limitations (deterministic by design, documented rather than hidden):
/// a task is never split across slots; each free slot is tried only at its
/// earliest feasible start, so a slot that spans a local-day boundary is not
/// retried on the next day when the first day is at capacity.
pub fn propose_schedule(
    tasks: &[ScheduleTask],
    availability_windows: &[(UtcMillis, UtcMillis)],
    fixed: &[ResolvedEvent],
    constraints: &[Constraint],
) -> Result<ScheduleProposal, CalendarError> {
    validate_tasks(tasks)?;
    for c in constraints {
        c.validate()?;
    }
    let mut trace: Vec<String> = Vec::new();
    let mut violations: Vec<ConstraintViolation> = Vec::new();

    // Pre-existing violations among fixed events.
    let mut event_constraints: Vec<Constraint> = vec![Constraint::ImmutableAppointment];
    event_constraints.extend(
        constraints
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Constraint::ProtectedRest { .. } | Constraint::DailyCapacity { .. }
                )
            })
            .cloned(),
    );
    for v in check_constraints(fixed, &event_constraints)? {
        trace.push(format!(
            "pre-existing violation {:?} on {}: {}",
            v.constraint, v.subject, v.detail
        ));
        violations.push(v);
    }

    let mut slots = build_free_slots(availability_windows, fixed, constraints, &mut trace)?;
    for s in &slots {
        trace.push(format!(
            "free slot window {} {} .. {}",
            s.window_index, s.start, s.end
        ));
    }

    // Effective prerequisites and deadlines.
    let known: BTreeSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
    let mut prereqs: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut deadlines: BTreeMap<&str, UtcMillis> = BTreeMap::new();
    for t in tasks {
        prereqs
            .entry(t.id.as_str())
            .or_default()
            .extend(t.prerequisites.iter().cloned());
        if let Some(d) = t.deadline {
            deadlines.insert(t.id.as_str(), d);
        }
    }
    let mut capacity: Option<(u32, IanaZone)> = None;
    let mut rest: Vec<(crate::conflicts::DailyWindow, IanaZone)> = Vec::new();
    for c in constraints {
        match c {
            Constraint::PrerequisiteReady { task, requires } => {
                if let Some(set) = prereqs.get_mut(task.as_str()) {
                    set.insert(requires.clone());
                } else {
                    trace.push(format!(
                        "constraint PrerequisiteReady ignored: unknown task {task}"
                    ));
                }
            }
            Constraint::AssessmentDeadline { task_id, deadline } => {
                if known.contains(task_id.as_str()) {
                    let d = deadlines
                        .get(task_id.as_str())
                        .map_or(*deadline, |existing| (*existing).min(*deadline));
                    deadlines.insert(task_id.as_str(), d);
                } else {
                    trace.push(format!(
                        "constraint AssessmentDeadline ignored: unknown task {task_id}"
                    ));
                }
            }
            Constraint::DailyCapacity { minutes, zone } => {
                capacity = Some((*minutes, zone.clone()));
            }
            Constraint::ProtectedRest { daily_window, zone } => {
                rest.push((*daily_window, zone.clone()));
            }
            Constraint::ImmutableAppointment => {}
        }
    }

    // Daily capacity already consumed by fixed workload.
    let mut used_minutes: BTreeMap<CivilDate, i64> = BTreeMap::new();
    if let Some((_, zone)) = &capacity {
        for f in fixed.iter().filter(|f| f.kind.is_workload()) {
            let day = CivilDate::of(&to_civil(f.start, zone)?);
            *used_minutes.entry(day).or_insert(0) += f.span().as_whole_minutes();
        }
    }

    // Kahn-style ordering: a task is ready when all prerequisites are decided.
    let mut placements: Vec<Placement> = Vec::new();
    let mut unplaced: Vec<Unplaced> = Vec::new();
    let mut placed_end: BTreeMap<String, UtcMillis> = BTreeMap::new();
    let mut decided: BTreeSet<String> = BTreeSet::new();
    let mut remaining: Vec<&ScheduleTask> = tasks.iter().collect();

    loop {
        let mut ready: Vec<&ScheduleTask> = remaining
            .iter()
            .copied()
            .filter(|t| {
                prereqs.get(t.id.as_str()).is_none_or(|set| {
                    set.iter()
                        .all(|p| decided.contains(p) || !known.contains(p.as_str()))
                })
            })
            .collect();
        if ready.is_empty() {
            break;
        }
        ready.sort_by_key(|t| sort_key(t));
        let Some(task) = ready.first().copied() else {
            break;
        };
        remaining.retain(|t| t.id != task.id);
        decided.insert(task.id.clone());
        let empty = BTreeSet::new();
        let reqs = prereqs.get(task.id.as_str()).unwrap_or(&empty);

        if let Some(missing) = reqs.iter().find(|p| !known.contains(p.as_str())) {
            trace.push(format!(
                "task {} unplaced: unknown prerequisite {missing}",
                task.id
            ));
            unplaced.push(Unplaced {
                task_id: task.id.clone(),
                reason: UnplacedReason::UnknownPrerequisite {
                    requires: missing.clone(),
                },
            });
            continue;
        }
        if let Some(req) = reqs.iter().find(|p| !placed_end.contains_key(p.as_str())) {
            trace.push(format!(
                "task {} unplaced: prerequisite {req} was not placed",
                task.id
            ));
            unplaced.push(Unplaced {
                task_id: task.id.clone(),
                reason: UnplacedReason::PrerequisiteUnplaced {
                    requires: req.clone(),
                },
            });
            continue;
        }

        let mut earliest = task.earliest.unwrap_or(UtcMillis::from_millis(i64::MIN));
        for p in reqs {
            if let Some(end) = placed_end.get(p.as_str()) {
                earliest = earliest.max(*end);
            }
        }
        let deadline = deadlines.get(task.id.as_str()).copied();
        let needed = DurationMs::from_minutes_u32(task.minutes);

        let mut chosen: Option<(usize, UtcMillis, UtcMillis)> = None;
        let mut deadline_blocked = false;
        let mut capacity_blocked = false;
        for (idx, slot) in slots.iter().enumerate() {
            let start = slot.start.max(earliest);
            let Ok(end) = start.checked_add(needed) else {
                continue;
            };
            if end > slot.end {
                continue;
            }
            if let Some(d) = deadline {
                if end > d {
                    deadline_blocked = true;
                    continue;
                }
            }
            if let Some((cap, zone)) = &capacity {
                let day = CivilDate::of(&to_civil(start, zone)?);
                let used = used_minutes.get(&day).copied().unwrap_or(0);
                if used + i64::from(task.minutes) > i64::from(*cap) {
                    capacity_blocked = true;
                    continue;
                }
            }
            // Defensive: the slot carving already removed rest windows.
            let mut in_rest = false;
            for (w, zone) in &rest {
                if rest_overlap_span((start, end), w, zone)?.is_some() {
                    in_rest = true;
                    break;
                }
            }
            if in_rest {
                continue;
            }
            chosen = Some((idx, start, end));
            break;
        }

        match chosen {
            Some((idx, start, end)) => {
                let window_index = slots[idx].window_index;
                subtract(&mut slots, start, end);
                slots.sort_by_key(|s| (s.start, s.end, s.window_index));
                if let Some((_, zone)) = &capacity {
                    let day = CivilDate::of(&to_civil(start, zone)?);
                    *used_minutes.entry(day).or_insert(0) += i64::from(task.minutes);
                }
                trace.push(format!(
                    "task {} ({:?}) placed {} .. {} in window {}",
                    task.id, task.priority_class, start, end, window_index
                ));
                placed_end.insert(task.id.clone(), end);
                placements.push(Placement {
                    task_id: task.id.clone(),
                    start,
                    end,
                    window_index,
                });
            }
            None => {
                let reason = if deadline_blocked {
                    UnplacedReason::DeadlineUnreachable {
                        deadline: deadline.unwrap_or(UtcMillis::from_millis(i64::MAX)),
                    }
                } else if capacity_blocked {
                    UnplacedReason::DailyCapacityExhausted
                } else {
                    UnplacedReason::NoFittingWindow
                };
                trace.push(format!(
                    "task {} ({:?}) unplaced: {reason:?}",
                    task.id, task.priority_class
                ));
                unplaced.push(Unplaced {
                    task_id: task.id.clone(),
                    reason,
                });
            }
        }
    }

    // Anything left is part of a prerequisite cycle.
    remaining.sort_by_key(|t| sort_key(t));
    for task in remaining {
        trace.push(format!("task {} unplaced: prerequisite cycle", task.id));
        unplaced.push(Unplaced {
            task_id: task.id.clone(),
            reason: UnplacedReason::PrerequisiteCycle,
        });
    }

    for u in &unplaced {
        violations.push(ConstraintViolation {
            constraint: u.reason.constraint_kind(),
            subject: u.task_id.clone(),
            detail: format!("{:?}", u.reason),
        });
    }

    let feasibility = if violations.is_empty() {
        Feasibility::Feasible
    } else {
        Feasibility::Infeasible { violations }
    };
    Ok(ScheduleProposal {
        placements,
        unplaced,
        feasibility,
        constraint_trace: trace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::civil::Adjustment;
    use crate::events::EventKind;
    use g9_domain::CalendarEventId;
    use pretty_assertions::assert_eq;

    fn ms(h: i64) -> UtcMillis {
        UtcMillis::from_millis(h * 3_600_000)
    }

    fn task(id: &str, minutes: u32, class: PriorityClass) -> ScheduleTask {
        ScheduleTask {
            id: id.into(),
            title: id.into(),
            minutes,
            priority_class: class,
            earliest: None,
            deadline: None,
            prerequisites: vec![],
        }
    }

    fn fixed(
        id: &str,
        start: UtcMillis,
        end: UtcMillis,
        kind: EventKind,
        immutable: bool,
    ) -> ResolvedEvent {
        ResolvedEvent {
            event_id: CalendarEventId::parse(id).unwrap(),
            occurrence_seq: 0,
            start,
            end,
            kind,
            immutable,
            all_day: false,
            adjustment: Adjustment::None,
        }
    }

    #[test]
    fn greedy_places_in_priority_order_and_never_overlaps_fixed() {
        let tasks = vec![
            task("low", 60, PriorityClass::PreferredWorkload),
            task("high", 60, PriorityClass::SafetyOrAssessmentDeadline),
            task("mid", 60, PriorityClass::DueReview),
        ];
        let windows = vec![(ms(8), ms(12))];
        let fixed = vec![fixed("appt", ms(9), ms(10), EventKind::Appointment, true)];
        let p = propose_schedule(
            &tasks,
            &windows,
            &fixed,
            &[Constraint::ImmutableAppointment],
        )
        .unwrap();
        assert_eq!(p.feasibility, Feasibility::Feasible);
        let order: Vec<(&str, UtcMillis)> = p
            .placements
            .iter()
            .map(|x| (x.task_id.as_str(), x.start))
            .collect();
        assert_eq!(
            order,
            vec![("high", ms(8)), ("mid", ms(10)), ("low", ms(11))]
        );
        for pl in &p.placements {
            assert!(pl.end <= ms(9) || pl.start >= ms(10), "{pl:?}");
        }
        assert!(p.constraint_trace.iter().any(|l| l.contains("task high")));
    }

    #[test]
    fn prerequisites_order_placement_and_cycles_are_reported() {
        let mut a = task("a", 60, PriorityClass::PreferredWorkload);
        let mut b = task("b", 60, PriorityClass::SafetyOrAssessmentDeadline);
        b.prerequisites = vec!["a".into()];
        a.prerequisites = vec![];
        let mut c = task("c", 30, PriorityClass::DueReview);
        c.prerequisites = vec!["d".into()];
        let mut d = task("d", 30, PriorityClass::DueReview);
        d.prerequisites = vec!["c".into()];
        let mut e = task("e", 30, PriorityClass::DueReview);
        e.prerequisites = vec!["ghost".into()];
        let p = propose_schedule(&[a, b, c, d, e], &[(ms(8), ms(12))], &[], &[]).unwrap();
        let order: Vec<&str> = p.placements.iter().map(|x| x.task_id.as_str()).collect();
        assert_eq!(order, vec!["a", "b"]);
        assert!(p.placements[1].start >= p.placements[0].end);
        let reasons: Vec<(String, UnplacedReason)> = p
            .unplaced
            .iter()
            .map(|u| (u.task_id.clone(), u.reason.clone()))
            .collect();
        assert_eq!(
            reasons,
            vec![
                (
                    "e".to_string(),
                    UnplacedReason::UnknownPrerequisite {
                        requires: "ghost".into()
                    }
                ),
                ("c".to_string(), UnplacedReason::PrerequisiteCycle),
                ("d".to_string(), UnplacedReason::PrerequisiteCycle),
            ]
        );
        assert_eq!(p.placements.len() + p.unplaced.len(), 5);
        assert!(
            matches!(p.feasibility, Feasibility::Infeasible { ref violations } if violations.len() == 3)
        );
    }

    #[test]
    fn dependent_of_unplaced_prerequisite_is_unplaced_with_reason() {
        let big = task("big", 600, PriorityClass::OperatorPriority);
        let mut dep = task("dep", 30, PriorityClass::OperatorPriority);
        dep.prerequisites = vec!["big".into()];
        let p = propose_schedule(&[big, dep], &[(ms(8), ms(12))], &[], &[]).unwrap();
        assert!(p.placements.is_empty());
        assert_eq!(p.unplaced[0].reason, UnplacedReason::NoFittingWindow);
        assert_eq!(
            p.unplaced[1].reason,
            UnplacedReason::PrerequisiteUnplaced {
                requires: "big".into()
            }
        );
    }

    #[test]
    fn deadline_and_capacity_reasons() {
        let mut late = task("late", 60, PriorityClass::SafetyOrAssessmentDeadline);
        late.deadline = Some(ms(8));
        let p = propose_schedule(&[late], &[(ms(8), ms(12))], &[], &[]).unwrap();
        assert_eq!(
            p.unplaced[0].reason,
            UnplacedReason::DeadlineUnreachable { deadline: ms(8) }
        );

        let via_constraint = task("exam", 60, PriorityClass::SafetyOrAssessmentDeadline);
        let p = propose_schedule(
            &[via_constraint],
            &[(ms(8), ms(12))],
            &[],
            &[Constraint::AssessmentDeadline {
                task_id: "exam".into(),
                deadline: ms(8),
            }],
        )
        .unwrap();
        assert_eq!(
            p.unplaced[0].reason,
            UnplacedReason::DeadlineUnreachable { deadline: ms(8) }
        );

        let tasks = vec![
            task("a", 240, PriorityClass::DueReview),
            task("b", 240, PriorityClass::DueReview),
        ];
        let p = propose_schedule(
            &tasks,
            &[(ms(8), ms(20))],
            &[],
            &[Constraint::DailyCapacity {
                minutes: 420,
                zone: IanaZone::utc(),
            }],
        )
        .unwrap();
        assert_eq!(p.placements.len(), 1);
        assert_eq!(p.unplaced[0].reason, UnplacedReason::DailyCapacityExhausted);
    }

    #[test]
    fn invalid_inputs_are_refused() {
        assert!(matches!(
            propose_schedule(
                &[task("a", 0, PriorityClass::DueReview)],
                &[(ms(1), ms(2))],
                &[],
                &[]
            )
            .unwrap_err(),
            CalendarError::InvalidTask { .. }
        ));
        assert!(matches!(
            propose_schedule(
                &[
                    task("a", 1, PriorityClass::DueReview),
                    task("a", 1, PriorityClass::DueReview)
                ],
                &[(ms(1), ms(2))],
                &[],
                &[]
            )
            .unwrap_err(),
            CalendarError::InvalidTask { .. }
        ));
        assert_eq!(
            propose_schedule(
                &[task("a", 1, PriorityClass::DueReview)],
                &[(ms(2), ms(1))],
                &[],
                &[]
            )
            .unwrap_err(),
            CalendarError::InvalidWindow(0)
        );
        assert!(propose_schedule(
            &[],
            &[],
            &[],
            &[Constraint::DailyCapacity {
                minutes: 0,
                zone: IanaZone::utc()
            }]
        )
        .is_err());
    }

    #[test]
    fn proposal_serializes() {
        let p = propose_schedule(
            &[task("a", 30, PriorityClass::DueReview)],
            &[(ms(1), ms(2))],
            &[],
            &[],
        )
        .unwrap();
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["feasibility"]["kind"], "FEASIBLE");
        let back: ScheduleProposal = serde_json::from_value(json).unwrap();
        assert_eq!(back, p);
    }
}
