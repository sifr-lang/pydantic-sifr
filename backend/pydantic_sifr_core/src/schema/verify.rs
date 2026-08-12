use core::fmt;

use super::{CompilerProgramEnvelope, ContractVersions, MAX_PROGRAM_BYTES, ProgramHeader};

const SUPPORTED_FEATURES: u64 = 0;
const MAX_SHAPE_IDENTITY_BYTES: usize = 4096;

/// A schema program whose compiler envelope matches one concrete Sifr type.
///
/// Fields are private so package and application code cannot manufacture a
/// verified value. Schema semantics have already been checked by Sifr const
/// specialization before this value can exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSchemaProgram<'a> {
    header: ProgramHeader<'a>,
    canonical_bytes: &'a [u8],
}

impl<'a> VerifiedSchemaProgram<'a> {
    #[must_use]
    pub const fn header(&self) -> ProgramHeader<'a> {
        self.header
    }

    #[must_use]
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.canonical_bytes
    }
}

/// Check only the immutable compiler envelope before user input is read.
///
/// `expected_program_identity` and `expected_shape_identity` come from the
/// monomorphized generated bridge for the concrete result type. The payload is
/// intentionally not decoded here. The Sifr package canonicalizes and verifies
/// it exactly once during const specialization.
pub fn verify_program<'a>(
    envelope: CompilerProgramEnvelope<'a>,
    expected_program_identity: [u8; 32],
    expected_shape_identity: &str,
) -> Result<VerifiedSchemaProgram<'a>, SchemaVerificationError> {
    if envelope.header.versions != ContractVersions::CURRENT {
        return Err(SchemaVerificationError::ContractVersionMismatch {
            expected: ContractVersions::CURRENT,
            actual: envelope.header.versions,
        });
    }
    if envelope.header.feature_bitmap & !SUPPORTED_FEATURES != 0 {
        return Err(SchemaVerificationError::UnsupportedFeatures(
            envelope.header.feature_bitmap,
        ));
    }
    if envelope.header.shape_identity.is_empty()
        || envelope.header.shape_identity.len() > MAX_SHAPE_IDENTITY_BYTES
    {
        return Err(SchemaVerificationError::InvalidShapeIdentity);
    }
    if envelope.header.program_identity != expected_program_identity {
        return Err(SchemaVerificationError::ProgramIdentityMismatch);
    }
    if envelope.header.shape_identity != expected_shape_identity {
        return Err(SchemaVerificationError::ShapeIdentityMismatch);
    }
    if envelope.canonical_bytes.is_empty() {
        return Err(SchemaVerificationError::EmptyProgram);
    }
    if envelope.canonical_bytes.len() > MAX_PROGRAM_BYTES {
        return Err(SchemaVerificationError::ProgramTooLarge);
    }
    Ok(VerifiedSchemaProgram {
        header: envelope.header,
        canonical_bytes: envelope.canonical_bytes,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaVerificationError {
    ContractVersionMismatch {
        expected: ContractVersions,
        actual: ContractVersions,
    },
    UnsupportedFeatures(u64),
    InvalidShapeIdentity,
    ProgramIdentityMismatch,
    ShapeIdentityMismatch,
    EmptyProgram,
    ProgramTooLarge,
}

impl fmt::Display for SchemaVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractVersionMismatch { expected, actual } => write!(
                f,
                "schema contract mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::UnsupportedFeatures(features) => {
                write!(f, "schema program uses unsupported features 0x{features:x}")
            }
            Self::InvalidShapeIdentity => f.write_str("schema shape identity is invalid"),
            Self::ProgramIdentityMismatch => {
                f.write_str("schema program identity does not match the generated bridge")
            }
            Self::ShapeIdentityMismatch => {
                f.write_str("schema shape identity does not match the generated bridge")
            }
            Self::EmptyProgram => f.write_str("schema program has no canonical bytes"),
            Self::ProgramTooLarge => f.write_str("schema program exceeds the static byte limit"),
        }
    }
}

impl std::error::Error for SchemaVerificationError {}
