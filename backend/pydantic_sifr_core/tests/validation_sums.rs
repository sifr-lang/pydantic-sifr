use std::sync::atomic::{AtomicUsize, Ordering};

use num_bigint::BigInt;
use pydantic_sifr_core::{
    AliasSegment, CollectionConstraints, DiscriminatorPath, EnumSchema, EnumVariant, ExtraPolicy,
    FieldDefault, InputProfile, JsonLimits, LiteralSchema, LiteralValue, LocationItem, ModelField,
    ModelSchema, NativeValue, PreparedSchema, Schema, SchemaErrorOverride, StringConstraints,
    TaggedUnionChoice, TaggedUnionSchema, UnionChoice, UnionMode, UnionSchema, ValidatedArena,
    ValidatedValue, ValidationError, ValidationLimits, ValidationOptions, parse_json, validate,
    validate_json_and_construct,
};
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    ConstructToken, NodeId, ShapeIdentity, StructuralConstruct, StructuralContractError,
    StructuralEdgeKind, StructuralKind, StructuralSource, StructuralType, enum_shape, metadata,
    primitive, union as structural_union,
};

#[derive(Debug, Eq, PartialEq)]
enum Status {
    Ready,
    Large,
}

impl StructuralType for Status {
    fn shape_identity() -> ShapeIdentity {
        enum_shape(
            "tests.Status",
            &[("Ready", Some(4)), ("Large", Some(9))],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for Status {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let description = source.node(node)?;
        if description.kind() != StructuralKind::Enum
            || description.nominal_identity() != Some("tests.Status")
        {
            return Err(StructuralContractError::KindMismatch);
        }
        let [edge] = description.edges() else {
            return Err(StructuralContractError::ArityMismatch);
        };
        let child = edge.node();
        let variant = match edge.kind() {
            StructuralEdgeKind::ActiveMember {
                name: "Ready",
                index: 0,
            } => 0,
            StructuralEdgeKind::ActiveMember {
                name: "Large",
                index: 1,
            } => 1,
            _ => return Err(StructuralContractError::MemberMismatch),
        };
        let value = i64::structural_construct_at(source, child, token)?;
        match (variant, value) {
            (0, 4) => Ok(Self::Ready),
            (1, 9) => Ok(Self::Large),
            _ => Err(StructuralContractError::MemberMismatch),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum IntOrString {
    Integer(SifrInt),
    String(String),
}

static DEFAULT_CALLS: AtomicUsize = AtomicUsize::new(0);
static SHORT_CIRCUIT_CALLS: AtomicUsize = AtomicUsize::new(0);

fn counted_default() -> NativeValue {
    DEFAULT_CALLS.fetch_add(1, Ordering::SeqCst);
    NativeValue::String("default".to_owned())
}

fn short_circuit_default() -> NativeValue {
    SHORT_CIRCUIT_CALLS.fetch_add(1, Ordering::SeqCst);
    NativeValue::String("default".to_owned())
}

impl StructuralType for IntOrString {
    fn shape_identity() -> ShapeIdentity {
        structural_union(&[SifrInt::shape_identity(), String::shape_identity()])
    }
}

impl StructuralConstruct for IntOrString {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let description = source.node(node)?;
        if description.kind() != StructuralKind::Union || description.nominal_identity().is_some() {
            return Err(StructuralContractError::KindMismatch);
        }
        let [edge] = description.edges() else {
            return Err(StructuralContractError::ArityMismatch);
        };
        let child = edge.node();
        let index = match edge.kind() {
            StructuralEdgeKind::ActiveMember {
                name: "member",
                index: 0,
            } => Ok(0),
            StructuralEdgeKind::ActiveMember {
                name: "member",
                index: 1,
            } => Ok(1),
            _ => Err(StructuralContractError::MemberMismatch),
        }?;
        match index {
            0 => SifrInt::structural_construct_at(source, child, token).map(Self::Integer),
            1 => String::structural_construct_at(source, child, token).map(Self::String),
            _ => Err(StructuralContractError::MemberMismatch),
        }
    }
}

fn literal(values: Vec<LiteralValue>) -> Schema {
    Schema::Literal(
        LiteralSchema::new(values).unwrap_or_else(|error| panic!("literal schema failed: {error}")),
    )
}

fn choice(label: &'static str, schema: Schema) -> UnionChoice {
    UnionChoice { label, schema }
}

fn union(choices: Vec<UnionChoice>, mode: UnionMode, auto_collapse: bool) -> Schema {
    Schema::Union(
        UnionSchema::new(choices, mode, auto_collapse, None)
            .unwrap_or_else(|error| panic!("union schema failed: {error}")),
    )
}

fn required(name: &'static str, schema: Schema) -> ModelField {
    ModelField::required(name, schema)
}

fn model(name: &'static str, fields: Vec<ModelField>) -> Schema {
    Schema::Model(
        ModelSchema::new(
            name,
            primitive(name),
            fields,
            ExtraPolicy::Ignore,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("model schema failed: {error}")),
    )
}

fn json(schema: &Schema, input: &[u8]) -> Result<ValidatedArena, ValidationError> {
    json_with_options(schema, input, ValidationOptions::default())
}

fn json_with_options(
    schema: &Schema,
    input: &[u8],
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    let input = parse_json(input, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    validate(
        schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..options
        },
    )
}

fn require_error(result: Result<ValidatedArena, ValidationError>) -> ValidationError {
    match result {
        Ok(_) => panic!("expected validation error"),
        Err(error) => error,
    }
}

fn union_root(arena: &ValidatedArena) -> &pydantic_sifr_core::UnionValue {
    let Some(ValidatedValue::Union(value)) = arena.get(arena.root()) else {
        panic!("expected union root");
    };
    value
}

#[test]
fn literals_preserve_exact_bool_integer_and_mixed_type_members() {
    let schema = literal(vec![
        LiteralValue::Bool(true),
        LiteralValue::Integer(BigInt::from(1)),
        LiteralValue::String("one".to_owned()),
    ]);
    let boolean = json(&schema, b"true").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&boolean).index(), 0);
    assert!(matches!(
        boolean.get(union_root(&boolean).value()),
        Some(ValidatedValue::Bool(true))
    ));

    let integer = json(&schema, b"1").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&integer).index(), 1);
    assert!(matches!(
        integer.get(union_root(&integer).value()),
        Some(ValidatedValue::ExactInt(value)) if *value == BigInt::from(1)
    ));

