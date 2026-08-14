use std::collections::BTreeSet;

use num_bigint::BigInt;
use sifr_runtime::interop::structural::{
    ShapeIdentity, enum_shape, metadata, primitive, unary_container, union,
};

use super::{AliasSegment, ErrorDetail, Schema, ValidationError};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LiteralValue {
    None,
    Bool(bool),
    Integer(BigInt),
    String(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum LiteralKind {
    None,
    Bool,
    Integer,
    String,
    Bytes,
}

impl LiteralValue {
    pub(crate) const fn kind(&self) -> LiteralKind {
        match self {
            Self::None => LiteralKind::None,
            Self::Bool(_) => LiteralKind::Bool,
            Self::Integer(_) => LiteralKind::Integer,
            Self::String(_) => LiteralKind::String,
            Self::Bytes(_) => LiteralKind::Bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralSchema {
    values: Vec<LiteralValue>,
    kinds: Vec<LiteralKind>,
    layout: CanonicalSumLayout,
}

impl LiteralSchema {
    pub fn new(values: Vec<LiteralValue>) -> Result<Self, ValidationError> {
        if values.is_empty() {
            return Err(schema_error(
                "A literal schema must declare at least one value",
                "nonempty literal values",
            ));
        }
        let mut unique = BTreeSet::new();
        let mut kinds = Vec::new();
        for value in &values {
            if !unique.insert(value.clone()) {
                return Err(schema_error(
                    "Literal schema values must be unique",
                    "unique literal values",
                ));
            }
            if !kinds.contains(&value.kind()) {
                kinds.push(value.kind());
            }
        }
        kinds.sort_by_key(|kind| literal_sort_key(*kind));
        let layout = CanonicalSumLayout::from_members(
            kinds
                .iter()
                .map(|kind| CanonicalMember {
                    identity: literal_kind_identity(*kind),
                    sort_key: literal_sort_key(*kind),
                })
                .collect(),
        )?;
        Ok(Self {
            values,
            kinds,
            layout,
        })
    }

    #[must_use]
    pub fn values(&self) -> &[LiteralValue] {
        &self.values
    }

    pub(crate) const fn layout(&self) -> &CanonicalSumLayout {
        &self.layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    pub name: &'static str,
    pub input: LiteralValue,
    pub discriminant: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSchema {
    name: &'static str,
    structural_identity: ShapeIdentity,
    variants: Vec<EnumVariant>,
}

impl EnumSchema {
    pub fn new(name: &'static str, variants: Vec<EnumVariant>) -> Result<Self, ValidationError> {
        if name.is_empty() || variants.is_empty() {
            return Err(schema_error(
                "An enum schema needs a name and at least one variant",
                "named enum variants",
            ));
        }
        let mut names = BTreeSet::new();
        let mut inputs = BTreeSet::new();
        let mut discriminants = BTreeSet::new();
        for variant in &variants {
            if variant.name.is_empty()
                || !names.insert(variant.name)
                || !inputs.insert(variant.input.clone())
                || !discriminants.insert(variant.discriminant)
            {
                return Err(schema_error(
                    "Enum names, inputs, and discriminants must be unique",
                    "unique enum variants",
                ));
            }
        }
        let members = variants
            .iter()
            .map(|variant| (variant.name, Some(variant.discriminant)))
            .collect::<Vec<_>>();
        Ok(Self {
            name,
            structural_identity: enum_shape(name, &members, metadata(&[])),
            variants,
        })
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn structural_identity(&self) -> ShapeIdentity {
        self.structural_identity
    }

    #[must_use]
    pub fn variants(&self) -> &[EnumVariant] {
        &self.variants
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnionMode {
    Smart,
    LeftToRight,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnionChoice {
    pub label: &'static str,
    pub schema: Schema,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchemaErrorOverride {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnionSchema {
    choices: Vec<UnionChoice>,
    mode: UnionMode,
    auto_collapse: bool,
    error: Option<SchemaErrorOverride>,
    layout: CanonicalSumLayout,
}

impl UnionSchema {
    pub fn new(
        choices: Vec<UnionChoice>,
        mode: UnionMode,
        auto_collapse: bool,
        error: Option<SchemaErrorOverride>,
    ) -> Result<Self, ValidationError> {
        verify_choices(&choices)?;
        let layout =
            CanonicalSumLayout::from_schemas(choices.iter().map(|choice| &choice.schema), 0)?;
        Ok(Self {
            choices,
            mode,
            auto_collapse,
            error,
            layout,
        })
    }

    #[must_use]
    pub fn choices(&self) -> &[UnionChoice] {
        &self.choices
    }

    #[must_use]
    pub const fn mode(&self) -> UnionMode {
        self.mode
    }

    #[must_use]
    pub const fn auto_collapse(&self) -> bool {
        self.auto_collapse
    }

    #[must_use]
    pub const fn error(&self) -> Option<SchemaErrorOverride> {
        self.error
    }

    pub(crate) const fn layout(&self) -> &CanonicalSumLayout {
        &self.layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscriminatorPath {
    pub segments: Vec<AliasSegment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaggedUnionChoice {
    pub label: &'static str,
    pub tags: Vec<LiteralValue>,
    pub schema: Schema,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaggedUnionSchema {
    discriminator: DiscriminatorPath,
    choices: Vec<TaggedUnionChoice>,
    error: Option<SchemaErrorOverride>,
    layout: CanonicalSumLayout,
}

impl TaggedUnionSchema {
    pub fn new(
        discriminator: DiscriminatorPath,
        choices: Vec<TaggedUnionChoice>,
        error: Option<SchemaErrorOverride>,
    ) -> Result<Self, ValidationError> {
        if discriminator.segments.is_empty() || choices.is_empty() {
            return Err(schema_error(
                "A tagged union needs a discriminator path and choices",
                "tagged union choices",
            ));
        }
        let mut labels = BTreeSet::new();
        let mut tags = BTreeSet::new();
        for choice in &choices {
            if choice.label.is_empty() || choice.tags.is_empty() || !labels.insert(choice.label) {
                return Err(schema_error(
                    "Tagged union labels and tag lists must be nonempty and unique",
                    "unique tagged union choices",
                ));
            }
            for tag in &choice.tags {
                if !tags.insert(tag.clone()) {
                    return Err(schema_error(
                        "Tagged union discriminator values must be unique",
                        "unique discriminator values",
                    ));
                }
            }
        }
        let layout =
            CanonicalSumLayout::from_schemas(choices.iter().map(|choice| &choice.schema), 0)?;
        Ok(Self {
            discriminator,
            choices,
            error,
            layout,
        })
    }

    #[must_use]
    pub const fn discriminator(&self) -> &DiscriminatorPath {
        &self.discriminator
    }

    #[must_use]
    pub fn choices(&self) -> &[TaggedUnionChoice] {
        &self.choices
    }

    #[must_use]
    pub const fn error(&self) -> Option<SchemaErrorOverride> {
        self.error
    }

    pub(crate) const fn layout(&self) -> &CanonicalSumLayout {
        &self.layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalMember {
    pub(crate) identity: ShapeIdentity,
    sort_key: (u8, String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalSumLayout {
    members: Vec<CanonicalMember>,
    identity: ShapeIdentity,
    optional: bool,
}

impl CanonicalSumLayout {
    fn from_schemas<'schema>(
        schemas: impl IntoIterator<Item = &'schema Schema>,
        depth: usize,
    ) -> Result<Self, ValidationError> {
        let mut members = Vec::new();
        for schema in schemas {
            collect_members(schema, depth, &mut members)?;
        }
        Self::from_members(members)
    }

    fn from_members(mut members: Vec<CanonicalMember>) -> Result<Self, ValidationError> {
        members.sort_by(|left, right| {
            left.sort_key
                .cmp(&right.sort_key)
                .then_with(|| left.identity.as_bytes().cmp(right.identity.as_bytes()))
        });
        members.dedup_by_key(|member| member.identity);
        let identities = members
            .iter()
            .map(|member| member.identity)
            .collect::<Vec<_>>();
        let none = primitive("None");
        let optional = identities.len() == 2 && identities[0] == none;
        let identity = match identities.as_slice() {
            [] => {
                return Err(schema_error(
                    "A sum schema must contain a structural member",
                    "nonempty structural sum",
                ));
            }
            [identity] => *identity,
            [_, value] if optional => unary_container("optional", *value),
            _ => union(&identities),
        };
        Ok(Self {
            members,
            identity,
            optional,
        })
    }

    pub(crate) const fn identity(&self) -> ShapeIdentity {
        self.identity
    }

    pub(crate) fn members(&self) -> &[CanonicalMember] {
        &self.members
    }

    pub(crate) const fn is_direct(&self) -> bool {
        self.members.len() == 1
    }

    pub(crate) const fn is_optional(&self) -> bool {
        self.optional
    }

    pub(crate) fn index_of(&self, identity: ShapeIdentity) -> Option<usize> {
        self.members
            .iter()
            .position(|member| member.identity == identity)
    }
}

pub(crate) fn nullable_layout(inner: &Schema) -> Result<CanonicalSumLayout, ValidationError> {
    CanonicalSumLayout::from_schemas([&Schema::None, inner], 0)
}

fn collect_members(
    schema: &Schema,
    depth: usize,
    members: &mut Vec<CanonicalMember>,
) -> Result<(), ValidationError> {
    if depth > 256 {
        return Err(schema_error(
            "Schema nesting exceeds the static preparation limit",
            "bounded static schema",
        ));
    }
    match schema {
        Schema::Literal(schema) => members.extend(schema.layout.members.iter().cloned()),
        Schema::Union(schema) => members.extend(schema.layout.members.iter().cloned()),
        Schema::TaggedUnion(schema) => members.extend(schema.layout.members.iter().cloned()),
        Schema::Nullable(inner) => {
            members.push(CanonicalMember {
                identity: primitive("None"),
                sort_key: (0, String::new()),
            });
            collect_members(inner, depth + 1, members)?;
        }
        Schema::EmbeddedJson(inner) => collect_members(inner, depth + 1, members)?,
        Schema::Definitions(definitions) => {
            collect_members(definitions.root(), depth + 1, members)?;
        }
        _ => members.push(CanonicalMember {
            identity: schema.structural_identity_at(depth + 1)?,
            sort_key: schema_sort_key(schema),
        }),
    }
    Ok(())
}

pub(crate) fn schema_sort_key(schema: &Schema) -> (u8, String) {
    // Keep these categories synchronized with
    // sifr_type_system/src/union.rs::type_sort_key. Each package schema maps
    // to the Sifr type that owns its generated structural union position.
    match schema {
        Schema::None => (0, String::new()),
        Schema::Bool => (1, String::new()),
        Schema::Integer { target, .. } if *target == super::IntegerTarget::Exact => {
            (2, String::new())
        }
        Schema::Integer { target, .. } => (3, target.name().to_owned()),
        Schema::Float(_) => (4, String::new()),
        Schema::String(_) | Schema::Url(_) => (5, String::new()),
        Schema::Bytes(_) | Schema::Uuid { .. } => (6, String::new()),
        Schema::List { .. } | Schema::Generator { .. } => (10, String::new()),
        Schema::Mapping { .. } => (11, String::new()),
        Schema::Set { .. } => (12, String::new()),
        Schema::Tuple(_) => (13, String::new()),
        Schema::Model(model) => (31, bare_nominal_name(model.name).to_owned()),
        Schema::DefinitionRef { sort_key, .. } => sort_key.clone(),
        Schema::FrozenSet { .. } => (31, "frozenset".to_owned()),
        Schema::Fraction(_) => (34, "Fraction".to_owned()),
        Schema::Complex(_) => (34, "Complex".to_owned()),
        Schema::Temporal(schema) => (34, format!("{:?}", schema.kind)),
        Schema::Pattern(_) => (34, "Pattern".to_owned()),
        Schema::Enum(schema) => (38, bare_nominal_name(schema.name).to_owned()),
        Schema::Decimal(_) => (41, String::new()),
        Schema::Literal(_)
        | Schema::Nullable(_)
        | Schema::Union(_)
        | Schema::TaggedUnion(_)
        | Schema::Definitions(_)
        | Schema::EmbeddedJson(_) => (41, String::new()),
    }
}

pub(crate) fn bare_nominal_name(name: &str) -> &str {
    let start = name.rfind('.').map_or(0, |index| index + 1);
    &name[start..]
}

const fn literal_sort_key(kind: LiteralKind) -> (u8, String) {
    match kind {
        LiteralKind::None => (0, String::new()),
        LiteralKind::Bool => (1, String::new()),
        LiteralKind::Integer => (2, String::new()),
        LiteralKind::String => (5, String::new()),
        LiteralKind::Bytes => (6, String::new()),
    }
}

fn literal_kind_identity(kind: LiteralKind) -> ShapeIdentity {
    primitive(match kind {
        LiteralKind::None => "None",
        LiteralKind::Bool => "bool",
        LiteralKind::Integer => "int",
        LiteralKind::String => "str",
        LiteralKind::Bytes => "bytes",
    })
}

fn verify_choices(choices: &[UnionChoice]) -> Result<(), ValidationError> {
    if choices.is_empty() {
        return Err(schema_error(
            "A union schema must declare at least one choice",
            "nonempty union choices",
        ));
    }
    let mut labels = BTreeSet::new();
    if choices
        .iter()
        .any(|choice| choice.label.is_empty() || !labels.insert(choice.label))
    {
        return Err(schema_error(
            "Union choice labels must be nonempty and unique",
            "unique union choice labels",
        ));
    }
    Ok(())
}

fn schema_error(message: &'static str, expected: &'static str) -> ValidationError {
    ValidationError::one(ErrorDetail::new("schema_invalid", message).expected(expected))
}

#[cfg(test)]
mod tests {
    use sifr_runtime::interop::structural::{primitive, union as structural_union};

    use super::*;
    use crate::validation::{
        CollectionConstraints, EnumSchema, EnumVariant, ExtraPolicy, LiteralValue, ModelSchema,
        RelativeTimeConstraint, TemporalKind, TemporalSchema, UnionChoice, UnionMode, UnionSchema,
    };

    #[test]
    fn canonical_layout_matches_compiler_order_for_nominal_temporal_and_enum_members() {
        let model = Schema::Model(
            ModelSchema::new(
                "tests.Record",
                primitive("tests.Record"),
                Vec::new(),
                ExtraPolicy::Ignore,
                false,
                true,
            )
            .unwrap_or_else(|error| panic!("model schema failed: {error}")),
        );
        let frozen = Schema::FrozenSet {
            item: Box::new(Schema::Bool),
            constraints: CollectionConstraints::default(),
        };
        let temporal = Schema::Temporal(TemporalSchema {
            kind: TemporalKind::DateTime,
            relative: Some(RelativeTimeConstraint::Past),
        });
        let enumeration = Schema::Enum(
            EnumSchema::new(
                "tests.Status",
                vec![EnumVariant {
                    name: "Ready",
                    input: LiteralValue::String("ready".to_owned()),
                    discriminant: 1,
                }],
            )
            .unwrap_or_else(|error| panic!("enum schema failed: {error}")),
        );
        let identities = [&model, &frozen, &temporal, &enumeration].map(|schema| {
            schema
                .structural_identity_at(0)
                .unwrap_or_else(|error| panic!("schema identity failed: {error}"))
        });
        let build = |schemas: Vec<Schema>| {
            UnionSchema::new(
                schemas
                    .into_iter()
                    .enumerate()
                    .map(|(index, schema)| UnionChoice {
                        label: ["model", "frozen", "temporal", "enum"][index],
                        schema,
                    })
                    .collect(),
                UnionMode::Smart,
                true,
                None,
            )
            .unwrap_or_else(|error| panic!("union schema failed: {error}"))
        };
        let forward = build(vec![
            model.clone(),
            frozen.clone(),
            temporal.clone(),
            enumeration.clone(),
        ]);
        let reverse = build(vec![enumeration, temporal, frozen, model]);
        assert_eq!(forward.layout().members(), reverse.layout().members());

        let positions = identities.map(|identity| {
            forward
                .layout()
                .index_of(identity)
                .unwrap_or_else(|| panic!("union member is missing"))
        });
        assert!(positions[0] < positions[2]);
        assert!(positions[1] < positions[2]);
        assert!(positions[2] < positions[3]);
    }

    #[test]
    fn canonical_class_order_uses_bare_names_across_modules_and_frozen_sets() {
        let model = |name: &'static str| {
            Schema::Model(
                ModelSchema::new(
                    name,
                    primitive(name),
                    Vec::new(),
                    ExtraPolicy::Ignore,
                    false,
                    true,
                )
                .unwrap_or_else(|error| panic!("model schema failed: {error}")),
            )
        };
        let alpha = model("zoo.Alpha");
        let beta = model("main.Beta");
        let record = model("tests.Record");
        let frozen = Schema::FrozenSet {
            item: Box::new(Schema::Bool),
            constraints: CollectionConstraints::default(),
        };
        let layout = CanonicalSumLayout::from_schemas([&beta, &alpha, &frozen, &record], 0)
            .unwrap_or_else(|error| panic!("canonical layout failed: {error}"));
        let position = |schema: &Schema| {
            let identity = schema
                .structural_identity_at(0)
                .unwrap_or_else(|error| panic!("schema identity failed: {error}"));
            layout
                .index_of(identity)
                .unwrap_or_else(|| panic!("union member is missing"))
        };

        assert!(position(&alpha) < position(&beta));
        assert!(position(&record) < position(&frozen));
        let expected = [&alpha, &beta, &record, &frozen].map(|schema| {
            schema
                .structural_identity_at(0)
                .unwrap_or_else(|error| panic!("schema identity failed: {error}"))
        });
        assert_eq!(layout.identity(), structural_union(&expected));
    }

    #[test]
    fn canonical_enum_order_uses_bare_names_across_modules() {
        let enumeration = |name: &'static str, input: &'static str, discriminant: i64| {
            Schema::Enum(
                EnumSchema::new(
                    name,
                    vec![EnumVariant {
                        name: "Only",
                        input: LiteralValue::String(input.to_owned()),
                        discriminant,
                    }],
                )
                .unwrap_or_else(|error| panic!("enum schema failed: {error}")),
            )
        };
        let alpha = enumeration("zoo.Alpha", "alpha", 1);
        let beta = enumeration("main.Beta", "beta", 2);
        let layout = CanonicalSumLayout::from_schemas([&beta, &alpha], 0)
            .unwrap_or_else(|error| panic!("canonical layout failed: {error}"));
        let expected = [&alpha, &beta].map(|schema| {
            schema
                .structural_identity_at(0)
                .unwrap_or_else(|error| panic!("schema identity failed: {error}"))
        });

        assert_eq!(layout.identity(), structural_union(&expected));
    }
}
