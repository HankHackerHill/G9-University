//! Civil (wall-clock) time resolution against IANA zones (M03, ARCHITECTURE §7).
//!
//! DST policy (binding):
//! - a nonexistent local time (spring-forward gap) is shifted forward by the
//!   length of the gap — the requested wall time interpreted with the offset
//!   in force *before* the transition — and flagged [`Adjustment::Gap`].
//!   Example: `2026-03-29 02:30 Europe/Berlin` does not exist; it resolves to
//!   `03:30 CEST` (`01:30Z`), the first valid instant at which the requested
//!   wall-clock distance past the transition has elapsed;
//! - an ambiguous local time (fall-back overlap) resolves to the earlier
//!   instant, i.e. the pre-transition offset, and is flagged
//!   [`Adjustment::Overlap`] with the alternative instant;
//! - everything else is [`Adjustment::None`].
//!
//! Instants are always [`UtcMillis`]; local time is never stored without its
//! zone. The resolution uses `chrono_tz::Tz::from_local_datetime`, which
//! yields `LocalResult::{Single, Ambiguous, None}`.

use crate::error::CalendarError;
use chrono::{Datelike, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeDelta, TimeZone};
use chrono_tz::Tz;
use g9_domain::{CivilDateTime, IanaZone, UtcMillis};
use serde::{Deserialize, Serialize};

/// Maximum distance (hours) scanned forward to find the end of a DST gap.
/// Real gaps are at most 24 h (date-line changes such as Samoa 2011).
pub const MAX_GAP_SCAN_HOURS: u32 = 48;

/// Milliseconds in a nominal day (all-day events are date-only; their real
/// span in instants is computed from local midnights and may be 23 h or 25 h).
pub const DAY_MS: i64 = 86_400_000;

/// Day of the week. Serialized as `MON`..`SUN`; RRULE codes are `MO`..`SU`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Weekday {
    /// Monday.
    Mon,
    /// Tuesday.
    Tue,
    /// Wednesday.
    Wed,
    /// Thursday.
    Thu,
    /// Friday.
    Fri,
    /// Saturday.
    Sat,
    /// Sunday.
    Sun,
}

impl Weekday {
    /// All days, Monday first.
    pub const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    /// Two-letter RFC 5545 code.
    pub const fn rrule_code(self) -> &'static str {
        match self {
            Weekday::Mon => "MO",
            Weekday::Tue => "TU",
            Weekday::Wed => "WE",
            Weekday::Thu => "TH",
            Weekday::Fri => "FR",
            Weekday::Sat => "SA",
            Weekday::Sun => "SU",
        }
    }

    /// Parse an RFC 5545 two-letter code (case-insensitive).
    pub fn from_rrule_code(code: &str) -> Option<Weekday> {
        match code.to_ascii_uppercase().as_str() {
            "MO" => Some(Weekday::Mon),
            "TU" => Some(Weekday::Tue),
            "WE" => Some(Weekday::Wed),
            "TH" => Some(Weekday::Thu),
            "FR" => Some(Weekday::Fri),
            "SA" => Some(Weekday::Sat),
            "SU" => Some(Weekday::Sun),
            _ => None,
        }
    }

    /// Days since Monday (0..=6).
    pub const fn days_from_monday(self) -> u8 {
        match self {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        }
    }

    /// Convert from chrono.
    pub const fn from_chrono(w: chrono::Weekday) -> Weekday {
        match w {
            chrono::Weekday::Mon => Weekday::Mon,
            chrono::Weekday::Tue => Weekday::Tue,
            chrono::Weekday::Wed => Weekday::Wed,
            chrono::Weekday::Thu => Weekday::Thu,
            chrono::Weekday::Fri => Weekday::Fri,
            chrono::Weekday::Sat => Weekday::Sat,
            chrono::Weekday::Sun => Weekday::Sun,
        }
    }
}

/// A civil (wall-clock) date without a time, used for all-day events and
/// recurrence `UNTIL` bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CivilDate {
    /// Proleptic Gregorian year.
    pub year: i32,
    /// Month 1..=12.
    pub month: u8,
    /// Day of month 1..=31 (validated against the month).
    pub day: u8,
}

