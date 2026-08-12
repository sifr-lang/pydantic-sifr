use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::{ErrorRegistry, RegistryError};

use super::kind::ChildCount;
use super::{ContractVersions, NodeIndex, SchemaKind, SchemaProgram, canonical};

const SUPPORTED_FEATURES: u64 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSchemaProgram {
    program: SchemaProgram,
    canonical_payload: Vec<u8>,
}

impl VerifiedSchemaProgram {
    #[must_use]
    pub const fn program(&self) -> &SchemaProgram {
        &self.program
    }

    #[must_use]
    pub fn canonical_payload(&self) -> &[u8] {
        &self.canonical_payload
    }
}

/// Decode and verify canonical program bytes without exposing an unsealed value.
pub fn load_program(
    bytes: &[u8],
    registry: &ErrorRegistry,
) -> Result<VerifiedSchemaProgram, SchemaVerificationError> {
    let program =
        serde_json::from_slice(bytes).map_err(|error| SchemaVerificationError::Decode {
            line: error.line(),
            column: error.column(),
            message: decode_category(error.classify()).to_owned(),
        })?;
    verify_program(program, registry)
}

const fn decode_category(category: serde_json::error::Category) -> &'static str {
    match category {
        serde_json::error::Category::Io => "input/output error",
        serde_json::error::Category::Syntax => "syntax error",
        serde_json::error::Category::Data => "invalid schema data",
        serde_json::error::Category::Eof => "unexpected end of input",
    }
}

/// Verify an immutable compiler-emitted program envelope before input is read.
pub fn verify_program(
    program: SchemaProgram,
    registry: &ErrorRegistry,
) -> Result<VerifiedSchemaProgram, SchemaVerificationError> {
    verify_header(&program)?;
    if program.nodes.is_empty() {
        return Err(SchemaVerificationError::EmptyProgram);
    }
    checked_node(&program, program.root)?;
    let definitions = collect_definitions(&program)?;
    for (position, node) in program.nodes.iter().enumerate() {
        let index = NodeIndex::new(
            u32::try_from(position).map_err(|_| SchemaVerificationError::ProgramTooLarge)?,
        );
        verify_node(&program, index, node.kind, &definitions, registry)?;
    }
    reject_unreachable_nodes(&program, &definitions)?;
    reject_direct_cycles(&program)?;
    let canonical_payload = canonical::canonical_payload(&program)
        .map_err(|error| SchemaVerificationError::CanonicalEncoding(error.to_string()))?;
    let actual_hash = canonical::payload_sha256(&program)
        .map_err(|error| SchemaVerificationError::CanonicalEncoding(error.to_string()))?;
    if program.header.payload_sha256 != actual_hash {
        return Err(SchemaVerificationError::PayloadHashMismatch);
    }
    Ok(VerifiedSchemaProgram {
        program,
        canonical_payload,
    })
}

fn verify_header(program: &SchemaProgram) -> Result<(), SchemaVerificationError> {
    if program.header.versions != ContractVersions::CURRENT {
        return Err(SchemaVerificationError::ContractVersionMismatch {
            expected: ContractVersions::CURRENT,
            actual: program.header.versions,
        });
    }
    if program.header.shape_identity.is_empty() || program.header.shape_identity.len() > 4096 {
        return Err(SchemaVerificationError::InvalidShapeIdentity);
    }
    if program.header.feature_bitmap & !SUPPORTED_FEATURES != 0 {
        return Err(SchemaVerificationError::UnsupportedFeatures(
            program.header.feature_bitmap,
        ));
    }
    if program.header.payload_sha256.len() != 64
        || !program
            .header
            .payload_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(SchemaVerificationError::InvalidPayloadHash);
    }
    Ok(())
}

fn collect_definitions(
    program: &SchemaProgram,
) -> Result<BTreeMap<&str, NodeIndex>, SchemaVerificationError> {
    let mut definitions = BTreeMap::new();
    for (position, node) in program.nodes.iter().enumerate() {
        if let Some(name) = node.definition.as_deref() {
            if name.is_empty() || name.len() > 512 {
                return Err(SchemaVerificationError::InvalidDefinition(name.to_owned()));
            }
            let index = NodeIndex::new(
                u32::try_from(position).map_err(|_| SchemaVerificationError::ProgramTooLarge)?,
            );
            if definitions.insert(name, index).is_some() {
                return Err(SchemaVerificationError::DuplicateDefinition(
                    name.to_owned(),
                ));
            }
        }
    }
    Ok(definitions)
}

