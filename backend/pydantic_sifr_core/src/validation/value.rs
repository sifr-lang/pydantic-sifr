use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_complex::Complex64;
use num_rational::BigRational;

use crate::{Arena, ArenaError, ArenaId};

pub type ValueId = ArenaId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DateValue {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeValue {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub microsecond: u32,
    pub offset_seconds: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DateTimeValue {
    pub date: DateValue,
    pub time: TimeValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurationValue {
    pub positive: bool,
    pub days: u32,
    pub seconds: u32,
    pub microseconds: u32,
}

#[derive(Clone, Debug)]
pub struct PatternValue {
    source: String,
    flags: u8,
    compiled: regex::Regex,
}

impl PatternValue {
    pub(crate) const fn new(source: String, flags: u8, compiled: regex::Regex) -> Self {
        Self {
            source,
            flags,
            compiled,
        }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    #[must_use]
    pub fn is_match(&self, value: &str) -> bool {
        self.compiled.is_match(value)
    }
}

impl PartialEq for PatternValue {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.flags == other.flags
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelValue {
    name: String,
    fields: Vec<(String, ValueId)>,
    extras: Vec<(String, ValueId)>,
    validated_field_count: usize,
}

impl ModelValue {
    pub(crate) const fn new(
        name: String,
        fields: Vec<(String, ValueId)>,
        extras: Vec<(String, ValueId)>,
        validated_field_count: usize,
    ) -> Self {
        Self {
            name,
            fields,
            extras,
            validated_field_count,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn fields(&self) -> &[(String, ValueId)] {
        &self.fields
    }

    #[must_use]
    pub fn extras(&self) -> &[(String, ValueId)] {
        &self.extras
    }

    #[must_use]
    pub const fn validated_field_count(&self) -> usize {
        self.validated_field_count
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ValidatedValue {
    None,
    Bool(bool),
    ExactInt(BigInt),
    FixedInt { kind: &'static str, value: BigInt },
    Float(f64),
    Decimal(BigDecimal),
    Fraction(BigRational),
    Complex(Complex64),
    String(String),
    Bytes(Vec<u8>),
    Date(DateValue),
    Time(TimeValue),
    DateTime(DateTimeValue),
    Duration(DurationValue),
    Uuid([u8; 16]),
    Url(String),
    Pattern(PatternValue),
    Nullable(Option<ValueId>),
    Model(ModelValue),
    Sequence(Vec<ValueId>),
    Mapping(Vec<(ValueId, ValueId)>),
    Set(Vec<ValueId>),
    FrozenSet(Vec<ValueId>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedArena {
    root: ValueId,
    values: Arena<ValidatedValue>,
}

impl ValidatedArena {
    pub(crate) const fn new(root: ValueId, values: Arena<ValidatedValue>) -> Self {
        Self { root, values }
    }

    #[must_use]
    pub const fn root(&self) -> ValueId {
        self.root
    }

    #[must_use]
    pub fn get(&self, id: ValueId) -> Option<&ValidatedValue> {
        self.values.get(id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn into_parts(self) -> (ValueId, Vec<ValidatedValue>) {
        (self.root, self.values.into_values())
    }
}

impl ValidatedValue {
    pub(crate) fn remap_ids(&mut self, offset: usize) -> Result<(), ArenaError> {
        match self {
            Self::Sequence(ids) | Self::Set(ids) | Self::FrozenSet(ids) => {
                for id in ids {
                    *id = remap_id(*id, offset)?;
                }
            }
            Self::Mapping(entries) => {
                for (key, value) in entries {
                    *key = remap_id(*key, offset)?;
                    *value = remap_id(*value, offset)?;
                }
            }
            Self::Nullable(Some(id)) => *id = remap_id(*id, offset)?,
            Self::Model(model) => {
                for (_, id) in &mut model.fields {
                    *id = remap_id(*id, offset)?;
                }
                for (_, id) in &mut model.extras {
                    *id = remap_id(*id, offset)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn remap_id(id: ValueId, offset: usize) -> Result<ValueId, ArenaError> {
    let raw = usize::try_from(id.raw()).map_err(|_| ArenaError::CapacityExceeded)?;
    let remapped = raw
        .checked_add(offset)
        .ok_or(ArenaError::CapacityExceeded)?;
    ArenaId::from_usize(remapped)
}

pub(crate) fn push_value(
    arena: &mut Arena<ValidatedValue>,
    value: ValidatedValue,
) -> Result<ValueId, ArenaError> {
    arena.push(value)
}