impl CivilDate {
    /// Validate and construct a date.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, CalendarError> {
        NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day))
            .map(|_| Self { year, month, day })
            .ok_or_else(|| CalendarError::InvalidCivil(format!("{year}-{month:02}-{day:02}")))
    }

    /// The date part of a civil date-time.
    pub const fn of(civil: &CivilDateTime) -> Self {
        Self {
            year: civil.year,
            month: civil.month,
            day: civil.day,
        }
    }

    /// Convert to chrono; `None` if the fields are not a real date.
    pub fn to_naive(self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(self.year, u32::from(self.month), u32::from(self.day))
    }

    /// Convert from chrono.
    pub fn from_naive(d: NaiveDate) -> Self {
        Self {
            year: d.year(),
            month: d.month() as u8,
            day: d.day() as u8,
        }
    }

    /// Local midnight on this date.
    pub const fn midnight(self) -> CivilDateTime {
        CivilDateTime {
            year: self.year,
            month: self.month,
            day: self.day,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }

    /// This date at the given wall time.
    pub fn at(self, hour: u8, minute: u8, second: u8) -> Result<CivilDateTime, CalendarError> {
        CivilDateTime::new(self.year, self.month, self.day, hour, minute, second)
            .map_err(CalendarError::from)
    }

    /// Day of the week.
    pub fn weekday(self) -> Result<Weekday, CalendarError> {
        self.to_naive()
            .map(|d| Weekday::from_chrono(d.weekday()))
            .ok_or_else(|| CalendarError::InvalidCivil(self.to_string()))
    }

    /// Add (or subtract) whole days.
    pub fn add_days(self, days: i64) -> Result<Self, CalendarError> {
        let d = self
            .to_naive()
            .ok_or_else(|| CalendarError::InvalidCivil(self.to_string()))?;
        let delta = TimeDelta::try_days(days).ok_or(CalendarError::OutOfRange)?;
        d.checked_add_signed(delta)
            .map(Self::from_naive)
            .ok_or(CalendarError::OutOfRange)
    }

    /// Whether the year is a leap year.
    pub const fn is_leap_year(self) -> bool {
        (self.year % 4 == 0 && self.year % 100 != 0) || self.year % 400 == 0
    }
}

impl std::fmt::Display for CivilDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// How a civil time was adjusted when it was resolved to an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Adjustment {
    /// The local time existed exactly once.
    None,
    /// The local time did not exist (spring-forward gap); it was shifted
    /// forward by the gap length.
    Gap {
        /// The wall time that was requested.
        requested: CivilDateTime,
        /// The wall time the instant actually shows in the zone.
        shifted_to: CivilDateTime,
    },
    /// The local time existed twice (fall-back overlap); the earlier instant
    /// (pre-transition offset) was chosen.
    Overlap {
        /// UTC offset of the chosen instant, in minutes.
        chosen_offset_minutes: i32,
        /// The later instant with the post-transition offset.
        alternative: UtcMillis,
    },
}

impl Adjustment {
    /// Whether any adjustment took place.
    pub const fn is_adjusted(self) -> bool {
        !matches!(self, Adjustment::None)
    }
}

/// A civil time resolved to an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resolved {
    /// The resolved instant.
    pub instant: UtcMillis,
    /// What happened during resolution.
    pub adjustment: Adjustment,
    /// UTC offset of the zone at `instant`, in minutes (display aid).
    pub offset_minutes: i32,
}

fn format_civil(c: &CivilDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

fn offset_minutes_of<T: TimeZone>(dt: &chrono::DateTime<T>) -> i32 {
    dt.offset().fix().local_minus_utc() / 60
}

fn local_exists(tz: Tz, naive: &NaiveDateTime) -> bool {
    !matches!(tz.from_local_datetime(naive), LocalResult::None)
}

/// Resolve a civil date-time in a zone to an instant, applying the DST policy
/// documented at the top of this module.
pub fn resolve_local(civil: &CivilDateTime, zone: &IanaZone) -> Result<Resolved, CalendarError> {
    let naive = civil
        .to_naive()
        .ok_or_else(|| CalendarError::InvalidCivil(format_civil(civil)))?;
    let tz = zone.tz();
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Ok(Resolved {
            instant: UtcMillis::from_millis(dt.timestamp_millis()),
            adjustment: Adjustment::None,
            offset_minutes: offset_minutes_of(&dt),
        }),
        LocalResult::Ambiguous(earliest, latest) => Ok(Resolved {
            instant: UtcMillis::from_millis(earliest.timestamp_millis()),
            adjustment: Adjustment::Overlap {
                chosen_offset_minutes: offset_minutes_of(&earliest),
                alternative: UtcMillis::from_millis(latest.timestamp_millis()),
            },
            offset_minutes: offset_minutes_of(&earliest),
        }),
        LocalResult::None => resolve_gap(civil, naive, tz),
    }
}

