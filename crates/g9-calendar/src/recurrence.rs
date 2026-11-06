//! Recurrence rules (RFC 5545 subset) and occurrence expansion (M03).
//!
//! Supported: `FREQ=DAILY|WEEKLY|MONTHLY`, `INTERVAL`, `BYDAY` (DAILY/WEEKLY
//! only), and at most one of `UNTIL` (date, `YYYYMMDD`) or `COUNT`.
//!
//! Semantics:
//! - occurrences are generated purely from the rule and the series start; the
//!   start date is an occurrence only if the rule selects it (a `WEEKLY;BYDAY=MO`
//!   rule starting on a Tuesday first fires on the following Monday);
//! - `WEEKLY` without `BYDAY` repeats on the start's weekday; with `BYDAY`, the
//!   listed days of every `INTERVAL`-th ISO week (Monday first) are selected;
//! - `MONTHLY` repeats on the start's day-of-month and *skips* months that lack
//!   that day (31st in a 30-day month, 29 February outside leap years); a
//!   yearly 29-February series is `FREQ=MONTHLY;INTERVAL=12` from a 29 February;
//! - `COUNT` counts emitted occurrences from the series start, not only the
//!   ones inside the requested range; `seq` numbers them from 0;
//! - every occurrence keeps the series' wall time and is resolved with
//!   [`resolve_local`], so DST adjustments are reported per occurrence.

use crate::civil::{resolve_local, CivilDate, Resolved, Weekday};
use crate::error::CalendarError;
use chrono::{Datelike, NaiveDate, TimeDelta};
use g9_domain::{CivilDateTime, IanaZone, UtcMillis};
use serde::{Deserialize, Serialize};

/// Safety limit on scanned candidates per expansion (skipped candidates count).
pub const MAX_CANDIDATES: u32 = 200_000;

/// Recurrence frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Frequency {
    /// Every `INTERVAL` days.
    Daily,
    /// Every `INTERVAL` weeks.
    Weekly,
    /// Every `INTERVAL` months on the start's day-of-month.
    Monthly,
}

impl Frequency {
    /// RFC 5545 keyword.
    pub const fn keyword(self) -> &'static str {
        match self {
            Frequency::Daily => "DAILY",
            Frequency::Weekly => "WEEKLY",
            Frequency::Monthly => "MONTHLY",
        }
    }
}

/// A recurrence rule (stored with the event's civil start and zone).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecurrenceRule {
    /// Frequency.
    pub freq: Frequency,
    /// Interval between periods (>= 1).
    pub interval: u32,
    /// Selected weekdays (DAILY: filter; WEEKLY: days of the week; MONTHLY: must be empty).
    #[serde(default)]
    pub by_day: Vec<Weekday>,
    /// Inclusive last date of the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<CivilDate>,
    /// Maximum number of occurrences (>= 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

impl RecurrenceRule {
    /// Construct a rule with no `BYDAY`, `UNTIL` or `COUNT`.
    pub fn new(freq: Frequency, interval: u32) -> Result<Self, CalendarError> {
        let rule = Self {
            freq,
            interval,
            by_day: Vec::new(),
            until: None,
            count: None,
        };
        rule.validate()?;
        Ok(rule)
    }

    /// Set `BYDAY` (deduplicated, Monday-first order).
    pub fn with_by_day(mut self, days: impl IntoIterator<Item = Weekday>) -> Self {
        let mut v: Vec<Weekday> = days.into_iter().collect();
        v.sort();
        v.dedup();
        self.by_day = v;
        self
    }

    /// Set `UNTIL`.
    pub fn with_until(mut self, until: CivilDate) -> Self {
        self.until = Some(until);
        self
    }

    /// Set `COUNT`.
    pub fn with_count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    /// Check structural invariants.
    pub fn validate(&self) -> Result<(), CalendarError> {
        if self.interval == 0 {
            return Err(CalendarError::InvalidRule("INTERVAL must be >= 1".into()));
        }
        if self.until.is_some() && self.count.is_some() {
            return Err(CalendarError::InvalidRule(
                "UNTIL and COUNT are mutually exclusive".into(),
            ));
        }
        if self.count == Some(0) {
            return Err(CalendarError::InvalidRule("COUNT must be >= 1".into()));
        }
        if let Some(u) = self.until {
            if u.to_naive().is_none() {
                return Err(CalendarError::InvalidRule(format!(
                    "UNTIL {u} is not a date"
                )));
            }
        }
        if self.freq == Frequency::Monthly && !self.by_day.is_empty() {
            return Err(CalendarError::InvalidRule(
                "BYDAY is not supported with FREQ=MONTHLY".into(),
            ));
        }
        let mut sorted = self.by_day.clone();
        sorted.sort();
        sorted.dedup();
        if sorted != self.by_day {
            return Err(CalendarError::InvalidRule(
                "BYDAY must be deduplicated and in Monday-first order".into(),
            ));
        }
        Ok(())
    }

