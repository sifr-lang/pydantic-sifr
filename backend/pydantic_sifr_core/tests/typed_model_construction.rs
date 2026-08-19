use std::collections::{BTreeMap, HashMap, HashSet};

use pydantic_sifr_core::{
    ExtraPolicy, InputProfile, IntegerConstraints, IntegerTarget, JsonIntegerProfile, JsonLimits,
    JsonSchemaMode, JsonSchemaOptions, ModelField, ModelSchema, NativeValue, PreparedSchema,
    Schema, SerializationOptions, SerializationPlan, StringConstraints, ValidatedArena,
    ValidationOptions, generate_json_schema, serialize_json, serialize_structural,
    validate_json_and_construct, validate_native_and_construct, validate_strings_and_construct,
    validate_structural_strings_and_construct,
};
use sifr_runtime::interop::structural::{
    ConstructToken, NodeId, NominalField, ShapeIdentity, StructuralConstruct,
    StructuralContractError, StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind,
    StructuralProject, StructuralSource, StructuralType, StructuralVisitor, VisitControl, metadata,
    nominal_record, unary_container,
};

#[derive(Debug, Eq, PartialEq)]
struct User {
    id: i64,
    name: String,
    note: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct IntRoot {
    root: i64,
}

impl StructuralType for IntRoot {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "IntRoot",
            &[],
            &[nominal_field::<i64>("root")],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for IntRoot {
    fn structural_construct_at<Source: StructuralSource>(
        source: &mut Source,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = record_nodes(source, node, "IntRoot", &["root"])?;
        Ok(Self {
            root: i64::structural_construct_at(source, nodes[0], token)?,
        })
    }
}

impl StructuralProject for IntRoot {
    fn structural_project<'value, V: StructuralVisitor<'value>>(
        &'value self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        let control = visitor.enter(StructuralEnter::new(
            StructuralKind::Record,
            Some("IntRoot"),
            1,
        ))?;
        if control == VisitControl::Continue {
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField("root")))?;
            self.root.structural_project(visitor)?;
        }
        visitor.exit(StructuralKind::Record)
    }
}

struct UserStringInput {
    id: String,
    name: String,
    note: String,
}

impl StructuralType for UserStringInput {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "UserStringInput",
            &[],
            &[
                nominal_field::<String>("id"),
                nominal_field::<String>("name"),
                nominal_field::<String>("note"),
            ],
            metadata(&[]),
        )
    }
}

impl StructuralProject for UserStringInput {
    fn structural_project<'value, V: StructuralVisitor<'value>>(
        &'value self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        let control = visitor.enter(StructuralEnter::new(
            StructuralKind::Record,
            Some("UserStringInput"),
            3,
        ))?;
        if control == VisitControl::Continue {
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField("id")))?;
            self.id.structural_project(visitor)?;
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField("name")))?;
            self.name.structural_project(visitor)?;
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField("note")))?;
            self.note.structural_project(visitor)?;
        }
        visitor.exit(StructuralKind::Record)
    }
}

