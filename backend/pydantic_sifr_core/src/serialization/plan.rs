use core::fmt;

use sifr_runtime::interop::structural::ShapeIdentity;
use sifr_runtime::json::JsonIntegerProfile;

use crate::{
    NativeValue, PreparedSchema, SchemaTag,
    validation::{SchemaRef, StaticSerializer, static_serializers},
};

use super::{SelectionPath, SelectionSegment};

const MAX_SERIALIZER_PLAN_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SerializerNodeId(u32);

impl SerializerNodeId {
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SerializerFieldPlan {
    name: &'static str,
    alias: Option<String>,
    default: Option<NativeValue>,
    node: SerializerNodeId,
}

impl SerializerFieldPlan {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    #[must_use]
    pub fn default(&self) -> Option<&NativeValue> {
        self.default.as_ref()
    }

    #[must_use]
    pub const fn node(&self) -> SerializerNodeId {
        self.node
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SerializerNode {
    tag: SchemaTag,
    children: Vec<SerializerNodeId>,
    fields: Vec<SerializerFieldPlan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerializerCallbackKind {
    Field,
    Model,
    Computed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerializerWhenUsed {
    Always,
    UnlessNone,
    Json,
    JsonUnlessNone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializerCallbackPlan {
    slot: usize,
    kind: SerializerCallbackKind,
    target: &'static str,
    when_used: SerializerWhenUsed,
    takes_receiver: bool,
    output: SerializerNodeId,
    alias: Option<&'static str>,
}

impl SerializerCallbackPlan {
    #[must_use]
    pub const fn slot(&self) -> usize {
        self.slot
    }

    #[must_use]
    pub const fn kind(&self) -> SerializerCallbackKind {
        self.kind
    }

    #[must_use]
    pub const fn target(&self) -> &'static str {
        self.target
    }

    #[must_use]
    pub const fn when_used(&self) -> SerializerWhenUsed {
        self.when_used
    }

    #[must_use]
    pub const fn takes_receiver(&self) -> bool {
        self.takes_receiver
    }

    #[must_use]
    pub const fn output(&self) -> SerializerNodeId {
        self.output
    }

    #[must_use]
    pub const fn alias(&self) -> Option<&'static str> {
        self.alias
    }
}

impl SerializerNode {
    #[must_use]
    pub const fn tag(&self) -> SchemaTag {
        self.tag
    }

    #[must_use]
    pub fn children(&self) -> &[SerializerNodeId] {
        &self.children
    }

    #[must_use]
    pub fn fields(&self) -> &[SerializerFieldPlan] {
        &self.fields
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SerializationPlan {
    structural_identity: ShapeIdentity,
    integer_profile: JsonIntegerProfile,
    root: SerializerNodeId,
    nodes: Vec<SerializerNode>,
    field_policies: Vec<FieldPolicy>,
    callbacks: Vec<SerializerCallbackPlan>,
}

impl SerializationPlan {
    pub fn from_prepared(
        schema: PreparedSchema<'_>,
        integer_profile: JsonIntegerProfile,
    ) -> Result<Self, SerializationPlanError> {
        let mut builder = PlanBuilder {
            nodes: Vec::new(),
            field_policies: Vec::new(),
        };
        let root = builder.compile(schema.schema(), 0, &mut Vec::new())?;
        let callbacks = schema
            .static_program()
            .map(static_serializers)
            .transpose()
            .map_err(validation_error)?
            .unwrap_or_default()
            .into_iter()
            .map(|serializer| compile_callback(&mut builder, serializer))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            structural_identity: schema.structural_identity(),
            integer_profile,
            root,
            nodes: builder.nodes,
            field_policies: builder.field_policies,
            callbacks,
        })
    }

    #[must_use]
    pub const fn structural_identity(&self) -> ShapeIdentity {
        self.structural_identity
    }

    #[must_use]
    pub const fn integer_profile(&self) -> JsonIntegerProfile {
        self.integer_profile
    }

    #[must_use]
    pub const fn root(&self) -> SerializerNodeId {
        self.root
    }

    #[must_use]
    pub fn nodes(&self) -> &[SerializerNode] {
        &self.nodes
    }

    #[must_use]
    pub fn node(&self, id: SerializerNodeId) -> Option<&SerializerNode> {
        self.nodes.get(id.raw() as usize)
    }

    #[must_use]
    pub fn callbacks(&self) -> &[SerializerCallbackPlan] {
        &self.callbacks
    }

    pub fn set_field_alias(
        &mut self,
        path: &SelectionPath,
        alias: impl Into<String>,
    ) -> Result<(), SerializationPlanError> {
        let alias = alias.into();
        if alias.is_empty() {
            return Err(plan_error("serialization alias must not be empty"));
        }
        let mut matches = self
            .field_policies
            .iter_mut()
            .filter(|policy| policy.matches(path.segments()));
        let Some(policy) = matches.next() else {
            return Err(plan_error(
                "serialization alias path does not name a model field",
            ));
        };
        if matches.next().is_some() {
            return Err(plan_error("serialization alias path is ambiguous"));
        }
        policy.alias = Some(alias.clone());
        if let Some(field) = self
            .nodes
            .iter_mut()
            .flat_map(|node| node.fields.iter_mut())
            .find(|field| field.node == policy.node && field.name == policy.name)
        {
            field.alias = Some(alias);
        }
        Ok(())
    }

    pub(super) fn field_policy(&self, path: &[SelectionSegment]) -> Option<&FieldPolicy> {
        self.field_policies
            .iter()
            .find(|policy| policy.matches(path))
    }
}

fn compile_callback(
    builder: &mut PlanBuilder,
    serializer: StaticSerializer,
) -> Result<SerializerCallbackPlan, SerializationPlanError> {
    let kind = match serializer.kind {
        "field-serializer" if !serializer.target.is_empty() => SerializerCallbackKind::Field,
        "model-serializer" if serializer.target.is_empty() => SerializerCallbackKind::Model,
        "computed-field" if !serializer.target.is_empty() => SerializerCallbackKind::Computed,
        _ => return Err(plan_error("serializer callback target is invalid")),
    };
    let when_used = match serializer.when_used {
        "always" => SerializerWhenUsed::Always,
        "unless-none" => SerializerWhenUsed::UnlessNone,
        "json" => SerializerWhenUsed::Json,
        "json-unless-none" => SerializerWhenUsed::JsonUnlessNone,
        _ => return Err(plan_error("serializer callback mode is invalid")),
    };
    let mut output_path = match kind {
        SerializerCallbackKind::Field | SerializerCallbackKind::Computed => {
            vec![PlanPathSegment::Field(serializer.target)]
        }
        SerializerCallbackKind::Model => Vec::new(),
    };
    let output = builder.compile(serializer.output, 0, &mut output_path)?;
    if kind == SerializerCallbackKind::Computed {
        builder.field_policies.push(FieldPolicy {
            path: vec![PlanPathSegment::Field(serializer.target)],
            name: serializer.target,
            alias: serializer.alias.map(str::to_owned),
            default: None,
            node: output,
        });
    }
    Ok(SerializerCallbackPlan {
        slot: serializer.slot,
        kind,
        target: serializer.target,
        when_used,
        takes_receiver: serializer.takes_receiver,
        output,
        alias: serializer.alias,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializationPlanError {
    message: String,
}

impl SerializationPlanError {
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SerializationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SerializationPlanError {}

struct PlanBuilder {
    nodes: Vec<SerializerNode>,
    field_policies: Vec<FieldPolicy>,
}

impl PlanBuilder {
    fn compile(
        &mut self,
        schema: SchemaRef<'_>,
        depth: usize,
        path: &mut Vec<PlanPathSegment>,
    ) -> Result<SerializerNodeId, SerializationPlanError> {
        if depth > MAX_SERIALIZER_PLAN_DEPTH {
            return Err(plan_error("serializer plan exceeds the static depth limit"));
        }
        let tag = schema.tag().map_err(validation_error)?;
        let index = u32::try_from(self.nodes.len())
            .map_err(|_| plan_error("serializer plan exceeds the compact node limit"))?;
        let id = SerializerNodeId(index);
        self.nodes.push(SerializerNode {
            tag,
            children: Vec::new(),
            fields: Vec::new(),
        });

        let (children, fields) = if tag == SchemaTag::Model {
            let mut children = Vec::new();
            let mut fields = Vec::new();
            for field in schema
                .model()
                .map_err(validation_error)?
                .fields()
                .map_err(validation_error)?
            {
                if field.excluded().map_err(validation_error)? {
                    continue;
                }
                let name = field.name().map_err(validation_error)?;
                path.push(PlanPathSegment::Field(name));
                let child =
                    self.compile(field.schema().map_err(validation_error)?, depth + 1, path)?;
                let alias = field.serialization_alias();
                if alias.as_deref().is_some_and(str::is_empty) {
                    return Err(plan_error("serialization alias must not be empty"));
                }
                let default = field.materialized_default().map_err(validation_error)?;
                self.field_policies.push(FieldPolicy {
                    path: path.clone(),
                    name,
                    alias: alias.clone(),
                    default: default.clone(),
                    node: child,
                });
                path.pop();
                children.push(child);
                fields.push(SerializerFieldPlan {
                    name,
                    alias,
                    default,
                    node: child,
                });
            }
            (children, fields)
        } else {
            let count = schema.child_count().map_err(validation_error)?;
            let mut children = Vec::with_capacity(count);
            for child in 0..count {
                let pushed = collection_path_segment(tag, child);
                if let Some(segment) = pushed.clone() {
                    path.push(segment);
                }
                children.push(self.compile(
                    schema.child(child).map_err(validation_error)?,
                    depth + 1,
                    path,
                )?);
                if pushed.is_some() {
                    path.pop();
                }
            }
            (children, Vec::new())
        };

        let node = self
            .nodes
            .get_mut(id.raw() as usize)
            .ok_or_else(|| plan_error("serializer plan lost a reserved node"))?;
        node.children = children;
        node.fields = fields;
        Ok(id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PlanPathSegment {
    Field(&'static str),
    Index(Option<usize>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FieldPolicy {
    path: Vec<PlanPathSegment>,
    name: &'static str,
    alias: Option<String>,
    default: Option<NativeValue>,
    node: SerializerNodeId,
}

impl FieldPolicy {
    pub(super) fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    pub(super) fn default(&self) -> Option<&NativeValue> {
        self.default.as_ref()
    }

    fn matches(&self, path: &[SelectionSegment]) -> bool {
        self.path.len() == path.len()
            && self.path.iter().zip(path).all(|(planned, actual)| {
                matches!(
                    (planned, actual),
                    (PlanPathSegment::Field(left), SelectionSegment::Field(right)) if *left == right
                ) || matches!(
                    (planned, actual),
                    (PlanPathSegment::Index(None), SelectionSegment::Index(_))
                ) || matches!(
                    (planned, actual),
                    (PlanPathSegment::Index(Some(left)), SelectionSegment::Index(right)) if left == right
                )
            })
    }
}

const fn collection_path_segment(tag: SchemaTag, child: usize) -> Option<PlanPathSegment> {
    match tag {
        SchemaTag::List | SchemaTag::Set | SchemaTag::FrozenSet | SchemaTag::Generator => {
            Some(PlanPathSegment::Index(None))
        }
        SchemaTag::Tuple => Some(PlanPathSegment::Index(Some(child))),
        _ => None,
    }
}

fn validation_error(error: crate::ValidationError) -> SerializationPlanError {
    plan_error(error.to_string())
}

fn plan_error(message: impl Into<String>) -> SerializationPlanError {
    SerializationPlanError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use sifr_runtime::interop::structural::primitive;

    use super::*;
    use crate::{ExtraPolicy, ModelField, ModelSchema, Schema};

    #[test]
    fn structured_callback_policies_start_below_the_callback_target() {
        for kind in ["field-serializer", "computed-field"] {
            let output_schema = Box::leak(Box::new(Schema::Model(
                ModelSchema::new(
                    "Detail",
                    primitive("tests.Detail"),
                    vec![ModelField::required(
                        "label",
                        Schema::String(Default::default()),
                    )],
                    ExtraPolicy::Ignore,
                    false,
                    true,
                )
                .unwrap_or_else(|error| panic!("callback output schema failed: {error}")),
            )));
            let mut builder = PlanBuilder {
                nodes: Vec::new(),
                field_policies: Vec::new(),
            };

            compile_callback(
                &mut builder,
                StaticSerializer {
                    slot: 0,
                    kind,
                    target: "summary",
                    when_used: "always",
                    takes_receiver: true,
                    output: SchemaRef::owned(output_schema),
                    alias: None,
                },
            )
            .unwrap_or_else(|error| panic!("callback plan failed: {error}"));

            assert!(builder.field_policies.iter().any(|policy| {
                policy.matches(&[
                    SelectionSegment::Field("summary".to_owned()),
                    SelectionSegment::Field("label".to_owned()),
                ])
            }));
            assert!(
                !builder.field_policies.iter().any(|policy| {
                    policy.matches(&[SelectionSegment::Field("label".to_owned())])
                })
            );
        }
    }
}