    /// Canonical RRULE text (no `RRULE:` prefix), e.g.
    /// `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;UNTIL=20261231`.
    pub fn to_rrule(&self) -> String {
        to_rrule(self)
    }
}

/// Parse the supported RRULE subset. Accepts an optional `RRULE:` prefix,
/// case-insensitive keys/values, and rejects unknown or duplicate parts.
pub fn parse_rrule(text: &str) -> Result<RecurrenceRule, CalendarError> {
    let body = text.trim();
    let body = body
        .strip_prefix("RRULE:")
        .or_else(|| body.strip_prefix("rrule:"))
        .unwrap_or(body);
    if body.is_empty() {
        return Err(CalendarError::RruleParse("empty rule".into()));
    }
    let mut freq = None;
    let mut interval = 1u32;
    let mut by_day: Vec<Weekday> = Vec::new();
    let mut until = None;
    let mut count = None;
    let mut seen: Vec<String> = Vec::new();
    for part in body.split(';') {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| CalendarError::RruleParse(format!("part `{part}` is not KEY=VALUE")))?;
        let key = key.trim().to_ascii_uppercase();
        let value = value.trim();
        if seen.contains(&key) {
            return Err(CalendarError::RruleParse(format!("duplicate part {key}")));
        }
        seen.push(key.clone());
        match key.as_str() {
            "FREQ" => {
                freq = Some(match value.to_ascii_uppercase().as_str() {
                    "DAILY" => Frequency::Daily,
                    "WEEKLY" => Frequency::Weekly,
                    "MONTHLY" => Frequency::Monthly,
                    other => {
                        return Err(CalendarError::RruleParse(format!(
                            "unsupported FREQ `{other}` (DAILY, WEEKLY, MONTHLY)"
                        )))
                    }
                });
            }
            "INTERVAL" => {
                interval = value.parse::<u32>().map_err(|_| {
                    CalendarError::RruleParse("INTERVAL must be a positive integer".into())
                })?;
            }
            "BYDAY" => {
                for code in value.split(',') {
                    let day = Weekday::from_rrule_code(code.trim()).ok_or_else(|| {
                        CalendarError::RruleParse(format!("unsupported BYDAY value `{code}`"))
                    })?;
                    by_day.push(day);
                }
                by_day.sort();
                by_day.dedup();
            }
            "UNTIL" => {
                until = Some(parse_until(value)?);
            }
            "COUNT" => {
                count = Some(value.parse::<u32>().map_err(|_| {
                    CalendarError::RruleParse("COUNT must be a positive integer".into())
                })?);
            }
            other => {
                return Err(CalendarError::RruleParse(format!(
                    "unsupported part {other} (FREQ, INTERVAL, BYDAY, UNTIL, COUNT)"
                )))
            }
        }
    }
    let freq = freq.ok_or_else(|| CalendarError::RruleParse("FREQ is required".into()))?;
    let rule = RecurrenceRule {
        freq,
        interval,
        by_day,
        until,
        count,
    };
    rule.validate()?;
    Ok(rule)
}

fn parse_until(value: &str) -> Result<CivilDate, CalendarError> {
    if value.len() != 8 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CalendarError::RruleParse(
            "UNTIL must be a date in YYYYMMDD form".into(),
        ));
    }
    let year: i32 = value[0..4]
        .parse()
        .map_err(|_| CalendarError::RruleParse("UNTIL year".into()))?;
    let month: u8 = value[4..6]
        .parse()
        .map_err(|_| CalendarError::RruleParse("UNTIL month".into()))?;
    let day: u8 = value[6..8]
        .parse()
        .map_err(|_| CalendarError::RruleParse("UNTIL day".into()))?;
    CivilDate::new(year, month, day)
        .map_err(|_| CalendarError::RruleParse(format!("UNTIL {value} is not a valid date")))
}

