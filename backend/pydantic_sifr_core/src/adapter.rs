use core::{fmt, marker::PhantomData};

use sifr_runtime::interop::structural::{StructuralConstruct, StructuralProject, StructuralType};

use crate::{
    JsonIntegerProfile, JsonLimits, JsonSchemaError, JsonSchemaOptions, NativeValue,
    PreparedSchema, Schema, SerializationError, SerializationOptions, SerializationPlan,
    SerializationPlanError, ValidationError, ValidationOptions, generate_json_schema,
    serialize_json, serialize_structural, validate_json_and_construct,
    validate_native_and_construct, validate_strings_and_construct,
    validate_structural_and_construct,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeAdapterBuildErrorKind {
    InvalidSchema,
    ShapeMismatch,
    InvalidSerializationPlan,
}

#[derive(Debug)]
pub struct TypeAdapterBuildError {
    kind: TypeAdapterBuildErrorKind,
    message: String,
}

impl TypeAdapterBuildError {
    #[must_use]
    pub const fn kind(&self) -> TypeAdapterBuildErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(kind: TypeAdapterBuildErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for TypeAdapterBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TypeAdapterBuildError {}

pub struct TypeAdapter<'schema, T> {
    schema: &'schema Schema,
    prepared: PreparedSchema<'schema>,
    serializer: SerializationPlan,
    target: PhantomData<fn() -> T>,
}

impl<'schema, T: StructuralType> TypeAdapter<'schema, T> {
    pub fn new(
        schema: &'schema Schema,
        integer_profile: JsonIntegerProfile,
    ) -> Result<Self, TypeAdapterBuildError> {
        let prepared = PreparedSchema::new(schema).map_err(invalid_schema)?;
        if prepared.structural_identity() != T::shape_identity() {
            return Err(TypeAdapterBuildError::new(
                TypeAdapterBuildErrorKind::ShapeMismatch,
                "adapter target does not match the prepared structural shape",
            ));
        }
        let serializer = SerializationPlan::from_prepared(prepared, integer_profile)
            .map_err(invalid_serialization_plan)?;
        Ok(Self {
            schema,
            prepared,
            serializer,
            target: PhantomData,
        })
    }

    #[must_use]
    pub const fn prepared_schema(&self) -> PreparedSchema<'schema> {
        self.prepared
    }

    #[must_use]
    pub const fn serialization_plan(&self) -> &SerializationPlan {
        &self.serializer
    }

    pub fn json_schema(
        &self,
        options: JsonSchemaOptions,
    ) -> Result<serde_json::Value, JsonSchemaError> {
        generate_json_schema(self.schema, options, self.serializer.integer_profile())
    }
}

impl<T: StructuralConstruct> TypeAdapter<'_, T> {
    pub fn validate_json(
        &self,
        input: &[u8],
        input_limits: JsonLimits,
        options: ValidationOptions,
    ) -> Result<T, ValidationError> {
        validate_json_and_construct(&self.prepared, input, input_limits, options)
    }

    pub fn validate_native(
        &self,
        input: &NativeValue,
        input_limits: JsonLimits,
        options: ValidationOptions,
    ) -> Result<T, ValidationError> {
        validate_native_and_construct(&self.prepared, input, input_limits, options)
    }

    pub fn validate_strings(
        &self,
        input: &NativeValue,
        input_limits: JsonLimits,
        options: ValidationOptions,
    ) -> Result<T, ValidationError> {
        validate_strings_and_construct(&self.prepared, input, input_limits, options)
    }

    pub fn validate_structural<Input: StructuralProject>(
        &self,
        input: &Input,
        input_limits: JsonLimits,
        options: ValidationOptions,
    ) -> Result<T, ValidationError> {
        validate_structural_and_construct(&self.prepared, input, input_limits, options)
    }
}

impl<T: StructuralProject> TypeAdapter<'_, T> {
    pub fn dump_json(
        &self,
        value: &T,
        options: &SerializationOptions,
    ) -> Result<Vec<u8>, SerializationError> {
        serialize_json(&self.serializer, value, options)
    }

    pub fn dump_structural(
        &self,
        value: &T,
        options: &SerializationOptions,
    ) -> Result<NativeValue, SerializationError> {
        serialize_structural(&self.serializer, value, options)
    }
}

fn invalid_schema(error: ValidationError) -> TypeAdapterBuildError {
    TypeAdapterBuildError::new(TypeAdapterBuildErrorKind::InvalidSchema, error.to_string())
}

fn invalid_serialization_plan(error: SerializationPlanError) -> TypeAdapterBuildError {
    TypeAdapterBuildError::new(
        TypeAdapterBuildErrorKind::InvalidSerializationPlan,
        error.to_string(),
    )
}
