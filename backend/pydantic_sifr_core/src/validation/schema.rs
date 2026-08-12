use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_rational::BigRational;
use std::collections::{BTreeMap, BTreeSet};

use sifr_runtime::interop::structural::{
    NominalField, STATIC_PROGRAM_FORMAT_VERSION, STRUCTURAL_BRIDGE_CONTRACT_VERSION, ShapeIdentity,
    StaticProgramType, StructuralType, binary_container, metadata, nominal_record, primitive,
    tuple, unary_container,
};

use crate::NativeValue;

use super::{SchemaRef, ValidationError};

const MAX_SCHEMA_DEPTH: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternCompileError {
    message: String,
}

impl PatternCompileError {
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl core::fmt::Display for PatternCompileError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PatternCompileError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputProfile {
    Native,
    Json,
    Strings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerTarget {
    Exact,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntegerTarget {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exact => "int",
            Self::I8 => "int8",
            Self::I16 => "int16",
            Self::I32 => "int32",
            Self::I64 => "int64",
            Self::U8 => "uint8",
            Self::U16 => "uint16",
            Self::U32 => "uint32",
            Self::U64 => "uint64",
        }
    }

    #[must_use]
    pub fn bounds(self) -> Option<(BigInt, BigInt)> {
        match self {
            Self::Exact => None,
            Self::I8 => Some((BigInt::from(i8::MIN), BigInt::from(i8::MAX))),
            Self::I16 => Some((BigInt::from(i16::MIN), BigInt::from(i16::MAX))),
            Self::I32 => Some((BigInt::from(i32::MIN), BigInt::from(i32::MAX))),
            Self::I64 => Some((BigInt::from(i64::MIN), BigInt::from(i64::MAX))),
            Self::U8 => Some((BigInt::from(u8::MIN), BigInt::from(u8::MAX))),
            Self::U16 => Some((BigInt::from(u16::MIN), BigInt::from(u16::MAX))),
            Self::U32 => Some((BigInt::from(u32::MIN), BigInt::from(u32::MAX))),
            Self::U64 => Some((BigInt::from(u64::MIN), BigInt::from(u64::MAX))),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IntegerConstraints {
    pub greater_than: Option<BigInt>,
    pub greater_or_equal: Option<BigInt>,
    pub less_than: Option<BigInt>,
    pub less_or_equal: Option<BigInt>,
    pub multiple_of: Option<BigInt>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FloatConstraints {
    pub greater_than: Option<f64>,
    pub greater_or_equal: Option<f64>,
    pub less_than: Option<f64>,
    pub less_or_equal: Option<f64>,
    pub multiple_of: Option<f64>,
    pub allow_non_finite: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DecimalConstraints {
    pub greater_than: Option<BigDecimal>,
    pub greater_or_equal: Option<BigDecimal>,
    pub less_than: Option<BigDecimal>,
    pub less_or_equal: Option<BigDecimal>,
    pub multiple_of: Option<BigDecimal>,
    pub max_digits: Option<usize>,
    pub decimal_places: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FractionConstraints {
    pub greater_than: Option<BigRational>,
    pub greater_or_equal: Option<BigRational>,
    pub less_than: Option<BigRational>,
    pub less_or_equal: Option<BigRational>,
    pub multiple_of: Option<BigRational>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComplexConstraints {
    pub allow_non_finite: bool,
    pub magnitude_greater_or_equal: Option<f64>,
    pub magnitude_less_or_equal: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct StringPattern {
    source: String,
    compiled: regex::Regex,
}

impl StringPattern {
    pub fn compile(source: impl Into<String>) -> Result<Self, PatternCompileError> {
        let source = source.into();
        let compiled = regex::RegexBuilder::new(&source)
            .size_limit(1 << 20)
            .dfa_size_limit(2 << 20)
            .build()
            .map_err(|error| PatternCompileError {
                message: error.to_string(),
            })?;
        Ok(Self { source, compiled })
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn is_match(&self, value: &str) -> bool {
        self.compiled.is_match(value)
    }
}

impl PartialEq for StringPattern {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StringConstraints {
    pub strip_whitespace: bool,
    pub ascii_only: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<StringPattern>,
    pub to_upper: bool,
    pub to_lower: bool,
    pub coerce_numbers_to_str: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BytesJsonMode {
    #[default]
    Utf8,
    Base64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BytesConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub json_mode: BytesJsonMode,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CollectionConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalKind {
    Date,
    Time,
    DateTime,
    Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelativeTimeConstraint {
    Past,
    Future,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalSchema {
    pub kind: TemporalKind,
    pub relative: Option<RelativeTimeConstraint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSchema {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UrlConstraints {
    pub max_length: Option<usize>,
    pub allowed_schemes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AliasSegment {
    Field(&'static str),
    Index(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasPath {
    pub segments: Vec<AliasSegment>,
}

impl AliasPath {
    #[must_use]
    pub fn field(name: &'static str) -> Self {
        Self {
            segments: vec![AliasSegment::Field(name)],
        }
    }
}

#[derive(Clone, Debug)]
pub enum FieldDefault {
    Static(NativeValue),
    Factory(fn() -> NativeValue),
}

impl PartialEq for FieldDefault {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Static(left), Self::Static(right)) => left == right,
            (Self::Factory(left), Self::Factory(right)) => std::ptr::fn_addr_eq(*left, *right),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelField {
    pub name: &'static str,
    pub schema: Schema,
    pub input: bool,
    pub default: Option<FieldDefault>,
    pub validation_aliases: Vec<AliasPath>,
    pub metadata: BTreeMap<String, String>,
}

impl ModelField {
    #[must_use]
    pub fn required(name: &'static str, schema: Schema) -> Self {
        Self {
            name,
            schema,
            input: true,
            default: None,
            validation_aliases: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExtraPolicy {
    Ignore,
    Forbid,
    Allow {
        destination: &'static str,
        value_schema: Box<Schema>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelSchema {
    pub(crate) name: &'static str,
    pub(crate) structural_identity: ShapeIdentity,
    pub(crate) fields: Vec<ModelField>,
    pub(crate) extra: ExtraPolicy,
    pub(crate) populate_by_name: bool,
    pub(crate) location_by_alias: bool,
}

impl ModelSchema {
    pub fn new(
        name: &'static str,
        structural_identity: ShapeIdentity,
        fields: Vec<ModelField>,
        extra: ExtraPolicy,
        populate_by_name: bool,
        location_by_alias: bool,
    ) -> Result<Self, ValidationError> {
        verify_model_fields(&fields, &extra)?;
        Ok(Self {
            name,
            structural_identity,
            fields,
            extra,
            populate_by_name,
            location_by_alias,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PreparedSchema<'schema> {
    schema: SchemaRef<'schema>,
    structural_identity: ShapeIdentity,
}

impl<'schema> PreparedSchema<'schema> {
    pub fn new(schema: &'schema Schema) -> Result<Self, ValidationError> {
        Ok(Self {
            schema: SchemaRef::owned(schema),
            structural_identity: schema.structural_identity_at(0)?,
        })
    }

    pub fn from_static<T>() -> Result<PreparedSchema<'static>, ValidationError>
    where
        T: StaticProgramType + StructuralType,
    {
        let program = T::static_program();
        let header = program.header();
        program
            .verify_envelope(
                STATIC_PROGRAM_FORMAT_VERSION,
                header.structural_contract_version(),
                STRUCTURAL_BRIDGE_CONTRACT_VERSION,
                header.identity(),
                T::shape_identity(),
            )
            .map_err(|_| {
                schema_error("Static schema envelope is invalid", "valid schema envelope")
            })?;
        Ok(PreparedSchema {
            schema: SchemaRef::from_static_program(program.value())?,
            structural_identity: T::shape_identity(),
        })
    }

    #[must_use]
    pub const fn schema(&self) -> SchemaRef<'schema> {
        self.schema
    }

    #[must_use]
    pub const fn structural_identity(&self) -> ShapeIdentity {
        self.structural_identity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Schema {
    None,
    Bool,
    Integer {
        target: IntegerTarget,
        constraints: IntegerConstraints,
    },
    Float(FloatConstraints),
    Decimal(DecimalConstraints),
    Fraction(FractionConstraints),
    Complex(ComplexConstraints),
    String(StringConstraints),
    Bytes(BytesConstraints),
    Temporal(TemporalSchema),
    Uuid {
        version: Option<u8>,
    },
    Url(UrlConstraints),
    Pattern(PatternSchema),
    Nullable(Box<Self>),
    Model(ModelSchema),
    List {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    Tuple(Vec<Self>),
    Mapping {
        key: Box<Self>,
        value: Box<Self>,
        constraints: CollectionConstraints,
    },
    Set {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    FrozenSet {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    Generator {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    EmbeddedJson(Box<Self>),
}

impl Schema {
    #[must_use]
    pub fn exact_integer() -> Self {
        Self::Integer {
            target: IntegerTarget::Exact,
            constraints: IntegerConstraints::default(),
        }
    }

    fn structural_identity_at(&self, depth: usize) -> Result<ShapeIdentity, ValidationError> {
        if depth > MAX_SCHEMA_DEPTH {
            return Err(schema_error(
                "Schema nesting exceeds the static preparation limit",
                "bounded static schema",
            ));
        }
        let identity = match self {
            Self::None => primitive("None"),
            Self::Bool => primitive("bool"),
            Self::Integer { target, .. } => primitive(target.structural_name()),
            Self::Float(_) => primitive("f64"),
            Self::Decimal(_) => primitive("bigdecimal"),
            Self::Fraction(_) => primitive("pydantic_sifr.Fraction"),
            Self::Complex(_) => primitive("pydantic_sifr.Complex"),
            Self::String(_) | Self::Url(_) => primitive("str"),
            Self::Pattern(_) => primitive("pydantic_sifr.Pattern"),
            Self::Bytes(_) | Self::Uuid { .. } => primitive("bytes"),
            Self::Temporal(schema) => primitive(schema.structural_name()),
            Self::Nullable(inner) => {
                unary_container("optional", inner.structural_identity_at(depth + 1)?)
            }
            Self::Model(model) => model.structural_identity,
            Self::List { item, .. } | Self::Generator { item, .. } => {
                unary_container("list", item.structural_identity_at(depth + 1)?)
            }
            Self::Tuple(items) => tuple(
                &items
                    .iter()
                    .map(|item| item.structural_identity_at(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Self::Mapping { key, value, .. } => binary_container(
                "mapping",
                key.structural_identity_at(depth + 1)?,
                value.structural_identity_at(depth + 1)?,
            ),
            Self::Set { item, .. } => {
                unary_container("set", item.structural_identity_at(depth + 1)?)
            }
            Self::FrozenSet { item, .. } => {
                let item = item.structural_identity_at(depth + 1)?;
                nominal_record(
                    "sifr.collections.frozenset",
                    &[item],
                    &[NominalField {
                        name: "_values",
                        identity: unary_container("set", item),
                        required: true,
                        default_identity: None,
                    }],
                    metadata(&[]),
                )
            }
            Self::EmbeddedJson(inner) => inner.structural_identity_at(depth + 1)?,
        };
        Ok(identity)
    }
}

fn verify_model_fields(fields: &[ModelField], extra: &ExtraPolicy) -> Result<(), ValidationError> {
    let mut names = BTreeSet::new();
    for field in fields {
        if !names.insert(field.name) {
            return Err(schema_error(
                "Model field names must be unique",
                "unique model fields",
            ));
        }
    }
    let destination = match extra {
        ExtraPolicy::Allow {
            destination,
            value_schema,
        } => {
            let Some(field) = fields.iter().find(|field| field.name == *destination) else {
                return Err(schema_error(
                    "Extra destination must name a declared field",
                    "declared extra destination",
                ));
            };
            if field.input || field.default.is_some() || !extra_field_matches(field, value_schema) {
                return Err(schema_error(
                    "Extra destination must be one non-input mapping field without a default",
                    "typed extra destination",
                ));
            }
            Some(*destination)
        }
        ExtraPolicy::Ignore | ExtraPolicy::Forbid => None,
    };
    if fields
        .iter()
        .any(|field| !field.input && field.default.is_none() && Some(field.name) != destination)
    {
        return Err(schema_error(
            "A non-input field needs a default or must be the extra destination",
            "non-input field value source",
        ));
    }
    Ok(())
}

fn extra_field_matches(field: &ModelField, value_schema: &Schema) -> bool {
    matches!(
        &field.schema,
        Schema::Mapping { key, value, .. }
            if matches!(key.as_ref(), Schema::String(_)) && value.as_ref() == value_schema
    )
}

fn schema_error(message: &'static str, expected: &'static str) -> ValidationError {
    super::scalars::type_error("schema_invalid", message, expected)
}

impl IntegerTarget {
    const fn structural_name(self) -> &'static str {
        match self {
            Self::Exact => "int",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }
}

impl TemporalSchema {
    const fn structural_name(&self) -> &'static str {
        match self.kind {
            TemporalKind::Date => "pydantic_sifr.Date",
            TemporalKind::Time => "pydantic_sifr.Time",
            TemporalKind::DateTime => "pydantic_sifr.DateTime",
            TemporalKind::Duration => "pydantic_sifr.Duration",
        }
    }
}
