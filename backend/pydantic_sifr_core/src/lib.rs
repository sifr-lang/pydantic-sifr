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
    ObjectKind, SequenceKind, build_native_input, parse_json,
};
pub use plan::{ExecutionPlan, PlanOp};
pub use schema::{
    CompilerProgramEnvelope, ContractVersions, MAX_PROGRAM_BYTES, ProgramHeader, SchemaKind,
    SchemaVerificationError, VerifiedSchemaProgram, verify_program,
};
pub use validation::{
    AliasPath, AliasSegment, BytesConstraints, BytesJsonMode, ClockSnapshot, CollectionConstraints,
    ComplexConstraints, DateTimeValue, DateValue, DecimalConstraints, DurationValue, ErrorDetail,
    ExtraPolicy, FieldDefault, FloatConstraints, FractionConstraints, InputProfile,
    IntegerConstraints, IntegerTarget, LocationItem, ModelField, ModelSchema, ModelValue,
    PatternCompileError, PatternSchema, PatternValue, RelativeTimeConstraint, Schema,
    StringConstraints, StringPattern, TemporalKind, TemporalSchema, TimeValue, UrlConstraints,
    ValidatedArena, ValidatedIterator, ValidatedValue, ValidationError, ValidationLimits,
    ValidationOptions, ValueId, validate, validated_iterator,
};
