//! Python-free native foundations for `pydantic-sifr`.

pub mod arena;
pub mod errors;
pub mod input;
pub mod plan;
pub mod schema;

pub use arena::{Arena, ArenaError, ArenaId};
pub use errors::{BuiltInError, ErrorOverride, ErrorRegistry, RegistryError};
pub use input::{InputArena, InputId, InputValue, JsonInputError, JsonLimits, parse_json};
pub use plan::{ExecutionPlan, PlanError, PlanOp};
pub use schema::{
    ContractVersions, NodeIndex, ProgramHeader, SchemaKind, SchemaNode, SchemaProgram,
    SchemaVerificationError, VerifiedSchemaProgram, canonical_payload, load_program,
    verify_program,
};
