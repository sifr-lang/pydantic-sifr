//! Python-free native foundations for `pydantic-sifr`.

pub mod arena;
pub mod errors;
pub mod input;
pub mod plan;
pub mod schema;
pub mod validation;

pub use arena::{Arena, ArenaError, ArenaId};
pub use errors::{BuiltInError, ErrorOverride, ErrorRegistry, RegistryError};
pub use input::{
    InputArena, InputId, InputValue, JsonInputError, JsonLimits, NativeInputError, NativeValue,
    ObjectKind, SequenceKind, build_native_input, parse_json, project_structural_input,
};
pub use plan::{ExecutionPlan, PlanOp};
pub use schema::{
    CompilerProgramEnvelope, ContractVersions, MAX_PROGRAM_BYTES, ProgramHeader, SchemaKind,
    SchemaVerificationError, VerifiedSchemaProgram, verify_program,
};
pub use validation::{
    AliasPath, AliasSegment, BytesConstraints, BytesJsonMode, ClockSnapshot, CollectionConstraints,
    ComplexConstraints, DateTimeValue, DateValue, DecimalConstraints, DefinitionSchema,
    DefinitionsSchema, DiscriminatorPath, DurationValue, EnumSchema, EnumValue, EnumVariant,
    ErrorDetail, ExtraPolicy, FieldDefault, FloatConstraints, FractionConstraints, InputProfile,
    IntegerConstraints, IntegerTarget, LiteralSchema, LiteralValue, LocationItem, ModelField,
    ModelSchema, ModelValue, PatternCompileError, PatternSchema, PatternValue, PreparedSchema,
    RelativeTimeConstraint, Schema, SchemaErrorOverride, StringConstraints, StringPattern,
    TaggedUnionChoice, TaggedUnionSchema, TemporalKind, TemporalSchema, TimeValue, UnionChoice,
    UnionMode, UnionSchema, UnionValue, UrlConstraints, ValidatedArena, ValidatedIterator,
    ValidatedValue, ValidationError, ValidationLimits, ValidationOptions, ValueId, validate,
    validate_and_construct, validate_json_and_construct, validate_native_and_construct,
    validate_strings_and_construct, validate_structural_and_construct, validated_iterator,
};
