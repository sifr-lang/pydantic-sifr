use num_bigint::BigInt;
use num_traits::ToPrimitive;
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    NodeId, ShapeIdentity, StructuralContractError, StructuralEdgeKind, StructuralKind,
    StructuralNodeEdge, StructuralNodeRef, StructuralScalar, StructuralSource,
};

use crate::{Arena, ArenaId};

use super::{ModelValue, ValidatedArena, ValidatedValue, ValueId};

const SIFR_FROZENSET_NOMINAL_IDENTITY: &str = "sifr.collections.frozenset";
const PYDANTIC_URL_IDENTITY: &str = "pydantic_sifr.special_values.Url";
const PYDANTIC_MULTI_HOST_URL_IDENTITY: &str = "pydantic_sifr.special_values.MultiHostUrl";
const PYDANTIC_PATTERN_IDENTITY: &str = "pydantic_sifr.special_values.Pattern";

impl ValidatedArena {
    pub(crate) fn prepare_structural(
        &mut self,
        shape: ShapeIdentity,
    ) -> Result<(), StructuralContractError> {
        if self.edges.is_some() {
            return Err(StructuralContractError::AlreadyMoved);
        }
        expand_specialized_values(&mut self.values)?;
        self.shape = shape;
        self.descriptions = Some(build_structural_descriptions(&self.values)?);
        self.edges = Some(build_structural_edges(&self.values));
        self.moved = vec![false; self.values.len()];
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
        let (kind, nominal) = self
            .descriptions
            .as_ref()
            .and_then(|items| items.get(index))
            .copied()
            .ok_or(StructuralContractError::InvalidNode)?;
        let edges = self
            .edges
            .as_ref()
            .and_then(|items| items.get(index))
            .ok_or(StructuralContractError::InvalidNode)?;
        if edges.is_empty() && is_scalar_kind(kind) {
            Ok(StructuralNodeRef::scalar(kind))
        } else {
            Ok(StructuralNodeRef::aggregate(kind, nominal, edges))
        }
    }

