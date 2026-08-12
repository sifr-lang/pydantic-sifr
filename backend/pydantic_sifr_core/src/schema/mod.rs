mod canonical;
mod kind;
mod verify;

pub use canonical::canonical_payload;
pub use kind::SchemaKind;
pub use verify::{SchemaVerificationError, VerifiedSchemaProgram, load_program, verify_program};

use crate::ErrorOverride;

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct NodeIndex(u32);

impl NodeIndex {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> Result<usize, SchemaVerificationError> {
        usize::try_from(self.0).map_err(|_| SchemaVerificationError::IndexOutOfRange(self))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProgramHeader {
    pub versions: ContractVersions,
    pub feature_bitmap: u64,
    pub shape_identity: String,
    pub payload_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SchemaNode {
    pub kind: SchemaKind,
    #[serde(default)]
    pub children: Vec<NodeIndex>,
    #[serde(default)]
    pub definition: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub error_override: Option<ErrorOverride>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SchemaProgram {
    pub header: ProgramHeader,
    pub root: NodeIndex,
    pub nodes: Vec<SchemaNode>,
}
