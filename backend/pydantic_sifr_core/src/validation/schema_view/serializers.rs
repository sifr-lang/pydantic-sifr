use sifr_runtime::interop::structural::StaticProgramValue;

use super::{SchemaRef, bool_value, field, list, record, schema_error, string, usize_value};
use crate::validation::ValidationError;

#[derive(Clone, Copy, Debug)]
pub(crate) struct StaticSerializer {
    pub(crate) slot: usize,
    pub(crate) kind: &'static str,
    pub(crate) target: &'static str,
    pub(crate) when_used: &'static str,
    pub(crate) takes_receiver: bool,
    pub(crate) output: SchemaRef<'static>,
    pub(crate) alias: Option<&'static str>,
}

pub(crate) fn static_serializers(
    program: &'static StaticProgramValue,
) -> Result<Vec<StaticSerializer>, ValidationError> {
    let program_record = record(program, "static schema program")?;
    let Some((_, value)) = program_record
        .iter()
        .find(|(name, _)| *name == "serializers")
    else {
        return Ok(Vec::new());
    };
    let serializers = list(value, "schema serializers")?;
    let mut output = Vec::with_capacity(serializers.len());
    for serializer in serializers {
        let serializer = record(serializer, "schema serializer")?;
        let output_node = usize_value(field(serializer, "output")?, "serializer output")?;
        let alias = match field(serializer, "alias")? {
            StaticProgramValue::None => None,
            value => Some(string(value, "serializer alias")?),
        };
        let item = StaticSerializer {
            slot: usize_value(field(serializer, "slot")?, "serializer slot")?,
            kind: string(field(serializer, "kind")?, "serializer kind")?,
            target: string(field(serializer, "target")?, "serializer target")?,
            when_used: string(field(serializer, "when_used")?, "serializer when-used mode")?,
            takes_receiver: bool_value(
                field(serializer, "takes_receiver")?,
                "serializer receiver mode",
            )?,
            output: static_program_schema_node(program, output_node)?,
            alias,
        };
        if !matches!(
            item.kind,
            "field-serializer" | "model-serializer" | "computed-field"
        ) || !matches!(
            item.when_used,
            "always" | "unless-none" | "json" | "json-unless-none"
        ) {
            return Err(schema_error("Static serializer metadata is invalid"));
        }
        output.push(item);
    }
    Ok(output)
}

fn static_program_schema_node(
    value: &'static StaticProgramValue,
    index: usize,
) -> Result<SchemaRef<'static>, ValidationError> {
    let program = record(value, "static schema program")?;
    let nodes = list(field(program, "nodes")?, "schema nodes")?;
    if index >= nodes.len() {
        return Err(schema_error("Static schema node is out of range"));
    }
    Ok(SchemaRef::Static(super::StaticSchemaRef { nodes, index }))
}
