use std::collections::{BTreeMap, HashMap, HashSet};

use pydantic_sifr_core::{
    ExtraPolicy, InputProfile, IntegerConstraints, IntegerTarget, JsonLimits, ModelField,
    ModelSchema, NativeValue, PreparedSchema, Schema, StringConstraints, ValidatedArena,
    ValidationOptions, validate_json_and_construct, validate_native_and_construct,
    validate_strings_and_construct,
};
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    ConstructToken, NodeId, NominalField, ShapeIdentity, StructuralConstruct,
    StructuralContractError, StructuralEdgeKind, StructuralKind, StructuralSource, StructuralType,
    metadata, nominal_record, unary_container,
};

#[derive(Debug, Eq, PartialEq)]
struct User {
    id: SifrInt,
    name: String,
    note: Option<String>,
}

impl StructuralType for User {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "User",
            &[],
            &[
                nominal_field::<SifrInt>("id"),
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
            id: SifrInt::structural_construct_at(source, nodes[0], token)?,
            name: String::structural_construct_at(source, nodes[1], token)?,
            note: Option::<String>::structural_construct_at(source, nodes[2], token)?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct WithExtras {
    id: SifrInt,
    extras: HashMap<String, SifrInt>,
}

#[derive(Debug, Eq, PartialEq)]
struct FrozenStrings(HashSet<String>);

impl StructuralType for FrozenStrings {
    fn shape_identity() -> ShapeIdentity {
        unary_container("frozenset", String::shape_identity())
    }
}

impl StructuralConstruct for FrozenStrings {
    fn structural_construct_at<Source: StructuralSource>(
        source: &mut Source,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let description = source.node(node)?;
        if description.kind() != StructuralKind::FrozenSet {
            return Err(StructuralContractError::KindMismatch);
        }
        let nodes = description
            .edges()
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                if edge.kind() == StructuralEdgeKind::Index(index) {
                    Ok(edge.node())
                } else {
                    Err(StructuralContractError::MemberMismatch)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes
            .into_iter()
            .map(|node| String::structural_construct_at(source, node, token))
            .collect::<Result<HashSet<_>, _>>()
            .map(Self)
    }
}

impl StructuralType for WithExtras {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "WithExtras",
            &[],
            &[
                nominal_field::<SifrInt>("id"),
                nominal_field::<HashMap<String, SifrInt>>("extras"),
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
            id: SifrInt::structural_construct_at(source, nodes[0], token)?,
            extras: HashMap::<String, SifrInt>::structural_construct_at(source, nodes[1], token)?,
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
                ModelField::required("id", Schema::exact_integer()),
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
            id: SifrInt::from_i64(1),
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
    assert_eq!(native.id, SifrInt::from_i64(2));
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
    assert_eq!(strings.id, SifrInt::from_i64(3));
    assert_eq!(strings.name, "Sam");
}

#[test]
fn typed_extra_destination_constructs_without_an_intermediate_model_tree() {
    let schema = Schema::Model(
        ModelSchema::new(
            "WithExtras",
            WithExtras::shape_identity(),
            vec![
                ModelField::required("id", Schema::exact_integer()),
                ModelField {
                    name: "extras",
                    schema: Schema::Mapping {
                        key: Box::new(Schema::String(StringConstraints::default())),
                        value: Box::new(Schema::exact_integer()),
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
                value_schema: Box::new(Schema::exact_integer()),
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
    assert_eq!(value.id, SifrInt::from_i64(4));
    assert_eq!(value.extras["score"], SifrInt::from_i64(8));
    assert_eq!(value.extras["rank"], SifrInt::from_i64(9));
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
    let frozen = validate_json_and_construct::<FrozenStrings>(
        &prepared(&frozen_schema),
        br#"["a","b","a"]"#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("frozenset construction failed: {error}"));
    assert_eq!(frozen.0.len(), 2);
    assert_ne!(
        <HashSet<String> as StructuralType>::shape_identity(),
        FrozenStrings::shape_identity()
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
