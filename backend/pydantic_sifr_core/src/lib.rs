//! Python-free native foundations for `pydantic-sifr`.

mod adapter;
pub mod arena;
pub mod input;
pub mod json_schema;
pub mod serialization;
mod specialized_numeric;
pub mod validation;

pub use adapter::{TypeAdapter, TypeAdapterBuildError, TypeAdapterBuildErrorKind};
pub use arena::{Arena, ArenaError, ArenaId};
pub use input::{
    InputArena, InputId, InputValue, JsonInputError, JsonLimits, NativeInputError, NativeValue,
    ObjectKind, SequenceKind, build_native_input, parse_json, project_structural_input,
};
pub use json_schema::{
    JSON_SCHEMA_DIALECT, JsonSchemaError, JsonSchemaErrorKind, JsonSchemaMode, JsonSchemaOptions,
    generate_json_schema, generate_prepared_json_schema, generate_prepared_json_schema_bytes,
};
pub use serialization::{
    JsonIntegerProfile, JsonIntegerRangeError, SelectionPath, SelectionSegment, SerializationError,
    SerializationErrorKind, SerializationOptions, SerializationPlan, SerializationPlanError,
    SerializerFieldPlan, SerializerNode, SerializerNodeId, serialize_json, serialize_structural,
};
pub use specialized_numeric::{Complex, Fraction, FractionError};
pub use validation::{
    AliasPath, AliasSegment, BytesConstraints, BytesJsonMode, ChainSchema, ClockSnapshot,
    CollectionConstraints, ComplexConstraints, DateTimeValue, DateValue, DecimalConstraints,
    DefinitionSchema, DefinitionsSchema, DiscriminatorPath, DurationValue, EnumSchema, EnumValue,
    EnumVariant, ErrorDetail, ExtraPolicy, FieldDefault, FloatConstraints, FractionConstraints,
    InputProfile, IntegerConstraints, IntegerTarget, JsonOrStructuralSchema, LaxOrStrictSchema,
    LiteralSchema, LiteralValue, LocationItem, ModelField, ModelSchema, ModelValue,
    PatternCompileError, PatternSchema, PatternValue, PreparedSchema, RelativeTimeConstraint,
    Schema, SchemaErrorOverride, SchemaTag, StringConstraints, StringPattern, TaggedUnionChoice,
    TaggedUnionSchema, TemporalKind, TemporalSchema, TimeValue, UnionChoice, UnionMode,
    UnionSchema, UnionValue, UrlConstraints, ValidatedArena, ValidatedIterator, ValidatedValue,
    ValidationError, ValidationLimits, ValidationOptions, ValueId, validate,
    validate_and_construct, validate_json_and_construct, validate_json_strings_and_construct,
    validate_native_and_construct, validate_strings_and_construct,
    validate_structural_and_construct, validated_iterator,
};
