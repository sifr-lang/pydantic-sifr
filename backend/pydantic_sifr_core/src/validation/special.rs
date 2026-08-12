use regex::RegexBuilder;
use speedate::{Date, DateTime, DateTimeConfig, Duration, Time, TimestampUnit};
use url::Url;
use uuid::Uuid;

use crate::InputValue;

use super::{
    ClockSnapshot, DateTimeValue, DateValue, DurationValue, ErrorDetail, InputProfile,
    PatternSchema, PatternValue, RelativeTimeConstraint, Schema, TemporalKind, TemporalSchema,
    TimeValue, UrlConstraints, ValidatedValue, ValidationError,
};

const FLAG_CASE_INSENSITIVE: u8 = 1;
const FLAG_MULTI_LINE: u8 = 2;
const FLAG_DOT_MATCHES_NEW_LINE: u8 = 4;

pub(crate) fn validate_special(
    schema: &Schema,
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    clock: ClockSnapshot,
) -> Option<Result<ValidatedValue, ValidationError>> {
    let result = match schema {
        Schema::Temporal(schema) => validate_temporal(schema, input, strict, profile, clock),
        Schema::Uuid { version } => validate_uuid(input, strict, profile, *version),
        Schema::Url(constraints) => validate_url(input, strict, profile, constraints),
        Schema::Pattern(schema) => validate_pattern(input, strict, profile, schema),
        _ => return None,
    };
    Some(result)
}

fn validate_temporal(
    schema: &TemporalSchema,
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    clock: ClockSnapshot,
) -> Result<ValidatedValue, ValidationError> {
    let (source, native_kind) = match input {
        InputValue::Date(value) => (value.as_str(), Some(TemporalKind::Date)),
        InputValue::Time(value) => (value.as_str(), Some(TemporalKind::Time)),
        InputValue::DateTime(value) => (value.as_str(), Some(TemporalKind::DateTime)),
        InputValue::Duration(value) => (value.as_str(), Some(TemporalKind::Duration)),
        InputValue::String(value) => (value.as_str(), None),
        _ => {
            return Err(type_error(
                "temporal_type",
                "Input must be a temporal value",
                temporal_name(schema.kind),
            ));
        }
    };
    if strict && profile == InputProfile::Native && native_kind != Some(schema.kind) {
        return Err(type_error(
            "temporal_type",
            "Native temporal kind does not match the schema",
            temporal_name(schema.kind),
        ));
    }
    let value = match schema.kind {
        TemporalKind::Date => {
            let value =
                Date::parse_str(source).map_err(|error| temporal_parse_error(error, "date"))?;
            validate_date_relative(value, schema.relative, clock)?;
            ValidatedValue::Date(DateValue {
                year: value.year,
                month: value.month,
                day: value.day,
            })
        }
        TemporalKind::Time => {
            if schema.relative.is_some() {
                return Err(type_error(
                    "schema_invalid",
                    "Relative constraints do not apply to time values",
                    "time without relative constraint",
                ));
            }
            let value =
                Time::parse_str(source).map_err(|error| temporal_parse_error(error, "time"))?;
            ValidatedValue::Time(time_value(value))
        }
        TemporalKind::DateTime => {
            let value = DateTime::parse_str(source)
                .map_err(|error| temporal_parse_error(error, "datetime"))?;
            validate_datetime_relative(value, schema.relative, clock)?;
            ValidatedValue::DateTime(datetime_value(value))
        }
        TemporalKind::Duration => {
            if schema.relative.is_some() {
                return Err(type_error(
                    "schema_invalid",
                    "Relative constraints do not apply to durations",
                    "duration without relative constraint",
                ));
            }
            let value = Duration::parse_str(source)
                .map_err(|error| temporal_parse_error(error, "duration"))?;
            ValidatedValue::Duration(DurationValue {
                positive: value.positive,
                days: value.day,
                seconds: value.second,
                microseconds: value.microsecond,
            })
        }
    };
    Ok(value)
}

