use core::fmt;
use std::cmp::Ordering;

use sifr_runtime::interop::structural::{
    StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind, StructuralProject,
    StructuralScalarRef, StructuralVisitor, VisitControl,
};

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
    validate_native_limits(limits)?;
    let mut builder = NativeBuilder {
        values: Arena::new(),
        limits,
        string_bytes: 0,
    };
    let root = builder.push(value, 0)?;
    Ok(InputArena::from_parts(root, builder.values))
}

pub fn project_structural_input<T: StructuralProject>(
    value: &T,
    limits: JsonLimits,
) -> Result<InputArena, NativeInputError> {
    validate_native_limits(limits)?;
    let mut builder = StructuralInputBuilder {
        values: Arena::new(),
        limits,
        string_bytes: 0,
        frames: Vec::new(),
        root: None,
    };
    value.structural_project(&mut builder)?;
    if !builder.frames.is_empty() {
        return Err(NativeInputError::Projection(
            "structural projection did not close every aggregate",
        ));
    }
    let root = builder.root.ok_or(NativeInputError::Projection(
        "structural projection produced no root value",
    ))?;
    Ok(InputArena::from_parts(root, builder.values))
}

struct StructuralInputBuilder<'value> {
    values: Arena<InputValue>,
    limits: JsonLimits,
    string_bytes: usize,
    frames: Vec<StructuralFrame<'value>>,
    root: Option<InputId>,
}

struct StructuralFrame<'value> {
    kind: StructuralKind,
    child_count: usize,
    pending_edge: Option<StructuralEdgeKind<'value>>,
    children: Vec<(StructuralEdgeKind<'value>, InputId)>,
}

impl<'value> StructuralVisitor<'value> for StructuralInputBuilder<'value> {
    type Error = NativeInputError;

    fn enter(&mut self, event: StructuralEnter<'value>) -> Result<VisitControl, Self::Error> {
        if self.frames.len() > self.limits.max_depth {
            return Err(NativeInputError::Limit("maximum depth"));
        }
        self.check_enter(event.kind(), event.child_count())?;
        self.frames.push(StructuralFrame {
            kind: event.kind(),
            child_count: event.child_count(),
            pending_edge: None,
            children: Vec::with_capacity(event.child_count()),
        });
        Ok(VisitControl::Continue)
    }

    fn edge(&mut self, edge: StructuralEdge<'value>) -> Result<(), Self::Error> {
        let frame = self.frames.last_mut().ok_or(NativeInputError::Projection(
            "structural edge has no parent aggregate",
        ))?;
        if frame.pending_edge.replace(edge.kind()).is_some() {
            return Err(NativeInputError::Projection(
                "structural edge has no projected child",
            ));
        }
        Ok(())
    }

    fn scalar(&mut self, value: StructuralScalarRef<'value>) -> Result<(), Self::Error> {
        let value = match value {
            StructuralScalarRef::None => InputValue::Null,
            StructuralScalarRef::Bool(value) => InputValue::Bool(value),
            StructuralScalarRef::SignedInteger { value, .. } => {
                InputValue::Integer(self.checked_integer_text(value)?)
            }
            StructuralScalarRef::UnsignedInteger { value, .. } => {
                InputValue::Integer(self.checked_integer_text(value)?)
            }
            StructuralScalarRef::ExactInteger(value) => {
                InputValue::Integer(self.checked_integer_text(value)?)
            }
            StructuralScalarRef::Float(value) => InputValue::Float(value),
            StructuralScalarRef::String(value) => {
                self.add_string_bytes(value.len())?;
                InputValue::String(value.to_owned())
            }
            StructuralScalarRef::Bytes(value) => {
                self.add_string_bytes(value.len())?;
                InputValue::Bytes(value.to_vec())
            }
            _ => {
                return Err(NativeInputError::Projection(
                    "structural scalar kind is not supported",
                ));
            }
        };
        let id = self.push(value)?;
        self.attach(id)
    }

    fn exit(&mut self, kind: StructuralKind) -> Result<(), Self::Error> {
        let frame = self.frames.pop().ok_or(NativeInputError::Projection(
            "structural aggregate exit has no matching entry",
        ))?;
        if frame.kind != kind || frame.pending_edge.is_some() {
            return Err(NativeInputError::Projection(
                "structural aggregate events are unbalanced",
            ));
        }
        if frame.children.len() != frame.child_count {
            return Err(NativeInputError::Projection(
                "structural aggregate child count is invalid",
            ));
        }
        if kind == StructuralKind::Optional {
            return self.finish_optional(frame);
        }
        let value = self.aggregate_value(frame)?;
        let id = self.push(value)?;
        self.attach(id)
    }
}

