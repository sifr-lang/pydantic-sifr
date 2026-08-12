use std::collections::{BTreeMap, HashMap};

use pydantic_sifr_core::{
    ExtraPolicy, InputProfile, JsonLimits, ModelField, ModelSchema, NativeValue, Schema,
    StringConstraints, ValidatedArena, ValidationOptions, validate_json_and_construct,
    validate_native_and_construct, validate_strings_and_construct,
};
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    ConstructToken, NodeId, NominalField, ShapeIdentity, StructuralConstruct,
    StructuralContractError, StructuralEdgeKind, StructuralKind, StructuralSource, StructuralType,
    metadata, nominal_record,
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
    Schema::Model(ModelSchema {
        name: "User",
        fields: vec![
            ModelField::required("id", Schema::exact_integer()),
            ModelField::required("name", Schema::String(StringConstraints::default())),
            ModelField::required(
                "note",
                Schema::Nullable(Box::new(Schema::String(StringConstraints::default()))),
            ),
        ],
        extra: ExtraPolicy::Ignore,
        populate_by_name: false,
        location_by_alias: true,
    })
}

#[test]
fn json_native_and_strings_entry_points_construct_the_target_directly() {
    fn assert_structural_source<T: StructuralSource>() {}
    assert_structural_source::<ValidatedArena>();

    let json = validate_json_and_construct::<User>(
        &user_schema(),
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
        &user_schema(),
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
        &user_schema(),
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
    let schema = Schema::Model(ModelSchema {
        name: "WithExtras",
        fields: vec![
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
        extra: ExtraPolicy::Allow {
            destination: "extras",
            value_schema: Box::new(Schema::exact_integer()),
        },
        populate_by_name: false,
        location_by_alias: true,
    });
    let value = validate_json_and_construct::<WithExtras>(
        &schema,
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