fn verify_node(
    program: &SchemaProgram,
    index: NodeIndex,
    kind: SchemaKind,
    definitions: &BTreeMap<&str, NodeIndex>,
    registry: &ErrorRegistry,
) -> Result<(), SchemaVerificationError> {
    let node = checked_node(program, index)?;
    if kind.is_rejected() {
        return Err(SchemaVerificationError::RejectedKind { index, kind });
    }
    match kind.expected_children() {
        ChildCount::Exact(expected) if node.children.len() != expected => {
            return Err(SchemaVerificationError::InvalidArity {
                index,
                expected,
                actual: node.children.len(),
            });
        }
        ChildCount::AtLeast(minimum) if node.children.len() < minimum => {
            return Err(SchemaVerificationError::InvalidArity {
                index,
                expected: minimum,
                actual: node.children.len(),
            });
        }
        _ => {}
    }
    for child in &node.children {
        checked_node(program, *child)?;
    }
    if kind == SchemaKind::DefinitionRef {
        let Some(name) = node.reference.as_deref() else {
            return Err(SchemaVerificationError::MissingDefinitionReference(index));
        };
        if !definitions.contains_key(name) {
            return Err(SchemaVerificationError::DanglingDefinition(name.to_owned()));
        }
    }
    if kind != SchemaKind::DefinitionRef && node.reference.is_some() {
        return Err(SchemaVerificationError::UnexpectedDefinitionReference(
            index,
        ));
    }
    if kind == SchemaKind::DefinitionRef && node.definition.is_some() {
        return Err(SchemaVerificationError::UnexpectedDefinition(index));
    }
    if let Some(error_override) = &node.error_override {
        registry
            .validate_override(error_override)
            .map_err(SchemaVerificationError::ErrorRegistry)?;
    }
    if kind == SchemaKind::CustomError && node.error_override.is_none() {
        return Err(SchemaVerificationError::MissingErrorOverride(index));
    }
    Ok(())
}

fn checked_node(
    program: &SchemaProgram,
    index: NodeIndex,
) -> Result<&super::SchemaNode, SchemaVerificationError> {
    let position = index.as_usize()?;
    program
        .nodes
        .get(position)
        .ok_or(SchemaVerificationError::IndexOutOfRange(index))
}

fn reject_unreachable_nodes(
    program: &SchemaProgram,
    definitions: &BTreeMap<&str, NodeIndex>,
) -> Result<(), SchemaVerificationError> {
    let mut pending = vec![program.root];
    let mut reachable = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if !reachable.insert(index) {
            continue;
        }
        let node = checked_node(program, index)?;
        pending.extend(node.children.iter().copied());
        if let Some(reference) = node.reference.as_deref()
            && let Some(target) = definitions.get(reference)
        {
            pending.push(*target);
        }
    }
    if reachable.len() != program.nodes.len() {
        let unreachable = (0..program.nodes.len())
            .filter_map(|position| u32::try_from(position).ok())
            .map(NodeIndex::new)
            .find(|index| !reachable.contains(index))
            .ok_or(SchemaVerificationError::ProgramTooLarge)?;
        return Err(SchemaVerificationError::UnreachableNode(unreachable));
    }
    Ok(())
}