impl<'value> StructuralInputBuilder<'value> {
    fn check_enter(
        &self,
        kind: StructuralKind,
        child_count: usize,
    ) -> Result<(), NativeInputError> {
        if child_count.saturating_add(1) > self.limits.max_nodes {
            return Err(NativeInputError::Limit("maximum node count"));
        }
        let item_count = match kind {
            StructuralKind::Record
            | StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => child_count,
            StructuralKind::Mapping if child_count.is_multiple_of(2) => child_count / 2,
            StructuralKind::Mapping => {
                return Err(NativeInputError::Projection(
                    "structural mapping child count is invalid",
                ));
            }
            StructuralKind::Optional if child_count <= 1 => 0,
            StructuralKind::Optional => {
                return Err(NativeInputError::Projection(
                    "structural optional child count is invalid",
                ));
            }
            _ => {
                return Err(NativeInputError::Projection(
                    "structural aggregate kind is not supported",
                ));
            }
        };
        self.check_collection(item_count)
    }

    fn checked_integer_text(&self, value: impl fmt::Display) -> Result<String, NativeInputError> {
        let value = value.to_string();
        if value.trim_start_matches(['-', '+']).len() > self.limits.max_integer_digits {
            Err(NativeInputError::Limit("maximum integer digits"))
        } else {
            Ok(value)
        }
    }

    fn push(&mut self, value: InputValue) -> Result<InputId, NativeInputError> {
        if self.values.len() >= self.limits.max_nodes {
            return Err(NativeInputError::Limit("maximum node count"));
        }
        self.values.push(value).map_err(NativeInputError::Arena)
    }

    fn attach(&mut self, id: InputId) -> Result<(), NativeInputError> {
        if let Some(frame) = self.frames.last_mut() {
            let edge = frame
                .pending_edge
                .take()
                .ok_or(NativeInputError::Projection(
                    "structural child has no declared edge",
                ))?;
            frame.children.push((edge, id));
            Ok(())
        } else if self.root.replace(id).is_none() {
            Ok(())
        } else {
            Err(NativeInputError::Projection(
                "structural projection produced multiple roots",
            ))
        }
    }

    fn finish_optional(&mut self, frame: StructuralFrame<'value>) -> Result<(), NativeInputError> {
        match frame.children.as_slice() {
            [] => {
                let id = self.push(InputValue::Null)?;
                self.attach(id)
            }
            [
                (
                    StructuralEdgeKind::ActiveMember {
                        name: "some",
                        index: 0,
                    },
                    id,
                ),
            ] => self.attach(*id),
            _ => Err(NativeInputError::Projection(
                "structural optional member is invalid",
            )),
        }
    }

