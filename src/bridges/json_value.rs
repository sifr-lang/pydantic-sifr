use pydantic_sifr_core::NativeValue;
use sifr_runtime::interop::structural::{
    ArenaNode, NodeId, StructuralArena, StructuralConstruct, StructuralEdgeKind, StructuralKind,
    StructuralNodeEdge, StructuralScalar, StructuralType, structural_construct,
};

const JSON_VALUE_IDENTITY: &str = "sifr.json.JsonValue";

pub(crate) fn construct_json_object<Output>(value: &NativeValue) -> Result<Output, String>
where
    Output: StructuralConstruct + StructuralType,
{
    let NativeValue::Object(entries) = value else {
        return Err("model_dump requires an object serializer result".to_owned());
    };
    let mut builder = JsonValueArenaBuilder { nodes: Vec::new() };
    let mut edges = Vec::with_capacity(entries.len().saturating_mul(2));
    for (index, (name, value)) in entries.iter().enumerate() {
        let key = builder.string(name.clone())?;
        let value = builder.json_value(value)?;
        edges.push(StructuralNodeEdge::new(
            StructuralEdgeKind::MappingKey(index),
            key,
        ));
        edges.push(StructuralNodeEdge::new(
            StructuralEdgeKind::MappingValue(index),
            value,
        ));
    }
    let root = builder.push(ArenaNode::aggregate(StructuralKind::Mapping, None, edges))?;
    let arena = StructuralArena::seal(Output::shape_identity(), root, builder.nodes)
        .map_err(|error| error.to_string())?;
    structural_construct(arena).map_err(|error| error.to_string())
}

struct JsonValueArenaBuilder {
    nodes: Vec<ArenaNode>,
}

impl JsonValueArenaBuilder {
    fn json_value(&mut self, value: &NativeValue) -> Result<NodeId, String> {
        let mut kind = "null";
        let mut bool_value = None;
        let mut int_value = None;
        let mut float_value = None;
        let mut str_value = None;
        let mut array_items = Vec::new();
        let mut object_items = Vec::new();
        match value {
            NativeValue::Null => {}
            NativeValue::Bool(value) => {
                kind = "bool";
                bool_value = Some(*value);
            }
            NativeValue::Integer(value) => {
                kind = "int";
                int_value = Some(value.parse::<i64>().map_err(|_| {
                    "model_dump integer is outside the Sifr bridge range".to_owned()
                })?);
            }
            NativeValue::Float(value) if value.is_finite() => {
                kind = "float";
                float_value = Some(*value);
            }
            NativeValue::String(value)
            | NativeValue::Decimal(value)
            | NativeValue::Date(value)
            | NativeValue::Time(value)
            | NativeValue::DateTime(value)
            | NativeValue::Duration(value)
            | NativeValue::Uuid(value)
            | NativeValue::Url(value) => {
                kind = "str";
                str_value = Some(value.clone());
            }
            NativeValue::Bytes(value) => {
                kind = "str";
                str_value = Some(
                    String::from_utf8(value.clone())
                        .map_err(|_| "model_dump cannot encode non-UTF-8 bytes".to_owned())?,
                );
            }
            NativeValue::Pattern { source, .. } => {
                kind = "str";
                str_value = Some(source.clone());
            }
            NativeValue::Fraction {
                numerator,
                denominator,
            } => {
                kind = "str";
                str_value = Some(format!("{numerator}/{denominator}"));
            }
            NativeValue::Complex { real, imaginary } => {
                kind = "str";
                str_value = Some(format!("{real}{imaginary:+}j"));
            }
            NativeValue::List(values)
            | NativeValue::Tuple(values)
            | NativeValue::Set(values)
            | NativeValue::FrozenSet(values) => {
                kind = "array";
                for value in values {
                    array_items.push(self.json_value(value)?);
                }
            }
            NativeValue::Object(entries) => {
                kind = "object";
                for (name, value) in entries {
                    object_items.push((name.clone(), self.json_value(value)?));
                }
            }
            NativeValue::Mapping(entries) => {
                kind = "object";
                for (key, value) in entries {
                    let NativeValue::String(name) = key else {
                        return Err("model_dump mapping keys must be strings".to_owned());
                    };
                    object_items.push((name.clone(), self.json_value(value)?));
                }
            }
            NativeValue::Float(_) => {
                return Err("model_dump cannot encode a non-finite float".to_owned());
            }
        }

        let kind = self.string(kind.to_owned())?;
        let bool_value = self.optional_bool(bool_value)?;
        let int_value = self.optional_int(int_value)?;
        let float_value = self.optional_float(float_value)?;
        let str_value = self.optional_string(str_value)?;
        let array_items = self.sequence(array_items)?;
        let object_items = self.object_sequence(object_items)?;
        self.push(ArenaNode::aggregate(
            StructuralKind::Record,
            Some(JSON_VALUE_IDENTITY),
            vec![
                field("kind", kind),
                field("bool_value", bool_value),
                field("int_value", int_value),
                field("float_value", float_value),
                field("str_value", str_value),
                field("array_items", array_items),
                field("object_items", object_items),
            ],
        ))
    }

