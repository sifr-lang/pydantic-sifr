use std::str::FromStr;

use num_bigint::BigInt;
use sifr_runtime::interop::structural::StaticProgramValue;

use super::{
    AliasPath, AliasSegment, ExtraPolicy, FieldDefault, IntegerConstraints, IntegerTarget,
    ModelField, ModelSchema, Schema, StringConstraints, ValidationError, scalars::type_error,
};

mod sums;
pub(crate) use sums::{StaticMetadata, StaticVariant};

#[derive(Clone, Copy, Debug)]
pub enum SchemaRef<'schema> {
    Owned(&'schema Schema),
    Static(StaticSchemaRef),
}

#[derive(Clone, Copy, Debug)]
pub struct StaticSchemaRef {
    nodes: &'static [StaticProgramValue],
    index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaTag {
    None,
    Bool,
    Integer,
    Float,
    Decimal,
    Fraction,
    Complex,
    String,
    Bytes,
    Temporal,
    Uuid,
    Url,
    Pattern,
    Literal,
    Enum,
    Nullable,
    Union,
    TaggedUnion,
    Definitions,
    DefinitionRef,
    Model,
    List,
    Tuple,
    Mapping,
    Set,
    FrozenSet,
    Generator,
    EmbeddedJson,
    LaxOrStrict,
    JsonOrStructural,
    Chain,
}

#[derive(Clone, Copy, Debug)]
pub enum ModelRef<'schema> {
    Owned(&'schema ModelSchema),
    Static(StaticModelRef),
}

#[derive(Clone, Copy, Debug)]
pub struct StaticModelRef {
    schema: StaticSchemaRef,
    value: &'static StaticProgramValue,
}

#[derive(Clone, Copy, Debug)]
pub enum FieldRef<'schema> {
    Owned(&'schema ModelField),
    Static(StaticFieldRef),
}

#[derive(Clone, Copy, Debug)]
pub struct StaticFieldRef {
    schema: StaticSchemaRef,
    value: &'static StaticProgramValue,
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultRef<'schema> {
    Owned(&'schema FieldDefault),
    Static(&'static StaticProgramValue),
}

impl<'schema> SchemaRef<'schema> {
    #[must_use]
    pub const fn owned(schema: &'schema Schema) -> Self {
        Self::Owned(schema)
    }

    pub fn from_static_program(
        value: &'static StaticProgramValue,
    ) -> Result<Self, ValidationError> {
        let record = record(value, "static schema program")?;
        let nodes = list(field(record, "nodes")?, "schema nodes")?;
        let root = usize_value(field(record, "root")?, "schema root")?;
        if root >= nodes.len() {
            return Err(schema_error("Static schema root is out of range"));
        }
        Ok(Self::Static(StaticSchemaRef { nodes, index: root }))
    }

    pub fn tag(self) -> Result<SchemaTag, ValidationError> {
        match self {
            Self::Owned(schema) => Ok(match schema {
                Schema::None => SchemaTag::None,
                Schema::Bool => SchemaTag::Bool,
                Schema::Integer { .. } => SchemaTag::Integer,
                Schema::Float(_) => SchemaTag::Float,
                Schema::Decimal(_) => SchemaTag::Decimal,
                Schema::Fraction(_) => SchemaTag::Fraction,
                Schema::Complex(_) => SchemaTag::Complex,
                Schema::String(_) => SchemaTag::String,
                Schema::Bytes(_) => SchemaTag::Bytes,
                Schema::Temporal(_) => SchemaTag::Temporal,
                Schema::Uuid { .. } => SchemaTag::Uuid,
                Schema::Url(_) => SchemaTag::Url,
                Schema::Pattern(_) => SchemaTag::Pattern,
                Schema::Literal(_) => SchemaTag::Literal,
                Schema::Enum(_) => SchemaTag::Enum,
                Schema::Nullable(_) => SchemaTag::Nullable,
                Schema::Union(_) => SchemaTag::Union,
                Schema::TaggedUnion(_) => SchemaTag::TaggedUnion,
                Schema::Definitions(_) => SchemaTag::Definitions,
                Schema::DefinitionRef { .. } => SchemaTag::DefinitionRef,
                Schema::Model(_) => SchemaTag::Model,
                Schema::List { .. } => SchemaTag::List,
                Schema::Tuple(_) => SchemaTag::Tuple,
                Schema::Mapping { .. } => SchemaTag::Mapping,
                Schema::Set { .. } => SchemaTag::Set,
                Schema::FrozenSet { .. } => SchemaTag::FrozenSet,
                Schema::Generator { .. } => SchemaTag::Generator,
                Schema::EmbeddedJson(_) => SchemaTag::EmbeddedJson,
                Schema::LaxOrStrict(_) => SchemaTag::LaxOrStrict,
                Schema::JsonOrStructural(_) => SchemaTag::JsonOrStructural,
                Schema::Chain(_) => SchemaTag::Chain,
            }),
            Self::Static(schema) => match schema.kind()? {
                "none" => Ok(SchemaTag::None),
                "bool" => Ok(SchemaTag::Bool),
                "int" => Ok(SchemaTag::Integer),
                "float" => Ok(SchemaTag::Float),
                "decimal" => Ok(SchemaTag::Decimal),
                "str" => Ok(SchemaTag::String),
                "bytes" => Ok(SchemaTag::Bytes),
                "literal" => Ok(SchemaTag::Literal),
                "nullable" => Ok(SchemaTag::Nullable),
                "union" => Ok(SchemaTag::Union),
                "enum" => Ok(SchemaTag::Enum),
                "tagged-union" => Ok(SchemaTag::TaggedUnion),
                "definitions" => Ok(SchemaTag::Definitions),
                "definition-ref" => Ok(SchemaTag::DefinitionRef),
                "model" => Ok(SchemaTag::Model),
                "list" => Ok(SchemaTag::List),
                "tuple" => Ok(SchemaTag::Tuple),
                "dict" => Ok(SchemaTag::Mapping),
                "set" => Ok(SchemaTag::Set),
                "lax-or-strict" => Ok(SchemaTag::LaxOrStrict),
                "json-or-structural" => Ok(SchemaTag::JsonOrStructural),
                "chain" => Ok(SchemaTag::Chain),
                _ => Err(schema_error("Static schema kind is not supported")),
            },
        }
    }

    pub fn child(self, index: usize) -> Result<Self, ValidationError> {
        match self {
            Self::Owned(schema) => owned_child(schema, index),
            Self::Static(schema) => schema.child(index).map(Self::Static),
        }
    }

    pub fn child_count(self) -> Result<usize, ValidationError> {
        match self {
            Self::Owned(schema) => Ok(match schema {
                Schema::Nullable(_) | Schema::Definitions(_) | Schema::EmbeddedJson(_) => 1,
                Schema::LaxOrStrict(_) | Schema::JsonOrStructural(_) => 2,
                Schema::Chain(schema) => schema.steps().len(),
                Schema::Union(schema) => schema.choices().len(),
                Schema::TaggedUnion(schema) => schema.choices().len(),
                Schema::List { .. }
                | Schema::Set { .. }
                | Schema::FrozenSet { .. }
                | Schema::Generator { .. } => 1,
                Schema::Tuple(items) => items.len(),
                Schema::Mapping { .. } => 2,
                _ => 0,
            }),
            Self::Static(schema) => schema.child_count(),
        }
    }

    pub fn model(self) -> Result<ModelRef<'schema>, ValidationError> {
        match self {
            Self::Owned(Schema::Model(model)) => Ok(ModelRef::Owned(model)),
            Self::Static(schema) => Ok(ModelRef::Static(schema.model()?)),
            _ => Err(schema_error("Schema node is not a model")),
        }
    }

    pub fn integer(self) -> Result<(IntegerTarget, IntegerConstraints), ValidationError> {
        match self {
            Self::Owned(Schema::Integer {
                target,
                constraints,
            }) => Ok((*target, constraints.clone())),
            Self::Static(schema) => Ok((schema.integer_target()?, schema.integer_constraints()?)),
            _ => Err(schema_error("Schema node is not an integer")),
        }
    }

    pub fn string(self) -> Result<StringConstraints, ValidationError> {
        match self {
            Self::Owned(Schema::String(constraints)) => Ok(constraints.clone()),
            Self::Static(schema) => schema.string_constraints(),
            _ => Err(schema_error("Schema node is not a string")),
        }
    }

    pub(crate) fn static_metadata(self) -> Result<Vec<StaticMetadata>, ValidationError> {
        sums::metadata(self)
    }

    pub(crate) fn static_variants(self) -> Result<Vec<StaticVariant>, ValidationError> {
        sums::variants(self)
    }

    pub(crate) fn static_definition(self) -> Result<&'static str, ValidationError> {
        sums::definition(self)
    }

