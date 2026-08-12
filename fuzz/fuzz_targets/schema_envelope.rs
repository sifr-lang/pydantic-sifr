#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{
    CompilerProgramEnvelope, ContractVersions, ProgramHeader, verify_program,
};

fuzz_target!(|data: &[u8]| {
    let mut identity = [0_u8; 32];
    let identity_count = data.len().min(identity.len());
    identity[..identity_count].copy_from_slice(&data[..identity_count]);

    let mut expected_identity = [0_u8; 32];
    let expected_start = identity_count;
    let expected_end = data.len().min(expected_start + expected_identity.len());
    expected_identity[..expected_end - expected_start]
        .copy_from_slice(&data[expected_start..expected_end]);

    let payload = &data[expected_end..];
    let shape = String::from_utf8_lossy(payload);
    let envelope = CompilerProgramEnvelope {
        header: ProgramHeader {
            versions: ContractVersions {
                schema_program: u16::from(data.first().copied().unwrap_or_default()),
                structural_contract: u16::from(data.get(1).copied().unwrap_or_default()),
                structural_call: u16::from(data.get(2).copied().unwrap_or_default()),
                callback_abi: u16::from(data.get(3).copied().unwrap_or_default()),
            },
            feature_bitmap: u64::from(data.get(4).copied().unwrap_or_default()),
            shape_identity: &shape,
            program_identity: identity,
        },
        canonical_bytes: payload,
    };
    let _ = verify_program(envelope, expected_identity, &shape);
});
