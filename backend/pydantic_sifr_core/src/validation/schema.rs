use bigdecimal::BigDecimal;
use num_bigint::BigInt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputProfile {
    Native,
    Json,
    Strings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerTarget {
    Exact,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntegerTarget {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exact => "int",
            Self::I8 => "int8",
            Self::I16 => "int16",
            Self::I32 => "int32",
            Self::I64 => "int64",
            Self::U8 => "uint8",
            Self::U16 => "uint16",
            Self::U32 => "uint32",
            Self::U64 => "uint64",
        }
    }

    #[must_use]
    pub fn bounds(self) -> Option<(BigInt, BigInt)> {
        match self {
            Self::Exact => None,
            Self::I8 => Some((BigInt::from(i8::MIN), BigInt::from(i8::MAX))),
            Self::I16 => Some((BigInt::from(i16::MIN), BigInt::from(i16::MAX))),
            Self::I32 => Some((BigInt::from(i32::MIN), BigInt::from(i32::MAX))),
            Self::I64 => Some((BigInt::from(i64::MIN), BigInt::from(i64::MAX))),
            Self::U8 => Some((BigInt::from(u8::MIN), BigInt::from(u8::MAX))),
            Self::U16 => Some((BigInt::from(u16::MIN), BigInt::from(u16::MAX))),
            Self::U32 => Some((BigInt::from(u32::MIN), BigInt::from(u32::MAX))),
            Self::U64 => Some((BigInt::from(u64::MIN), BigInt::from(u64::MAX))),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IntegerConstraints {
    pub greater_than: Option<BigInt>,
    pub greater_or_equal: Option<BigInt>,
    pub less_than: Option<BigInt>,
    pub less_or_equal: Option<BigInt>,
    pub multiple_of: Option<BigInt>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FloatConstraints {
    pub greater_than: Option<f64>,
    pub greater_or_equal: Option<f64>,
    pub less_than: Option<f64>,
    pub less_or_equal: Option<f64>,
    pub multiple_of: Option<f64>,
    pub allow_non_finite: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DecimalConstraints {
    pub greater_than: Option<BigDecimal>,
    pub greater_or_equal: Option<BigDecimal>,
    pub less_than: Option<BigDecimal>,
    pub less_or_equal: Option<BigDecimal>,
    pub multiple_of: Option<BigDecimal>,
    pub max_digits: Option<usize>,
    pub decimal_places: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComplexConstraints {
    pub allow_non_finite: bool,
    pub magnitude_greater_or_equal: Option<f64>,
    pub magnitude_less_or_equal: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct StringPattern {
    source: String,
    compiled: regex::Regex,
}

impl StringPattern {
    pub fn compile(source: impl Into<String>) -> Result<Self, regex::Error> {
        let source = source.into();
        let compiled = regex::Regex::new(&source)?;
        Ok(Self { source, compiled })
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn is_match(&self, value: &str) -> bool {
        self.compiled.is_match(value)
    }
}

impl PartialEq for StringPattern {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StringConstraints {
    pub strip_whitespace: bool,
    pub ascii_only: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<StringPattern>,
    pub to_upper: bool,
    pub to_lower: bool,
    pub coerce_numbers_to_str: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BytesConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CollectionConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalKind {
    Date,
    Time,
    DateTime,
    Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelativeTimeConstraint {
    Past,
    Future,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalSchema {
    pub kind: TemporalKind,
    pub relative: Option<RelativeTimeConstraint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSchema {
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Schema {
    None,
    Bool,
    Integer {
        target: IntegerTarget,
        constraints: IntegerConstraints,
    },
    Float(FloatConstraints),
    Decimal(DecimalConstraints),
    Fraction(IntegerConstraints),
    Complex(ComplexConstraints),
    String(StringConstraints),
    Bytes(BytesConstraints),
    Temporal(TemporalSchema),
    Uuid {
        version: Option<u8>,
    },
    Url,
    Pattern(PatternSchema),
    List {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    Tuple(Vec<Self>),
    Mapping {
        key: Box<Self>,
        value: Box<Self>,
        constraints: CollectionConstraints,
    },
    Set {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    FrozenSet {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    Generator {
        item: Box<Self>,
        constraints: CollectionConstraints,
    },
    EmbeddedJson(Box<Self>),
}

impl Schema {
    #[must_use]
    pub fn exact_integer() -> Self {
        Self::Integer {
            target: IntegerTarget::Exact,
            constraints: IntegerConstraints::default(),
        }
    }
}
