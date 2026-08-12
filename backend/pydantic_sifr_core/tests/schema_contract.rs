use std::collections::BTreeSet;

use proptest::prelude::*;
use pydantic_sifr_core::{
    ContractVersions, ErrorOverride, ErrorRegistry, NodeIndex, ProgramHeader, SchemaKind,
    SchemaNode, SchemaProgram, SchemaVerificationError, canonical_payload, load_program,
    verify_program,
};
use sha2::{Digest, Sha256};

fn node(kind: SchemaKind, children: Vec<NodeIndex>) -> SchemaNode {
    SchemaNode {
        kind,
        children,
        definition: None,
        reference: None,
        error_override: None,
    }
}

fn program(nodes: Vec<SchemaNode>) -> SchemaProgram {
    let mut program = SchemaProgram {
        header: ProgramHeader {
            versions: ContractVersions::CURRENT,
            feature_bitmap: 0,
            shape_identity: "fixture.Model{id:int}".to_owned(),
            payload_sha256: "0".repeat(64),
        },
        root: NodeIndex::new(0),
        nodes,
    };
    let payload = canonical_payload(&program).unwrap_or_else(|error| panic!("{error}"));
    program.header.payload_sha256 = hex::encode(Sha256::digest(payload));
    program
}

#[test]
fn core_schema_kind_universe_is_complete_and_unique() {
    let names: BTreeSet<String> = SchemaKind::ALL
        .iter()
        .map(|kind| serde_json::to_string(kind).unwrap_or_else(|error| panic!("{error}")))
        .collect();
    assert_eq!(SchemaKind::ALL.len(), 57);
    assert_eq!(names.len(), 57);
    let expected: BTreeSet<String> = [
        "any",
        "arguments",
        "arguments-v3",
        "bool",
        "bytes",
        "call",
        "callable",
        "chain",
        "complex",
        "computed-field",
        "custom-error",
        "dataclass",
        "dataclass-args",
        "dataclass-field",
        "date",
        "datetime",
        "decimal",
        "default",
        "definition-ref",
        "definitions",
        "dict",
        "enum",
        "float",
        "fraction",
        "frozenset",
        "function-after",
        "function-before",
        "function-plain",
        "function-wrap",
        "generator",
        "int",
        "invalid",
        "is-instance",
        "is-subclass",
        "json",
        "json-or-python",
        "lax-or-strict",
        "list",
        "literal",
        "missing-sentinel",
        "model",
        "model-field",
        "model-fields",
        "multi-host-url",
        "none",
        "nullable",
        "set",
        "str",
        "tagged-union",
        "time",
        "timedelta",
        "tuple",
        "typed-dict",
        "typed-dict-field",
        "union",
        "url",
        "uuid",
    ]
    .into_iter()
    .map(|name| format!("\"{name}\""))
    .collect();
    assert_eq!(names, expected);
}

#[test]
fn malformed_program_bytes_return_stable_decode_error() {
    let error = load_program(b"{\"header\":", &ErrorRegistry::default());
    assert!(matches!(
        error,
        Err(SchemaVerificationError::Decode {
            line: 1,
            column: 10,
            ..
        })
    ));
}

#[test]
fn verifies_canonical_format_one_program() {
    let verified = verify_program(
        program(vec![node(SchemaKind::Int, Vec::new())]),
        &ErrorRegistry::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(!verified.canonical_payload().is_empty());
    assert_eq!(verified.program().header.versions.schema_program, 1);
}

#[test]
fn rejects_hash_mismatch_and_unavailable_kinds() {
    let mut wrong_hash = program(vec![node(SchemaKind::Int, Vec::new())]);
    wrong_hash.header.payload_sha256 = "f".repeat(64);
    assert_eq!(
        verify_program(wrong_hash, &ErrorRegistry::default()),
        Err(SchemaVerificationError::PayloadHashMismatch)
    );

    let rejected = program(vec![node(SchemaKind::Invalid, Vec::new())]);
    assert!(matches!(
        verify_program(rejected, &ErrorRegistry::default()),
        Err(SchemaVerificationError::RejectedKind { .. })
    ));
}

#[test]
fn rejects_unknown_features_and_unreachable_nodes() {
    let mut unknown_feature = program(vec![node(SchemaKind::Int, Vec::new())]);
    unknown_feature.header.feature_bitmap = 1;
    let payload = canonical_payload(&unknown_feature).unwrap_or_else(|error| panic!("{error}"));
    unknown_feature.header.payload_sha256 = hex::encode(Sha256::digest(payload));
    assert!(matches!(
        verify_program(unknown_feature, &ErrorRegistry::default()),
        Err(SchemaVerificationError::UnsupportedFeatures(1))
    ));

    let unreachable = program(vec![
        node(SchemaKind::Int, Vec::new()),
        node(SchemaKind::Str, Vec::new()),
    ]);
    assert!(matches!(
        verify_program(unreachable, &ErrorRegistry::default()),
        Err(SchemaVerificationError::UnreachableNode(_))
    ));
}

#[test]
fn custom_errors_are_package_qualified_and_compositional() {
    let declaration = ErrorOverride {
        code: "example.positive".to_owned(),
        message: "Value must be greater than {minimum}".to_owned(),
        context_keys: vec!["minimum".to_owned()],
    };
    let registry =
        ErrorRegistry::new([declaration.clone()]).unwrap_or_else(|error| panic!("{error}"));
    let mut custom = node(SchemaKind::CustomError, vec![NodeIndex::new(1)]);
    custom.error_override = Some(declaration);
    let verified = verify_program(
        program(vec![custom, node(SchemaKind::Int, Vec::new())]),
        &registry,
    );
    assert!(verified.is_ok());

    let collision = ErrorOverride {
        code: "json_invalid".to_owned(),
        message: "changed".to_owned(),
        context_keys: Vec::new(),
    };
    assert!(ErrorRegistry::new([collision]).is_err());
}

#[test]
fn rejects_dangling_references_and_direct_cycles() {
    let mut reference = node(SchemaKind::DefinitionRef, Vec::new());
    reference.reference = Some("missing".to_owned());
    assert!(matches!(
        verify_program(program(vec![reference]), &ErrorRegistry::default()),
        Err(SchemaVerificationError::DanglingDefinition(_))
    ));

    let cyclic = node(SchemaKind::List, vec![NodeIndex::new(0)]);
    assert!(matches!(
        verify_program(program(vec![cyclic]), &ErrorRegistry::default()),
        Err(SchemaVerificationError::DirectCycle(_))
    ));
}

proptest! {
    #[test]
    fn malformed_schema_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let result = std::panic::catch_unwind(|| {
            if let Ok(program) = serde_json::from_slice::<SchemaProgram>(&bytes) {
                let _ = verify_program(program, &ErrorRegistry::default());
            }
        });
        prop_assert!(result.is_ok());
    }
}
