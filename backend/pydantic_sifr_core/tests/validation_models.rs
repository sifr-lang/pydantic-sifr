use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use pydantic_sifr_core::{
    AliasPath, AliasSegment, ExtraPolicy, FieldDefault, InputProfile, JsonLimits, LocationItem,
    ModelField, ModelSchema, NativeValue, Schema, StringConstraints, ValidatedArena,
    ValidatedValue, ValidationError, ValidationOptions, build_native_input, parse_json, validate,
};

static DEFAULT_CALLS: AtomicUsize = AtomicUsize::new(0);

fn default_name() -> NativeValue {
    DEFAULT_CALLS.fetch_add(1, Ordering::SeqCst);
    NativeValue::String("default".to_owned())
}

fn required(name: &str, schema: Schema) -> ModelField {
    ModelField::required(name, schema)
}

fn model(fields: Vec<ModelField>, extra: ExtraPolicy) -> Schema {
    Schema::Model(ModelSchema {
        name: "User".to_owned(),
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
        .find(|(field, _)| field == name)
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
                AliasSegment::Field("payload".to_owned()),
                AliasSegment::Index(0),
                AliasSegment::Field("value".to_owned()),
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
            vec![required("id", Schema::exact_integer())],
            ExtraPolicy::Allow {
                destination: "extras".to_owned(),
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
        name: "Child".to_owned(),
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
}

#[test]
fn field_metadata_and_name_population_are_static_schema_inputs() {
    let mut field = required("identifier", Schema::exact_integer());
    field.validation_aliases = vec![AliasPath::field("id")];
    field.metadata = BTreeMap::from([("description".to_owned(), "stable id".to_owned())]);
    let schema = Schema::Model(ModelSchema {
        name: "User".to_owned(),
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
