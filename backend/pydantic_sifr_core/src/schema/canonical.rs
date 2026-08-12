use sha2::{Digest, Sha256};

use super::SchemaProgram;

#[derive(serde::Serialize)]
struct CanonicalProgram<'a> {
    versions: super::ContractVersions,
    feature_bitmap: u64,
    shape_identity: &'a str,
    root: super::NodeIndex,
    nodes: &'a [super::SchemaNode],
}

pub fn canonical_payload(program: &SchemaProgram) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&CanonicalProgram {
        versions: program.header.versions,
        feature_bitmap: program.header.feature_bitmap,
        shape_identity: &program.header.shape_identity,
        root: program.root,
        nodes: &program.nodes,
    })
}

pub(crate) fn payload_sha256(program: &SchemaProgram) -> Result<String, serde_json::Error> {
    canonical_payload(program).map(|payload| hex::encode(Sha256::digest(payload)))
}