    let error = require_error(json(&schema, b"false"));
    assert_eq!(error.details()[0].code, "literal_error");
}

#[test]
fn enum_inputs_map_string_and_large_integer_metadata_to_sifr_tags() {
    let large = BigInt::from(i64::MAX) + BigInt::from(10_u8);
    let schema = Schema::Enum(
        EnumSchema::new(
            "tests.Status",
            vec![
                EnumVariant {
                    name: "Ready",
                    input: LiteralValue::String("ready".to_owned()),
                    discriminant: 4,
                },
                EnumVariant {
                    name: "Large",
                    input: LiteralValue::Integer(large.clone()),
                    discriminant: 9,
                },
            ],
        )
        .unwrap_or_else(|error| panic!("enum schema failed: {error}")),
    );
    let ready = json(&schema, br#""ready""#).unwrap_or_else(|error| panic!("{error}"));
    let Some(ValidatedValue::Enum(value)) = ready.get(ready.root()) else {
        panic!("expected enum root");
    };
    assert_eq!(value.name(), "tests.Status");
    assert_eq!(value.variant(), "Ready");
    assert_eq!(value.index(), 0);

    let large =
        json(&schema, large.to_string().as_bytes()).unwrap_or_else(|error| panic!("{error}"));
    let Some(ValidatedValue::Enum(value)) = large.get(large.root()) else {
        panic!("expected enum root");
    };
    assert_eq!(value.variant(), "Large");
    assert_eq!(value.index(), 1);
}

#[test]
fn validated_enum_and_union_values_construct_typed_structural_targets() {
    let enumeration = Schema::Enum(
        EnumSchema::new(
            "tests.Status",
            vec![
                EnumVariant {
                    name: "Ready",
                    input: LiteralValue::String("ready".to_owned()),
                    discriminant: 4,
                },
                EnumVariant {
                    name: "Large",
                    input: LiteralValue::Integer(BigInt::from(i64::MAX) + BigInt::from(10_u8)),
                    discriminant: 9,
                },
            ],
        )
        .unwrap_or_else(|error| panic!("enum schema failed: {error}")),
    );
    let prepared = PreparedSchema::new(&enumeration)
        .unwrap_or_else(|error| panic!("enum preparation failed: {error}"));
    let output = validate_json_and_construct::<Status>(
        &prepared,
        br#""ready""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("enum construction failed: {error}"));
    assert_eq!(output, Status::Ready);

    let schema = union(
        vec![
            choice("string", Schema::String(StringConstraints::default())),
            choice("integer", Schema::exact_integer()),
        ],
        UnionMode::Smart,
        true,
    );
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("union preparation failed: {error}"));
    let output = validate_json_and_construct::<IntOrString>(
        &prepared,
        br#""value""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("union construction failed: {error}"));
    assert_eq!(output, IntOrString::String("value".to_owned()));
}

#[test]
fn canonical_sum_layout_constructs_optional_singleton_and_duplicate_targets() {
    let optional = union(
        vec![
            choice("integer", Schema::exact_integer()),
            choice("none", Schema::None),
        ],
        UnionMode::Smart,
        false,
    );
    let prepared = PreparedSchema::new(&optional)
        .unwrap_or_else(|error| panic!("optional preparation failed: {error}"));
    let none = validate_json_and_construct::<Option<SifrInt>>(
        &prepared,
        b"null",
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("optional construction failed: {error}"));
    assert_eq!(none, None);
    let some = validate_json_and_construct::<Option<SifrInt>>(
        &prepared,
        b"7",
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("optional construction failed: {error}"));
    assert_eq!(some, Some(SifrInt::from(7_i64)));

    for schema in [
        union(
            vec![choice("integer", Schema::exact_integer())],
            UnionMode::Smart,
            false,
        ),
        union(
            vec![
                choice("first", Schema::exact_integer()),
                choice("second", Schema::exact_integer()),
            ],
            UnionMode::Smart,
            false,
        ),
    ] {
        let prepared = PreparedSchema::new(&schema)
            .unwrap_or_else(|error| panic!("direct preparation failed: {error}"));
        let value = validate_json_and_construct::<SifrInt>(
            &prepared,
            b"11",
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("direct construction failed: {error}"));
        assert_eq!(value, SifrInt::from(11_i64));
    }
}

#[test]
fn smart_union_prefers_exact_type_while_left_to_right_prefers_first_success() {
    let choices = || {
        vec![
            choice("integer", Schema::exact_integer()),
            choice("boolean", Schema::Bool),
        ]
    };
    let smart = json(&union(choices(), UnionMode::Smart, true), b"true")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&smart).index(), 0);

    let ordered = json(&union(choices(), UnionMode::LeftToRight, true), b"true")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&ordered).index(), 1);

    let string_or_integer = union(
        vec![
            choice("integer", Schema::exact_integer()),
            choice("string", Schema::String(StringConstraints::default())),
        ],
        UnionMode::Smart,
        true,
    );
    let output = json(&string_or_integer, br#""1""#).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 1);
}

