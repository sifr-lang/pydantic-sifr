use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltInError {
    pub code: &'static str,
    pub message: &'static str,
    pub context: &'static [&'static str],
}

const BUILT_INS: &[BuiltInError] = &[
    BuiltInError {
        code: "json_invalid",
        message: "Invalid JSON: {error}",
        context: &["error"],
    },
    BuiltInError {
        code: "schema_invalid",
        message: "Invalid schema: {error}",
        context: &["error"],
    },
    BuiltInError {
        code: "input_limit_exceeded",
        message: "Input limit exceeded: {limit}",
        context: &["limit"],
    },
    BuiltInError {
        code: "internal_program_load",
        message: "Schema program could not be loaded",
        context: &[],
    },
];

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ErrorOverride {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub context_keys: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ErrorRegistry {
    custom: BTreeMap<String, ErrorOverride>,
}

impl ErrorRegistry {
    pub fn new(custom: impl IntoIterator<Item = ErrorOverride>) -> Result<Self, RegistryError> {
        let mut entries = BTreeMap::new();
        for declaration in custom {
            validate_declaration(&declaration)?;
            if let Some(built_in) = Self::built_in(&declaration.code) {
                validate_built_in_override(built_in, &declaration)?;
                continue;
            }
            if entries
                .insert(declaration.code.clone(), declaration.clone())
                .is_some()
            {
                return Err(RegistryError::DuplicateCode(declaration.code));
            }
        }
        Ok(Self { custom: entries })
    }

    #[must_use]
    pub fn built_in(code: &str) -> Option<&'static BuiltInError> {
        BUILT_INS.iter().find(|entry| entry.code == code)
    }

    #[must_use]
    pub fn custom(&self, code: &str) -> Option<&ErrorOverride> {
        self.custom.get(code)
    }

    pub fn validate_override(&self, declaration: &ErrorOverride) -> Result<(), RegistryError> {
        validate_declaration(declaration)?;
        if let Some(built_in) = Self::built_in(&declaration.code) {
            return validate_built_in_override(built_in, declaration);
        }
        match self.custom.get(&declaration.code) {
            Some(registered) if registered == declaration => Ok(()),
            Some(_) => Err(RegistryError::ConflictingDeclaration(
                declaration.code.clone(),
            )),
            None => Err(RegistryError::UnknownCode(declaration.code.clone())),
        }
    }
}

fn validate_declaration(declaration: &ErrorOverride) -> Result<(), RegistryError> {
    validate_code(&declaration.code)?;
    if declaration.message.is_empty() || declaration.message.len() > 512 {
        return Err(RegistryError::InvalidMessage(declaration.code.clone()));
    }
    let declared: BTreeSet<&str> = declaration
        .context_keys
        .iter()
        .map(String::as_str)
        .collect();
    if declared.len() != declaration.context_keys.len() {
        return Err(RegistryError::DuplicateContextKey(declaration.code.clone()));
    }
    let used = placeholders(&declaration.message)
        .map_err(|()| RegistryError::InvalidMessage(declaration.code.clone()))?;
    if used != declared {
        return Err(RegistryError::ContextMismatch(declaration.code.clone()));
    }
    Ok(())
}

fn validate_code(code: &str) -> Result<(), RegistryError> {
    if code.is_empty() || code.len() > 96 {
        return Err(RegistryError::InvalidCode(code.to_owned()));
    }
    let mut segments = code.split('.');
    let first = segments.next().unwrap_or_default();
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    if !valid_segment(first) || !segments.all(valid_segment) {
        return Err(RegistryError::InvalidCode(code.to_owned()));
    }
    if !code.contains('.') && SelfCode::is_custom(code) {
        return Err(RegistryError::CustomCodeNeedsPackage(code.to_owned()));
    }
    Ok(())
}

struct SelfCode;

impl SelfCode {
    fn is_custom(code: &str) -> bool {
        !BUILT_INS.iter().any(|entry| entry.code == code)
    }
}

fn placeholders(message: &str) -> Result<BTreeSet<&str>, ()> {
    let mut keys = BTreeSet::new();
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                let start = index + 1;
                let Some(relative_end) = bytes[start..].iter().position(|byte| *byte == b'}')
                else {
                    return Err(());
                };
                let end = start + relative_end;
                let key = &message[start..end];
                if key.is_empty()
                    || !key.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                {
                    return Err(());
                }
                keys.insert(key);
                index = end + 1;
            }
            b'}' => return Err(()),
            _ => index += 1,
        }
    }
    Ok(keys)
}

fn validate_built_in_override(
    built_in: &BuiltInError,
    declaration: &ErrorOverride,
) -> Result<(), RegistryError> {
    let context: Vec<String> = built_in.context.iter().map(ToString::to_string).collect();
    if declaration.message == built_in.message && declaration.context_keys == context {
        Ok(())
    } else {
        Err(RegistryError::BuiltInCollision(declaration.code.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    InvalidCode(String),
    CustomCodeNeedsPackage(String),
    InvalidMessage(String),
    DuplicateContextKey(String),
    ContextMismatch(String),
    DuplicateCode(String),
    BuiltInCollision(String),
    UnknownCode(String),
    ConflictingDeclaration(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCode(code) => write!(f, "invalid error code `{code}`"),
            Self::CustomCodeNeedsPackage(code) => {
                write!(f, "custom error code `{code}` is not package-qualified")
            }
            Self::InvalidMessage(code) => write!(f, "invalid message for error code `{code}`"),
            Self::DuplicateContextKey(code) => {
                write!(f, "duplicate context key for error code `{code}`")
            }
            Self::ContextMismatch(code) => {
                write!(f, "message context does not match error code `{code}`")
            }
            Self::DuplicateCode(code) => write!(f, "duplicate error code `{code}`"),
            Self::BuiltInCollision(code) => {
                write!(f, "error override changes built-in code `{code}`")
            }
            Self::UnknownCode(code) => write!(f, "unknown error code `{code}`"),
            Self::ConflictingDeclaration(code) => {
                write!(f, "conflicting declaration for error code `{code}`")
            }
        }
    }
}

impl std::error::Error for RegistryError {}