/// Canonical RRULE text for a rule (see [`RecurrenceRule::to_rrule`]).
pub fn to_rrule(rule: &RecurrenceRule) -> String {
    let mut parts = vec![format!("FREQ={}", rule.freq.keyword())];
    if rule.interval != 1 {
        parts.push(format!("INTERVAL={}", rule.interval));
    }
    if !rule.by_day.is_empty() {
        let days: Vec<&str> = rule.by_day.iter().map(|d| d.rrule_code()).collect();
        parts.push(format!("BYDAY={}", days.join(",")));
    }
    if let Some(u) = rule.until {
        parts.push(format!("UNTIL={:04}{:02}{:02}", u.year, u.month, u.day));
    }
    if let Some(c) = rule.count {
        parts.push(format!("COUNT={c}"));
    }
    parts.join(";")
}

/// One occurrence of a recurring series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Occurrence {
    /// 0-based index of this occurrence from the series start (skipped
    /// candidates do not consume a number).
    pub seq: u32,
    /// The wall time the series intends for this occurrence.
    pub civil: CivilDateTime,
    /// The instant it resolves to (with any DST adjustment).
    pub resolved: Resolved,
}

/// A candidate produced by one rule step: `date` is `None` when the step
/// yields no occurrence (missing day-of-month, weekday filtered out, before
/// the series start); `probe` is a date at or before where the occurrence
/// would have been, used only to decide when the range end is passed.
struct Candidate {
    date: Option<NaiveDate>,
    probe: NaiveDate,
}

fn days(n: i64) -> Result<TimeDelta, CalendarError> {
    TimeDelta::try_days(n).ok_or(CalendarError::OutOfRange)
}

fn step_candidates(
    rule: &RecurrenceRule,
    start: NaiveDate,
    k: u32,
) -> Result<Vec<Candidate>, CalendarError> {
    let interval = i64::from(rule.interval);
    let k = i64::from(k);
    match rule.freq {
        Frequency::Daily => {
            let date = start
                .checked_add_signed(days(k * interval)?)
                .ok_or(CalendarError::OutOfRange)?;
            let selected = rule.by_day.is_empty()
                || rule.by_day.contains(&Weekday::from_chrono(date.weekday()));
            Ok(vec![Candidate {
                date: selected.then_some(date),
                probe: date,
            }])
        }
        Frequency::Weekly => {
            let start_wd = Weekday::from_chrono(start.weekday());
            let monday = start
                .checked_sub_signed(days(i64::from(start_wd.days_from_monday()))?)
                .ok_or(CalendarError::OutOfRange)?;
            let base = monday
                .checked_add_signed(days(k * 7 * interval)?)
                .ok_or(CalendarError::OutOfRange)?;
            let selected: Vec<Weekday> = if rule.by_day.is_empty() {
                vec![start_wd]
            } else {
                rule.by_day.clone()
            };
            let mut out = Vec::with_capacity(selected.len());
            for wd in selected {
                let date = base
                    .checked_add_signed(days(i64::from(wd.days_from_monday()))?)
                    .ok_or(CalendarError::OutOfRange)?;
                out.push(Candidate {
                    date: (date >= start).then_some(date),
                    probe: date,
                });
            }
            Ok(out)
        }
        Frequency::Monthly => {
            let index = i64::from(start.year()) * 12 + i64::from(start.month0()) + k * interval;
            let year =
                i32::try_from(index.div_euclid(12)).map_err(|_| CalendarError::OutOfRange)?;
            let month =
                u32::try_from(index.rem_euclid(12) + 1).map_err(|_| CalendarError::OutOfRange)?;
            let probe = NaiveDate::from_ymd_opt(year, month, 1).ok_or(CalendarError::OutOfRange)?;
            let date = NaiveDate::from_ymd_opt(year, month, start.day());
            Ok(vec![Candidate { date, probe }])
        }
    }
}

