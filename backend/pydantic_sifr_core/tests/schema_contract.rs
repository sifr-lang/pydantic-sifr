use std::collections::BTreeSet;

use proptest::prelude::*;
use pydantic_sifr_core::{
    CompilerProgramEnvelope, ContractVersions, ErrorOverride, ErrorRegistry, ExecutionPlan,
    ProgramHeader, SchemaKind, SchemaVerificationError, verify_program,
};

const SHAPE_IDENTITY: &str = "class:main.SchemaContractProbe:args[]:meta[]:fields[value:int:required:pydantic.error.package:str=str:7:example,pydantic.error.code:str=str:8:positive,pydantic.error.message:str=str:16:Must be positive]";
const PROGRAM_IDENTITY_HEX: &str =
    "d6591c059d855809f03be42c991d73a91a42e50c6c141810b3e6195c8efdca72";
const SIFR_PROGRAM_TEXT: &str =
    include_str!("../../../tests/static_program/schema_contract_program.txt");

fn sifr_program_bytes() -> &'static [u8] {
    SIFR_PROGRAM_TEXT
        .strip_suffix('\n')
        .unwrap_or(SIFR_PROGRAM_TEXT)
        .as_bytes()
}

fn program_identity() -> [u8; 32] {
    let bytes = hex::decode(PROGRAM_IDENTITY_HEX)
        .unwrap_or_else(|error| panic!("invalid test identity: {error}"));
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("test identity must contain 32 bytes"))
}

fn envelope<'a>(
    bytes: &'a [u8],
    identity: [u8; 32],
    shape_identity: &'a str,
) -> CompilerProgramEnvelope<'a> {
    CompilerProgramEnvelope {
        header: ProgramHeader {
            versions: ContractVersions::CURRENT,
            feature_bitmap: 0,
            shape_identity,
            program_identity: identity,
        },
        canonical_bytes: bytes,
    }
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
fn verifies_exact_sifr_emitted_program_envelope() {
    let identity = program_identity();
    let verified = verify_program(
        envelope(sifr_program_bytes(), identity, SHAPE_IDENTITY),
        identity,
        SHAPE_IDENTITY,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(verified.canonical_bytes(), sifr_program_bytes());
    assert!(
        std::str::from_utf8(verified.canonical_bytes())
            .is_ok_and(|text| text.contains("schema_program_version=int:1"))
    );
    let plan = ExecutionPlan::from_verified(&verified);
    assert_eq!(plan.program_identity(), identity);
    assert_eq!(plan.operations().len(), 2);
}

#[test]
fn rejects_each_corrupt_envelope_field_with_stable_error() {
    let identity = program_identity();
    let mut wrong_version = envelope(sifr_program_bytes(), identity, SHAPE_IDENTITY);
    wrong_version.header.versions.schema_program = 2;
    assert!(matches!(
        verify_program(wrong_version, identity, SHAPE_IDENTITY),
        Err(SchemaVerificationError::ContractVersionMismatch { .. })
    ));

    let mut wrong_feature = envelope(sifr_program_bytes(), identity, SHAPE_IDENTITY);
    wrong_feature.header.feature_bitmap = 1;
    assert_eq!(
        verify_program(wrong_feature, identity, SHAPE_IDENTITY),
        Err(SchemaVerificationError::UnsupportedFeatures(1))
    );

    assert_eq!(
        verify_program(
            envelope(sifr_program_bytes(), [0; 32], SHAPE_IDENTITY),
            identity,
            SHAPE_IDENTITY,
        ),
        Err(SchemaVerificationError::ProgramIdentityMismatch)
    );
    assert_eq!(
        verify_program(
            envelope(sifr_program_bytes(), identity, "class:other.Model"),
            identity,
            SHAPE_IDENTITY,
        ),
        Err(SchemaVerificationError::ShapeIdentityMismatch)
    );
    assert_eq!(
        verify_program(
            envelope(&[], identity, SHAPE_IDENTITY),
            identity,
            SHAPE_IDENTITY
        ),
        Err(SchemaVerificationError::EmptyProgram)
    );
}

#[test]
fn large_payload_check_is_iterative_and_stack_bounded() {
    let identity = program_identity();
    let payload = vec![b'x'; 1_000_000];
    let result = verify_program(
        envelope(&payload, identity, SHAPE_IDENTITY),
        identity,
        SHAPE_IDENTITY,
    );
    assert!(result.is_ok());
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
    assert_eq!(registry.custom(&declaration.code), Some(&declaration));

    let collision = ErrorOverride {
        code: "json_invalid".to_owned(),
        message: "changed".to_owned(),
        context_keys: Vec::new(),
    };
    assert!(ErrorRegistry::new([collision]).is_err());
}

proptest! {
    #[test]
    fn plausible_compiler_envelopes_verify_without_graph_parsing(
        payload in proptest::collection::vec(any::<u8>(), 1..4096),
        identity in any::<[u8; 32]>(),
        shape in "[a-zA-Z][a-zA-Z0-9_.:{}\\[\\]-]{0,127}",
    ) {
        let verified = verify_program(envelope(&payload, identity, &shape), identity, &shape);
        prop_assert!(verified.is_ok());
    }

    #[test]
    fn arbitrary_envelope_fields_never_panic(
        payload in proptest::collection::vec(any::<u8>(), 0..8192),
        identity in any::<[u8; 32]>(),
        expected_identity in any::<[u8; 32]>(),
        shape in ".{0,512}",
        expected_shape in ".{0,512}",
        schema_version in any::<u16>(),
        features in any::<u64>(),
    ) {
        let mut candidate = envelope(&payload, identity, &shape);
        candidate.header.versions.schema_program = schema_version;
        candidate.header.feature_bitmap = features;
        let result = std::panic::catch_unwind(|| {
            let _ = verify_program(candidate, expected_identity, &expected_shape);
        });
        prop_assert!(result.is_ok());
    }
}
