use core::fmt;

use crate::{Arena, ArenaError};

use super::{InputArena, InputId, InputValue, JsonLimits, ObjectKind, SequenceKind};

const HARD_MAX_DEPTH: usize = 256;

/// Crate-neutral structural input used by generated Sifr projections.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeValue {
    Null,
    Bool(bool),
    Integer(String),
    Float(f64),
    Decimal(String),
    Complex {
        real: f64,
        imaginary: f64,
    },
    String(String),
    Bytes(Vec<u8>),
    Date(String),
    Time(String),
    DateTime(String),
    Duration(String),
    Uuid(String),
    Url(String),
    Pattern {
        source: String,
        flags: u8,
    },
    Fraction {
        numerator: String,
        denominator: String,
    },
    List(Vec<Self>),
    Tuple(Vec<Self>),
    Set(Vec<Self>),
    FrozenSet(Vec<Self>),
    Object(Vec<(String, Self)>),
    Mapping(Vec<(Self, Self)>),
}

pub fn build_native_input(
    value: &NativeValue,
    limits: JsonLimits,
) -> Result<InputArena, NativeInputError> {
    if limits.max_depth == 0 || limits.max_depth > HARD_MAX_DEPTH {
        return Err(NativeInputError::Limit("valid maximum depth"));
    }
    let mut builder = NativeBuilder {
        values: Arena::new(),
        limits,
        string_bytes: 0,
    };
    let root = builder.push(value, 0)?;
    Ok(InputArena::from_parts(root, builder.values))
}

struct NativeBuilder {
    values: Arena<InputValue>,
    limits: JsonLimits,
    string_bytes: usize,
}

impl NativeBuilder {
    fn push(&mut self, value: &NativeValue, depth: usize) -> Result<InputId, NativeInputError> {
        if depth > self.limits.max_depth {
            return Err(NativeInputError::Limit("maximum depth"));
        }
        if self.values.len() >= self.limits.max_nodes {
            return Err(NativeInputError::Limit("maximum node count"));
        }
        let value = match value {
            NativeValue::Null => InputValue::Null,
            NativeValue::Bool(value) => InputValue::Bool(*value),
            NativeValue::Integer(value) => {
                if value.trim_start_matches('-').len() > self.limits.max_integer_digits {
                    return Err(NativeInputError::Limit("maximum integer digits"));
                }
                InputValue::Integer(value.clone())
            }
            NativeValue::Float(value) => InputValue::Float(*value),
            NativeValue::Decimal(value) => {
                self.check_decimal_digits(value)?;
                InputValue::Decimal(value.clone())
            }
            NativeValue::Complex { real, imaginary } => InputValue::Complex {
                real: *real,
                imaginary: *imaginary,
            },
            NativeValue::String(value) => {
                self.add_string_bytes(value.len())?;
                InputValue::String(value.clone())
            }
            NativeValue::Bytes(value) => {
                self.add_string_bytes(value.len())?;
                InputValue::Bytes(value.clone())
            }
            NativeValue::Date(value) => self.push_special_text(value, InputValue::Date)?,
            NativeValue::Time(value) => self.push_special_text(value, InputValue::Time)?,
            NativeValue::DateTime(value) => self.push_special_text(value, InputValue::DateTime)?,
            NativeValue::Duration(value) => self.push_special_text(value, InputValue::Duration)?,
            NativeValue::Uuid(value) => self.push_special_text(value, InputValue::Uuid)?,
            NativeValue::Url(value) => self.push_special_text(value, InputValue::Url)?,
            NativeValue::Pattern { source, flags } => {
                self.add_string_bytes(source.len())?;
                InputValue::Pattern {
                    source: source.clone(),
                    flags: *flags,
                }
            }
            NativeValue::Fraction {
                numerator,
                denominator,
            } => {
                self.check_integer_digits(numerator)?;
                self.check_integer_digits(denominator)?;
                InputValue::Fraction {
                    numerator: numerator.clone(),
                    denominator: denominator.clone(),
                }
            }
            NativeValue::List(values) => self.push_sequence(values, SequenceKind::List, depth)?,
            NativeValue::Tuple(values) => self.push_sequence(values, SequenceKind::Tuple, depth)?,
            NativeValue::Set(values) => self.push_sequence(values, SequenceKind::Set, depth)?,
            NativeValue::FrozenSet(values) => {
                self.push_sequence(values, SequenceKind::FrozenSet, depth)?
            }
            NativeValue::Object(values) => {
                self.check_collection(values.len())?;
                let mut children = Vec::with_capacity(values.len());
                for (key, child) in values {
                    self.add_string_bytes(key.len())?;
                    children.push((key.clone(), self.push(child, depth + 1)?));
                }
                InputValue::Object {
                    kind: ObjectKind::Object,
                    entries: children,
                }
            }
            NativeValue::Mapping(values) => {
                self.check_collection(values.len())?;
                let mut children = Vec::with_capacity(values.len());
                for (key, value) in values {
                    let key = self.push(key, depth + 1)?;
                    let value = self.push(value, depth + 1)?;
                    children.push((key, value));
                }
                InputValue::Mapping(children)
            }
        };
        self.values.push(value).map_err(NativeInputError::Arena)
    }

    fn check_collection(&self, length: usize) -> Result<(), NativeInputError> {
        if length > self.limits.max_collection_items {
            Err(NativeInputError::Limit("maximum collection items"))
        } else {
            Ok(())
        }
    }

    fn push_special_text(
        &mut self,
        value: &str,
        constructor: fn(String) -> InputValue,
    ) -> Result<InputValue, NativeInputError> {
        self.add_string_bytes(value.len())?;
        Ok(constructor(value.to_owned()))
    }

    fn push_sequence(
        &mut self,
        values: &[NativeValue],
        kind: SequenceKind,
        depth: usize,
    ) -> Result<InputValue, NativeInputError> {
        self.check_collection(values.len())?;
        let mut children = Vec::with_capacity(values.len());
        for child in values {
            children.push(self.push(child, depth + 1)?);
        }
        Ok(InputValue::Sequence {
            kind,
            items: children,
        })
    }

    fn check_integer_digits(&self, value: &str) -> Result<(), NativeInputError> {
        if value.trim_start_matches(['-', '+']).len() > self.limits.max_integer_digits {
            Err(NativeInputError::Limit("maximum integer digits"))
        } else {
            Ok(())
        }
    }

    fn check_decimal_digits(&self, value: &str) -> Result<(), NativeInputError> {
        let mantissa = value.split_once(['e', 'E']).map_or(value, |parts| parts.0);
        let digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
        if digits > self.limits.max_integer_digits {
            Err(NativeInputError::Limit("maximum numeric digits"))
        } else {
            Ok(())
        }
    }

    fn add_string_bytes(&mut self, amount: usize) -> Result<(), NativeInputError> {
        self.string_bytes = self
            .string_bytes
            .checked_add(amount)
            .ok_or(NativeInputError::Limit("total string bytes"))?;
        if self.string_bytes > self.limits.max_string_bytes {
            return Err(NativeInputError::Limit("total string bytes"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeInputError {
    Limit(&'static str),
    Arena(ArenaError),
}

impl fmt::Display for NativeInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(kind) => write!(f, "native input limit exceeded: {kind}"),
            Self::Arena(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for NativeInputError {}