    fn aggregate_value(
        &mut self,
        frame: StructuralFrame<'value>,
    ) -> Result<InputValue, NativeInputError> {
        match frame.kind {
            StructuralKind::Record => {
                self.check_collection(frame.children.len())?;
                let mut entries = Vec::with_capacity(frame.children.len());
                for (edge, id) in frame.children {
                    let StructuralEdgeKind::RecordField(name) = edge else {
                        return Err(NativeInputError::Projection(
                            "structural record edge is invalid",
                        ));
                    };
                    self.add_string_bytes(name.len())?;
                    entries.push((name.to_owned(), id));
                }
                Ok(InputValue::Object {
                    kind: ObjectKind::Object,
                    entries,
                })
            }
            StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => {
                self.check_collection(frame.children.len())?;
                let mut items = Vec::with_capacity(frame.children.len());
                for (index, (edge, id)) in frame.children.into_iter().enumerate() {
                    if edge != StructuralEdgeKind::Index(index) {
                        return Err(NativeInputError::Projection(
                            "structural sequence edge is invalid",
                        ));
                    }
                    items.push(id);
                }
                if matches!(frame.kind, StructuralKind::Set | StructuralKind::FrozenSet) {
                    items.sort_by(|left, right| compare_input_ids(&self.values, *left, *right));
                }
                let kind = match frame.kind {
                    StructuralKind::Sequence => SequenceKind::List,
                    StructuralKind::Tuple => SequenceKind::Tuple,
                    StructuralKind::Set => SequenceKind::Set,
                    StructuralKind::FrozenSet => SequenceKind::FrozenSet,
                    _ => {
                        return Err(NativeInputError::Projection(
                            "structural sequence kind is invalid",
                        ));
                    }
                };
                Ok(InputValue::Sequence { kind, items })
            }
            StructuralKind::Mapping => {
                if !frame.children.len().is_multiple_of(2) {
                    return Err(NativeInputError::Projection(
                        "structural mapping child count is invalid",
                    ));
                }
                let pair_count = frame.children.len() / 2;
                self.check_collection(pair_count)?;
                let mut entries = Vec::with_capacity(pair_count);
                for (index, pair) in frame.children.chunks_exact(2).enumerate() {
                    if pair[0].0 != StructuralEdgeKind::MappingKey(index)
                        || pair[1].0 != StructuralEdgeKind::MappingValue(index)
                    {
                        return Err(NativeInputError::Projection(
                            "structural mapping edge is invalid",
                        ));
                    }
                    entries.push((pair[0].1, pair[1].1));
                }
                entries.sort_by(|left, right| {
                    compare_input_ids(&self.values, left.0, right.0)
                        .then_with(|| compare_input_ids(&self.values, left.1, right.1))
                });
                Ok(InputValue::Mapping(entries))
            }
            _ => Err(NativeInputError::Projection(
                "structural aggregate kind is not supported",
            )),
        }
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

fn validate_native_limits(limits: JsonLimits) -> Result<(), NativeInputError> {
    if limits.max_depth == 0
        || limits.max_nodes == 0
        || limits.max_string_bytes == 0
        || limits.max_integer_digits == 0
        || limits.max_collection_items == 0
        || limits.max_depth > HARD_MAX_DEPTH
    {
        Err(NativeInputError::Limit("limits must be greater than zero"))
    } else {
        Ok(())
    }
}

fn compare_input_ids(values: &Arena<InputValue>, left: InputId, right: InputId) -> Ordering {
    let (Some(left), Some(right)) = (values.get(left), values.get(right)) else {
        return left.raw().cmp(&right.raw());
    };
    input_rank(left)
        .cmp(&input_rank(right))
        .then_with(|| match (left, right) {
            (InputValue::Null, InputValue::Null) => Ordering::Equal,
            (InputValue::Bool(left), InputValue::Bool(right)) => left.cmp(right),
            (InputValue::Integer(left), InputValue::Integer(right))
            | (InputValue::String(left), InputValue::String(right)) => left.cmp(right),
            (InputValue::Float(left), InputValue::Float(right)) => left.total_cmp(right),
            (InputValue::Bytes(left), InputValue::Bytes(right)) => left.cmp(right),
            (
                InputValue::Sequence {
                    kind: left_kind,
                    items: left_items,
                },
                InputValue::Sequence {
                    kind: right_kind,
                    items: right_items,
                },
            ) => sequence_rank(*left_kind)
                .cmp(&sequence_rank(*right_kind))
                .then_with(|| compare_id_slices(values, left_items, right_items)),
            (
                InputValue::Object {
                    kind: left_kind,
                    entries: left_entries,
                },
                InputValue::Object {
                    kind: right_kind,
                    entries: right_entries,
                },
            ) => object_rank(*left_kind)
                .cmp(&object_rank(*right_kind))
                .then_with(|| compare_object_entries(values, left_entries, right_entries)),
            (InputValue::Mapping(left), InputValue::Mapping(right)) => {
                compare_mapping_entries(values, left, right)
            }
            _ => Ordering::Equal,
        })
}

fn compare_id_slices(values: &Arena<InputValue>, left: &[InputId], right: &[InputId]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let order = compare_input_ids(values, *left, *right);
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_object_entries(
    values: &Arena<InputValue>,
    left: &[(String, InputId)],
    right: &[(String, InputId)],
) -> Ordering {
    for ((left_name, left_id), (right_name, right_id)) in left.iter().zip(right) {
        let order = left_name
            .cmp(right_name)
            .then_with(|| compare_input_ids(values, *left_id, *right_id));
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_mapping_entries(
    values: &Arena<InputValue>,
    left: &[(InputId, InputId)],
    right: &[(InputId, InputId)],
) -> Ordering {
    for ((left_key, left_value), (right_key, right_value)) in left.iter().zip(right) {
        let order = compare_input_ids(values, *left_key, *right_key)
            .then_with(|| compare_input_ids(values, *left_value, *right_value));
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

const fn input_rank(value: &InputValue) -> u8 {
    match value {
        InputValue::Null => 0,
        InputValue::Bool(_) => 1,
        InputValue::Integer(_) => 2,
        InputValue::Float(_) => 3,
        InputValue::String(_) => 4,
        InputValue::Bytes(_) => 5,
        InputValue::Sequence { .. } => 6,
        InputValue::Object { .. } => 7,
        InputValue::Mapping(_) => 8,
        _ => 9,
    }
}

const fn sequence_rank(kind: SequenceKind) -> u8 {
    match kind {
        SequenceKind::JsonArray => 0,
        SequenceKind::List => 1,
        SequenceKind::Tuple => 2,
        SequenceKind::Set => 3,
        SequenceKind::FrozenSet => 4,
    }
}

const fn object_rank(kind: ObjectKind) -> u8 {
    match kind {
        ObjectKind::JsonObject => 0,
        ObjectKind::Object => 1,
    }
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
    Projection(&'static str),
}

impl fmt::Display for NativeInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(kind) => write!(f, "native input limit exceeded: {kind}"),
            Self::Arena(error) => error.fmt(f),
            Self::Projection(error) => write!(f, "structural input is invalid: {error}"),
        }
    }
}

impl std::error::Error for NativeInputError {}
