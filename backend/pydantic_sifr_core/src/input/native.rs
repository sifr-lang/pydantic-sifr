use core::fmt;

use crate::{Arena, ArenaError};

use super::{InputArena, InputId, InputValue, JsonLimits};

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
    Fraction {
        numerator: String,
        denominator: String,
    },
    Sequence(Vec<Self>),
    Object(Vec<(String, Self)>),
    Mapping(Vec<(Self, Self)>),
}

pub fn build_native_input(
    value: &NativeValue,
    limits: JsonLimits,
) -> Result<InputArena, NativeInputError> {
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
            NativeValue::Decimal(value) => InputValue::Decimal(value.clone()),
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
            NativeValue::Fraction {
                numerator,
                denominator,
            } => InputValue::Fraction {
                numerator: numerator.clone(),
                denominator: denominator.clone(),
            },
            NativeValue::Sequence(values) => {
                self.check_collection(values.len())?;
                let mut children = Vec::with_capacity(values.len());
                for child in values {
                    children.push(self.push(child, depth + 1)?);
                }
                InputValue::Array(children)
            }
            NativeValue::Object(values) => {
                self.check_collection(values.len())?;
                let mut children = Vec::with_capacity(values.len());
                for (key, child) in values {
                    self.add_string_bytes(key.len())?;
                    children.push((key.clone(), self.push(child, depth + 1)?));
                }
                InputValue::Object(children)
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