    fn optional_bool(&mut self, value: Option<bool>) -> Result<NodeId, String> {
        let child = value
            .map(|value| {
                self.push(ArenaNode::scalar(
                    StructuralKind::Bool,
                    StructuralScalar::Bool(value),
                ))
            })
            .transpose()?;
        self.optional(child)
    }

    fn optional_int(&mut self, value: Option<i64>) -> Result<NodeId, String> {
        let child = value
            .map(|value| {
                self.push(ArenaNode::scalar(
                    StructuralKind::SignedInteger,
                    StructuralScalar::SignedInteger {
                        value: i128::from(value),
                        width: 64,
                    },
                ))
            })
            .transpose()?;
        self.optional(child)
    }

    fn optional_float(&mut self, value: Option<f64>) -> Result<NodeId, String> {
        let child = value
            .map(|value| {
                self.push(ArenaNode::scalar(
                    StructuralKind::Float,
                    StructuralScalar::Float(value),
                ))
            })
            .transpose()?;
        self.optional(child)
    }

    fn optional_string(&mut self, value: Option<String>) -> Result<NodeId, String> {
        let child = value.map(|value| self.string(value)).transpose()?;
        self.optional(child)
    }

    fn optional(&mut self, child: Option<NodeId>) -> Result<NodeId, String> {
        let edges = child
            .map(|child| {
                vec![StructuralNodeEdge::new(
                    StructuralEdgeKind::ActiveMember {
                        name: "some",
                        index: 0,
                    },
                    child,
                )]
            })
            .unwrap_or_default();
        self.push(ArenaNode::aggregate(StructuralKind::Optional, None, edges))
    }

    fn sequence(&mut self, items: Vec<NodeId>) -> Result<NodeId, String> {
        let edges = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| StructuralNodeEdge::new(StructuralEdgeKind::Index(index), item))
            .collect();
        self.push(ArenaNode::aggregate(StructuralKind::Sequence, None, edges))
    }

    fn object_sequence(&mut self, items: Vec<(String, NodeId)>) -> Result<NodeId, String> {
        let mut tuples = Vec::with_capacity(items.len());
        for (name, value) in items {
            let name = self.string(name)?;
            let tuple = self.push(ArenaNode::aggregate(
                StructuralKind::Tuple,
                None,
                vec![
                    StructuralNodeEdge::new(StructuralEdgeKind::Index(0), name),
                    StructuralNodeEdge::new(StructuralEdgeKind::Index(1), value),
                ],
            ))?;
            tuples.push(tuple);
        }
        self.sequence(tuples)
    }

    fn string(&mut self, value: String) -> Result<NodeId, String> {
        self.push(ArenaNode::scalar(
            StructuralKind::String,
            StructuralScalar::String(value),
        ))
    }

    fn push(&mut self, node: ArenaNode) -> Result<NodeId, String> {
        let index = u32::try_from(self.nodes.len())
            .map_err(|_| "model_dump output exceeds the structural arena limit".to_owned())?;
        self.nodes.push(node);
        Ok(NodeId::new(index))
    }
}

const fn field(name: &'static str, node: NodeId) -> StructuralNodeEdge<'static> {
    StructuralNodeEdge::new(StructuralEdgeKind::RecordField(name), node)
}