    fn take_scalar(&mut self, id: NodeId) -> Result<StructuralScalar, StructuralContractError> {
        let index = usize::try_from(id.get()).map_err(|_| StructuralContractError::InvalidNode)?;
        let value_id =
            ArenaId::from_usize(index).map_err(|_| StructuralContractError::InvalidNode)?;
        let moved = self
            .moved
            .get_mut(index)
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

fn expand_specialized_values(
    values: &mut Arena<ValidatedValue>,
) -> Result<(), StructuralContractError> {
    let original_len = values.len();
    for index in 0..original_len {
        let id = ArenaId::from_usize(index).map_err(|_| StructuralContractError::InvalidNode)?;
        let Some(slot) = values.get_mut(id) else {
            return Err(StructuralContractError::InvalidNode);
        };
        let value = std::mem::replace(slot, ValidatedValue::None);
        let replacement = match value {
            ValidatedValue::Url(value) => {
                specialized_text_model(values, PYDANTIC_URL_IDENTITY, value)?
            }
            ValidatedValue::MultiHostUrl(value) => {
                specialized_text_model(values, PYDANTIC_MULTI_HOST_URL_IDENTITY, value)?
            }
            ValidatedValue::Fraction(value) => {
                let (numerator, denominator) = value.into_raw();
                let numerator = push(values, ValidatedValue::ExactInt(numerator))?;
                let denominator = push(values, ValidatedValue::ExactInt(denominator))?;
                ValidatedValue::Tuple(vec![numerator, denominator])
            }
            ValidatedValue::Complex(value) => {
                let real = push(values, ValidatedValue::Float(value.re))?;
                let imaginary = push(values, ValidatedValue::Float(value.im))?;
                ValidatedValue::Tuple(vec![real, imaginary])
            }
            ValidatedValue::Date(value) => ValidatedValue::Tuple(vec![
                signed(values, i128::from(value.year), 16)?,
                unsigned(values, u128::from(value.month), 8)?,
                unsigned(values, u128::from(value.day), 8)?,
            ]),
            ValidatedValue::Time(value) => time_tuple(values, &value)?,
            ValidatedValue::DateTime(value) => {
                let year = signed(values, i128::from(value.date.year), 16)?;
                let month = unsigned(values, u128::from(value.date.month), 8)?;
                let day = unsigned(values, u128::from(value.date.day), 8)?;
                let date = push(values, ValidatedValue::Tuple(vec![year, month, day]))?;
                let time_value = time_tuple(values, &value.time)?;
                let time = push(values, time_value)?;
                ValidatedValue::Tuple(vec![date, time])
            }
            ValidatedValue::Duration(value) => ValidatedValue::Tuple(vec![
                push(values, ValidatedValue::Bool(value.positive))?,
                unsigned(values, u128::from(value.days), 32)?,
                unsigned(values, u128::from(value.seconds), 32)?,
                unsigned(values, u128::from(value.microseconds), 32)?,
            ]),
            ValidatedValue::Pattern(value) => {
                let (source, flags) = value.into_parts();
                let source = push(values, ValidatedValue::String(source))?;
                let flags = unsigned(values, u128::from(flags), 8)?;
                ValidatedValue::Model(ModelValue::new(
                    PYDANTIC_PATTERN_IDENTITY,
                    vec![("source", source), ("flags", flags)],
                    Vec::new(),
                    2,
                ))
            }
            ValidatedValue::FrozenSet(items) => {
                let values_field = push(values, ValidatedValue::Set(items))?;
                ValidatedValue::Model(ModelValue::new(
                    SIFR_FROZENSET_NOMINAL_IDENTITY,
                    vec![("_values", values_field)],
                    Vec::new(),
                    1,
                ))
            }
            other => other,
        };
        let Some(slot) = values.get_mut(id) else {
            return Err(StructuralContractError::InvalidNode);
        };
        *slot = replacement;
    }
    Ok(())
}

fn specialized_text_model(
    values: &mut Arena<ValidatedValue>,
    identity: &'static str,
    value: String,
) -> Result<ValidatedValue, StructuralContractError> {
    let value = push(values, ValidatedValue::String(value))?;
    Ok(ValidatedValue::Model(ModelValue::new(
        identity,
        vec![("value", value)],
        Vec::new(),
        1,
    )))
}

fn time_tuple(
    values: &mut Arena<ValidatedValue>,
    value: &super::TimeValue,
) -> Result<ValidatedValue, StructuralContractError> {
    let offset = match value.offset_seconds {
        Some(offset) => {
            let child = signed(values, i128::from(offset), 32)?;
            ValidatedValue::Nullable(Some(child))
        }
        None => ValidatedValue::Nullable(None),
    };
    let offset = push(values, offset)?;
    Ok(ValidatedValue::Tuple(vec![
        unsigned(values, u128::from(value.hour), 8)?,
        unsigned(values, u128::from(value.minute), 8)?,
        unsigned(values, u128::from(value.second), 8)?,
        unsigned(values, u128::from(value.microsecond), 32)?,
        offset,
    ]))
}

fn signed(
    values: &mut Arena<ValidatedValue>,
    value: i128,
    width: u16,
) -> Result<ValueId, StructuralContractError> {
    let kind = signed_kind(width)?;
    push(
        values,
        ValidatedValue::FixedInt {
            kind,
            value: BigInt::from(value),
        },
    )
}

fn unsigned(
    values: &mut Arena<ValidatedValue>,
    value: u128,
    width: u16,
) -> Result<ValueId, StructuralContractError> {
    let kind = unsigned_kind(width)?;
    push(
        values,
        ValidatedValue::FixedInt {
            kind,
            value: BigInt::from(value),
        },
    )
}

const fn signed_kind(width: u16) -> Result<&'static str, StructuralContractError> {
    match width {
        8 => Ok("int8"),
        16 => Ok("int16"),
        32 => Ok("int32"),
        64 => Ok("int64"),
        _ => Err(StructuralContractError::ScalarMismatch),
    }
}

const fn unsigned_kind(width: u16) -> Result<&'static str, StructuralContractError> {
    match width {
        8 => Ok("uint8"),
        16 => Ok("uint16"),
        32 => Ok("uint32"),
        64 => Ok("uint64"),
        _ => Err(StructuralContractError::ScalarMismatch),
    }
}

