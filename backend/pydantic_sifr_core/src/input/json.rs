use core::fmt;
use std::collections::BTreeSet;

use jiter::JsonValue;

use crate::{Arena, ArenaError, ArenaId};

const HARD_MAX_DEPTH: usize = 256;

pub type InputId = ArenaId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceKind {
    JsonArray,
    List,
    Tuple,
    Set,
    FrozenSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectKind {
    JsonObject,
    Object,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputValue {
    Null,
    Bool(bool),
    Integer(String),
    Float(f64),
    Decimal(String),
    Complex {
        real: f64,
        imaginary: f64,
    },
    String(String),
    Bytes(Vec<u8>),
    Date(String),
    Time(String),
    DateTime(String),
    Duration(String),
    Uuid(String),
    Url(String),
    Pattern {
        source: String,
        flags: u8,
    },
    Fraction {
        numerator: String,
        denominator: String,
    },
    Sequence {
        kind: SequenceKind,
        items: Vec<InputId>,
    },
    Object {
        kind: ObjectKind,
        entries: Vec<(String, InputId)>,
    },
    Mapping(Vec<(InputId, InputId)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputArena {
    root: InputId,
    values: Arena<InputValue>,
}

impl InputArena {
    pub(crate) const fn from_parts(root: InputId, values: Arena<InputValue>) -> Self {
        Self { root, values }
    }

    #[must_use]
    pub const fn root(&self) -> InputId {
        self.root
    }

    #[must_use]
    pub fn get(&self, id: InputId) -> Option<&InputValue> {
        self.values.get(id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonLimits {
    pub max_input_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_string_bytes: usize,
    pub max_integer_digits: usize,
    pub max_collection_items: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_nodes: 1_000_000,
            max_string_bytes: 64 * 1024 * 1024,
            max_integer_digits: 4_300,
            max_collection_items: 1_000_000,
        }
    }
}

/// Parse one complete JSON document without Python or runtime plugins.
pub fn parse_json(data: &[u8], limits: JsonLimits) -> Result<InputArena, JsonInputError> {
    validate_limits(limits)?;
    if data.len() > limits.max_input_bytes {
        return Err(JsonInputError::limit("maximum input bytes"));
    }
    validate_integer_digits(data, limits.max_integer_digits)?;
    let parsed = JsonValue::parse(data, false).map_err(|error| {
        let position = error.get_position(data);
        JsonInputError {
            code: "json_invalid",
            message: error.error_type.to_string(),
            offset: error.index,
            line: position.line,
            column: position.column,
            path: Vec::new(),
        }
    })?;
    let mut state = BuildState {
        arena: Arena::new(),
        limits,
        string_bytes: 0,
    };
    let root = state.push_value(&parsed, 0, &mut Vec::new())?;
    Ok(InputArena {
        root,
        values: state.arena,
    })
}

fn validate_limits(limits: JsonLimits) -> Result<(), JsonInputError> {
    if limits.max_input_bytes == 0
        || limits.max_depth == 0
        || limits.max_nodes == 0
        || limits.max_integer_digits == 0
        || limits.max_collection_items == 0
        || limits.max_string_bytes == 0
        || limits.max_depth > HARD_MAX_DEPTH
    {
        return Err(JsonInputError::limit("limits must be greater than zero"));
    }
    Ok(())
}

fn validate_integer_digits(data: &[u8], limit: usize) -> Result<(), JsonInputError> {
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < data.len() {
        let byte = data[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        let start = if byte == b'-' { index + 1 } else { index };
        if data.get(start).is_some_and(u8::is_ascii_digit) {
            let mut end = start;
            while data.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            let is_integer = !matches!(data.get(end), Some(b'.' | b'e' | b'E'));
            if is_integer && end - start > limit {
                return Err(JsonInputError {
                    code: "json_integer_limit",
                    message: format!("integer digit limit exceeded: {limit}"),
                    offset: index,
                    line: 1,
                    column: index + 1,
                    path: Vec::new(),
                });
            }
            index = end;
            continue;
        }
        index += 1;
    }
    Ok(())
}

struct BuildState {
    arena: Arena<InputValue>,
    limits: JsonLimits,
    string_bytes: usize,
}

impl BuildState {
    fn push_value(
        &mut self,
        value: &JsonValue<'_>,
        depth: usize,
        path: &mut Vec<String>,
    ) -> Result<InputId, JsonInputError> {
        if depth > self.limits.max_depth {
            return Err(JsonInputError::at_limit("maximum depth", path));
        }
        if self.arena.len() >= self.limits.max_nodes {
            return Err(JsonInputError::at_limit("maximum node count", path));
        }
        let owned = match value {
            JsonValue::Null => InputValue::Null,
            JsonValue::Bool(value) => InputValue::Bool(*value),
            JsonValue::Int(value) => InputValue::Integer(value.to_string()),
            JsonValue::BigInt(value) => InputValue::Integer(value.to_string()),
            JsonValue::Float(value) => InputValue::Float(*value),
            JsonValue::Str(value) => {
                self.add_string_bytes(value.len(), path)?;
                InputValue::String(value.to_string())
            }
            JsonValue::Array(values) => {
                if values.len() > self.limits.max_collection_items {
                    return Err(JsonInputError::at_limit("maximum collection items", path));
                }
                let mut children = Vec::with_capacity(values.len());
                for (index, child) in values.iter().enumerate() {
                    path.push(index.to_string());
                    let child_id = self.push_value(child, depth + 1, path)?;
                    path.pop();
                    children.push(child_id);
                }
                InputValue::Sequence {
                    kind: SequenceKind::JsonArray,
                    items: children,
                }
            }
            JsonValue::Object(entries) => {
                if entries.len() > self.limits.max_collection_items {
                    return Err(JsonInputError::at_limit("maximum collection items", path));
                }
                let mut keys = BTreeSet::new();
                let mut children = Vec::with_capacity(entries.len());
                for (key, child) in entries.iter() {
                    if !keys.insert(key.as_ref()) {
                        return Err(JsonInputError {
                            code: "json_invalid",
                            message: format!("duplicate object key `{key}`"),
                            offset: 0,
                            line: 1,
                            column: 1,
                            path: path.clone(),
                        });
                    }
                    self.add_string_bytes(key.len(), path)?;
                    path.push(key.to_string());
                    let child_id = self.push_value(child, depth + 1, path)?;
                    path.pop();
                    children.push((key.to_string(), child_id));
                }
                InputValue::Object {
                    kind: ObjectKind::JsonObject,
                    entries: children,
                }
            }
        };
        self.arena.push(owned).map_err(JsonInputError::arena)
    }

    fn add_string_bytes(&mut self, amount: usize, path: &[String]) -> Result<(), JsonInputError> {
        self.string_bytes = self
            .string_bytes
            .checked_add(amount)
            .ok_or_else(|| JsonInputError::at_limit("total string bytes", path))?;
        if self.string_bytes > self.limits.max_string_bytes {
            return Err(JsonInputError::at_limit("total string bytes", path));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonInputError {
    pub code: &'static str,
    pub message: String,
    pub offset: usize,
    pub line: usize,
    pub column: usize,
    pub path: Vec<String>,
}

impl JsonInputError {
    fn limit(message: &str) -> Self {
        Self::at_limit(message, &[])
    }

    fn at_limit(message: &str, path: &[String]) -> Self {
        Self {
            code: "input_limit_exceeded",
            message: message.to_owned(),
            offset: 0,
            line: 1,
            column: 1,
            path: path.to_vec(),
        }
    }

    fn arena(error: ArenaError) -> Self {
        Self::limit(&error.to_string())
    }
}

impl fmt::Display for JsonInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {} column {}",
            self.message, self.line, self.column
        )
    }
}

impl std::error::Error for JsonInputError {}