impl StructuralType for User {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "User",
            &[],
            &[
                nominal_field::<i64>("id"),
                nominal_field::<String>("name"),
                nominal_field::<Option<String>>("note"),
            ],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for User {
    fn structural_construct_at<Source: StructuralSource>(
        source: &mut Source,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = record_nodes(source, node, "User", &["id", "name", "note"])?;
        Ok(Self {
            id: i64::structural_construct_at(source, nodes[0], token)?,
            name: String::structural_construct_at(source, nodes[1], token)?,
            note: Option::<String>::structural_construct_at(source, nodes[2], token)?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct WithExtras {
    id: i64,
    extras: HashMap<String, i64>,
}

#[derive(Debug, Eq, PartialEq)]
struct SifrFrozenStrings {
    values: HashSet<String>,
}

impl StructuralType for SifrFrozenStrings {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "sifr.collections.frozenset",
            &[String::shape_identity()],
            &[NominalField {
                name: "_values",
                identity: unary_container("set", String::shape_identity()),
                required: true,
                default_identity: None,
            }],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for SifrFrozenStrings {
    fn structural_construct_at<Source: StructuralSource>(
        source: &mut Source,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = record_nodes(source, node, "sifr.collections.frozenset", &["_values"])?;
        Ok(Self {
            values: HashSet::<String>::structural_construct_at(source, nodes[0], token)?,
        })
    }
}

impl StructuralType for WithExtras {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "WithExtras",
            &[],
            &[
                nominal_field::<i64>("id"),
                nominal_field::<HashMap<String, i64>>("extras"),
            ],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for WithExtras {
    fn structural_construct_at<Source: StructuralSource>(
        source: &mut Source,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = record_nodes(source, node, "WithExtras", &["id", "extras"])?;
        Ok(Self {
            id: i64::structural_construct_at(source, nodes[0], token)?,
            extras: HashMap::<String, i64>::structural_construct_at(source, nodes[1], token)?,
        })
    }
}

fn nominal_field<T: StructuralType>(name: &'static str) -> NominalField<'static> {
    NominalField {
        name,
        identity: T::shape_identity(),
        required: true,
        default_identity: None,
    }
}

fn prepared(schema: &Schema) -> PreparedSchema<'_> {
    PreparedSchema::new(schema).unwrap_or_else(|error| panic!("schema preparation failed: {error}"))
}

fn record_nodes<Source: StructuralSource>(
    source: &Source,
    node: NodeId,
    identity: &str,
    fields: &[&str],
) -> Result<Vec<NodeId>, StructuralContractError> {
    let description = source.node(node)?;
    if description.kind() != StructuralKind::Record {
        return Err(StructuralContractError::KindMismatch);
    }
    if description.nominal_identity() != Some(identity) {
        return Err(StructuralContractError::MemberMismatch);
    }
    if description.edges().len() != fields.len() {
        return Err(StructuralContractError::ArityMismatch);
    }
    for (index, field) in fields.iter().enumerate() {
        if description.edges()[index].kind() != StructuralEdgeKind::RecordField(field) {
            return Err(StructuralContractError::MemberMismatch);
        }
    }
    Ok(description
        .edges()
        .iter()
        .map(sifr_runtime::interop::structural::StructuralNodeEdge::node)
        .collect())
}

fn user_schema() -> Schema {
    Schema::Model(
        ModelSchema::new(
            "User",
            User::shape_identity(),
            vec![
                ModelField::required(
                    "id",
                    Schema::Integer {
                        target: IntegerTarget::I64,
                        constraints: IntegerConstraints::default(),
                    },
                ),
                ModelField::required("name", Schema::String(StringConstraints::default())),
                ModelField::required(
                    "note",
                    Schema::Nullable(Box::new(Schema::String(StringConstraints::default()))),
                ),
            ],
            ExtraPolicy::Ignore,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("user schema failed: {error}")),
    )
}

#[test]
fn json_native_and_strings_entry_points_construct_the_target_directly() {
    fn assert_structural_source<T: StructuralSource>() {}
    assert_structural_source::<ValidatedArena>();

    let schema = user_schema();
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let json = validate_json_and_construct::<User>(
        &prepared,
        br#"{"id":1,"name":"Ada","note":null}"#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("JSON construction failed: {error}"));
    assert_eq!(
        json,
        User {
            id: 1,
            name: "Ada".to_owned(),
            note: None,
        }
    );

    let native = validate_native_and_construct::<User>(
        &prepared,
        &NativeValue::Object(vec![
            ("id".to_owned(), NativeValue::Integer("2".to_owned())),
            ("name".to_owned(), NativeValue::String("Lin".to_owned())),
            ("note".to_owned(), NativeValue::String("native".to_owned())),
        ]),
        JsonLimits::default(),
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("native construction failed: {error}"));
    assert_eq!(native.id, 2);
    assert_eq!(native.note.as_deref(), Some("native"));

    let strings = validate_strings_and_construct::<User>(
        &prepared,
        &NativeValue::Object(vec![
            ("id".to_owned(), NativeValue::String("3".to_owned())),
            ("name".to_owned(), NativeValue::String("Sam".to_owned())),
            ("note".to_owned(), NativeValue::String("text".to_owned())),
        ]),
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("strings construction failed: {error}"));
    assert_eq!(strings.id, 3);
    assert_eq!(strings.name, "Sam");
}

#[test]
fn string_structural_entry_point_constructs_bare_and_nested_targets() {
    let integer = Schema::Integer {
        target: IntegerTarget::I64,
        constraints: IntegerConstraints::default(),
    };
    let integer = prepared(&integer);
    let bare = validate_structural_strings_and_construct::<i64, String>(
        &integer,
        &"41".to_owned(),
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("bare string construction failed: {error}"));
    assert_eq!(bare, 41);

    let user_schema = user_schema();
    let user_schema = prepared(&user_schema);
    let input = UserStringInput {
        id: "42".to_owned(),
        name: "Ada".to_owned(),
        note: "ready".to_owned(),
    };
    let user = validate_structural_strings_and_construct::<User, _>(
        &user_schema,
        &input,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("nested string construction failed: {error}"));
    assert_eq!(user.id, 42);
    assert_eq!(user.note.as_deref(), Some("ready"));
}

#[test]
fn root_model_uses_scalar_validation_serialization_and_json_schema() {
    let schema = Schema::Model(
        ModelSchema::new_root(
            "IntRoot",
            IntRoot::shape_identity(),
            ModelField::required(
                "root",
                Schema::Integer {
                    target: IntegerTarget::I64,
                    constraints: IntegerConstraints::default(),
                },
            ),
        )
        .unwrap_or_else(|error| panic!("root model schema failed: {error}")),
    );
    let prepared = prepared(&schema);
    let value = validate_structural_strings_and_construct::<IntRoot, String>(
        &prepared,
        &"41".to_owned(),
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("root model validation failed: {error}"));
    assert_eq!(value, IntRoot { root: 41 });

    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("root model plan failed: {error}"));
    let options = SerializationOptions::default();
    assert_eq!(
        serialize_structural(&plan, &value, &options)
            .unwrap_or_else(|error| panic!("root model projection failed: {error}")),
        NativeValue::Integer("41".to_owned()),
    );
    assert_eq!(
        serialize_json(&plan, &value, &options)
            .unwrap_or_else(|error| panic!("root model JSON failed: {error}")),
        b"41",
    );

    let document = generate_json_schema(
        &schema,
        JsonSchemaOptions::new(JsonSchemaMode::Validation, true),
        JsonIntegerProfile::Exact,
    )
    .unwrap_or_else(|error| panic!("root model JSON Schema failed: {error}"));
    assert_eq!(document["type"], "integer");
    assert!(document.get("properties").is_none());
}

#[test]
fn typed_extra_destination_constructs_without_an_intermediate_model_tree() {
    let schema = Schema::Model(
        ModelSchema::new(
            "WithExtras",
            WithExtras::shape_identity(),
            vec![
                ModelField::required(
                    "id",
                    Schema::Integer {
                        target: IntegerTarget::I64,
                        constraints: IntegerConstraints::default(),
                    },
                ),
                ModelField {
                    name: "extras",
                    schema: Schema::Mapping {
                        key: Box::new(Schema::String(StringConstraints::default())),
                        value: Box::new(Schema::Integer {
                            target: IntegerTarget::I64,
                            constraints: IntegerConstraints::default(),
                        }),
                        constraints: pydantic_sifr_core::CollectionConstraints::default(),
                    },
                    input: false,
                    default: None,
                    validation_aliases: Vec::new(),
                    metadata: BTreeMap::new(),
                },
            ],
            ExtraPolicy::Allow {
                destination: "extras",
                value_schema: Box::new(Schema::Integer {
                    target: IntegerTarget::I64,
                    constraints: IntegerConstraints::default(),
                }),
            },
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("extra schema failed: {error}")),
    );
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let value = validate_json_and_construct::<WithExtras>(
        &prepared,
        br#"{"id":4,"score":8,"rank":9}"#,
        JsonLimits::default(),
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("extra construction failed: {error}"));
    assert_eq!(value.id, 4);
    assert_eq!(value.extras["score"], 8);
    assert_eq!(value.extras["rank"], 9);
}

#[test]
fn structural_shape_mismatch_is_rejected_before_node_construction() {
    let result = validate_json_and_construct::<String>(
        &prepared(&Schema::exact_integer()),
        b"1",
        JsonLimits::default(),
        ValidationOptions::default(),
    );
    let error = match result {
        Ok(_) => panic!("integer schema must not construct a string target"),
        Err(error) => error,
    };
    assert_eq!(error.details()[0].code, "internal_construction");
    assert_eq!(
        error.details()[0].context.get("error").map(String::as_str),
        Some("structural shape identity mismatch")
    );
}

#[test]
fn every_fixed_width_integer_constructs_with_its_declared_width() {
    let cases: &[(IntegerTarget, &str)] = &[
        (IntegerTarget::I8, "-8"),
        (IntegerTarget::I16, "-16"),
        (IntegerTarget::I32, "-32"),
        (IntegerTarget::I64, "-64"),
        (IntegerTarget::U8, "8"),
        (IntegerTarget::U16, "16"),
        (IntegerTarget::U32, "32"),
        (IntegerTarget::U64, "64"),
    ];
    let schema = |target| Schema::Integer {
        target,
        constraints: IntegerConstraints::default(),
    };
    assert_eq!(
        validate_json_and_construct::<i8>(
            &prepared(&schema(cases[0].0)),
            cases[0].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("i8 construction failed: {error}")),
        -8
    );
    assert_eq!(
        validate_json_and_construct::<i16>(
            &prepared(&schema(cases[1].0)),
            cases[1].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("i16 construction failed: {error}")),
        -16
    );
    assert_eq!(
        validate_json_and_construct::<i32>(
            &prepared(&schema(cases[2].0)),
            cases[2].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("i32 construction failed: {error}")),
        -32
    );
    assert_eq!(
        validate_json_and_construct::<i64>(
            &prepared(&schema(cases[3].0)),
            cases[3].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("i64 construction failed: {error}")),
        -64
    );
    assert_eq!(
        validate_json_and_construct::<u8>(
            &prepared(&schema(cases[4].0)),
            cases[4].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("u8 construction failed: {error}")),
        8
    );
    assert_eq!(
        validate_json_and_construct::<u16>(
            &prepared(&schema(cases[5].0)),
            cases[5].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("u16 construction failed: {error}")),
        16
    );
    assert_eq!(
        validate_json_and_construct::<u32>(
            &prepared(&schema(cases[6].0)),
            cases[6].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("u32 construction failed: {error}")),
        32
    );
    assert_eq!(
        validate_json_and_construct::<u64>(
            &prepared(&schema(cases[7].0)),
            cases[7].1.as_bytes(),
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("u64 construction failed: {error}")),
        64
    );
}

#[test]
fn set_and_frozenset_have_distinct_satisfiable_contracts() {
    let set_schema = Schema::Set {
        item: Box::new(Schema::String(StringConstraints::default())),
        constraints: pydantic_sifr_core::CollectionConstraints::default(),
    };
    let set = validate_json_and_construct::<HashSet<String>>(
        &prepared(&set_schema),
        br#"["a","b","a"]"#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("set construction failed: {error}"));
    assert_eq!(set.len(), 2);

    let frozen_schema = Schema::FrozenSet {
        item: Box::new(Schema::String(StringConstraints::default())),
        constraints: pydantic_sifr_core::CollectionConstraints::default(),
    };
    let frozen = validate_json_and_construct::<SifrFrozenStrings>(
        &prepared(&frozen_schema),
        br#"["a","b","a"]"#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("frozenset construction failed: {error}"));
    assert_eq!(frozen.values.len(), 2);
    assert_ne!(
        <HashSet<String> as StructuralType>::shape_identity(),
        SifrFrozenStrings::shape_identity()
    );
}

#[test]
fn prepared_schema_rejects_unbounded_static_nesting() {
    let mut schema = Schema::String(StringConstraints::default());
    for _ in 0..258 {
        schema = Schema::List {
            item: Box::new(schema),
            constraints: pydantic_sifr_core::CollectionConstraints::default(),
        };
    }
    let error = PreparedSchema::new(&schema)
        .err()
        .unwrap_or_else(|| panic!("deep static schema must fail preparation"));
    assert_eq!(error.details()[0].code, "schema_invalid");
}
