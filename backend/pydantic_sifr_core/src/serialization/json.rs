use sifr_runtime::interop::structural::{
    StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind, StructuralProject,
    StructuralScalarRef, StructuralVisitor, VisitControl,
};

use crate::JsonLimits;

use super::{SerializationError, SerializationErrorKind, SerializationPlan, output::verify_shape};

const HARD_MAX_DEPTH: usize = 256;

pub fn serialize_json<T: StructuralProject>(
    plan: &SerializationPlan,
    value: &T,
    limits: JsonLimits,
) -> Result<Vec<u8>, SerializationError> {
    verify_shape::<T>(plan)?;
    validate_limits(limits)?;
    let mut writer = JsonWriter {
        output: Vec::new(),
        limits,
        nodes: 0,
        string_bytes: 0,
        frames: Vec::new(),
        root_complete: false,
    };
    value.structural_project(&mut writer)?;
    if !writer.frames.is_empty() || !writer.root_complete {
        return Err(projection_error(
            "structural projection did not produce one complete JSON value",
        ));
    }
    writer.check_output_size()?;
    Ok(writer.output)
}

struct JsonWriter<'value> {
    output: Vec<u8>,
    limits: JsonLimits,
    nodes: usize,
    string_bytes: usize,
    frames: Vec<JsonFrame<'value>>,
    root_complete: bool,
}

struct JsonFrame<'value> {
    kind: StructuralKind,
    child_count: usize,
    completed: usize,
    pending_edge: Option<StructuralEdgeKind<'value>>,
}

impl<'value> StructuralVisitor<'value> for JsonWriter<'value> {
    type Error = SerializationError;

    fn enter(&mut self, event: StructuralEnter<'value>) -> Result<VisitControl, Self::Error> {
        self.prepare_value(false)?;
        self.add_node()?;
        if self.frames.len() > self.limits.max_depth {
            return Err(limit_error("maximum depth"));
        }
        self.check_aggregate(event.kind(), event.child_count())?;
        match event.kind() {
            StructuralKind::Record | StructuralKind::Mapping => self.output.push(b'{'),
            StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => self.output.push(b'['),
            StructuralKind::Optional
            | StructuralKind::Union
            | StructuralKind::Enum
            | StructuralKind::Refined => {}
            _ => {
                return Err(projection_error(
                    "structural aggregate kind is not supported",
                ));
            }
        }
        self.frames.push(JsonFrame {
            kind: event.kind(),
            child_count: event.child_count(),
            completed: 0,
            pending_edge: None,
        });
        self.check_output_size()?;
        Ok(VisitControl::Continue)
    }

    fn edge(&mut self, edge: StructuralEdge<'value>) -> Result<(), Self::Error> {
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| projection_error("structural edge has no parent aggregate"))?;
        if frame.completed >= frame.child_count || frame.pending_edge.replace(edge.kind()).is_some()
        {
            return Err(projection_error("structural edge has no projected child"));
        }
        Ok(())
    }

    fn scalar(&mut self, value: StructuralScalarRef<'value>) -> Result<(), Self::Error> {
        let is_string = matches!(value, StructuralScalarRef::String(_));
        self.prepare_value(is_string)?;
        self.add_node()?;
        match value {
            StructuralScalarRef::None => self.output.extend_from_slice(b"null"),
            StructuralScalarRef::Bool(true) => self.output.extend_from_slice(b"true"),
            StructuralScalarRef::Bool(false) => self.output.extend_from_slice(b"false"),
            StructuralScalarRef::SignedInteger { value, .. } => self.write_integer(value)?,
            StructuralScalarRef::UnsignedInteger { value, .. } => self.write_integer(value)?,
            StructuralScalarRef::ExactInteger(value) => self.write_integer(value)?,
            StructuralScalarRef::Float(value) if value.is_finite() => {
                serde_json::to_writer(&mut self.output, &value)
                    .map_err(|_| projection_error("finite float could not be encoded"))?;
            }
            StructuralScalarRef::Float(_) => {
                return Err(SerializationError::new(
                    SerializationErrorKind::UnsupportedJsonValue,
                    "non-finite floats are not JSON values",
                ));
            }
            StructuralScalarRef::String(value) => self.write_string(value)?,
            StructuralScalarRef::Bytes(_) => {
                return Err(SerializationError::new(
                    SerializationErrorKind::UnsupportedJsonValue,
                    "bytes require an explicit JSON output policy",
                ));
            }
            _ => return Err(projection_error("structural scalar kind is not supported")),
        }
        self.complete_value()?;
        self.check_output_size()
    }

    fn exit(&mut self, kind: StructuralKind) -> Result<(), Self::Error> {
        let frame = self
            .frames
            .pop()
            .ok_or_else(|| projection_error("structural aggregate exit has no matching entry"))?;
        if frame.kind != kind
            || frame.pending_edge.is_some()
            || frame.completed != frame.child_count
        {
            return Err(projection_error(
                "structural aggregate events are unbalanced",
            ));
        }
        match kind {
            StructuralKind::Record | StructuralKind::Mapping => self.output.push(b'}'),
            StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => self.output.push(b']'),
            StructuralKind::Optional if frame.child_count == 0 => {
                self.output.extend_from_slice(b"null");
            }
            StructuralKind::Optional
            | StructuralKind::Union
            | StructuralKind::Enum
            | StructuralKind::Refined => {}
            _ => {
                return Err(projection_error(
                    "structural aggregate kind is not supported",
                ));
            }
        }
        self.complete_value()?;
        self.check_output_size()
    }
}