/// Gap resolution: locate the end of the gap exactly (minute scan, then
/// second refinement), derive the pre/post offsets from the transition
/// instant, and shift the requested wall time forward by the gap length.
fn resolve_gap(
    requested: &CivilDateTime,
    naive: NaiveDateTime,
    tz: Tz,
) -> Result<Resolved, CalendarError> {
    let one_minute = TimeDelta::try_minutes(1).ok_or(CalendarError::OutOfRange)?;
    let one_second = TimeDelta::try_seconds(1).ok_or(CalendarError::OutOfRange)?;
    let limit = MAX_GAP_SCAN_HOURS * 60;

    let mut probe = naive;
    let mut minutes = 0u32;
    while !local_exists(tz, &probe) {
        minutes += 1;
        if minutes > limit {
            return Err(CalendarError::UnresolvableGap(MAX_GAP_SCAN_HOURS));
        }
        probe = probe
            .checked_add_signed(one_minute)
            .ok_or(CalendarError::OutOfRange)?;
    }
    let mut gap_end = probe;
    loop {
        let prev = gap_end
            .checked_sub_signed(one_second)
            .ok_or(CalendarError::OutOfRange)?;
        if prev < naive || !local_exists(tz, &prev) {
            break;
        }
        gap_end = prev;
    }

    let transition = tz
        .from_local_datetime(&gap_end)
        .earliest()
        .ok_or(CalendarError::UnresolvableGap(MAX_GAP_SCAN_HOURS))?;
    let offset_after = transition.offset().fix().local_minus_utc();
    let before = transition
        .naive_utc()
        .checked_sub_signed(one_second)
        .ok_or(CalendarError::OutOfRange)?;
    let offset_before = tz.offset_from_utc_datetime(&before).fix().local_minus_utc();
    let gap_seconds = offset_after - offset_before;
    if gap_seconds <= 0 {
        return Err(CalendarError::UnresolvableGap(MAX_GAP_SCAN_HOURS));
    }
    let gap = TimeDelta::try_seconds(i64::from(gap_seconds)).ok_or(CalendarError::OutOfRange)?;
    let shifted = naive
        .checked_add_signed(gap)
        .ok_or(CalendarError::OutOfRange)?;
    let before_delta =
        TimeDelta::try_seconds(i64::from(offset_before)).ok_or(CalendarError::OutOfRange)?;
    let instant_utc = naive
        .checked_sub_signed(before_delta)
        .ok_or(CalendarError::OutOfRange)?;
    let local_check = tz.from_utc_datetime(&instant_utc).naive_local();
    if local_check != shifted {
        return Err(CalendarError::UnresolvableGap(MAX_GAP_SCAN_HOURS));
    }
    Ok(Resolved {
        instant: UtcMillis::from_millis(instant_utc.and_utc().timestamp_millis()),
        adjustment: Adjustment::Gap {
            requested: *requested,
            shifted_to: CivilDateTime::from_naive(shifted),
        },
        offset_minutes: offset_after / 60,
    })
}

/// Convert an instant to the wall-clock time shown in a zone.
pub fn to_civil(instant: UtcMillis, zone: &IanaZone) -> Result<CivilDateTime, CalendarError> {
    let dt = zone
        .tz()
        .timestamp_millis_opt(instant.as_millis())
        .single()
        .ok_or(CalendarError::OutOfRange)?;
    Ok(CivilDateTime::from_naive(dt.naive_local()))
}

/// UTC offset (minutes) of a zone at an instant.
pub fn offset_minutes_at(instant: UtcMillis, zone: &IanaZone) -> Result<i32, CalendarError> {
    let dt = zone
        .tz()
        .timestamp_millis_opt(instant.as_millis())
        .single()
        .ok_or(CalendarError::OutOfRange)?;
    Ok(offset_minutes_of(&dt))
}

/// Add (or subtract) whole civil days, preserving the wall time.
pub fn add_civil_days(civil: &CivilDateTime, days: i64) -> Result<CivilDateTime, CalendarError> {
    let date = CivilDate::of(civil).add_days(days)?;
    Ok(CivilDateTime {
        year: date.year,
        month: date.month,
        day: date.day,
        hour: civil.hour,
        minute: civil.minute,
        second: civil.second,
    })
}

/// Day of the week of a civil date-time.
pub fn civil_weekday(civil: &CivilDateTime) -> Result<Weekday, CalendarError> {
    CivilDate::of(civil).weekday()
}

/// Minutes since local midnight of a civil date-time.
pub fn minute_of_day(civil: &CivilDateTime) -> u16 {
    u16::from(civil.hour) * 60 + u16::from(civil.minute)
}

