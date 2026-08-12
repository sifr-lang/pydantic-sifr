mod kind;
mod verify;

pub use kind::SchemaKind;
pub use verify::{SchemaVerificationError, VerifiedSchemaProgram, verify_program};

/// Maximum accepted size of one compiler-sealed schema payload.
pub const MAX_PROGRAM_BYTES: usize = 4 * 1024 * 1024;

/// Versions carried by the package-owned static schema program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContractVersions {
    pub schema_program: u16,
    pub structural_contract: u16,
    pub structural_call: u16,
    pub callback_abi: u16,
}

impl ContractVersions {
    pub const CURRENT: Self = Self {
        schema_program: 1,
        structural_contract: 1,
        structural_call: 1,
        callback_abi: 1,
    };
}

/// Header copied from a compiler-sealed `sifr.meta.StaticProgram`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramHeader<'a> {
    pub versions: ContractVersions,
    pub feature_bitmap: u64,
    pub shape_identity: &'a str,
    pub program_identity: [u8; 32],
}

/// Borrowed view of an immutable compiler-emitted schema program.
///
/// The Sifr compiler owns construction and sealing. The core only checks the
/// envelope. It does not decode or semantically verify the schema graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerProgramEnvelope<'a> {
    pub header: ProgramHeader<'a>,
    pub canonical_bytes: &'a [u8],
}