fn validate_date_relative(
    value: Date,
    relative: Option<RelativeTimeConstraint>,
    clock: ClockSnapshot,
) -> Result<(), ValidationError> {
    let config = DateTimeConfig::builder()
        .timestamp_unit(TimestampUnit::Second)
        .build();
    let current =
        DateTime::from_timestamp_with_config(clock.unix_seconds, clock.microsecond, &config)
            .map_err(|error| temporal_parse_error(error, "clock"))?
            .date;
    let value_tuple = (value.year, value.month, value.day);
    let current_tuple = (current.year, current.month, current.day);
    validate_relative(value_tuple.cmp(&current_tuple), relative, "date")
}

fn validate_datetime_relative(
    value: DateTime,
    relative: Option<RelativeTimeConstraint>,
    clock: ClockSnapshot,
) -> Result<(), ValidationError> {
    let value_micros =
        i128::from(value.timestamp_tz()) * 1_000_000 + i128::from(value.time.microsecond);
    let clock_micros = i128::from(clock.unix_seconds) * 1_000_000 + i128::from(clock.microsecond);
    validate_relative(value_micros.cmp(&clock_micros), relative, "datetime")
}

fn validate_relative(
    ordering: std::cmp::Ordering,
    relative: Option<RelativeTimeConstraint>,
    kind: &'static str,
) -> Result<(), ValidationError> {
    match relative {
        Some(RelativeTimeConstraint::Past) if !ordering.is_lt() => Err(ValidationError::one(
            ErrorDetail::new(relative_code(kind, "past"), "Input must be in the past")
                .expected(format!("past {kind}")),
        )),
        Some(RelativeTimeConstraint::Future) if !ordering.is_gt() => Err(ValidationError::one(
            ErrorDetail::new(relative_code(kind, "future"), "Input must be in the future")
                .expected(format!("future {kind}")),
        )),
        _ => Ok(()),
    }
}

fn relative_code(kind: &'static str, direction: &'static str) -> &'static str {
    match (kind, direction) {
        ("date", "past") => "date_past",
        ("date", "future") => "date_future",
        (_, "past") => "datetime_past",
        _ => "datetime_future",
    }
}

fn validate_uuid(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    required_version: Option<u8>,
) -> Result<ValidatedValue, ValidationError> {
    let source = match input {
        InputValue::Uuid(value) => value.as_str(),
        InputValue::String(value) if !strict || profile != InputProfile::Native => value.as_str(),
        _ => return Err(type_error("uuid_type", "Input must be a UUID", "uuid")),
    };
    let value = Uuid::parse_str(source).map_err(|error| {
        ValidationError::one(
            ErrorDetail::new("uuid_parsing", "Input must be a valid UUID")
                .expected("uuid")
                .context("error", error.to_string()),
        )
    })?;
    if let Some(required) = required_version {
        let actual = value.get_version_num();
        if actual != usize::from(required) {
            return Err(ValidationError::one(
                ErrorDetail::new("uuid_version", "UUID version does not match")
                    .expected(format!("uuid version {required}"))
                    .context("expected_version", required.to_string())
                    .context("actual_version", actual.to_string()),
            ));
        }
    }
    Ok(ValidatedValue::Uuid(*value.as_bytes()))
}

