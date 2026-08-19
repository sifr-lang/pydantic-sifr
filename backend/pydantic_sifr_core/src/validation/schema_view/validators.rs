use super::{SchemaRef, StaticSchemaRef, field, schema_error, usize_value};
use crate::validation::ValidationError;

impl SchemaRef<'_> {
    pub(crate) fn validator_slot(self) -> Result<usize, ValidationError> {
        match self {
            Self::Static(schema) => schema.validator_slot(),
            _ => Err(schema_error("Validator schema node is not static")),
        }
    }
}

impl StaticSchemaRef {
    fn validator_slot(self) -> Result<usize, ValidationError> {
        usize_value(
            field(self.node_record()?, "validator_slot")?,
            "validator slot",
        )
    }
}
