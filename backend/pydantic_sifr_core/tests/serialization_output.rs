use pydantic_sifr_core::{
    ExtraPolicy, FieldDefault, IntegerConstraints, IntegerTarget, JsonIntegerProfile, JsonLimits,
    ModelField, ModelSchema, NativeValue, PreparedSchema, Schema, SelectionPath, SelectionSegment,
    SerializationErrorKind, SerializationOptions, SerializationPlan, serialize_json,
    serialize_structural,
};
use sifr_runtime::interop::structural::{
    ShapeIdentity, StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind,
    StructuralProject, StructuralType, StructuralVisitor, VisitControl, primitive,
};
use sifr_runtime::{DEFAULT_MAX_INTEGER_DIGITS, SifrInt};

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

    let structural = serialize_structural(&plan, &value, &SerializationOptions::default())
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

    let json = serialize_json(&plan, &value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"));
    assert_eq!(
        json,
        br#"{"name":"after","scores":[1,2,3],"note":"current"}"#
    );
}

#[test]
fn schema_excluded_fields_are_absent_from_all_outputs() {
    let name = ModelField::required("name", Schema::String(Default::default()));
    let scores = ModelField::required(
        "scores",
        Schema::List {
            item: Box::new(Schema::Integer {
                target: IntegerTarget::I64,
                constraints: IntegerConstraints::default(),
            }),
            constraints: Default::default(),
        },
    );
    let mut note = ModelField::required(
        "note",
        Schema::Nullable(Box::new(Schema::String(Default::default()))),
    );
    note.metadata
        .insert("pydantic.exclude".to_owned(), "true".to_owned());
    let schema = Schema::Model(
        ModelSchema::new(
            "CurrentModel",
            CurrentModel::shape_identity(),
            vec![name, scores, note],
            ExtraPolicy::Ignore,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("model schema failed: {error}")),
    );
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let value = CurrentModel {
        name: "visible".to_owned(),
        scores: vec![1, 2],
        note: Some("hidden".to_owned()),
    };

    let json = serialize_json(&plan, &value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"));
    assert_eq!(json, br#"{"name":"visible","scores":[1,2]}"#);
    let structural = serialize_structural(&plan, &value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("structural serialization failed: {error}"));
    assert_eq!(
        structural,
        NativeValue::Object(vec![
            ("name".to_owned(), NativeValue::String("visible".to_owned())),
            (
                "scores".to_owned(),
                NativeValue::List(vec![
                    NativeValue::Integer("1".to_owned()),
                    NativeValue::Integer("2".to_owned()),
                ]),
            ),
        ])
    );
}

#[test]
fn output_rejects_shape_mismatches_before_projection() {
    let schema = Schema::String(Default::default());
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let value = 7_i64;

    let Err(error) = serialize_json(&plan, &value, &SerializationOptions::default()) else {
        panic!("a mismatched typed value must not serialize");
    };
    assert_eq!(error.kind(), SerializationErrorKind::ShapeMismatch);
}

#[test]
fn streaming_json_enforces_output_limits_and_explicit_byte_policy() {
    let string_schema = Schema::String(Default::default());
    let prepared = PreparedSchema::new(&string_schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let string_plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let options = SerializationOptions {
        limits: JsonLimits {
            max_input_bytes: 4,
            ..JsonLimits::default()
        },
        ..SerializationOptions::default()
    };
    let Err(error) = serialize_json(&string_plan, &"large".to_owned(), &options) else {
        panic!("bounded JSON output must reject an oversized document");
    };
    assert_eq!(error.kind(), SerializationErrorKind::Limit);

    let bytes_schema = Schema::Bytes(Default::default());
    let prepared = PreparedSchema::new(&bytes_schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let bytes_plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let Err(error) = serialize_json(
        &bytes_plan,
        &ByteValue(vec![1_u8, 2_u8]),
        &SerializationOptions::default(),
    ) else {
        panic!("bytes require a later explicit JSON policy");
    };
    assert_eq!(error.kind(), SerializationErrorKind::UnsupportedJsonValue);
}

#[test]
fn typed_recursive_selections_aliases_and_omit_policies_share_precedence() {
    let mut plan = current_model_plan();
    let root = plan
        .node(plan.root())
        .unwrap_or_else(|| panic!("serializer root is missing"));
    assert_eq!(root.fields()[0].alias(), Some("schema_name"));
    plan.set_field_alias(&SelectionPath::field("name"), "display_name")
        .unwrap_or_else(|error| panic!("alias plan failed: {error}"));
    let value = CurrentModel {
        name: "after".to_owned(),
        scores: vec![1, 2, 3],
        note: None,
    };
    let options = SerializationOptions {
        by_alias: true,
        exclude_none: true,
        include: vec![
            SelectionPath::field("name"),
            SelectionPath::new(vec![
                SelectionSegment::Field("scores".to_owned()),
                SelectionSegment::Index(1),
            ]),
            SelectionPath::field("note"),
        ],
        ..SerializationOptions::default()
    };

    let json = serialize_json(&plan, &value, &options)
        .unwrap_or_else(|error| panic!("selected JSON failed: {error}"));
    assert_eq!(json, br#"{"display_name":"after","scores":[2]}"#);
    let structural = serialize_structural(&plan, &value, &options)
        .unwrap_or_else(|error| panic!("selected structural output failed: {error}"));
    assert_eq!(
        structural,
        NativeValue::Object(vec![
            (
                "display_name".to_owned(),
                NativeValue::String("after".to_owned()),
            ),
            (
                "scores".to_owned(),
                NativeValue::List(vec![NativeValue::Integer("2".to_owned())]),
            ),
        ])
    );
}

#[test]
fn default_omission_precedes_nested_selection() {
    let plan = current_model_plan();
    let value = CurrentModel {
        name: "after".to_owned(),
        scores: vec![1, 2, 3],
        note: None,
    };
    let options = SerializationOptions {
        exclude_defaults: true,
        include: vec![
            SelectionPath::field("name"),
            SelectionPath::field("scores"),
            SelectionPath::field("note"),
        ],
        exclude: vec![SelectionPath::new(vec![
            SelectionSegment::Field("scores".to_owned()),
            SelectionSegment::Index(0),
        ])],
        ..SerializationOptions::default()
    };

    let json = serialize_json(&plan, &value, &options)
        .unwrap_or_else(|error| panic!("default-filtered JSON failed: {error}"));
    assert_eq!(json, br#"{}"#);
    let structural = serialize_structural(&plan, &value, &options)
        .unwrap_or_else(|error| panic!("default-filtered structural output failed: {error}"));
    assert_eq!(structural, NativeValue::Object(Vec::new()));
}

#[test]
fn selected_integer_profiles_use_the_runtime_encoder_recursively() {
    let schema = Schema::List {
        item: Box::new(Schema::Integer {
            target: IntegerTarget::I64,
            constraints: IntegerConstraints::default(),
        }),
        constraints: Default::default(),
    };
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::StringInts)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));

    let json = serialize_json(
        &plan,
        &vec![-7_i64, 9_i64],
        &SerializationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("string-integer JSON failed: {error}"));
    assert_eq!(json, br#"["-7","9"]"#);

    let exact_schema = Schema::Integer {
        target: IntegerTarget::Exact,
        constraints: IntegerConstraints::default(),
    };
    let exact_prepared = PreparedSchema::new(&exact_schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let exact_plan = SerializationPlan::from_prepared(exact_prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let exact_value = SifrInt::parse_decimal(
        "1234567890123456789012345678901234567890",
        DEFAULT_MAX_INTEGER_DIGITS,
    )
    .unwrap_or_else(|error| panic!("exact integer setup failed: {error}"));
    let exact_json = serialize_json(&exact_plan, &exact_value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("exact-integer JSON failed: {error}"));
    assert_eq!(exact_json, b"1234567890123456789012345678901234567890");
}

#[test]
fn string_integer_profile_applies_to_default_comparison() {
    let schema = current_model_schema();
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::StringInts)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let value = CurrentModel {
        name: "after".to_owned(),
        scores: vec![1_i64, 2_i64, 3_i64],
        note: None,
    };
    let options = SerializationOptions {
        exclude_defaults: true,
        ..SerializationOptions::default()
    };

    let json = serialize_json(&plan, &value, &options)
        .unwrap_or_else(|error| panic!("string-integer default filtering failed: {error}"));
    assert_eq!(json, b"{}");
}

#[test]
fn web_integer_range_error_preserves_typed_profile_and_model_path() {
    let schema = current_model_schema();
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Web)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let value = CurrentModel {
        name: "range".to_owned(),
        scores: vec![9_007_199_254_740_992_i64],
        note: None,
    };

    let Err(error) = serialize_json(&plan, &value, &SerializationOptions::default()) else {
        panic!("json.web must reject integers outside the JavaScript-safe range");
    };
    assert_eq!(error.kind(), SerializationErrorKind::IntegerRange);
    let range = error
        .integer_range_error()
        .unwrap_or_else(|| panic!("typed integer range payload is missing"));
    assert_eq!(range.profile(), "json.web");
    assert_eq!(range.path(), "$.scores[0]");
}

fn current_model_plan() -> SerializationPlan {
    let schema = current_model_schema();
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"))
}

fn current_model_schema() -> Schema {
    let mut name = ModelField::required("name", Schema::String(Default::default()));
    name.default = Some(FieldDefault::Static(NativeValue::String(
        "after".to_owned(),
    )));
    name.metadata.insert(
        "pydantic.serialization_alias".to_owned(),
        "schema_name".to_owned(),
    );
    let mut note = ModelField::required(
        "note",
        Schema::Nullable(Box::new(Schema::String(Default::default()))),
    );
    note.default = Some(FieldDefault::Static(NativeValue::Null));
    let mut scores = ModelField::required(
        "scores",
        Schema::List {
            item: Box::new(Schema::Integer {
                target: IntegerTarget::I64,
                constraints: IntegerConstraints::default(),
            }),
            constraints: Default::default(),
        },
    );
    scores.default = Some(FieldDefault::Static(NativeValue::List(vec![
        NativeValue::Integer("1".to_owned()),
        NativeValue::Integer("2".to_owned()),
        NativeValue::Integer("3".to_owned()),
    ])));
    Schema::Model(
        ModelSchema::new(
            "CurrentModel",
            CurrentModel::shape_identity(),
            vec![name, scores, note],
            ExtraPolicy::Ignore,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("model schema failed: {error}")),
    )
}