impl JsonWriter<'_> {
    fn prepare_value(&mut self, string_scalar: bool) -> Result<(), SerializationError> {
        let Some(frame) = self.frames.last_mut() else {
            if self.root_complete {
                return Err(projection_error(
                    "structural projection produced multiple roots",
                ));
            }
            return Ok(());
        };
        let edge = frame
            .pending_edge
            .take()
            .ok_or_else(|| projection_error("structural child has no declared edge"))?;
        match frame.kind {
            StructuralKind::Record => {
                let StructuralEdgeKind::RecordField(name) = edge else {
                    return Err(projection_error("structural record edge is invalid"));
                };
                if frame.completed > 0 {
                    self.output.push(b',');
                }
                self.string_bytes = self
                    .string_bytes
                    .checked_add(name.len())
                    .ok_or_else(|| limit_error("total string bytes"))?;
                if self.string_bytes > self.limits.max_string_bytes {
                    return Err(limit_error("total string bytes"));
                }
                write_json_string(&mut self.output, name)?;
                self.output.push(b':');
            }
            StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => {
                if edge != StructuralEdgeKind::Index(frame.completed) {
                    return Err(projection_error("structural sequence edge is invalid"));
                }
                if frame.completed > 0 {
                    self.output.push(b',');
                }
            }
            StructuralKind::Mapping if frame.completed.is_multiple_of(2) => {
                let index = frame.completed / 2;
                if edge != StructuralEdgeKind::MappingKey(index) || !string_scalar {
                    return Err(SerializationError::new(
                        SerializationErrorKind::UnsupportedJsonValue,
                        "JSON mapping keys must be structural strings",
                    ));
                }
                if index > 0 {
                    self.output.push(b',');
                }
            }
            StructuralKind::Mapping => {
                let index = frame.completed / 2;
                if edge != StructuralEdgeKind::MappingValue(index) {
                    return Err(projection_error("structural mapping value edge is invalid"));
                }
                self.output.push(b':');
            }
            StructuralKind::Optional
            | StructuralKind::Union
            | StructuralKind::Enum
            | StructuralKind::Refined => {
                if !matches!(edge, StructuralEdgeKind::ActiveMember { .. }) {
                    return Err(projection_error("structural active-member edge is invalid"));
                }
            }
            _ => {
                return Err(projection_error(
                    "structural aggregate kind is not supported",
                ));
            }
        }
        Ok(())
    }

    fn complete_value(&mut self) -> Result<(), SerializationError> {
        if let Some(frame) = self.frames.last_mut() {
            frame.completed = frame
                .completed
                .checked_add(1)
                .ok_or_else(|| limit_error("maximum node count"))?;
            if frame.completed > frame.child_count {
                return Err(projection_error(
                    "structural aggregate has too many children",
                ));
            }
        } else if core::mem::replace(&mut self.root_complete, true) {
            return Err(projection_error(
                "structural projection produced multiple roots",
            ));
        }
        Ok(())
    }

    fn check_aggregate(
        &self,
        kind: StructuralKind,
        child_count: usize,
    ) -> Result<(), SerializationError> {
        let items = match kind {
            StructuralKind::Record
            | StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => child_count,
            StructuralKind::Mapping if child_count.is_multiple_of(2) => child_count / 2,
            StructuralKind::Mapping => {
                return Err(projection_error(
                    "structural mapping child count is invalid",
                ));
            }
            StructuralKind::Optional if child_count <= 1 => child_count,
            StructuralKind::Union | StructuralKind::Enum | StructuralKind::Refined
                if child_count == 1 =>
            {
                child_count
            }
            _ => {
                return Err(projection_error(
                    "structural aggregate child count is invalid",
                ));
            }
        };
        if items > self.limits.max_collection_items {
            Err(limit_error("maximum collection items"))
        } else {
            Ok(())
        }
    }

    fn add_node(&mut self) -> Result<(), SerializationError> {
        if self.nodes >= self.limits.max_nodes {
            return Err(limit_error("maximum node count"));
        }
        self.nodes += 1;
        Ok(())
    }

    fn write_integer(&mut self, value: impl core::fmt::Display) -> Result<(), SerializationError> {
        let text = value.to_string();
        if text.trim_start_matches(['-', '+']).len() > self.limits.max_integer_digits {
            return Err(limit_error("maximum integer digits"));
        }
        self.output.extend_from_slice(text.as_bytes());
        Ok(())
    }

    fn write_string(&mut self, value: &str) -> Result<(), SerializationError> {
        self.string_bytes = self
            .string_bytes
            .checked_add(value.len())
            .ok_or_else(|| limit_error("total string bytes"))?;
        if self.string_bytes > self.limits.max_string_bytes {
            return Err(limit_error("total string bytes"));
        }
        write_json_string(&mut self.output, value)
    }

    fn check_output_size(&self) -> Result<(), SerializationError> {
        if self.output.len() > self.limits.max_input_bytes {
            Err(limit_error("maximum JSON output bytes"))
        } else {
            Ok(())
        }
    }
}

fn write_json_string(output: &mut Vec<u8>, value: &str) -> Result<(), SerializationError> {
    serde_json::to_writer(output, value)
        .map_err(|_| projection_error("string could not be encoded as JSON"))
}

fn validate_limits(limits: JsonLimits) -> Result<(), SerializationError> {
    if limits.max_input_bytes == 0
        || limits.max_depth == 0
        || limits.max_nodes == 0
        || limits.max_string_bytes == 0
        || limits.max_integer_digits == 0
        || limits.max_collection_items == 0
        || limits.max_depth > HARD_MAX_DEPTH
    {
        Err(limit_error("limits must be greater than zero"))
    } else {
        Ok(())
    }
}

fn projection_error(message: &'static str) -> SerializationError {
    SerializationError::new(SerializationErrorKind::InvalidProjection, message)
}

fn limit_error(message: &'static str) -> SerializationError {
    SerializationError::new(SerializationErrorKind::Limit, message)
}
