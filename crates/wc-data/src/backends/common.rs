//! Shared helpers for provider DTO mapping.

use time::{Date, Month, OffsetDateTime, Time, format_description::well_known::Iso8601};

use crate::domain::{Calendar, MatchStatus, Stage};
use crate::error::{DataError, Result};

pub fn parse_time(value: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &Iso8601::DEFAULT)
        .map_err(|err| DataError::Decode(err.to_string()))
}

pub fn stage_from_label(label: &str) -> Option<Stage> {
    let lower = label.to_ascii_lowercase();
    if lower.contains("third") || lower.contains("3rd") {
        Some(Stage::ThirdPlace)
    } else if lower.contains("final") && !lower.contains("semi") && !lower.contains("quarter") {
        Some(Stage::Final)
    } else if lower.contains("semi") {
        Some(Stage::SemiFinal)
    } else if lower.contains("quarter") {
        Some(Stage::QuarterFinal)
    } else if lower.contains("32") {
        Some(Stage::RoundOf32)
    } else if lower.contains("16") {
        Some(Stage::RoundOf16)
    } else if lower.contains("group") || lower == "regular season" {
        Some(Stage::GroupStage)
    } else {
        None
    }
}

pub fn stage_for_date(calendar: &Calendar, kickoff: OffsetDateTime, fallback: Stage) -> Stage {
    calendar
        .stages
        .iter()
        .find(|window| kickoff >= window.start && kickoff <= window.end)
        .map_or(fallback, |window| window.stage)
}

pub fn group_from_text(value: Option<&str>) -> Option<String> {
    let text = value?;
    let lower = text.to_ascii_lowercase();
    let marker = "group ";
    let idx = lower.find(marker)?;
    let rest = &text[idx + marker.len()..];
    rest.chars()
        .find(char::is_ascii_alphabetic)
        .map(|ch| ch.to_ascii_uppercase().to_string())
}

pub fn parse_u8_str(value: Option<&str>) -> Option<u8> {
    value?.parse::<u8>().ok()
}

pub fn f64_to_u8(value: Option<f64>) -> u8 {
    value
        .unwrap_or_default()
        .round()
        .clamp(0.0, f64::from(u8::MAX)) as u8
}

pub fn f64_to_u16(value: Option<f64>) -> u16 {
    value
        .unwrap_or_default()
        .round()
        .clamp(0.0, f64::from(u16::MAX)) as u16
}

pub fn f64_to_i16(value: Option<f64>) -> i16 {
    value
        .unwrap_or_default()
        .round()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

pub fn minute_from_clock(text: Option<&str>) -> Option<u16> {
    let text = text?.trim();
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u16>().ok()
}

pub fn day_bounds(day: Date) -> (OffsetDateTime, OffsetDateTime) {
    let start = day.with_time(Time::MIDNIGHT).assume_utc();
    let end = (day
        .next_day()
        .unwrap_or_else(|| Date::from_calendar_date(9999, Month::December, 31).unwrap_or(day)))
    .with_time(Time::MIDNIGHT)
    .assume_utc();
    (start, end)
}

pub fn api_status(short: &str, elapsed: Option<u16>, detail: Option<String>) -> MatchStatus {
    match short {
        "NS" | "TBD" => MatchStatus::Scheduled,
        "1H" | "2H" | "ET" | "BT" | "P" | "LIVE" => MatchStatus::Live {
            minute: elapsed,
            detail,
        },
        "HT" => MatchStatus::HalfTime,
        "FT" | "AET" => MatchStatus::FullTime,
        "PEN" => MatchStatus::Penalties,
        "PST" => MatchStatus::Postponed,
        "CANC" | "ABD" => MatchStatus::Canceled,
        _ => MatchStatus::Unknown,
    }
}
