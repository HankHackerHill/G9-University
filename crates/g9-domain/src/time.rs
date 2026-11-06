//! Time primitives (handoff v6.0 §7, §8): UTC integer epoch milliseconds for
//! instants, IANA zone identifiers for human calendars, explicit non-negative
//! durations, and civil (wall-clock) date-times that are resolved to instants
//! only by the calendar crate (which owns DST gap/overlap semantics).

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Errors for time primitives.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimeError {
    #[error("duration must be non-negative, got {0} ms")]
    NegativeDuration(i64),
    #[error("unknown IANA time zone")]
    UnknownZone,
    #[error("invalid civil date-time: {0}")]
    InvalidCivil(String),
    #[error("instant arithmetic overflow")]
    Overflow,
}

/// A UTC instant as integer milliseconds since the Unix epoch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UtcMillis(i64);

impl UtcMillis {
    pub const UNIX_EPOCH: UtcMillis = UtcMillis(0);

    pub const fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }

    /// Current wall-clock time. Callers that need a consistent time base for
    /// elapsed-time accounting (session timer) must NOT use this for intervals;
    /// see the calendar crate's monotonic timer.
    pub fn now() -> Self {
        Self(Utc::now().timestamp_millis())
    }

    pub fn checked_add(self, d: DurationMs) -> Result<Self, TimeError> {
        self.0.checked_add(d.0).map(Self).ok_or(TimeError::Overflow)
    }

    pub fn checked_sub(self, d: DurationMs) -> Result<Self, TimeError> {
        self.0.checked_sub(d.0).map(Self).ok_or(TimeError::Overflow)
    }

    /// Elapsed duration from `earlier` to `self`; `None` if `earlier` is later.
    pub fn duration_since(self, earlier: UtcMillis) -> Option<DurationMs> {
        let diff = self.0.checked_sub(earlier.0)?;
        DurationMs::new(diff).ok()
    }

    pub fn to_datetime(self) -> Option<DateTime<Utc>> {
        Utc.timestamp_millis_opt(self.0).single()
    }

    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt.timestamp_millis())
    }

    /// RFC 3339 rendering (millisecond precision) for logs and exports.
    pub fn to_rfc3339(self) -> String {
        match self.to_datetime() {
            Some(dt) => dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            None => format!("<out-of-range {} ms>", self.0),
        }
    }
}

impl fmt::Debug for UtcMillis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UtcMillis({} = {})", self.0, self.to_rfc3339())
    }
}

impl fmt::Display for UtcMillis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

/// A non-negative elapsed duration in integer milliseconds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct DurationMs(i64);

impl DurationMs {
    pub const ZERO: DurationMs = DurationMs(0);

    pub fn new(ms: i64) -> Result<Self, TimeError> {
        if ms < 0 {
            Err(TimeError::NegativeDuration(ms))
        } else {
            Ok(Self(ms))
        }
    }

    pub const fn from_minutes_u32(minutes: u32) -> Self {
        Self(minutes as i64 * 60_000)
    }

    pub const fn from_seconds_u32(seconds: u32) -> Self {
        Self(seconds as i64 * 1_000)
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }

    pub const fn as_whole_minutes(self) -> i64 {
        self.0 / 60_000
    }

    pub fn checked_add(self, other: DurationMs) -> Result<Self, TimeError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(TimeError::Overflow)
    }
}

impl TryFrom<i64> for DurationMs {
    type Error = TimeError;
    fn try_from(ms: i64) -> Result<Self, Self::Error> {
        Self::new(ms)
    }
}

impl From<DurationMs> for i64 {
    fn from(d: DurationMs) -> i64 {
        d.0
    }
}

impl fmt::Debug for DurationMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DurationMs({})", self.0)
    }
}

/// A validated IANA time zone identifier such as `Europe/Berlin`.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IanaZone(String);

impl IanaZone {
    pub const UTC: &'static str = "UTC";

    pub fn parse(s: &str) -> Result<Self, TimeError> {
        chrono_tz::Tz::from_str(s)
            .map(|tz| Self(tz.name().to_string()))
            .map_err(|_| TimeError::UnknownZone)
    }

    pub fn utc() -> Self {
        Self("UTC".to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn tz(&self) -> chrono_tz::Tz {
        // Validated at construction; fall back to UTC only on an impossible state.
        chrono_tz::Tz::from_str(&self.0).unwrap_or(chrono_tz::UTC)
    }
}

impl TryFrom<String> for IanaZone {
    type Error = TimeError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<IanaZone> for String {
    fn from(z: IanaZone) -> String {
        z.0
    }
}

impl fmt::Debug for IanaZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IanaZone({})", self.0)
    }
}

impl fmt::Display for IanaZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A civil (wall-clock) date-time without a zone. Recurring events and operator
/// intent are stored in civil time plus an [`IanaZone`]; occurrences are resolved
/// to [`UtcMillis`] by the calendar crate, which must handle DST gaps and overlaps
/// explicitly (handoff §8 "spring-forward/fall-back").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CivilDateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl CivilDateTime {
    pub fn new(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<Self, TimeError> {
        let date = chrono::NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day))
            .ok_or_else(|| TimeError::InvalidCivil(format!("{year}-{month:02}-{day:02}")))?;
        date.and_hms_opt(u32::from(hour), u32::from(minute), u32::from(second))
            .ok_or_else(|| TimeError::InvalidCivil(format!("{hour:02}:{minute:02}:{second:02}")))?;
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    pub fn to_naive(self) -> Option<chrono::NaiveDateTime> {
        chrono::NaiveDate::from_ymd_opt(self.year, u32::from(self.month), u32::from(self.day))?
            .and_hms_opt(
                u32::from(self.hour),
                u32::from(self.minute),
                u32::from(self.second),
            )
    }

    pub fn from_naive(n: chrono::NaiveDateTime) -> Self {
        use chrono::{Datelike, Timelike};
        Self {
            year: n.year(),
            month: n.month() as u8,
            day: n.day() as u8,
            hour: n.hour() as u8,
            minute: n.minute() as u8,
            second: n.second() as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_rejects_negative() {
        assert_eq!(DurationMs::new(-1), Err(TimeError::NegativeDuration(-1)));
        assert_eq!(DurationMs::from_minutes_u32(420).as_whole_minutes(), 420);
    }

    #[test]
    fn zone_validation() {
        assert!(IanaZone::parse("Europe/Berlin").is_ok());
        assert!(IanaZone::parse("America/New_York").is_ok());
        assert_eq!(IanaZone::parse("Mars/Olympus"), Err(TimeError::UnknownZone));
        let z: Result<IanaZone, _> = serde_json::from_str("\"Not/AZone\"");
        assert!(z.is_err());
    }

    #[test]
    fn civil_validation() {
        assert!(CivilDateTime::new(2028, 2, 29, 9, 0, 0).is_ok()); // leap day
        assert!(CivilDateTime::new(2027, 2, 29, 9, 0, 0).is_err());
        assert!(CivilDateTime::new(2026, 11, 5, 24, 0, 0).is_err());
    }

    #[test]
    fn instant_rfc3339() {
        let t = UtcMillis::from_millis(1_762_300_800_000);
        assert_eq!(t.to_rfc3339(), "2025-11-05T00:00:00.000Z");
        assert_eq!(
            t.duration_since(UtcMillis::from_millis(1_762_300_000_000)),
            Some(DurationMs::new(800_000).unwrap())
        );
        assert_eq!(
            UtcMillis::from_millis(5).duration_since(UtcMillis::from_millis(10)),
            None
        );
    }
}