fn push(
    values: &mut Arena<ValidatedValue>,
    value: ValidatedValue,
) -> Result<ValueId, StructuralContractError> {
    values
        .push(value)
        .map_err(|_| StructuralContractError::InvalidNode)
}

fn build_structural_descriptions(
    values: &Arena<ValidatedValue>,
) -> Result<Vec<(StructuralKind, Option<&'static str>)>, StructuralContractError> {
    values.values().iter().map(structural_description).collect()
}

fn structural_description(
    value: &ValidatedValue,
) -> Result<(StructuralKind, Option<&'static str>), StructuralContractError> {
    let description = match value {
        ValidatedValue::None => (StructuralKind::None, None),
        ValidatedValue::Bool(_) => (StructuralKind::Bool, None),
        ValidatedValue::ExactInt(_) => (StructuralKind::ExactInteger, None),
        ValidatedValue::FixedInt { kind, .. } if kind.starts_with("uint") => {
            (StructuralKind::UnsignedInteger, None)
        }
        ValidatedValue::FixedInt { .. } => (StructuralKind::SignedInteger, None),
        ValidatedValue::Float(_) => (StructuralKind::Float, None),
        ValidatedValue::Decimal(_) | ValidatedValue::String(_) => (StructuralKind::String, None),
        ValidatedValue::Bytes(_) | ValidatedValue::Uuid(_) => (StructuralKind::Bytes, None),
        ValidatedValue::Sequence(_) => (StructuralKind::Sequence, None),
        ValidatedValue::Tuple(_) => (StructuralKind::Tuple, None),
        ValidatedValue::Mapping(_) => (StructuralKind::Mapping, None),
        ValidatedValue::Set(_) => (StructuralKind::Set, None),
        ValidatedValue::FrozenSet(_) => (StructuralKind::FrozenSet, None),
        ValidatedValue::Nullable(_) => (StructuralKind::Optional, None),
        ValidatedValue::Enum(value) => (StructuralKind::Enum, Some(value.name)),
        ValidatedValue::Union(_) => (StructuralKind::Union, None),
        ValidatedValue::Model(model) => (StructuralKind::Record, Some(model.name)),
        ValidatedValue::Url(_)
        | ValidatedValue::MultiHostUrl(_)
        | ValidatedValue::Fraction(_)
        | ValidatedValue::Complex(_)
        | ValidatedValue::Date(_)
        | ValidatedValue::Time(_)
        | ValidatedValue::DateTime(_)
        | ValidatedValue::Duration(_)
        | ValidatedValue::Pattern(_) => return Err(StructuralContractError::KindMismatch),
    };
    Ok(description)
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
            ValidatedValue::Enum(value) => vec![StructuralNodeEdge::new(
                StructuralEdgeKind::ActiveMember {
                    name: value.variant,
                    index: value.index,
                },
                node_id(value.discriminant),
            )],
            ValidatedValue::Union(value) => vec![StructuralNodeEdge::new(
                StructuralEdgeKind::ActiveMember {
                    name: "member",
                    index: value.index,
                },
                node_id(value.value),
            )],
            ValidatedValue::Model(model) => model
                .fields()
                .iter()
                .map(|(name, id)| {
                    StructuralNodeEdge::new(StructuralEdgeKind::RecordField(name), node_id(*id))
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
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
        ValidatedValue::Decimal(value) => Ok(StructuralScalar::String(value.to_string())),
        ValidatedValue::String(value) => Ok(StructuralScalar::String(value)),
        ValidatedValue::Bytes(value) => Ok(StructuralScalar::Bytes(value)),
        ValidatedValue::Uuid(value) => Ok(StructuralScalar::Bytes(value.to_vec())),
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
    let (unsigned, width) = match kind {
        "int8" => (false, 8),
        "int16" => (false, 16),
        "int32" => (false, 32),
        "int64" => (false, 64),
        "uint8" => (true, 8),
        "uint16" => (true, 16),
        "uint32" => (true, 32),
        "uint64" => (true, 64),
        _ => return Err(StructuralContractError::ScalarMismatch),
    };
    if unsigned {
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