fn reject_direct_cycles(program: &SchemaProgram) -> Result<(), SchemaVerificationError> {
    fn visit(
        program: &SchemaProgram,
        index: NodeIndex,
        active: &mut BTreeSet<NodeIndex>,
        complete: &mut BTreeSet<NodeIndex>,
    ) -> Result<(), SchemaVerificationError> {
        if complete.contains(&index) {
            return Ok(());
        }
        if !active.insert(index) {
            return Err(SchemaVerificationError::DirectCycle(index));
        }
        let node = checked_node(program, index)?;
        if node.kind != SchemaKind::DefinitionRef {
            for child in &node.children {
                visit(program, *child, active, complete)?;
            }
        }
        active.remove(&index);
        complete.insert(index);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for position in 0..program.nodes.len() {
        let index = NodeIndex::new(
            u32::try_from(position).map_err(|_| SchemaVerificationError::ProgramTooLarge)?,
        );
        visit(program, index, &mut active, &mut complete)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaVerificationError {
    EmptyProgram,
    ProgramTooLarge,
    ContractVersionMismatch {
        expected: ContractVersions,
        actual: ContractVersions,
    },
    InvalidShapeIdentity,
    UnsupportedFeatures(u64),
    InvalidPayloadHash,
    PayloadHashMismatch,
    IndexOutOfRange(NodeIndex),
    RejectedKind {
        index: NodeIndex,
        kind: SchemaKind,
    },
    InvalidArity {
        index: NodeIndex,
        expected: usize,
        actual: usize,
    },
    InvalidDefinition(String),
    DuplicateDefinition(String),
    MissingDefinitionReference(NodeIndex),
    UnexpectedDefinitionReference(NodeIndex),
    UnexpectedDefinition(NodeIndex),
    DanglingDefinition(String),
    DirectCycle(NodeIndex),
    UnreachableNode(NodeIndex),
    MissingErrorOverride(NodeIndex),
    ErrorRegistry(RegistryError),
    CanonicalEncoding(String),
    Decode {
        line: usize,
        column: usize,
        message: String,
    },
}

impl fmt::Display for SchemaVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProgram => f.write_str("schema program has no nodes"),
            Self::ProgramTooLarge => f.write_str("schema program exceeds the compact index range"),
            Self::ContractVersionMismatch { expected, actual } => write!(
                f,
                "schema contract mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::InvalidShapeIdentity => f.write_str("schema shape identity is invalid"),
            Self::UnsupportedFeatures(features) => {
                write!(f, "schema program uses unsupported features 0x{features:x}")
            }
            Self::InvalidPayloadHash => f.write_str("schema payload hash is not lowercase SHA-256"),
            Self::PayloadHashMismatch => f.write_str("schema payload hash does not match payload"),
            Self::IndexOutOfRange(index) => {
                write!(f, "schema node index {} is out of range", index.raw())
            }
            Self::RejectedKind { index, kind } => {
                write!(f, "schema node {} uses rejected kind {kind:?}", index.raw())
            }
            Self::InvalidArity {
                index,
                expected,
                actual,
            } => write!(
                f,
                "schema node {} expected {expected} children but found {actual}",
                index.raw()
            ),
            Self::InvalidDefinition(name) => write!(f, "invalid definition `{name}`"),
            Self::DuplicateDefinition(name) => write!(f, "duplicate definition `{name}`"),
            Self::MissingDefinitionReference(index) => {
                write!(f, "definition reference {} has no identity", index.raw())
            }
            Self::UnexpectedDefinitionReference(index) => {
                write!(f, "schema node {} has an unexpected reference", index.raw())
            }
            Self::UnexpectedDefinition(index) => {
                write!(
                    f,
                    "definition reference {} also declares a definition",
                    index.raw()
                )
            }
            Self::DanglingDefinition(name) => write!(f, "dangling definition `{name}`"),
            Self::DirectCycle(index) => {
                write!(f, "schema has a direct child cycle at node {}", index.raw())
            }
            Self::UnreachableNode(index) => {
                write!(
                    f,
                    "schema node {} is unreachable from the root",
                    index.raw()
                )
            }
            Self::MissingErrorOverride(index) => {
                write!(f, "custom-error node {} has no override", index.raw())
            }
            Self::ErrorRegistry(error) => write!(f, "invalid error override: {error}"),
            Self::CanonicalEncoding(error) => write!(f, "canonical encoding failed: {error}"),
            Self::Decode {
                line,
                column,
                message,
            } => write!(
                f,
                "schema program decode failed at line {line} column {column}: {message}"
            ),
        }
    }
}

impl std::error::Error for SchemaVerificationError {}
