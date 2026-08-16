use core::fmt;

use sifr_runtime::interop::structural::ShapeIdentity;

use crate::{PreparedSchema, SchemaTag, validation::SchemaRef};

const MAX_SERIALIZER_PLAN_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SerializerNodeId(u32);

impl SerializerNodeId {
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializerFieldPlan {
    name: &'static str,
    node: SerializerNodeId,
}

impl SerializerFieldPlan {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn node(&self) -> SerializerNodeId {
        self.node
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializerNode {
    tag: SchemaTag,
    children: Vec<SerializerNodeId>,
    fields: Vec<SerializerFieldPlan>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializationPlan {
    structural_identity: ShapeIdentity,
    root: SerializerNodeId,
    nodes: Vec<SerializerNode>,
}

impl SerializationPlan {
    pub fn from_prepared(schema: PreparedSchema<'_>) -> Result<Self, SerializationPlanError> {
        let mut builder = PlanBuilder { nodes: Vec::new() };
        let root = builder.compile(schema.schema(), 0)?;
        Ok(Self {
            structural_identity: schema.structural_identity(),
            root,
            nodes: builder.nodes,
        })
    }

    #[must_use]
    pub const fn structural_identity(&self) -> ShapeIdentity {
        self.structural_identity
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
}

impl PlanBuilder {
    fn compile(
        &mut self,
        schema: SchemaRef<'_>,
        depth: usize,
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
                let child = self.compile(field.schema().map_err(validation_error)?, depth + 1)?;
                children.push(child);
                fields.push(SerializerFieldPlan {
                    name: field.name().map_err(validation_error)?,
                    node: child,
                });
            }
            (children, fields)
        } else {
            let count = schema.child_count().map_err(validation_error)?;
            let mut children = Vec::with_capacity(count);
            for child in 0..count {
                children
                    .push(self.compile(schema.child(child).map_err(validation_error)?, depth + 1)?);
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

fn validation_error(error: crate::ValidationError) -> SerializationPlanError {
    plan_error(error.to_string())
}

fn plan_error(message: impl Into<String>) -> SerializationPlanError {
    SerializationPlanError {
        message: message.into(),
    }
}
