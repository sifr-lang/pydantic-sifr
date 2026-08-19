use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use sifr_runtime::interop::structural::ShapeIdentity;

use crate::InputId;

use super::{Schema, SchemaRef, ValidationError, ValidationState, ValueId, schema_view::SchemaTag};

#[derive(Clone, Debug, PartialEq)]
pub struct DefinitionSchema {
    pub name: &'static str,
    pub schema: Schema,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinitionsSchema {
    root: Box<Schema>,
    definitions: Vec<DefinitionSchema>,
    scope: BTreeMap<&'static str, Arc<Schema>>,
}

impl DefinitionsSchema {
    pub fn new(root: Schema, definitions: Vec<DefinitionSchema>) -> Result<Self, ValidationError> {
        let mut names = BTreeSet::new();
        for definition in &definitions {
            if definition.name.is_empty() || !names.insert(definition.name) {
                return Err(schema_error(
                    "Definition identities must be nonempty and unique",
                ));
            }
        }
        let scope = definitions
            .iter()
            .map(|definition| (definition.name, Arc::new(definition.schema.clone())))
            .collect();
        let schema = Self {
            root: Box::new(root),
            definitions,
            scope,
        };
        let mut visited = BTreeSet::new();
        verify_references(&schema.root, &schema.scope, &mut visited)?;
        for target in schema.scope.values() {
            verify_references(target.as_ref(), &schema.scope, &mut visited)?;
            root_identity(target.as_ref(), &schema.scope, &mut BTreeSet::new())?;
        }
        schema.structural_identity()?;
        Ok(schema)
    }

    #[must_use]
    pub fn root(&self) -> &Schema {
        &self.root
    }

    #[must_use]
    pub fn definitions(&self) -> &[DefinitionSchema] {
        &self.definitions
    }

    pub(crate) fn scope(&self) -> BTreeMap<&'static str, Arc<Schema>> {
        self.scope.clone()
    }

    pub(crate) fn definition(&self, name: &'static str) -> Option<&Schema> {
        self.scope.get(name).map(Arc::as_ref)
    }

    pub(crate) fn structural_identity(&self) -> Result<ShapeIdentity, ValidationError> {
        root_identity(&self.root, &self.scope, &mut BTreeSet::new())
    }
}

pub(crate) fn validate_definitions(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    match schema {
        SchemaRef::Owned(Schema::Definitions(definitions)) => {
            state.push_definition_scope(definitions.scope());
            let result = state.validate_node(SchemaRef::owned(definitions.root()), input_id, depth);
            state.pop_definition_scope();
            result
        }
        SchemaRef::Owned(Schema::DefinitionRef { name, .. }) => {
            let target = state.definition(name).ok_or_else(|| {
                schema_error("Definition reference is not present in its enclosing scope")
            })?;
            validate_reference(
                state,
                name,
                SchemaRef::owned(target.as_ref()),
                input_id,
                depth,
            )
        }
        SchemaRef::Static(_) if schema.tag()? == SchemaTag::DefinitionRef => {
            let name = schema.static_reference()?;
            let target = schema.static_definition_target()?;
            if !static_reference_target_is_supported(target)? {
                return Err(reference_target_error());
            }
            validate_reference(state, name, target, input_id, depth)
        }
        _ => Err(schema_error("Schema node is not a definition control node")),
    }
}

fn validate_reference(
    state: &mut ValidationState<'_>,
    name: &'static str,
    target: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    if !state.enter_reference(input_id, name) {
        return Err(super::scalars::type_error(
            "recursion_loop",
            "Input contains a cyclic definition reference",
            "acyclic recursive input",
        ));
    }
    let result = state.validate_node(target, input_id, depth + 1);
    state.leave_reference(input_id, name);
    result
}

fn root_identity(
    schema: &Schema,
    definitions: &BTreeMap<&'static str, Arc<Schema>>,
    active: &mut BTreeSet<&'static str>,
) -> Result<ShapeIdentity, ValidationError> {
    match schema {
        Schema::DefinitionRef { name, .. } => {
            if !active.insert(name) {
                return Err(schema_error("Definition references form an identity cycle"));
            }
            let target = definitions
                .get(name)
                .ok_or_else(|| schema_error("Definition reference is not declared"))?;
            let identity = root_identity(target.as_ref(), definitions, active);
            active.remove(name);
            identity
        }
        Schema::Definitions(_) => Err(schema_error("Definition scopes cannot be nested")),
        _ => schema.structural_identity_at(0),
    }
}

