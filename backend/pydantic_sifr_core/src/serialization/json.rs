use sifr_runtime::interop::structural::{
    StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind, StructuralProject,
    StructuralScalarRef, StructuralVisitor, VisitControl,
};

use crate::JsonLimits;

use super::{
    SelectionSegment, SerializationError, SerializationErrorKind, SerializationOptions,
    SerializationPlan,
    output::{native_json_bytes, verify_shape},
    selection::selected,
};

const HARD_MAX_DEPTH: usize = 256;

pub fn serialize_json<T: StructuralProject>(
    plan: &SerializationPlan,
    value: &T,
    options: &SerializationOptions,
) -> Result<Vec<u8>, SerializationError> {
    verify_shape::<T>(plan)?;
    validate_limits(options.limits)?;
    let mut writer = JsonWriter {
        output: Vec::new(),
        raw_output: Vec::new(),
        plan,
        options,
        nodes: 0,
        string_bytes: 0,
        frames: Vec::new(),
        path: Vec::new(),
        root_complete: false,
    };
    value.structural_project(&mut writer)?;
    if !writer.frames.is_empty() || !writer.path.is_empty() || !writer.root_complete {
        return Err(projection_error(
            "structural projection did not produce one complete JSON value",
        ));
    }
    writer.check_output_size()?;
    Ok(writer.output)
}

struct JsonWriter<'value, 'config> {
    output: Vec<u8>,
    raw_output: Vec<u8>,
    plan: &'config SerializationPlan,
    options: &'config SerializationOptions,
    nodes: usize,
    string_bytes: usize,
    frames: Vec<JsonFrame<'value>>,
    path: Vec<SelectionSegment>,
    root_complete: bool,
}

struct JsonFrame<'value> {
    kind: StructuralKind,
    child_count: usize,
    completed: usize,
    emitted_children: usize,
    pending_edge: Option<StructuralEdgeKind<'value>>,
    prepared: PreparedValue,
}

#[derive(Clone, Copy)]
struct PreparedValue {
    emit: bool,
    prefix_start: usize,
    path_pushed: bool,
    capture_raw: bool,
    owns_capture: bool,
    raw_start: usize,
    raw_value_start: usize,
}

impl<'value, 'config> StructuralVisitor<'value> for JsonWriter<'value, 'config> {
    type Error = SerializationError;

    fn enter(&mut self, event: StructuralEnter<'value>) -> Result<VisitControl, Self::Error> {
        let prepared = self.prepare_value(false)?;
        self.add_node()?;
        if self.frames.len() > self.options.limits.max_depth {
            return Err(limit_error("maximum depth"));
        }
        self.check_aggregate(event.kind(), event.child_count())?;
        if prepared.emit {
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
                _ => return Err(unsupported_aggregate()),
            }
        }
        if prepared.capture_raw {
            match event.kind() {
                StructuralKind::Record | StructuralKind::Mapping => self.raw_output.push(b'{'),
                StructuralKind::Sequence
                | StructuralKind::Tuple
                | StructuralKind::Set
                | StructuralKind::FrozenSet => self.raw_output.push(b'['),
                StructuralKind::Optional
                | StructuralKind::Union
                | StructuralKind::Enum
                | StructuralKind::Refined => {}
                _ => return Err(unsupported_aggregate()),
            }
        }
        self.frames.push(JsonFrame {
            kind: event.kind(),
            child_count: event.child_count(),
            completed: 0,
            emitted_children: 0,
            pending_edge: None,
            prepared,
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
        let prepared = self.prepare_value(matches!(value, StructuralScalarRef::String(_)))?;
        self.add_node()?;
        if let StructuralScalarRef::String(value) = value {
            self.add_string_bytes(value.len())?;
        }
        let is_none = matches!(value, StructuralScalarRef::None);
        if prepared.emit {
            self.write_scalar(value)?;
        }
        if prepared.capture_raw {
            write_raw_scalar(&mut self.raw_output, value)?;
        }
        self.complete_value(prepared, is_none)?;
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
        let is_none = kind == StructuralKind::Optional && frame.child_count == 0;
        if frame.prepared.emit {
            match kind {
                StructuralKind::Record | StructuralKind::Mapping => self.output.push(b'}'),
                StructuralKind::Sequence
                | StructuralKind::Tuple
                | StructuralKind::Set
                | StructuralKind::FrozenSet => self.output.push(b']'),
                StructuralKind::Optional if is_none => self.output.extend_from_slice(b"null"),
                StructuralKind::Optional
                | StructuralKind::Union
                | StructuralKind::Enum
                | StructuralKind::Refined => {}
                _ => return Err(unsupported_aggregate()),
            }
        }
        if frame.prepared.capture_raw {
            match kind {
                StructuralKind::Record | StructuralKind::Mapping => self.raw_output.push(b'}'),
                StructuralKind::Sequence
                | StructuralKind::Tuple
                | StructuralKind::Set
                | StructuralKind::FrozenSet => self.raw_output.push(b']'),
                StructuralKind::Optional if is_none => {
                    self.raw_output.extend_from_slice(b"null");
                }
                StructuralKind::Optional
                | StructuralKind::Union
                | StructuralKind::Enum
                | StructuralKind::Refined => {}
                _ => return Err(unsupported_aggregate()),
            }
        }
        self.complete_value(frame.prepared, is_none)?;
        self.check_output_size()
    }
}

