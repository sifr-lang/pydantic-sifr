use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_complex::Complex64;
use num_rational::BigRational;
use num_traits::ToPrimitive;
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    NodeId, ShapeIdentity, StructuralContractError, StructuralEdgeKind, StructuralKind,
    StructuralNodeEdge, StructuralNodeRef, StructuralScalar, StructuralSource, primitive,
};

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
    name: &'static str,
    fields: Vec<(&'static str, ValueId)>,
    extras: Vec<(String, ValueId)>,
    validated_field_count: usize,
}

impl ModelValue {
    pub(crate) const fn new(
        name: &'static str,
        fields: Vec<(&'static str, ValueId)>,
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
        self.name
    }

    #[must_use]
    pub fn fields(&self) -> &[(&'static str, ValueId)] {
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
    Tuple(Vec<ValueId>),
    Mapping(Vec<(ValueId, ValueId)>),
    Set(Vec<ValueId>),
    FrozenSet(Vec<ValueId>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedArena {
    root: ValueId,
    values: Arena<ValidatedValue>,
    shape: ShapeIdentity,
    edges: Vec<Vec<StructuralNodeEdge<'static>>>,
    moved: Vec<bool>,
}

impl ValidatedArena {
    pub(crate) fn new(root: ValueId, values: Arena<ValidatedValue>) -> Self {
        let edges = build_structural_edges(&values);
        let moved = vec![false; values.len()];
        Self {
            root,
            values,
            shape: primitive("pydantic_sifr.untyped"),
            edges,
            moved,
        }
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

    pub(crate) fn set_shape(&mut self, shape: ShapeIdentity) {
        self.shape = shape;
    }
}

impl ValidatedValue {
    pub(crate) fn remap_ids(&mut self, offset: usize) -> Result<(), ArenaError> {
        match self {
            Self::Sequence(ids) | Self::Tuple(ids) | Self::Set(ids) | Self::FrozenSet(ids) => {
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

impl StructuralSource for ValidatedArena {
    fn shape_identity(&self) -> ShapeIdentity {
        self.shape
    }

    fn root(&self) -> NodeId {
        node_id(self.root)
    }

    fn node(&self, id: NodeId) -> Result<StructuralNodeRef<'_>, StructuralContractError> {
        let index = usize::try_from(id.get()).map_err(|_| StructuralContractError::InvalidNode)?;
        let value = self
            .values
            .values()
            .get(index)
            .ok_or(StructuralContractError::InvalidNode)?;
        let edges = self
            .edges
            .get(index)
            .ok_or(StructuralContractError::InvalidNode)?;
        let (kind, nominal) = structural_description(value)?;
        if edges.is_empty() && is_scalar_kind(kind) {
            Ok(StructuralNodeRef::scalar(kind))
        } else {
            Ok(StructuralNodeRef::aggregate(kind, nominal, edges))
        }
    }

    fn take_scalar(&mut self, id: NodeId) -> Result<StructuralScalar, StructuralContractError> {
        let value_id = ArenaId::from_usize(id.get() as usize)
            .map_err(|_| StructuralContractError::InvalidNode)?;
        let moved = self
            .moved
            .get_mut(id.get() as usize)
            .ok_or(StructuralContractError::InvalidNode)?;
        if *moved {
            return Err(StructuralContractError::AlreadyMoved);
        }
        let value = self
            .values
            .get_mut(value_id)
            .ok_or(StructuralContractError::InvalidNode)?;
        let scalar = take_structural_scalar(value)?;
        *moved = true;
        Ok(scalar)
    }
}

fn build_structural_edges(values: &Arena<ValidatedValue>) -> Vec<Vec<StructuralNodeEdge<'static>>> {
    values
        .values()
        .iter()
        .map(|value| match value {
            ValidatedValue::Sequence(items)
            | ValidatedValue::Tuple(items)
            | ValidatedValue::Set(items)
            | ValidatedValue::FrozenSet(items) => items
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    StructuralNodeEdge::new(StructuralEdgeKind::Index(index), node_id(*id))
                })
                .collect(),
            ValidatedValue::Mapping(entries) => entries
                .iter()
                .enumerate()
                .flat_map(|(index, (key, value))| {
                    [
                        StructuralNodeEdge::new(
                            StructuralEdgeKind::MappingKey(index),
                            node_id(*key),
                        ),
                        StructuralNodeEdge::new(
                            StructuralEdgeKind::MappingValue(index),
                            node_id(*value),
                        ),
                    ]
                })
                .collect(),
            ValidatedValue::Nullable(Some(id)) => vec![StructuralNodeEdge::new(
                StructuralEdgeKind::ActiveMember {
                    name: "some",
                    index: 0,
                },
                node_id(*id),
            )],
            ValidatedValue::Model(model) => model
                .fields
                .iter()
                .map(|(name, id)| {
                    StructuralNodeEdge::new(StructuralEdgeKind::RecordField(name), node_id(*id))
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn structural_description(
    value: &ValidatedValue,
) -> Result<(StructuralKind, Option<&'static str>), StructuralContractError> {
    let description = match value {
        ValidatedValue::None => (StructuralKind::None, None),
        ValidatedValue::Bool(_) => (StructuralKind::Bool, None),
        ValidatedValue::ExactInt(_) => (StructuralKind::ExactInteger, None),
        ValidatedValue::FixedInt { kind, .. } if kind.starts_with('u') => {
            (StructuralKind::UnsignedInteger, None)
        }
        ValidatedValue::FixedInt { .. } => (StructuralKind::SignedInteger, None),
        ValidatedValue::Float(_) => (StructuralKind::Float, None),
        ValidatedValue::String(_) => (StructuralKind::String, None),
        ValidatedValue::Bytes(_) => (StructuralKind::Bytes, None),
        ValidatedValue::Sequence(_) => (StructuralKind::Sequence, None),
        ValidatedValue::Tuple(_) => (StructuralKind::Tuple, None),
        ValidatedValue::Mapping(_) => (StructuralKind::Mapping, None),
        ValidatedValue::Set(_) => (StructuralKind::Set, None),
        ValidatedValue::FrozenSet(_) => (StructuralKind::FrozenSet, None),
        ValidatedValue::Nullable(_) => (StructuralKind::Optional, None),
        ValidatedValue::Model(model) => (StructuralKind::Record, Some(model.name)),
        _ => return Err(StructuralContractError::KindMismatch),
    };
    Ok(description)
}

const fn is_scalar_kind(kind: StructuralKind) -> bool {
    matches!(
        kind,
        StructuralKind::None
            | StructuralKind::Bool
            | StructuralKind::SignedInteger
            | StructuralKind::UnsignedInteger
            | StructuralKind::ExactInteger
            | StructuralKind::Float
            | StructuralKind::String
            | StructuralKind::Bytes
    )
}

fn take_structural_scalar(
    value: &mut ValidatedValue,
) -> Result<StructuralScalar, StructuralContractError> {
    match std::mem::replace(value, ValidatedValue::None) {
        ValidatedValue::None => Ok(StructuralScalar::None),
        ValidatedValue::Bool(value) => Ok(StructuralScalar::Bool(value)),
        ValidatedValue::ExactInt(value) => {
            Ok(StructuralScalar::ExactInteger(SifrInt::from_bigint(value)))
        }
        ValidatedValue::FixedInt { kind, value } => fixed_integer_scalar(kind, &value),
        ValidatedValue::Float(value) => Ok(StructuralScalar::Float(value)),
        ValidatedValue::String(value) => Ok(StructuralScalar::String(value)),
        ValidatedValue::Bytes(value) => Ok(StructuralScalar::Bytes(value)),
        other => {
            *value = other;
            Err(StructuralContractError::ScalarMismatch)
        }
    }
}

fn fixed_integer_scalar(
    kind: &'static str,
    value: &BigInt,
) -> Result<StructuralScalar, StructuralContractError> {
    let width = kind
        .trim_start_matches(['i', 'u'])
        .parse::<u16>()
        .map_err(|_| StructuralContractError::ScalarMismatch)?;
    if kind.starts_with('u') {
        value
            .to_u128()
            .map(|value| StructuralScalar::UnsignedInteger { value, width })
            .ok_or(StructuralContractError::ScalarMismatch)
    } else {
        value
            .to_i128()
            .map(|value| StructuralScalar::SignedInteger { value, width })
            .ok_or(StructuralContractError::ScalarMismatch)
    }
}

const fn node_id(id: ValueId) -> NodeId {
    NodeId::new(id.raw())
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
