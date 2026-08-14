use sifr_runtime::interop::structural::StaticProgramValue;

use super::{SchemaRef, StaticSchemaRef, field, list, record, string};
use crate::validation::{SchemaErrorOverride, ValidationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaticMetadata {
    pub(crate) key: &'static str,
    pub(crate) value: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticVariant {
    pub(crate) name: &'static str,
    pub(crate) discriminant: i64,
    pub(crate) metadata: Vec<StaticMetadata>,
}

pub(super) fn metadata(schema: SchemaRef<'_>) -> Result<Vec<StaticMetadata>, ValidationError> {
    let schema = static_schema(schema)?;
    parse_metadata(field(schema.node_record()?, "metadata")?)
}

pub(super) fn variants(schema: SchemaRef<'_>) -> Result<Vec<StaticVariant>, ValidationError> {
    let schema = static_schema(schema)?;
    list(field(schema.node_record()?, "variants")?, "schema variants")?
        .iter()
        .map(|value| {
            let variant = record(value, "schema variant")?;
            let discriminant = match field(variant, "value")? {
                StaticProgramValue::Integer(value) | StaticProgramValue::String(value) => value
                    .parse()
                    .map_err(|_| super::schema_error("Static enum discriminant is invalid"))?,
                _ => return Err(super::schema_error("Static enum discriminant is missing")),
            };
            Ok(StaticVariant {
                name: string(field(variant, "name")?, "enum variant name")?,
                discriminant,
                metadata: parse_metadata(field(variant, "metadata")?)?,
            })
        })
        .collect()
}

pub(super) fn definition(schema: SchemaRef<'_>) -> Result<&'static str, ValidationError> {
    let schema = static_schema(schema)?;
    string(
        field(schema.node_record()?, "definition")?,
        "schema definition",
    )
}

pub(super) fn error_override(
    schema: SchemaRef<'_>,
) -> Result<Option<SchemaErrorOverride>, ValidationError> {
    let schema = static_schema(schema)?;
    let record = schema.node_record()?;
    match (
        field(record, "error_code")?,
        field(record, "error_message")?,
    ) {
        (StaticProgramValue::None, StaticProgramValue::None) => Ok(None),
        (code, message) => Ok(Some(SchemaErrorOverride {
            code: string(code, "schema error code")?,
            message: string(message, "schema error message")?,
        })),
    }
}

fn parse_metadata(
    value: &'static StaticProgramValue,
) -> Result<Vec<StaticMetadata>, ValidationError> {
    list(value, "schema metadata")?
        .iter()
        .map(|value| {
            let metadata = record(value, "schema metadata item")?;
            Ok(StaticMetadata {
                key: string(field(metadata, "key")?, "metadata key")?,
                value: string(field(metadata, "value")?, "metadata value")?,
            })
        })
        .collect()
}

fn static_schema(schema: SchemaRef<'_>) -> Result<StaticSchemaRef, ValidationError> {
    match schema {
        SchemaRef::Static(schema) => Ok(schema),
        SchemaRef::Owned(_) => Err(super::schema_error("Schema is not a static schema")),
    }
}