impl JsonWriter<'_, '_> {
    fn prepare_value(&mut self, string_scalar: bool) -> Result<PreparedValue, SerializationError> {
        let Some(parent_index) = self.frames.len().checked_sub(1) else {
            if self.root_complete {
                return Err(projection_error(
                    "structural projection produced multiple roots",
                ));
            }
            return Ok(PreparedValue {
                emit: true,
                prefix_start: self.output.len(),
                path_pushed: false,
                capture_raw: false,
                owns_capture: false,
                raw_start: self.raw_output.len(),
                raw_value_start: self.raw_output.len(),
            });
        };
        let (kind, completed, emitted_children, parent_emits, parent_capture, edge) = {
            let frame = &mut self.frames[parent_index];
            let edge = frame
                .pending_edge
                .take()
                .ok_or_else(|| projection_error("structural child has no declared edge"))?;
            (
                frame.kind,
                frame.completed,
                frame.emitted_children,
                frame.prepared.emit,
                frame.prepared.capture_raw,
                edge,
            )
        };
        let prefix_start = self.output.len();
        let mut path_pushed = false;
        let mut emit = parent_emits;
        let raw_start = self.raw_output.len();
        let mut capture_raw = parent_capture;
        let mut owns_capture = false;
        match kind {
            StructuralKind::Record => {
                let StructuralEdgeKind::RecordField(name) = edge else {
                    return Err(projection_error("structural record edge is invalid"));
                };
                self.add_string_bytes(name.len())?;
                self.path.push(SelectionSegment::Field(name.to_owned()));
                path_pushed = true;
                emit &= selected(self.options, &self.path);
                let has_default = self.options.exclude_defaults
                    && self
                        .plan
                        .field_policy(&self.path)
                        .and_then(super::plan::FieldPolicy::default)
                        .is_some();
                if has_default && !capture_raw {
                    capture_raw = true;
                    owns_capture = true;
                }
                if capture_raw && !owns_capture {
                    if completed > 0 {
                        self.raw_output.push(b',');
                    }
                    write_json_string(&mut self.raw_output, name)?;
                    self.raw_output.push(b':');
                }
                if emit {
                    if emitted_children > 0 {
                        self.output.push(b',');
                    }
                    let output_name = if self.options.by_alias {
                        self.plan
                            .field_policy(&self.path)
                            .and_then(super::plan::FieldPolicy::alias)
                            .unwrap_or(name)
                    } else {
                        name
                    };
                    write_json_string(&mut self.output, output_name)?;
                    self.output.push(b':');
                }
            }
            StructuralKind::Sequence
            | StructuralKind::Tuple
            | StructuralKind::Set
            | StructuralKind::FrozenSet => {
                if edge != StructuralEdgeKind::Index(completed) {
                    return Err(projection_error("structural sequence edge is invalid"));
                }
                self.path.push(SelectionSegment::Index(completed));
                path_pushed = true;
                emit &= selected(self.options, &self.path);
                if capture_raw && completed > 0 {
                    self.raw_output.push(b',');
                }
                if emit && emitted_children > 0 {
                    self.output.push(b',');
                }
            }
            StructuralKind::Mapping if completed.is_multiple_of(2) => {
                let index = completed / 2;
                if edge != StructuralEdgeKind::MappingKey(index) {
                    return Err(projection_error("structural mapping key edge is invalid"));
                }
                if (emit || capture_raw) && !string_scalar {
                    return Err(SerializationError::new(
                        SerializationErrorKind::UnsupportedJsonValue,
                        "JSON mapping keys must be structural strings",
                    ));
                }
                if emit && index > 0 {
                    self.output.push(b',');
                }
                if capture_raw && index > 0 {
                    self.raw_output.push(b',');
                }
            }
            StructuralKind::Mapping => {
                let index = completed / 2;
                if edge != StructuralEdgeKind::MappingValue(index) {
                    return Err(projection_error("structural mapping value edge is invalid"));
                }
                if emit {
                    self.output.push(b':');
                }
                if capture_raw {
                    self.raw_output.push(b':');
                }
            }
            StructuralKind::Optional
            | StructuralKind::Union
            | StructuralKind::Enum
            | StructuralKind::Refined => {
                if !matches!(edge, StructuralEdgeKind::ActiveMember { .. }) {
                    return Err(projection_error("structural active-member edge is invalid"));
                }
            }
            _ => return Err(unsupported_aggregate()),
        }
        if emit {
            self.frames[parent_index].emitted_children += 1;
        }
        Ok(PreparedValue {
            emit,
            prefix_start,
            path_pushed,
            capture_raw,
            owns_capture,
            raw_start,
            raw_value_start: self.raw_output.len(),
        })
    }