fn validate_url(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    constraints: &UrlConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let source = match input {
        InputValue::Url(value) => value.as_str(),
        InputValue::String(value) if !strict || profile != InputProfile::Native => value.as_str(),
        _ => return Err(type_error("url_type", "Input must be a URL", "url")),
    };
    if constraints
        .max_length
        .is_some_and(|maximum| source.chars().count() > maximum)
    {
        return Err(ValidationError::one(
            ErrorDetail::new("url_too_long", "URL is too long")
                .expected("URL within the configured length")
                .context(
                    "max_length",
                    constraints.max_length.unwrap_or_default().to_string(),
                ),
        ));
    }
    let value = Url::parse(source).map_err(|error| {
        ValidationError::one(
            ErrorDetail::new("url_parsing", "Input must be a valid absolute URL")
                .expected("absolute url")
                .context("error", error.to_string()),
        )
    })?;
    if value.cannot_be_a_base() && value.scheme() != "mailto" {
        return Err(type_error(
            "url_scheme",
            "URL scheme does not support hierarchical URLs",
            "hierarchical url",
        ));
    }
    if !constraints.allowed_schemes.is_empty()
        && !constraints
            .allowed_schemes
            .iter()
            .any(|scheme| scheme == value.scheme())
    {
        return Err(ValidationError::one(
            ErrorDetail::new("url_scheme", "URL scheme is not allowed")
                .expected("configured URL scheme")
                .context("scheme", value.scheme()),
        ));
    }
    Ok(ValidatedValue::Url(value.to_string()))
}

fn validate_pattern(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    schema: &PatternSchema,
) -> Result<ValidatedValue, ValidationError> {
    let (source, input_flags) = match input {
        InputValue::Pattern { source, flags } => (source.as_str(), *flags),
        InputValue::String(value) if !strict || profile != InputProfile::Native => {
            (value.as_str(), 0)
        }
        _ => {
            return Err(type_error(
                "pattern_type",
                "Input must be a regular expression pattern",
                "pattern",
            ));
        }
    };
    let schema_flags = (u8::from(schema.case_insensitive) * FLAG_CASE_INSENSITIVE)
        | (u8::from(schema.multi_line) * FLAG_MULTI_LINE)
        | (u8::from(schema.dot_matches_new_line) * FLAG_DOT_MATCHES_NEW_LINE);
    let flags = input_flags | schema_flags;
    if flags & !(FLAG_CASE_INSENSITIVE | FLAG_MULTI_LINE | FLAG_DOT_MATCHES_NEW_LINE) != 0 {
        return Err(type_error(
            "pattern_flags",
            "Pattern contains unsupported flags",
            "supported pattern flags",
        ));
    }
    let compiled = RegexBuilder::new(source)
        .case_insensitive(flags & FLAG_CASE_INSENSITIVE != 0)
        .multi_line(flags & FLAG_MULTI_LINE != 0)
        .dot_matches_new_line(flags & FLAG_DOT_MATCHES_NEW_LINE != 0)
        .size_limit(1 << 20)
        .dfa_size_limit(2 << 20)
        .build()
        .map_err(|error| {
            ValidationError::one(
                ErrorDetail::new(
                    "pattern_parsing",
                    "Input must be a valid regular expression",
                )
                .expected("Rust regex syntax")
                .context("error", error.to_string()),
            )
        })?;
    Ok(ValidatedValue::Pattern(PatternValue::new(
        source.to_owned(),
        flags,
        compiled,
    )))
}

fn datetime_value(value: DateTime) -> DateTimeValue {
    DateTimeValue {
        date: DateValue {
            year: value.date.year,
            month: value.date.month,
            day: value.date.day,
        },
        time: time_value(value.time),
    }
}

fn time_value(value: Time) -> TimeValue {
    TimeValue {
        hour: value.hour,
        minute: value.minute,
        second: value.second,
        microsecond: value.microsecond,
        offset_seconds: value.tz_offset,
    }
}

fn temporal_parse_error(error: speedate::ParseError, expected: &'static str) -> ValidationError {
    ValidationError::one(
        ErrorDetail::new("temporal_parsing", "Input must be a valid temporal value")
            .expected(expected)
            .context("error", error.to_string()),
    )
}

const fn temporal_name(kind: TemporalKind) -> &'static str {
    match kind {
        TemporalKind::Date => "date",
        TemporalKind::Time => "time",
        TemporalKind::DateTime => "datetime",
        TemporalKind::Duration => "duration",
    }
}

fn type_error(
    code: &'static str,
    message: &'static str,
    expected: &'static str,
) -> ValidationError {
    super::scalars::type_error(code, message, expected)
}
