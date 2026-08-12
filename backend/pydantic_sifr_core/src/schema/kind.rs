#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub enum SchemaKind {
    #[serde(rename = "computed-field")]
    ComputedField,
    #[serde(rename = "dataclass-field")]
    DataclassField,
    #[serde(rename = "model-field")]
    ModelField,
    #[serde(rename = "typed-dict-field")]
    TypedDictField,
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "arguments")]
    Arguments,
    #[serde(rename = "arguments-v3")]
    ArgumentsV3,
    #[serde(rename = "bool")]
    Bool,
    #[serde(rename = "bytes")]
    Bytes,
    #[serde(rename = "call")]
    Call,
    #[serde(rename = "callable")]
    Callable,
    #[serde(rename = "chain")]
    Chain,
    #[serde(rename = "complex")]
    Complex,
    #[serde(rename = "custom-error")]
    CustomError,
    #[serde(rename = "dataclass")]
    Dataclass,
    #[serde(rename = "dataclass-args")]
    DataclassArgs,
    #[serde(rename = "date")]
    Date,
    #[serde(rename = "datetime")]
    DateTime,
    #[serde(rename = "decimal")]
    Decimal,
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "definition-ref")]
    DefinitionRef,
    #[serde(rename = "definitions")]
    Definitions,
    #[serde(rename = "dict")]
    Dict,
    #[serde(rename = "enum")]
    Enum,
    #[serde(rename = "float")]
    Float,
    #[serde(rename = "fraction")]
    Fraction,
    #[serde(rename = "frozenset")]
    FrozenSet,
    #[serde(rename = "function-after")]
    FunctionAfter,
    #[serde(rename = "function-before")]
    FunctionBefore,
    #[serde(rename = "function-plain")]
    FunctionPlain,
    #[serde(rename = "function-wrap")]
    FunctionWrap,
    #[serde(rename = "generator")]
    Generator,
    #[serde(rename = "int")]
    Int,
    #[serde(rename = "invalid")]
    Invalid,
    #[serde(rename = "is-instance")]
    IsInstance,
    #[serde(rename = "is-subclass")]
    IsSubclass,
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "json-or-python")]
    JsonOrPython,
    #[serde(rename = "lax-or-strict")]
    LaxOrStrict,
    #[serde(rename = "list")]
    List,
    #[serde(rename = "literal")]
    Literal,
    #[serde(rename = "missing-sentinel")]
    MissingSentinel,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "model-fields")]
    ModelFields,
    #[serde(rename = "multi-host-url")]
    MultiHostUrl,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "nullable")]
    Nullable,
    #[serde(rename = "set")]
    Set,
    #[serde(rename = "str")]
    Str,
    #[serde(rename = "tagged-union")]
    TaggedUnion,
    #[serde(rename = "time")]
    Time,
    #[serde(rename = "timedelta")]
    TimeDelta,
    #[serde(rename = "tuple")]
    Tuple,
    #[serde(rename = "typed-dict")]
    TypedDict,
    #[serde(rename = "union")]
    Union,
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "uuid")]
    Uuid,
}

impl SchemaKind {
    pub const ALL: [Self; 57] = [
        Self::ComputedField,
        Self::DataclassField,
        Self::ModelField,
        Self::TypedDictField,
        Self::Any,
        Self::Arguments,
        Self::ArgumentsV3,
        Self::Bool,
        Self::Bytes,
        Self::Call,
        Self::Callable,
        Self::Chain,
        Self::Complex,
        Self::CustomError,
        Self::Dataclass,
        Self::DataclassArgs,
        Self::Date,
        Self::DateTime,
        Self::Decimal,
        Self::Default,
        Self::DefinitionRef,
        Self::Definitions,
        Self::Dict,
        Self::Enum,
        Self::Float,
        Self::Fraction,
        Self::FrozenSet,
        Self::FunctionAfter,
        Self::FunctionBefore,
        Self::FunctionPlain,
        Self::FunctionWrap,
        Self::Generator,
        Self::Int,
        Self::Invalid,
        Self::IsInstance,
        Self::IsSubclass,
        Self::Json,
        Self::JsonOrPython,
        Self::LaxOrStrict,
        Self::List,
        Self::Literal,
        Self::MissingSentinel,
        Self::Model,
        Self::ModelFields,
        Self::MultiHostUrl,
        Self::None,
        Self::Nullable,
        Self::Set,
        Self::Str,
        Self::TaggedUnion,
        Self::Time,
        Self::TimeDelta,
        Self::Tuple,
        Self::TypedDict,
        Self::Union,
        Self::Url,
        Self::Uuid,
    ];

    #[must_use]
    pub const fn is_rejected(self) -> bool {
        matches!(
            self,
            Self::Any
                | Self::Arguments
                | Self::ArgumentsV3
                | Self::Call
                | Self::Callable
                | Self::Invalid
                | Self::IsInstance
                | Self::IsSubclass
                | Self::MissingSentinel
        )
    }

    #[must_use]
    pub const fn expected_children(self) -> ChildCount {
        match self {
            Self::CustomError
            | Self::Default
            | Self::FrozenSet
            | Self::Generator
            | Self::Json
            | Self::List
            | Self::Model
            | Self::Nullable
            | Self::Set => ChildCount::Exact(1),
            Self::Dict | Self::JsonOrPython | Self::LaxOrStrict => ChildCount::Exact(2),
            Self::Chain | Self::Definitions | Self::TaggedUnion | Self::Tuple | Self::Union => {
                ChildCount::AtLeast(1)
            }
            Self::DefinitionRef => ChildCount::Exact(0),
            _ => ChildCount::Exact(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildCount {
    Exact(usize),
    AtLeast(usize),
}