    pub(crate) fn static_error(
        self,
    ) -> Result<Option<super::SchemaErrorOverride>, ValidationError> {
        sums::error_override(self)
    }

    pub(crate) fn static_reference(self) -> Result<&'static str, ValidationError> {
        match self {
            Self::Static(schema) => schema.reference(),
            _ => Err(schema_error(
                "Schema node is not a static definition reference",
            )),
        }
    }

    pub(crate) fn static_definition_target(self) -> Result<Self, ValidationError> {
        match self {
            Self::Static(schema) => schema.definition_target().map(Self::Static),
            _ => Err(schema_error(
                "Schema node is not a static definition reference",
            )),
        }
    }

    pub(crate) fn default_strict(self) -> Result<bool, ValidationError> {
        match self {
            Self::Owned(Schema::LaxOrStrict(schema)) => Ok(schema.default_strict()),
            Self::Static(_) => self
                .static_metadata()?
                .iter()
                .find(|item| item.key == "pydantic.strict.default")
                .map_or(Ok(false), |item| match item.value {
                    "true" => Ok(true),
                    "false" => Ok(false),
                    _ => Err(schema_error("Static strict default is invalid")),
                }),
            _ => Err(schema_error("Schema node is not a strictness control")),
        }
    }
}

impl StaticSchemaRef {
    fn value(self) -> Result<&'static StaticProgramValue, ValidationError> {
        self.nodes
            .get(self.index)
            .ok_or_else(|| schema_error("Static schema node is out of range"))
    }

    fn node_record(self) -> Result<&'static [(&'static str, StaticProgramValue)], ValidationError> {
        record(self.value()?, "static schema node")
    }

    fn kind(self) -> Result<&'static str, ValidationError> {
        string(field(self.node_record()?, "kind")?, "schema kind")
    }

    fn child(self, index: usize) -> Result<Self, ValidationError> {
        let children = list(field(self.node_record()?, "children")?, "schema children")?;
        let child = children
            .get(index)
            .ok_or_else(|| schema_error("Static schema child is missing"))?;
        let index = usize_value(child, "schema child")?;
        if index >= self.nodes.len() {
            return Err(schema_error("Static schema child is out of range"));
        }
        Ok(Self {
            nodes: self.nodes,
            index,
        })
    }

    fn child_count(self) -> Result<usize, ValidationError> {
        list(field(self.node_record()?, "children")?, "schema children").map(<[_]>::len)
    }

    fn reference(self) -> Result<&'static str, ValidationError> {
        string(
            field(self.node_record()?, "reference")?,
            "definition reference",
        )
    }

    fn definition_target(self) -> Result<Self, ValidationError> {
        let reference = self.reference()?;
        let index = self
            .nodes
            .iter()
            .enumerate()
            .find_map(|(index, value)| {
                let record = record(value, "static schema node").ok()?;
                let definition = field(record, "definition").ok()?;
                matches!(definition, StaticProgramValue::String(value) if *value == reference)
                    .then_some(index)
            })
            .ok_or_else(|| schema_error("Static definition reference is unresolved"))?;
        Ok(Self {
            nodes: self.nodes,
            index,
        })
    }

    fn model(self) -> Result<StaticModelRef, ValidationError> {
        let value = field(self.node_record()?, "model")?;
        if matches!(value, StaticProgramValue::None) {
            return Err(schema_error("Static model payload is missing"));
        }
        Ok(StaticModelRef {
            schema: self,
            value,
        })
    }

    fn integer_constraints(self) -> Result<IntegerConstraints, ValidationError> {
        let value = field(self.node_record()?, "integer_constraints")?;
        if matches!(value, StaticProgramValue::None) {
            return Ok(IntegerConstraints::default());
        }
        let value = record(value, "integer constraints")?;
        Ok(IntegerConstraints {
            greater_than: optional_big_int(field(value, "greater_than")?)?,
            greater_or_equal: optional_big_int(field(value, "greater_or_equal")?)?,
            less_than: optional_big_int(field(value, "less_than")?)?,
            less_or_equal: optional_big_int(field(value, "less_or_equal")?)?,
            multiple_of: optional_big_int(field(value, "multiple_of")?)?,
        })
    }

    fn integer_target(self) -> Result<IntegerTarget, ValidationError> {
        let value = field(self.node_record()?, "integer_target")?;
        let target = string(value, "integer target")?;
        match target {
            "int" => Ok(IntegerTarget::Exact),
            "int8" => Ok(IntegerTarget::I8),
            "int16" => Ok(IntegerTarget::I16),
            "int32" => Ok(IntegerTarget::I32),
            "int64" => Ok(IntegerTarget::I64),
            "uint8" => Ok(IntegerTarget::U8),
            "uint16" => Ok(IntegerTarget::U16),
            "uint32" => Ok(IntegerTarget::U32),
            "uint64" => Ok(IntegerTarget::U64),
            _ => Err(schema_error("Static integer target is invalid")),
        }
    }

    fn string_constraints(self) -> Result<StringConstraints, ValidationError> {
        let value = field(self.node_record()?, "string_constraints")?;
        if matches!(value, StaticProgramValue::None) {
            return Ok(StringConstraints::default());
        }
        let value = record(value, "string constraints")?;
        Ok(StringConstraints {
            min_length: optional_usize(field(value, "min_length")?)?,
            max_length: optional_usize(field(value, "max_length")?)?,
            strip_whitespace: bool_value(field(value, "strip_whitespace")?, "strip_whitespace")?,
            to_lower: bool_value(field(value, "to_lower")?, "to_lower")?,
            to_upper: bool_value(field(value, "to_upper")?, "to_upper")?,
            ascii_only: bool_value(field(value, "ascii_only")?, "ascii_only")?,
            ..StringConstraints::default()
        })
    }
}

