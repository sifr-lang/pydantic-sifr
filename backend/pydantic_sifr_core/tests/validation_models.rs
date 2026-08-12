use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use pydantic_sifr_core::{
    AliasPath, AliasSegment, ExtraPolicy, FieldDefault, InputProfile, JsonLimits, LocationItem,
    ModelField, ModelSchema, NativeValue, Schema, StringConstraints, ValidatedArena,
    ValidatedValue, ValidationError, ValidationOptions, build_native_input, parse_json, validate,
};
use sifr_runtime::interop::structural::primitive;

static DEFAULT_CALLS: AtomicUsize = AtomicUsize::new(0);

fn default_name() -> NativeValue {
    DEFAULT_CALLS.fetch_add(1, Ordering::SeqCst);
    NativeValue::String("default".to_owned())
}

fn required(name: &'static str, schema: Schema) -> ModelField {
    ModelField::required(name, schema)
}

fn extra_field(value: Schema) -> ModelField {
    ModelField {
        name: "extras",
        schema: Schema::Mapping {
            key: Box::new(Schema::String(StringConstraints::default())),
            value: Box::new(value),
            constraints: pydantic_sifr_core::CollectionConstraints::default(),
        },
        input: false,
        default: None,
        validation_aliases: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

fn model(fields: Vec<ModelField>, extra: ExtraPolicy) -> Schema {
    Schema::Model(ModelSchema {
        name: "User",
        structural_identity: primitive("test.User"),
        fields,
        extra,
        populate_by_name: false,
        location_by_alias: true,
    })
}

fn root_model(arena: &ValidatedArena) -> &pydantic_sifr_core::ModelValue {
    let Some(ValidatedValue::Model(value)) = arena.get(arena.root()) else {
        panic!("expected model root");
    };
    value
}

fn field_value<'a>(arena: &'a ValidatedArena, name: &str) -> &'a ValidatedValue {
    let model = root_model(arena);
    let (_, id) = model
        .fields()
        .iter()
        .find(|(field, _)| *field == name)
        .unwrap_or_else(|| panic!("missing field {name}"));
    arena
        .get(*id)
        .unwrap_or_else(|| panic!("missing value for {name}"))
}

fn require_error(result: Result<ValidatedArena, ValidationError>) -> ValidationError {
    match result {
        Ok(_) => panic!("expected validation error"),
        Err(error) => error,
    }
}

