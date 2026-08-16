use pydantic_sifr_core::{
    ExtraPolicy, IntegerConstraints, IntegerTarget, JsonLimits, ModelField, ModelSchema,
    NativeValue, PreparedSchema, Schema, SerializationErrorKind, SerializationPlan, serialize_json,
    serialize_structural,
};
use sifr_runtime::interop::structural::{
    ShapeIdentity, StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind,
    StructuralProject, StructuralType, StructuralVisitor, VisitControl, primitive,
};

struct CurrentModel {
    name: String,
    scores: Vec<i64>,
    note: Option<String>,
}

struct ByteValue(Vec<u8>);

impl StructuralType for ByteValue {
    fn shape_identity() -> ShapeIdentity {
        primitive("bytes")
    }
}

impl StructuralProject for ByteValue {
    fn structural_project<'value, V: StructuralVisitor<'value>>(
        &'value self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        visitor.scalar(sifr_runtime::interop::structural::StructuralScalarRef::Bytes(&self.0))
    }
}

impl StructuralType for CurrentModel {
    fn shape_identity() -> ShapeIdentity {
        primitive("tests.CurrentModel")
    }

    fn nominal_identity() -> Option<&'static str> {
        Some("tests.CurrentModel")
    }
}

impl StructuralProject for CurrentModel {
    fn structural_project<'value, V: StructuralVisitor<'value>>(
        &'value self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        let control = visitor.enter(StructuralEnter::new(
            StructuralKind::Record,
            Self::nominal_identity(),
            3,
        ))?;
        if control == VisitControl::Continue {
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField("name")))?;
            self.name.structural_project(visitor)?;
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField(
                "scores",
            )))?;
            self.scores.structural_project(visitor)?;
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField("note")))?;
            self.note.structural_project(visitor)?;
        }
        visitor.exit(StructuralKind::Record)
    }
}

#[test]
fn structural_and_json_outputs_read_current_typed_values() {
    let plan = current_model_plan();
    let mut value = CurrentModel {
        name: "before".to_owned(),
        scores: vec![1, 2],
        note: None,
    };
    value.name = "after".to_owned();
    value.scores.push(3);
    value.note = Some("current".to_owned());

    let structural = serialize_structural(&plan, &value, JsonLimits::default())
        .unwrap_or_else(|error| panic!("structural serialization failed: {error}"));
    assert_eq!(
        structural,
        NativeValue::Object(vec![
            ("name".to_owned(), NativeValue::String("after".to_owned())),
            (
                "scores".to_owned(),
                NativeValue::List(vec![
                    NativeValue::Integer("1".to_owned()),
                    NativeValue::Integer("2".to_owned()),
                    NativeValue::Integer("3".to_owned()),
                ]),
            ),
            ("note".to_owned(), NativeValue::String("current".to_owned()),),
        ])
    );

    let json = serialize_json(&plan, &value, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"));
    assert_eq!(
        json,
        br#"{"name":"after","scores":[1,2,3],"note":"current"}"#
    );
}

#[test]
fn output_rejects_shape_mismatches_before_projection() {
    let schema = Schema::String(Default::default());
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let value = 7_i64;

    let Err(error) = serialize_json(&plan, &value, JsonLimits::default()) else {
        panic!("a mismatched typed value must not serialize");
    };
    assert_eq!(error.kind(), SerializationErrorKind::ShapeMismatch);
}

#[test]
fn streaming_json_enforces_output_limits_and_explicit_byte_policy() {
    let string_schema = Schema::String(Default::default());
    let prepared = PreparedSchema::new(&string_schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let string_plan = SerializationPlan::from_prepared(prepared)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let limits = JsonLimits {
        max_input_bytes: 4,
        ..JsonLimits::default()
    };
    let Err(error) = serialize_json(&string_plan, &"large".to_owned(), limits) else {
        panic!("bounded JSON output must reject an oversized document");
    };
    assert_eq!(error.kind(), SerializationErrorKind::Limit);

    let bytes_schema = Schema::Bytes(Default::default());
    let prepared = PreparedSchema::new(&bytes_schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let bytes_plan = SerializationPlan::from_prepared(prepared)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let Err(error) = serialize_json(
        &bytes_plan,
        &ByteValue(vec![1_u8, 2_u8]),
        JsonLimits::default(),
    ) else {
        panic!("bytes require a later explicit JSON policy");
    };
    assert_eq!(error.kind(), SerializationErrorKind::UnsupportedJsonValue);
}

fn current_model_plan() -> SerializationPlan {
    let schema = Schema::Model(
        ModelSchema::new(
            "CurrentModel",
            CurrentModel::shape_identity(),
            vec![
                ModelField::required("name", Schema::String(Default::default())),
                ModelField::required(
                    "scores",
                    Schema::List {
                        item: Box::new(Schema::Integer {
                            target: IntegerTarget::I64,
                            constraints: IntegerConstraints::default(),
                        }),
                        constraints: Default::default(),
                    },
                ),
                ModelField::required(
                    "note",
                    Schema::Nullable(Box::new(Schema::String(Default::default()))),
                ),
            ],
            ExtraPolicy::Ignore,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("model schema failed: {error}")),
    );
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    SerializationPlan::from_prepared(prepared)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"))
}