impl<'schema> ModelRef<'schema> {
    pub fn name(self) -> Result<&'static str, ValidationError> {
        match self {
            Self::Owned(model) => Ok(model.name),
            Self::Static(model) => model.text("name"),
        }
    }

    pub fn fields(self) -> Result<Vec<FieldRef<'schema>>, ValidationError> {
        match self {
            Self::Owned(model) => Ok(model.fields.iter().map(FieldRef::Owned).collect()),
            Self::Static(model) => model
                .fields()?
                .iter()
                .map(|value| {
                    Ok(FieldRef::Static(StaticFieldRef {
                        schema: model.schema,
                        value,
                    }))
                })
                .collect(),
        }
    }

    pub fn extra(self) -> Result<ExtraRef<'schema>, ValidationError> {
        match self {
            Self::Owned(model) => Ok(match &model.extra {
                ExtraPolicy::Ignore => ExtraRef::Ignore,
                ExtraPolicy::Forbid => ExtraRef::Forbid,
                ExtraPolicy::Allow {
                    destination,
                    value_schema,
                } => ExtraRef::Allow {
                    destination,
                    value_schema: SchemaRef::Owned(value_schema),
                },
            }),
            Self::Static(model) => match model.text("extra")? {
                "ignore" => Ok(ExtraRef::Ignore),
                "forbid" => Ok(ExtraRef::Forbid),
                _ => Err(schema_error("Static extra policy is not supported")),
            },
        }
    }

    pub fn populate_by_name(self) -> Result<bool, ValidationError> {
        match self {
            Self::Owned(model) => Ok(model.populate_by_name),
            Self::Static(model) => model.flag("populate_by_name"),
        }
    }

    pub fn location_by_alias(self) -> Result<bool, ValidationError> {
        match self {
            Self::Owned(model) => Ok(model.location_by_alias),
            Self::Static(model) => model.flag("location_by_alias"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ExtraRef<'schema> {
    Ignore,
    Forbid,
    Allow {
        destination: &'schema str,
        value_schema: SchemaRef<'schema>,
    },
}

impl StaticModelRef {
    fn model_record(
        self,
    ) -> Result<&'static [(&'static str, StaticProgramValue)], ValidationError> {
        record(self.value, "model payload")
    }

    fn text(self, name: &'static str) -> Result<&'static str, ValidationError> {
        string(field(self.model_record()?, name)?, name)
    }

    fn flag(self, name: &'static str) -> Result<bool, ValidationError> {
        bool_value(field(self.model_record()?, name)?, name)
    }

    fn fields(self) -> Result<&'static [StaticProgramValue], ValidationError> {
        list(field(self.model_record()?, "fields")?, "model fields")
    }
}