fn verify_references(
    schema: &Schema,
    definitions: &BTreeMap<&'static str, Arc<Schema>>,
    visited: &mut BTreeSet<&'static str>,
) -> Result<(), ValidationError> {
    match schema {
        Schema::DefinitionRef {
            name,
            structural_identity,
            sort_key,
        } => {
            let target = definitions
                .get(name)
                .ok_or_else(|| schema_error("Definition reference is not declared"))?;
            if !target.definition_reference_target_is_supported() {
                return Err(reference_target_error());
            }
            if target.structural_identity_at(0)? != *structural_identity
                || super::sum_schema::schema_sort_key(target.as_ref()) != *sort_key
            {
                return Err(schema_error(
                    "Definition reference identity does not match its target",
                ));
            }
            if visited.insert(name) {
                verify_references(target.as_ref(), definitions, visited)?;
            }
            Ok(())
        }
        Schema::Definitions(_) => Err(schema_error("Definition scopes cannot be nested")),
        Schema::Nullable(child) | Schema::EmbeddedJson(child) => {
            verify_references(child, definitions, visited)
        }
        Schema::LaxOrStrict(control) => {
            verify_references(control.lax(), definitions, visited)?;
            verify_references(control.strict(), definitions, visited)
        }
        Schema::JsonOrStructural(control) => {
            verify_references(control.json(), definitions, visited)?;
            verify_references(control.structural(), definitions, visited)
        }
        Schema::Chain(chain) => chain
            .steps()
            .iter()
            .try_for_each(|step| verify_references(step, definitions, visited)),
        Schema::Union(union) => union
            .choices()
            .iter()
            .try_for_each(|choice| verify_references(&choice.schema, definitions, visited)),
        Schema::TaggedUnion(union) => union
            .choices()
            .iter()
            .try_for_each(|choice| verify_references(&choice.schema, definitions, visited)),
        Schema::Model(model) => model
            .fields
            .iter()
            .try_for_each(|field| verify_references(&field.schema, definitions, visited)),
        Schema::List { item, .. }
        | Schema::Set { item, .. }
        | Schema::FrozenSet { item, .. }
        | Schema::Generator { item, .. } => verify_references(item, definitions, visited),
        Schema::Tuple(items) => items
            .iter()
            .try_for_each(|item| verify_references(item, definitions, visited)),
        Schema::Mapping { key, value, .. } => {
            verify_references(key, definitions, visited)?;
            verify_references(value, definitions, visited)
        }
        _ => Ok(()),
    }
}

fn schema_error(message: &'static str) -> ValidationError {
    super::scalars::type_error("schema_invalid", message, "valid definition scope")
}

fn static_reference_target_is_supported(target: SchemaRef<'_>) -> Result<bool, ValidationError> {
    Ok(!matches!(
        target.tag()?,
        SchemaTag::Literal
            | SchemaTag::Nullable
            | SchemaTag::Union
            | SchemaTag::TaggedUnion
            | SchemaTag::Definitions
            | SchemaTag::EmbeddedJson
    ))
}

fn reference_target_error() -> ValidationError {
    super::scalars::type_error(
        "schema_invalid",
        "Definition references cannot target flattened wrappers or definition scopes",
        "non-flattened definition target",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Arena, JsonLimits, parse_json};

    #[test]
    fn repeated_active_reference_returns_recursion_loop() {
        let input = parse_json(b"true", JsonLimits::default())
            .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
        let mut state = ValidationState {
            input: &input,
            values: Arena::new(),
            options: super::super::ValidationOptions::default(),
            definition_scopes: Arc::new(Vec::new()),
            active_references: Vec::new(),
            callbacks: None,
            skip_callbacks: false,
            serialization_input: false,
        };
        assert!(state.enter_reference(input.root(), "tests.Loop"));
        let error = match validate_reference(
            &mut state,
            "tests.Loop",
            SchemaRef::owned(&Schema::Bool),
            input.root(),
            0,
        ) {
            Ok(_) => panic!("expected recursion loop"),
            Err(error) => error,
        };
        assert_eq!(error.details()[0].code, "recursion_loop");
    }
}
