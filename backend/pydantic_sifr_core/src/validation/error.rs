use core::fmt;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocationItem {
    Field(String),
    Index(usize),
    MappingKey(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub location: Vec<LocationItem>,
    pub message: String,
    pub expected: String,
    pub context: BTreeMap<String, String>,
}

impl ErrorDetail {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            location: Vec::new(),
            message: message.into(),
            expected: String::new(),
            context: BTreeMap::new(),
        }
    }

    pub(crate) fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = expected.into();
        self
    }

    pub(crate) fn context(mut self, key: &str, value: impl Into<String>) -> Self {
        self.context.insert(key.to_owned(), value.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    details: Vec<ErrorDetail>,
    truncated: bool,
}

impl ValidationError {
    #[must_use]
    pub fn one(detail: ErrorDetail) -> Self {
        Self {
            details: vec![detail],
            truncated: false,
        }
    }

    #[must_use]
    pub fn details(&self) -> &[ErrorDetail] {
        &self.details
    }

    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(first) = self.details.first() {
            write!(f, "{}: {}", first.code, first.message)
        } else {
            f.write_str("validation failed")
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationLimits {
    pub max_depth: usize,
    pub max_collection_items: usize,
    pub max_string_bytes: usize,
    pub max_errors: usize,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_depth: 128,
            max_collection_items: 1_000_000,
            max_string_bytes: 64 * 1024 * 1024,
            max_errors: 128,
        }
    }
}