impl<'schema> FieldRef<'schema> {
    pub fn name(self) -> Result<&'static str, ValidationError> {
        match self {
            Self::Owned(field) => Ok(field.name),
            Self::Static(field) => field.text("name"),
        }
    }

    pub fn schema(self) -> Result<SchemaRef<'schema>, ValidationError> {
        match self {
            Self::Owned(field) => Ok(SchemaRef::Owned(&field.schema)),
            Self::Static(field) => {
                let index = usize_value(field.value("node")?, "field node")?;
                if index >= field.schema.nodes.len() {
                    return Err(schema_error("Static field node is out of range"));
                }
                Ok(SchemaRef::Static(StaticSchemaRef {
                    nodes: field.schema.nodes,
                    index,
                }))
            }
        }
    }

    pub fn input(self) -> bool {
        match self {
            Self::Owned(field) => field.input,
            Self::Static(_) => true,
        }
    }

    pub fn default(self) -> Result<Option<DefaultRef<'schema>>, ValidationError> {
        match self {
            Self::Owned(field) => Ok(field.default.as_ref().map(DefaultRef::Owned)),
            Self::Static(field) => {
                let value = field.value("default")?;
                if matches!(value, StaticProgramValue::None) {
                    Ok(None)
                } else {
                    Ok(Some(DefaultRef::Static(value)))
                }
            }
        }
    }

    pub fn aliases(self) -> Result<Vec<AliasPath>, ValidationError> {
        match self {
            Self::Owned(field) => Ok(field.validation_aliases.clone()),
            Self::Static(field) => {
                let values = list(field.value("validation_alias")?, "validation alias")?;
                if values.is_empty() {
                    return Ok(Vec::new());
                }
                let mut segments = Vec::with_capacity(values.len());
                for value in values {
                    let value = record(value, "alias segment")?;
                    match string(field_value(value, "kind")?, "alias kind")? {
                        "field" => segments.push(AliasSegment::Field(string(
                            field_value(value, "name")?,
                            "alias field",
                        )?)),
                        "index" => segments.push(AliasSegment::Index(usize_value(
                            field_value(value, "index")?,
                            "alias index",
                        )?)),
                        _ => return Err(schema_error("Static alias segment is invalid")),
                    }
                }
                Ok(vec![AliasPath { segments }])
            }
        }
    }
}

