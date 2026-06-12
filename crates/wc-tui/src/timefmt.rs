//! Kickoff-time formatting and timezone conversion.
//!
//! The local UTC offset is captured once at startup (on the main thread, before
//! any worker threads spawn — the `time` crate refuses to read it concurrently)
//! and threaded through to these helpers.

use time::format_description::FormatItem;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

use crate::config::TimezonePref;

const TIME_HM: &[FormatItem<'_>] = format_description!("[hour]:[minute]");
const DAY_TIME: &[FormatItem<'_>] = format_description!("[weekday repr:short] [hour]:[minute]");
const DATE_DAY: &[FormatItem<'_>] =
    format_description!("[weekday repr:short] [month repr:short] [day padding:none]");

/// Resolve the display offset for a preference, given the captured local offset.
#[must_use]
pub fn display_offset(pref: &TimezonePref, local: UtcOffset) -> UtcOffset {
    match pref {
        TimezonePref::Local => local,
        TimezonePref::Utc => UtcOffset::UTC,
        TimezonePref::FixedOffset(hours) => {
            UtcOffset::from_hms(*hours, 0, 0).unwrap_or(UtcOffset::UTC)
        }
    }
}

/// Convert a UTC instant to the display zone.
#[must_use]
pub fn to_display(dt: OffsetDateTime, pref: &TimezonePref, local: UtcOffset) -> OffsetDateTime {
    dt.to_offset(display_offset(pref, local))
}

/// Format a kickoff as `"Sat 14:00"` in the display zone.
#[must_use]
pub fn kickoff_day_time(dt: OffsetDateTime, pref: &TimezonePref, local: UtcOffset) -> String {
    to_display(dt, pref, local)
        .format(DAY_TIME)
        .unwrap_or_else(|_| "??".to_owned())
}

/// Format just the time-of-day as `"14:00"` in the display zone.
#[must_use]
pub fn time_hm(dt: OffsetDateTime, pref: &TimezonePref, local: UtcOffset) -> String {
    to_display(dt, pref, local)
        .format(TIME_HM)
        .unwrap_or_else(|_| "??:??".to_owned())
}

/// Format a date heading as `"Sat Jun 14"` in the display zone.
#[must_use]
pub fn date_heading(dt: OffsetDateTime, pref: &TimezonePref, local: UtcOffset) -> String {
    to_display(dt, pref, local)
        .format(DATE_DAY)
        .unwrap_or_else(|_| "??".to_owned())
}