#[test]
fn smart_union_uses_additive_nested_validated_field_counts_and_stable_ties() {
    let shallow = model(
        "tests.Shallow",
        vec![required("id", Schema::exact_integer())],
    );
    let deep = model(
        "tests.Deep",
        vec![
            required("id", Schema::exact_integer()),
            required("name", Schema::String(StringConstraints::default())),
        ],
    );
    let schema = union(
        vec![choice("shallow", shallow), choice("deep", deep)],
        UnionMode::Smart,
        true,
    );
    let output =
        json(&schema, br#"{"id":1,"name":"Ada"}"#).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 0);

    let nested_shallow = model(
        "tests.OuterShallow",
        vec![required(
            "child",
            model(
                "tests.ChildShallow",
                vec![required("id", Schema::exact_integer())],
            ),
        )],
    );
    let nested_deep = model(
        "tests.OuterDeep",
        vec![required(
            "child",
            model(
                "tests.ChildDeep",
                vec![
                    required("id", Schema::exact_integer()),
                    required("name", Schema::String(StringConstraints::default())),
                ],
            ),
        )],
    );
    let nested = union(
        vec![
            choice("outer-shallow", nested_shallow),
            choice("outer-deep", nested_deep),
        ],
        UnionMode::Smart,
        true,
    );
    let output = json(&nested, br#"{"child":{"id":1,"name":"Ada"}}"#)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 0);

    let tied = union(
        vec![
            choice(
                "first",
                model("tests.First", vec![required("id", Schema::exact_integer())]),
            ),
            choice(
                "second",
                model(
                    "tests.Second",
                    vec![required("id", Schema::exact_integer())],
                ),
            ),
        ],
        UnionMode::Smart,
        true,
    );
    let output = json(&tied, br#"{"id":1}"#).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 0);
}

#[test]
fn smart_scoring_does_not_execute_default_factories_twice() {
    DEFAULT_CALLS.store(0, Ordering::SeqCst);
    let mut defaulted = required("name", Schema::String(StringConstraints::default()));
    defaulted.default = Some(FieldDefault::Factory(counted_default));
    let schema = union(
        vec![
            choice(
                "model",
                model(
                    "tests.Defaulted",
                    vec![required("id", Schema::exact_integer()), defaulted],
                ),
            ),
            choice("integer", Schema::exact_integer()),
        ],
        UnionMode::Smart,
        true,
    );
    let output = json(&schema, br#"{"id":1}"#).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 1);
    assert_eq!(DEFAULT_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn smart_scoring_short_circuits_exact_uncounted_branches_before_defaults() {
    SHORT_CIRCUIT_CALLS.store(0, Ordering::SeqCst);
    let mapping = Schema::Mapping {
        key: Box::new(Schema::String(StringConstraints::default())),
        value: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let mut defaulted = required("name", Schema::String(StringConstraints::default()));
    defaulted.default = Some(FieldDefault::Factory(short_circuit_default));
    let schema = union(
        vec![
            choice("mapping", mapping),
            choice("model", model("tests.DefaultOnly", vec![defaulted])),
        ],
        UnionMode::Smart,
        true,
    );
    let output = json(&schema, b"{}").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 0);
    assert_eq!(SHORT_CIRCUIT_CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn smart_scoring_compares_exactness_when_only_one_branch_has_field_counts() {
    let mapping = Schema::Mapping {
        key: Box::new(Schema::String(StringConstraints::default())),
        value: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let lax_model = model(
        "tests.LaxModel",
        vec![required("id", Schema::String(StringConstraints::default()))],
    );
    let schema = union(
        vec![choice("model", lax_model), choice("mapping", mapping)],
        UnionMode::Smart,
        true,
    );
    let output = json(&schema, br#"{"id":1}"#).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 0);
}

#[test]
fn aggregate_union_errors_keep_labels_and_custom_overrides_replace_the_boundary() {
    let choices = vec![
        choice("integer", Schema::exact_integer()),
        choice("boolean", Schema::Bool),
    ];
    let schema = union(choices.clone(), UnionMode::Smart, true);
    let error = require_error(json(&schema, br#""bad""#));
    assert_eq!(error.details().len(), 2);
    assert_eq!(
        error.details()[0].location,
        vec![LocationItem::Branch("integer".to_owned())]
    );
    assert_eq!(
        error.details()[1].location,
        vec![LocationItem::Branch("boolean".to_owned())]
    );

    let schema = Schema::Union(
        UnionSchema::new(
            choices,
            UnionMode::Smart,
            true,
            Some(SchemaErrorOverride {
                code: "example.choice",
                message: "Input must match one declared choice",
            }),
        )
        .unwrap_or_else(|error| panic!("union schema failed: {error}")),
    );
    let error = require_error(json(&schema, br#""bad""#));
    assert_eq!(error.details().len(), 1);
    assert_eq!(error.details()[0].code, "example.choice");
}

#[test]
fn nested_union_errors_keep_field_before_branch_and_respect_error_limit() {
    let schema = model(
        "tests.Envelope",
        vec![required(
            "value",
            union(
                vec![
                    choice("integer", Schema::exact_integer()),
                    choice("boolean", Schema::Bool),
                    choice("none", Schema::None),
                ],
                UnionMode::Smart,
                true,
            ),
        )],
    );
    let limits = ValidationLimits {
        max_errors: 2,
        ..ValidationLimits::default()
    };
    let error = require_error(json_with_options(
        &schema,
        br#"{"value":"bad"}"#,
        ValidationOptions {
            limits,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details().len(), 2);
    assert!(error.is_truncated());
    assert_eq!(
        error.details()[0].location,
        vec![
            LocationItem::Field("value".to_owned()),
            LocationItem::Branch("integer".to_owned()),
        ]
    );
}

#[test]
fn tagged_union_uses_field_and_path_discriminators_with_stable_errors() {
    let cat = model(
        "tests.Cat",
        vec![
            required(
                "kind",
                literal(vec![LiteralValue::String("cat".to_owned())]),
            ),
            required("lives", Schema::exact_integer()),
        ],
    );
    let dog = model(
        "tests.Dog",
        vec![
            required(
                "kind",
                literal(vec![LiteralValue::String("dog".to_owned())]),
            ),
            required("name", Schema::String(StringConstraints::default())),
        ],
    );
    let tagged = Schema::TaggedUnion(
        TaggedUnionSchema::new(
            DiscriminatorPath {
                segments: vec![AliasSegment::Field("kind")],
            },
            vec![
                TaggedUnionChoice {
                    label: "cat",
                    tags: vec![LiteralValue::String("cat".to_owned())],
                    schema: cat,
                },
                TaggedUnionChoice {
                    label: "dog",
                    tags: vec![LiteralValue::String("dog".to_owned())],
                    schema: dog,
                },
            ],
            None,
        )
        .unwrap_or_else(|error| panic!("tagged schema failed: {error}")),
    );
    let output =
        json(&tagged, br#"{"kind":"dog","name":"Fido"}"#).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(union_root(&output).index(), 1);

    let missing = require_error(json(&tagged, br#"{"name":"Fido"}"#));
    assert_eq!(missing.details()[0].code, "union_tag_not_found");

    let invalid = require_error(json(&tagged, br#"{"kind":"bird"}"#));
    assert_eq!(invalid.details()[0].code, "union_tag_invalid");
}

#[test]
fn tagged_union_override_replaces_missing_and_invalid_discriminator_errors() {
    let tagged = Schema::TaggedUnion(
        TaggedUnionSchema::new(
            DiscriminatorPath {
                segments: vec![AliasSegment::Field("kind")],
            },
            vec![TaggedUnionChoice {
                label: "integer",
                tags: vec![LiteralValue::String("integer".to_owned())],
                schema: Schema::Mapping {
                    key: Box::new(Schema::String(StringConstraints::default())),
                    value: Box::new(Schema::exact_integer()),
                    constraints: CollectionConstraints::default(),
                },
            }],
            Some(SchemaErrorOverride {
                code: "example.tag",
                message: "Input must contain a declared tag",
            }),
        )
        .unwrap_or_else(|error| panic!("tagged schema failed: {error}")),
    );
    for input in [br#"{}"#.as_slice(), br#"{"kind":"other"}"#.as_slice()] {
        let error = require_error(json(&tagged, input));
        assert_eq!(error.details().len(), 1);
        assert_eq!(error.details()[0].code, "example.tag");
    }
}

#[test]
fn canonical_union_values_have_stable_set_keys() {
    let schema = Schema::Set {
        item: Box::new(union(
            vec![
                choice("string", Schema::String(StringConstraints::default())),
                choice("integer", Schema::exact_integer()),
            ],
            UnionMode::Smart,
            true,
        )),
        constraints: CollectionConstraints::default(),
    };
    let output = json(&schema, br#"[1,1,"1","1"]"#).unwrap_or_else(|error| panic!("{error}"));
    let Some(ValidatedValue::Set(values)) = output.get(output.root()) else {
        panic!("expected set root");
    };
    assert_eq!(values.len(), 2);
}

#[test]
fn tagged_union_can_read_a_nested_field_and_one_choice_auto_collapse_is_explicit() {
    let nested = model(
        "tests.Nested",
        vec![
            required(
                "meta",
                Schema::Mapping {
                    key: Box::new(Schema::String(StringConstraints::default())),
                    value: Box::new(Schema::String(StringConstraints::default())),
                    constraints: pydantic_sifr_core::CollectionConstraints::default(),
                },
            ),
            required("value", Schema::exact_integer()),
        ],
    );
    let tagged = Schema::TaggedUnion(
        TaggedUnionSchema::new(
            DiscriminatorPath {
                segments: vec![AliasSegment::Field("meta"), AliasSegment::Field("kind")],
            },
            vec![TaggedUnionChoice {
                label: "nested",
                tags: vec![LiteralValue::String("nested".to_owned())],
                schema: nested,
            }],
            None,
        )
        .unwrap_or_else(|error| panic!("tagged schema failed: {error}")),
    );
    let output = json(&tagged, br#"{"meta":{"kind":"nested"},"value":3}"#)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        matches!(output.get(output.root()), Some(ValidatedValue::Model(value)) if value.name() == "tests.Nested")
    );

    let collapsed = union(
        vec![choice("integer", Schema::exact_integer())],
        UnionMode::Smart,
        true,
    );
    let output = json(&collapsed, b"3").unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        output.get(output.root()),
        Some(ValidatedValue::ExactInt(_))
    ));

    let preserved = union(
        vec![choice("integer", Schema::exact_integer())],
        UnionMode::Smart,
        false,
    );
    let output = json(&preserved, b"3").unwrap_or_else(|error| panic!("{error}"));
    assert!(matches!(
        output.get(output.root()),
        Some(ValidatedValue::ExactInt(_))
    ));
}

#[test]
fn invalid_sum_declarations_fail_before_validation() {
    assert!(LiteralSchema::new(Vec::new()).is_err());
    assert!(
        LiteralSchema::new(vec![
            LiteralValue::String("x".to_owned()),
            LiteralValue::String("x".to_owned()),
        ])
        .is_err()
    );
    assert!(UnionSchema::new(Vec::new(), UnionMode::Smart, true, None).is_err());
    assert!(
        TaggedUnionSchema::new(
            DiscriminatorPath {
                segments: Vec::new(),
            },
            Vec::new(),
            None,
        )
        .is_err()
    );

    let duplicate = EnumSchema::new(
        "tests.Status",
        vec![
            EnumVariant {
                name: "One",
                input: LiteralValue::Integer(BigInt::from(1)),
                discriminant: 1,
            },
            EnumVariant {
                name: "Two",
                input: LiteralValue::Integer(BigInt::from(1)),
                discriminant: 2,
            },
        ],
    );
    assert!(duplicate.is_err());
}