#[test]
fn model_distinguishes_required_defaulted_and_nullable_fields() {
    DEFAULT_CALLS.store(0, Ordering::SeqCst);
    let mut name = required("name", Schema::String(StringConstraints::default()));
    name.default = Some(FieldDefault::Factory(default_name));
    let schema = model(
        vec![
            required("id", Schema::exact_integer()),
            name,
            ModelField {
                name: "status",
                schema: Schema::String(StringConstraints::default()),
                input: true,
                default: Some(FieldDefault::Static(NativeValue::String("new".to_owned()))),
                validation_aliases: Vec::new(),
                metadata: BTreeMap::new(),
            },
            required(
                "note",
                Schema::Nullable(Box::new(Schema::String(StringConstraints::default()))),
            ),
        ],
        ExtraPolicy::Ignore,
    );
    let input = parse_json(br#"{"id":1,"note":null}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let output = validate(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("model validation failed: {error}"));
    assert_eq!(DEFAULT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        field_value(&output, "name"),
        &ValidatedValue::String("default".to_owned())
    );
    assert_eq!(
        field_value(&output, "note"),
        &ValidatedValue::Nullable(None)
    );
    assert_eq!(
        field_value(&output, "status"),
        &ValidatedValue::String("new".to_owned())
    );
    assert_eq!(root_model(&output).validated_field_count(), 2);

    let missing = parse_json(br#"{"note":null}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let error = require_error(validate(
        &schema,
        &missing,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "missing");
    assert_eq!(
        error.details()[0].location,
        vec![LocationItem::Field("id".to_owned())]
    );
}

#[test]
fn aliases_and_alias_paths_select_values_and_control_error_locations() {
    let mut field = required("identifier", Schema::exact_integer());
    field.validation_aliases = vec![
        AliasPath {
            segments: vec![
                AliasSegment::Field("payload"),
                AliasSegment::Index(0),
                AliasSegment::Field("value"),
            ],
        },
        AliasPath::field("id"),
    ];
    let schema = model(vec![field], ExtraPolicy::Ignore);
    let input = parse_json(br#"{"payload":[{"value":"bad"}]}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let error = require_error(validate(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "int_parsing");
    assert_eq!(
        error.details()[0].location,
        vec![
            LocationItem::Field("payload".to_owned()),
            LocationItem::Index(0),
            LocationItem::Field("value".to_owned()),
        ]
    );

    let fallback = parse_json(br#"{"id":7}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let output = validate(
        &schema,
        &fallback,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("fallback alias failed: {error}"));
    assert!(matches!(
        field_value(&output, "identifier"),
        ValidatedValue::ExactInt(_)
    ));
}

#[test]
fn extra_policies_ignore_forbid_or_validate_typed_values() {
    let input = parse_json(br#"{"id":1,"score":"bad"}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let options = ValidationOptions {
        profile: InputProfile::Json,
        ..ValidationOptions::default()
    };

    let ignored = validate(
        &model(
            vec![required("id", Schema::exact_integer())],
            ExtraPolicy::Ignore,
        ),
        &input,
        options,
    )
    .unwrap_or_else(|error| panic!("ignore extras failed: {error}"));
    assert!(root_model(&ignored).extras().is_empty());

    let forbidden = require_error(validate(
        &model(
            vec![required("id", Schema::exact_integer())],
            ExtraPolicy::Forbid,
        ),
        &input,
        options,
    ));
    assert_eq!(forbidden.details()[0].code, "extra_forbidden");
    assert_eq!(
        forbidden.details()[0].location,
        vec![LocationItem::Field("score".to_owned())]
    );

    let allowed = require_error(validate(
        &model(
            vec![
                required("id", Schema::exact_integer()),
                extra_field(Schema::exact_integer()),
            ],
            ExtraPolicy::Allow {
                destination: "extras",
                value_schema: Box::new(Schema::exact_integer()),
            },
        ),
        &input,
        options,
    ));
    assert_eq!(allowed.details()[0].code, "int_parsing");
    assert_eq!(
        allowed.details()[0].location,
        vec![LocationItem::Field("score".to_owned())]
    );
}

#[test]
fn nested_model_errors_aggregate_with_stable_field_locations() {
    let child = ModelSchema {
        name: "Child",
        structural_identity: primitive("test.Child"),
        fields: vec![
            required("left", Schema::exact_integer()),
            required("right", Schema::exact_integer()),
        ],
        extra: ExtraPolicy::Ignore,
        populate_by_name: false,
        location_by_alias: true,
    };
    let schema = model(
        vec![required("child", Schema::Model(child))],
        ExtraPolicy::Ignore,
    );
    let input = parse_json(
        br#"{"child":{"left":"bad","right":null}}"#,
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let error = require_error(validate(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details().len(), 2);
    assert_eq!(
        error.details()[0].location,
        vec![
            LocationItem::Field("child".to_owned()),
            LocationItem::Field("left".to_owned()),
        ]
    );
    assert_eq!(
        error.details()[1].location,
        vec![
            LocationItem::Field("child".to_owned()),
            LocationItem::Field("right".to_owned()),
        ]
    );
}

#[test]
fn error_cap_marks_truncation_only_when_unprocessed_errors_can_remain() {
    let schema = model(
        vec![
            required("left", Schema::exact_integer()),
            required("right", Schema::exact_integer()),
        ],
        ExtraPolicy::Forbid,
    );
    let input = parse_json(br#"{"left":"bad","right":null}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let error = require_error(validate(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            limits: pydantic_sifr_core::ValidationLimits {
                max_errors: 2,
                ..pydantic_sifr_core::ValidationLimits::default()
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details().len(), 2);
    assert!(!error.is_truncated());

    let extra_first = parse_json(
        br#"{"extra":false,"left":1,"right":2}"#,
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let extra_error = require_error(validate(
        &schema,
        &extra_first,
        ValidationOptions {
            profile: InputProfile::Json,
            limits: pydantic_sifr_core::ValidationLimits {
                max_errors: 1,
                ..pydantic_sifr_core::ValidationLimits::default()
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(extra_error.details().len(), 1);
    assert!(!extra_error.is_truncated());
}

#[test]
fn strict_native_and_strings_profiles_share_the_model_engine() {
    let schema = model(vec![required("enabled", Schema::Bool)], ExtraPolicy::Ignore);
    let native = build_native_input(
        &NativeValue::Object(vec![(
            "enabled".to_owned(),
            NativeValue::String("true".to_owned()),
        )]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native model failed: {error}"));
    let output = validate(
        &schema,
        &native,
        ValidationOptions {
            profile: InputProfile::Strings,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strings model failed: {error}"));
    assert_eq!(field_value(&output, "enabled"), &ValidatedValue::Bool(true));

    let invalid_strings = build_native_input(
        &NativeValue::Object(vec![("enabled".to_owned(), NativeValue::Bool(true))]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native model failed: {error}"));
    let error = require_error(validate(
        &schema,
        &invalid_strings,
        ValidationOptions {
            profile: InputProfile::Strings,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "strings_type");

    let typed_native = build_native_input(
        &NativeValue::Object(vec![("enabled".to_owned(), NativeValue::Bool(true))]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native model failed: {error}"));
    let native_output = validate(
        &schema,
        &typed_native,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Native,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strict native model failed: {error}"));
    assert_eq!(
        field_value(&native_output, "enabled"),
        &ValidatedValue::Bool(true)
    );
}

#[test]
fn field_metadata_and_name_population_are_static_schema_inputs() {
    let mut field = required("identifier", Schema::exact_integer());
    field.validation_aliases = vec![AliasPath::field("id")];
    field.metadata = BTreeMap::from([("description".to_owned(), "stable id".to_owned())]);
    let schema = Schema::Model(ModelSchema {
        name: "User",
        structural_identity: primitive("test.User"),
        fields: vec![field.clone()],
        extra: ExtraPolicy::Ignore,
        populate_by_name: true,
        location_by_alias: false,
    });
    assert_eq!(field.metadata["description"], "stable id");
    let input = parse_json(br#"{"identifier":9}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let output = validate(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("name population failed: {error}"));
    assert!(matches!(
        field_value(&output, "identifier"),
        ValidatedValue::ExactInt(_)
    ));
}

#[test]
fn invalid_non_input_and_extra_destination_schemas_fail_before_input_validation() {
    let input = parse_json(br#"{}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON model failed: {error}"));
    let defaulted = ModelField {
        name: "derived",
        schema: Schema::String(StringConstraints::default()),
        input: false,
        default: Some(FieldDefault::Static(NativeValue::String(
            "ready".to_owned(),
        ))),
        validation_aliases: Vec::new(),
        metadata: BTreeMap::new(),
    };
    let output = validate(
        &model(vec![defaulted], ExtraPolicy::Ignore),
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("non-input default failed: {error}"));
    assert!(matches!(
        field_value(&output, "derived"),
        ValidatedValue::String(value) if value == "ready"
    ));

    let missing_destination = require_error(validate(
        &model(
            vec![],
            ExtraPolicy::Allow {
                destination: "extras",
                value_schema: Box::new(Schema::exact_integer()),
            },
        ),
        &input,
        ValidationOptions::default(),
    ));
    assert_eq!(missing_destination.details()[0].code, "schema_invalid");

    let orphan = ModelField {
        name: "orphan",
        schema: Schema::String(StringConstraints::default()),
        input: false,
        default: None,
        validation_aliases: Vec::new(),
        metadata: BTreeMap::new(),
    };
    let orphan_error = require_error(validate(
        &model(vec![orphan], ExtraPolicy::Ignore),
        &input,
        ValidationOptions::default(),
    ));
    assert_eq!(orphan_error.details()[0].code, "schema_invalid");

    let duplicate_error = require_error(validate(
        &model(
            vec![
                required("same", Schema::exact_integer()),
                required("same", Schema::exact_integer()),
            ],
            ExtraPolicy::Ignore,
        ),
        &input,
        ValidationOptions::default(),
    ));
    assert_eq!(duplicate_error.details()[0].code, "schema_invalid");
}