impl StaticFieldRef {
    fn record(self) -> Result<&'static [(&'static str, StaticProgramValue)], ValidationError> {
        record(self.value, "model field")
    }

    fn value(self, name: &'static str) -> Result<&'static StaticProgramValue, ValidationError> {
        field(self.record()?, name)
    }

    fn text(self, name: &'static str) -> Result<&'static str, ValidationError> {
        string(self.value(name)?, name)
    }
}

fn owned_child<'schema>(
    schema: &'schema Schema,
    index: usize,
) -> Result<SchemaRef<'schema>, ValidationError> {
    let child = match schema {
        Schema::Nullable(inner) | Schema::EmbeddedJson(inner) => {
            (index == 0).then_some(inner.as_ref())
        }
        Schema::List { item, .. }
        | Schema::Set { item, .. }
        | Schema::FrozenSet { item, .. }
        | Schema::Generator { item, .. } => (index == 0).then_some(item.as_ref()),
        Schema::Tuple(items) => items.get(index),
        Schema::Union(schema) => schema.choices().get(index).map(|choice| &choice.schema),
        Schema::TaggedUnion(schema) => schema.choices().get(index).map(|choice| &choice.schema),
        Schema::Mapping { key, value, .. } => match index {
            0 => Some(key.as_ref()),
            1 => Some(value.as_ref()),
            _ => None,
        },
        Schema::Definitions(schema) => (index == 0).then_some(schema.root()),
        Schema::LaxOrStrict(schema) => match index {
            0 => Some(schema.lax()),
            1 => Some(schema.strict()),
            _ => None,
        },
        Schema::JsonOrStructural(schema) => match index {
            0 => Some(schema.json()),
            1 => Some(schema.structural()),
            _ => None,
        },
        Schema::Chain(schema) => schema.steps().get(index),
        _ => None,
    }
    .ok_or_else(|| schema_error("Schema child is missing"))?;
    Ok(SchemaRef::Owned(child))
}

