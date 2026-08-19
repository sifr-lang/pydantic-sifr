use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_complex::Complex64;
use num_rational::BigRational;
use sifr_runtime::interop::structural::{
    ShapeIdentity, StructuralKind, StructuralNodeEdge, primitive,
};
use std::collections::HashMap;

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

    pub(super) fn into_parts(self) -> (String, u8) {
        (self.source, self.flags)
    }
}

impl PartialEq for PatternValue {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.flags == other.flags
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelValue {
    pub(super) name: &'static str,
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
pub struct EnumValue {
    pub(crate) name: &'static str,
    pub(crate) variant: &'static str,
    pub(crate) index: usize,
    pub(crate) discriminant: ValueId,
}

impl EnumValue {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn variant(&self) -> &'static str {
        self.variant
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnionValue {
    pub(crate) index: usize,
    pub(crate) value: ValueId,
}

impl UnionValue {
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn value(&self) -> ValueId {
        self.value
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
    MultiHostUrl(String),
    Pattern(PatternValue),
    Enum(EnumValue),
    Union(UnionValue),
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
    pub(super) root: ValueId,
    pub(super) values: Arena<ValidatedValue>,
    pub(super) shape: ShapeIdentity,
    pub(super) descriptions: Option<Vec<(StructuralKind, Option<&'static str>)>>,
    pub(super) edges: Option<Vec<Vec<StructuralNodeEdge<'static>>>>,
    pub(super) moved: Vec<bool>,
}

impl ValidatedArena {
    pub(crate) fn new(root: ValueId, values: Arena<ValidatedValue>) -> Self {
        Self {
            root,
            values,
            shape: primitive("pydantic_sifr.untyped"),
            descriptions: None,
            edges: None,
            moved: Vec::new(),
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

    pub(crate) fn selected(&self, root: ValueId) -> Option<Self> {
        self.values.get(root)?;
        let mut selected = self.clone();
        selected.root = root;
        Some(selected)
    }

    pub(crate) fn tuple_root_with_cloned_item(
        &self,
        first: ValueId,
        cloned_item: ValueId,
    ) -> Result<Self, ArenaError> {
        self.values.get(first).ok_or(ArenaError::CapacityExceeded)?;
        self.values
            .get(cloned_item)
            .ok_or(ArenaError::CapacityExceeded)?;
        let mut selected = self.clone();
        let cloned_item =
            self.clone_subtree(cloned_item, &mut selected.values, &mut HashMap::new())?;
        let root = selected
            .values
            .push(ValidatedValue::Tuple(vec![first, cloned_item]))?;
        selected.root = root;
        selected.descriptions = None;
        selected.edges = None;
        selected.moved.clear();
        Ok(selected)
    }

    fn clone_subtree(
        &self,
        id: ValueId,
        destination: &mut Arena<ValidatedValue>,
        cloned: &mut HashMap<ValueId, ValueId>,
    ) -> Result<ValueId, ArenaError> {
        if let Some(id) = cloned.get(&id) {
            return Ok(*id);
        }
        let mut value = self
            .values
            .get(id)
            .cloned()
            .ok_or(ArenaError::CapacityExceeded)?;
        match &mut value {
            ValidatedValue::Sequence(ids)
            | ValidatedValue::Tuple(ids)
            | ValidatedValue::Set(ids)
            | ValidatedValue::FrozenSet(ids) => {
                for child in ids {
                    *child = self.clone_subtree(*child, destination, cloned)?;
                }
            }
            ValidatedValue::Mapping(entries) => {
                for (key, value) in entries {
                    *key = self.clone_subtree(*key, destination, cloned)?;
                    *value = self.clone_subtree(*value, destination, cloned)?;
                }
            }
            ValidatedValue::Nullable(Some(child)) => {
                *child = self.clone_subtree(*child, destination, cloned)?;
            }
            ValidatedValue::Enum(value) => {
                value.discriminant = self.clone_subtree(value.discriminant, destination, cloned)?;
            }
            ValidatedValue::Union(value) => {
                value.value = self.clone_subtree(value.value, destination, cloned)?;
            }
            ValidatedValue::Model(model) => {
                for (_, child) in &mut model.fields {
                    *child = self.clone_subtree(*child, destination, cloned)?;
                }
                for (_, child) in &mut model.extras {
                    *child = self.clone_subtree(*child, destination, cloned)?;
                }
            }
            _ => {}
        }
        let cloned_id = destination.push(value)?;
        cloned.insert(id, cloned_id);
        Ok(cloned_id)
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
            Self::Enum(value) => value.discriminant = remap_id(value.discriminant, offset)?,
            Self::Union(value) => value.value = remap_id(value.value, offset)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuple_root_clones_the_second_item_into_distinct_storage() {
        let mut values = Arena::new();
        let field = values
            .push(ValidatedValue::String("value".to_owned()))
            .unwrap_or_else(|error| panic!("test field does not fit in the arena: {error}"));
        let model = values
            .push(ValidatedValue::Model(ModelValue::new(
                "test.Model",
                vec![("value", field)],
                Vec::new(),
                1,
            )))
            .unwrap_or_else(|error| panic!("test model does not fit in the arena: {error}"));
        let input = ValidatedArena::new(model, values);

        let tuple = input
            .tuple_root_with_cloned_item(model, field)
            .unwrap_or_else(|error| panic!("test tuple does not fit in the arena: {error}"));
        let ValidatedValue::Tuple(items) = tuple
            .get(tuple.root())
            .unwrap_or_else(|| panic!("tuple root is not addressable"))
        else {
            panic!("tuple helper must append a tuple root");
        };

        assert_eq!(items[0], model);
        assert_ne!(items[1], field);
        assert_eq!(tuple.get(items[1]), tuple.get(field));
        assert_eq!(tuple.len(), 4, "only the selected subtree is cloned");
    }
}