/// Expand a rule from a series start (civil time in `zone`) into the
/// occurrences whose instants fall in `[range.0, range.1)`.
pub fn occurrences(
    rule: &RecurrenceRule,
    start: &CivilDateTime,
    zone: &IanaZone,
    range: (UtcMillis, UtcMillis),
) -> Result<Vec<Occurrence>, CalendarError> {
    rule.validate()?;
    if range.0 >= range.1 {
        return Err(CalendarError::InvalidRange);
    }
    let start_naive = start
        .to_naive()
        .ok_or_else(|| CalendarError::InvalidCivil(crate::civil::display_civil(start)))?;
    let start_date = start_naive.date();
    let until = match rule.until {
        Some(u) => Some(
            u.to_naive()
                .ok_or_else(|| CalendarError::InvalidRule(format!("UNTIL {u}")))?,
        ),
        None => None,
    };
    if let Some(u) = until {
        if u < start_date {
            return Ok(Vec::new());
        }
    }

    let mut out = Vec::new();
    let mut emitted = 0u32;
    let mut scanned = 0u32;
    let mut k = 0u32;
    'outer: loop {
        for cand in step_candidates(rule, start_date, k)? {
            scanned += 1;
            if scanned > MAX_CANDIDATES {
                return Err(CalendarError::OccurrenceLimit(MAX_CANDIDATES));
            }
            let probe_date = cand.date.unwrap_or(cand.probe);
            if let Some(u) = until {
                if probe_date > u {
                    break 'outer;
                }
            }
            let civil = CivilDateTime {
                year: probe_date.year(),
                month: probe_date.month() as u8,
                day: probe_date.day() as u8,
                hour: start.hour,
                minute: start.minute,
                second: start.second,
            };
            let resolved = resolve_local(&civil, zone)?;
            if resolved.instant >= range.1 {
                break 'outer;
            }
            if cand.date.is_none() {
                continue;
            }
            let seq = emitted;
            emitted += 1;
            if resolved.instant >= range.0 {
                out.push(Occurrence {
                    seq,
                    civil,
                    resolved,
                });
            }
            if let Some(c) = rule.count {
                if emitted >= c {
                    break 'outer;
                }
            }
        }
        k = k
            .checked_add(1)
            .ok_or(CalendarError::OccurrenceLimit(MAX_CANDIDATES))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn range_utc(from: (i32, u32, u32), to: (i32, u32, u32)) -> (UtcMillis, UtcMillis) {
        use chrono::{TimeZone, Utc};
        let a = Utc
            .with_ymd_and_hms(from.0, from.1, from.2, 0, 0, 0)
            .single()
            .unwrap();
        let b = Utc
            .with_ymd_and_hms(to.0, to.1, to.2, 0, 0, 0)
            .single()
            .unwrap();
        (
            UtcMillis::from_millis(a.timestamp_millis()),
            UtcMillis::from_millis(b.timestamp_millis()),
        )
    }

    #[test]
    fn rrule_parse_roundtrip() {
        let cases = [
            "FREQ=DAILY",
            "FREQ=DAILY;INTERVAL=3;COUNT=10",
            "FREQ=WEEKLY;BYDAY=MO,WE,FR",
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;UNTIL=20261231",
            "FREQ=MONTHLY;INTERVAL=12;COUNT=4",
        ];
        for text in cases {
            let rule = parse_rrule(text).unwrap();
            assert_eq!(rule.to_rrule(), text);
            assert_eq!(parse_rrule(&rule.to_rrule()).unwrap(), rule);
            let json = serde_json::to_string(&rule).unwrap();
            let back: RecurrenceRule = serde_json::from_str(&json).unwrap();
            assert_eq!(back, rule);
        }
        // Non-canonical input normalizes.
        let rule = parse_rrule("rrule:freq=weekly;byday=fr,mo,mo;interval=1").unwrap();
        assert_eq!(rule.to_rrule(), "FREQ=WEEKLY;BYDAY=MO,FR");
    }

    #[test]
    fn rrule_rejects_unsupported_and_malformed() {
        for bad in [
            "",
            "FREQ=YEARLY",
            "FREQ=HOURLY",
            "INTERVAL=2",
            "FREQ=DAILY;INTERVAL=0",
            "FREQ=DAILY;COUNT=0",
            "FREQ=DAILY;UNTIL=20261231;COUNT=3",
            "FREQ=DAILY;UNTIL=2026-12-31",
            "FREQ=DAILY;UNTIL=20261231T000000Z",
            "FREQ=DAILY;UNTIL=20270229",
            "FREQ=DAILY;BYDAY=XX",
            "FREQ=DAILY;FREQ=WEEKLY",
            "FREQ=DAILY;BYMONTHDAY=1",
            "FREQ=MONTHLY;BYDAY=MO",
            "FREQ",
        ] {
            let err = parse_rrule(bad).unwrap_err();
            assert!(
                matches!(
                    err,
                    CalendarError::RruleParse(_) | CalendarError::InvalidRule(_)
                ),
                "{bad}: {err:?}"
            );
        }
    }

    #[test]
    fn daily_with_interval_and_count() {
        let rule = RecurrenceRule::new(Frequency::Daily, 2)
            .unwrap()
            .with_count(3);
        let start = CivilDateTime::new(2026, 1, 1, 9, 0, 0).unwrap();
        let occ = occurrences(
            &rule,
            &start,
            &IanaZone::utc(),
            range_utc((2025, 12, 1), (2027, 1, 1)),
        )
        .unwrap();
        let days: Vec<u8> = occ.iter().map(|o| o.civil.day).collect();
        assert_eq!(days, vec![1, 3, 5]);
        assert_eq!(occ.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn count_counts_from_series_start_not_range() {
        let rule = RecurrenceRule::new(Frequency::Daily, 1)
            .unwrap()
            .with_count(5);
        let start = CivilDateTime::new(2026, 1, 1, 9, 0, 0).unwrap();
        let occ = occurrences(
            &rule,
            &start,
            &IanaZone::utc(),
            range_utc((2026, 1, 4), (2026, 2, 1)),
        )
        .unwrap();
        let days: Vec<u8> = occ.iter().map(|o| o.civil.day).collect();
        assert_eq!(days, vec![4, 5]);
        assert_eq!(occ[0].seq, 3);
    }

    #[test]
    fn weekly_without_byday_uses_start_weekday_and_until_is_inclusive() {
        let rule = RecurrenceRule::new(Frequency::Weekly, 1)
            .unwrap()
            .with_until(CivilDate::new(2026, 1, 20).unwrap());
        let start = CivilDateTime::new(2026, 1, 6, 9, 0, 0).unwrap(); // Tuesday
        let occ = occurrences(
            &rule,
            &start,
            &IanaZone::utc(),
            range_utc((2026, 1, 1), (2026, 3, 1)),
        )
        .unwrap();
        let days: Vec<u8> = occ.iter().map(|o| o.civil.day).collect();
        assert_eq!(days, vec![6, 13, 20]);
    }

    #[test]
    fn weekly_byday_does_not_emit_before_start() {
        let rule = RecurrenceRule::new(Frequency::Weekly, 1)
            .unwrap()
            .with_by_day([Weekday::Mon, Weekday::Fri]);
        let start = CivilDateTime::new(2026, 1, 7, 9, 0, 0).unwrap(); // Wednesday
        let occ = occurrences(
            &rule,
            &start,
            &IanaZone::utc(),
            range_utc((2026, 1, 1), (2026, 1, 20)),
        )
        .unwrap();
        let days: Vec<u8> = occ.iter().map(|o| o.civil.day).collect();
        assert_eq!(days, vec![9, 12, 16, 19]);
    }

    #[test]
    fn until_before_start_yields_nothing() {
        let rule = RecurrenceRule::new(Frequency::Daily, 1)
            .unwrap()
            .with_until(CivilDate::new(2025, 1, 1).unwrap());
        let start = CivilDateTime::new(2026, 1, 1, 9, 0, 0).unwrap();
        let occ = occurrences(
            &rule,
            &start,
            &IanaZone::utc(),
            range_utc((2025, 1, 1), (2027, 1, 1)),
        )
        .unwrap();
        assert!(occ.is_empty());
    }

    #[test]
    fn invalid_range_and_rule_are_refused() {
        let rule = RecurrenceRule::new(Frequency::Daily, 1).unwrap();
        let start = CivilDateTime::new(2026, 1, 1, 9, 0, 0).unwrap();
        let (a, b) = range_utc((2026, 1, 1), (2026, 2, 1));
        assert_eq!(
            occurrences(&rule, &start, &IanaZone::utc(), (b, a)).unwrap_err(),
            CalendarError::InvalidRange
        );
        let bad = RecurrenceRule {
            freq: Frequency::Daily,
            interval: 0,
            by_day: vec![],
            until: None,
            count: None,
        };
        assert!(matches!(
            occurrences(&bad, &start, &IanaZone::utc(), (a, b)).unwrap_err(),
            CalendarError::InvalidRule(_)
        ));
        assert!(RecurrenceRule::new(Frequency::Daily, 0).is_err());
    }
}