fn record(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static [(&'static str, StaticProgramValue)], ValidationError> {
    match value {
        StaticProgramValue::Record(fields) => Ok(fields),
        _ => Err(schema_error_with_label(label)),
    }
}

fn field(
    fields: &'static [(&'static str, StaticProgramValue)],
    name: &'static str,
) -> Result<&'static StaticProgramValue, ValidationError> {
    fields
        .iter()
        .find(|(field, _)| *field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| schema_error("Static schema field is missing"))
}

fn field_value(
    fields: &'static [(&'static str, StaticProgramValue)],
    name: &'static str,
) -> Result<&'static StaticProgramValue, ValidationError> {
    field(fields, name)
}

fn list(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static [StaticProgramValue], ValidationError> {
    match value {
        StaticProgramValue::List(values) => Ok(values),
        _ => Err(schema_error_with_label(label)),
    }
}

fn string(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static str, ValidationError> {
    match value {
        StaticProgramValue::String(value) => Ok(value),
        _ => Err(schema_error_with_label(label)),
    }
}

fn bool_value(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<bool, ValidationError> {
    match value {
        StaticProgramValue::Bool(value) => Ok(*value),
        _ => Err(schema_error_with_label(label)),
    }
}

fn integer_text(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static str, ValidationError> {
    match value {
        StaticProgramValue::Integer(value) | StaticProgramValue::String(value) => Ok(value),
        _ => Err(schema_error_with_label(label)),
    }
}

fn usize_value(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<usize, ValidationError> {
    integer_text(value, label)?
        .parse::<usize>()
        .map_err(|_| schema_error_with_label(label))
}

fn optional_big_int(value: &'static StaticProgramValue) -> Result<Option<BigInt>, ValidationError> {
    if matches!(value, StaticProgramValue::None) {
        return Ok(None);
    }
    BigInt::from_str(integer_text(value, "integer constraint")?)
        .map(Some)
        .map_err(|_| schema_error("Static integer constraint is invalid"))
}

fn optional_usize(value: &'static StaticProgramValue) -> Result<Option<usize>, ValidationError> {
    if matches!(value, StaticProgramValue::None) {
        return Ok(None);
    }
    usize_value(value, "length constraint").map(Some)
}

fn schema_error(message: &'static str) -> ValidationError {
    type_error("schema_invalid", message, "valid compiler-emitted schema")
}

fn schema_error_with_label(label: &'static str) -> ValidationError {
    ValidationError::one(
        super::ErrorDetail::new("schema_invalid", "Static schema value has an invalid type")
            .expected(label),
    )
}