/// Render a civil date-time as `YYYY-MM-DDTHH:MM:SS` for traces and errors.
pub fn display_civil(civil: &CivilDateTime) -> String {
    format_civil(civil)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pretty_assertions::assert_eq;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> UtcMillis {
        UtcMillis::from_millis(
            Utc.with_ymd_and_hms(y, mo, d, h, mi, s)
                .single()
                .unwrap()
                .timestamp_millis(),
        )
    }

    #[test]
    fn single_resolution_has_no_adjustment() {
        let zone = IanaZone::parse("Europe/Berlin").unwrap();
        let c = CivilDateTime::new(2026, 11, 6, 12, 0, 0).unwrap();
        let r = resolve_local(&c, &zone).unwrap();
        assert_eq!(r.instant, utc(2026, 11, 6, 11, 0, 0));
        assert_eq!(r.adjustment, Adjustment::None);
        assert_eq!(r.offset_minutes, 60);
        assert_eq!(to_civil(r.instant, &zone).unwrap(), c);
    }

    #[test]
    fn utc_zone_never_adjusts() {
        let zone = IanaZone::utc();
        for (m, d) in [(3, 29), (10, 25), (3, 8), (11, 1)] {
            let c = CivilDateTime::new(2026, m, d, 2, 30, 0).unwrap();
            let r = resolve_local(&c, &zone).unwrap();
            assert_eq!(r.adjustment, Adjustment::None);
            assert_eq!(r.offset_minutes, 0);
            assert_eq!(to_civil(r.instant, &zone).unwrap(), c);
        }
    }

    #[test]
    fn gap_with_seconds_component_is_shifted_exactly() {
        let zone = IanaZone::parse("Europe/Berlin").unwrap();
        let c = CivilDateTime::new(2026, 3, 29, 2, 59, 59).unwrap();
        let r = resolve_local(&c, &zone).unwrap();
        assert_eq!(r.instant, utc(2026, 3, 29, 1, 59, 59));
        match r.adjustment {
            Adjustment::Gap { shifted_to, .. } => {
                assert_eq!(
                    shifted_to,
                    CivilDateTime::new(2026, 3, 29, 3, 59, 59).unwrap()
                );
            }
            other => panic!("expected gap, got {other:?}"),
        }
    }

    #[test]
    fn lord_howe_half_hour_gap() {
        // Lord Howe Island shifts by 30 minutes: 2026-10-04 02:00 -> 02:30.
        let zone = IanaZone::parse("Australia/Lord_Howe").unwrap();
        let c = CivilDateTime::new(2026, 10, 4, 2, 15, 0).unwrap();
        let r = resolve_local(&c, &zone).unwrap();
        match r.adjustment {
            Adjustment::Gap { shifted_to, .. } => {
                assert_eq!(
                    shifted_to,
                    CivilDateTime::new(2026, 10, 4, 2, 45, 0).unwrap()
                );
            }
            other => panic!("expected gap, got {other:?}"),
        }
        assert_eq!(to_civil(r.instant, &zone).unwrap().minute, 45);
    }

    #[test]
    fn civil_date_arithmetic_and_weekday() {
        let d = CivilDate::new(2026, 3, 2).unwrap();
        assert_eq!(d.weekday().unwrap(), Weekday::Mon);
        assert_eq!(d.add_days(6).unwrap(), CivilDate::new(2026, 3, 8).unwrap());
        assert_eq!(d.add_days(-1).unwrap(), CivilDate::new(2026, 3, 1).unwrap());
        assert!(CivilDate::new(2027, 2, 29).is_err());
        assert!(CivilDate::new(2028, 2, 29).unwrap().is_leap_year());
        assert_eq!(d.to_string(), "2026-03-02");
        let c = add_civil_days(&CivilDateTime::new(2026, 12, 31, 23, 59, 0).unwrap(), 1).unwrap();
        assert_eq!(c, CivilDateTime::new(2027, 1, 1, 23, 59, 0).unwrap());
    }

    #[test]
    fn weekday_codes_roundtrip() {
        for w in Weekday::ALL {
            assert_eq!(Weekday::from_rrule_code(w.rrule_code()), Some(w));
            assert_eq!(
                Weekday::from_rrule_code(&w.rrule_code().to_lowercase()),
                Some(w)
            );
        }
        assert_eq!(Weekday::from_rrule_code("XX"), None);
        assert_eq!(serde_json::to_string(&Weekday::Wed).unwrap(), "\"WED\"");
    }

    #[test]
    fn adjustment_serializes_with_kind_tag() {
        let v = serde_json::to_value(Adjustment::None).unwrap();
        assert_eq!(v["kind"], "NONE");
        let v = serde_json::to_value(Adjustment::Overlap {
            chosen_offset_minutes: 120,
            alternative: UtcMillis::from_millis(1),
        })
        .unwrap();
        assert_eq!(v["kind"], "OVERLAP");
        let back: Adjustment = serde_json::from_value(v).unwrap();
        assert!(back.is_adjusted());
    }
}