    fn complete_value(
        &mut self,
        prepared: PreparedValue,
        is_none: bool,
    ) -> Result<(), SerializationError> {
        let omit = prepared.emit
            && self
                .frames
                .last()
                .is_some_and(|parent| parent.kind == StructuralKind::Record)
            && ((self.options.exclude_none && is_none)
                || (self.options.exclude_defaults
                    && self
                        .plan
                        .field_policy(&self.path)
                        .and_then(super::plan::FieldPolicy::default)
                        .and_then(native_json_bytes)
                        .is_some_and(|default| {
                            default == self.raw_output[prepared.raw_value_start..]
                        })));
        if omit {
            self.output.truncate(prepared.prefix_start);
            if let Some(parent) = self.frames.last_mut() {
                parent.emitted_children = parent.emitted_children.saturating_sub(1);
            }
        }
        if prepared.path_pushed {
            self.path.pop();
        }
        if prepared.owns_capture {
            self.raw_output.truncate(prepared.raw_start);
        }
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

    fn write_scalar(&mut self, value: StructuralScalarRef<'_>) -> Result<(), SerializationError> {
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
            StructuralScalarRef::String(value) => {
                write_json_string(&mut self.output, value)?;
            }
            StructuralScalarRef::Bytes(_) => {
                return Err(SerializationError::new(
                    SerializationErrorKind::UnsupportedJsonValue,
                    "bytes require an explicit JSON output policy",
                ));
            }
            _ => return Err(projection_error("structural scalar kind is not supported")),
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
        if items > self.options.limits.max_collection_items {
            Err(limit_error("maximum collection items"))
        } else {
            Ok(())
        }
    }

    fn add_node(&mut self) -> Result<(), SerializationError> {
        if self.nodes >= self.options.limits.max_nodes {
            return Err(limit_error("maximum node count"));
        }
        self.nodes += 1;
        Ok(())
    }

    fn add_string_bytes(&mut self, amount: usize) -> Result<(), SerializationError> {
        self.string_bytes = self
            .string_bytes
            .checked_add(amount)
            .ok_or_else(|| limit_error("total string bytes"))?;
        if self.string_bytes > self.options.limits.max_string_bytes {
            Err(limit_error("total string bytes"))
        } else {
            Ok(())
        }
    }

    fn write_integer(&mut self, value: impl core::fmt::Display) -> Result<(), SerializationError> {
        let text = value.to_string();
        if text.trim_start_matches(['-', '+']).len() > self.options.limits.max_integer_digits {
            return Err(limit_error("maximum integer digits"));
        }
        self.output.extend_from_slice(text.as_bytes());
        Ok(())
    }

    fn check_output_size(&self) -> Result<(), SerializationError> {
        if self.output.len() > self.options.limits.max_input_bytes {
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

fn write_raw_scalar(
    output: &mut Vec<u8>,
    value: StructuralScalarRef<'_>,
) -> Result<(), SerializationError> {
    match value {
        StructuralScalarRef::None => output.extend_from_slice(b"null"),
        StructuralScalarRef::Bool(true) => output.extend_from_slice(b"true"),
        StructuralScalarRef::Bool(false) => output.extend_from_slice(b"false"),
        StructuralScalarRef::SignedInteger { value, .. } => {
            output.extend_from_slice(value.to_string().as_bytes());
        }
        StructuralScalarRef::UnsignedInteger { value, .. } => {
            output.extend_from_slice(value.to_string().as_bytes());
        }
        StructuralScalarRef::ExactInteger(value) => {
            output.extend_from_slice(value.to_string().as_bytes());
        }
        StructuralScalarRef::Float(value) if value.is_finite() => {
            serde_json::to_writer(output, &value)
                .map_err(|_| projection_error("finite float could not be encoded"))?;
        }
        StructuralScalarRef::String(value) => write_json_string(output, value)?,
        StructuralScalarRef::Float(_) | StructuralScalarRef::Bytes(_) => {
            return Err(SerializationError::new(
                SerializationErrorKind::UnsupportedJsonValue,
                "field default cannot be compared under the selected JSON policy",
            ));
        }
        _ => return Err(projection_error("structural scalar kind is not supported")),
    }
    Ok(())
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

fn unsupported_aggregate() -> SerializationError {
    projection_error("structural aggregate kind is not supported")
}

fn projection_error(message: &'static str) -> SerializationError {
    SerializationError::new(SerializationErrorKind::InvalidProjection, message)
}

fn limit_error(message: &'static str) -> SerializationError {
    SerializationError::new(SerializationErrorKind::Limit, message)
}
