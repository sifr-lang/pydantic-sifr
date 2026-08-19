use pydantic_sifr_core::{
    ComplexConstraints, DecimalConstraints, FractionConstraints, JsonLimits, NativeValue,
    PatternSchema, PreparedSchema, Schema, TemporalKind, TemporalSchema, UrlConstraints,
    ValidationOptions, validate_json_and_construct, validate_native_and_construct,
};
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    ConstructToken, NodeId, NominalField, ShapeIdentity, StructuralConstruct,
    StructuralContractError, StructuralEdgeKind, StructuralKind, StructuralScalar,
    StructuralSource, StructuralType, metadata, nominal_record, primitive,
};

#[derive(Debug, PartialEq)]
struct FractionParts(SifrInt, SifrInt);

impl StructuralType for FractionParts {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Fraction")
    }
}

impl StructuralConstruct for FractionParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = tuple_nodes(source, node, 2)?;
        Ok(Self(
            SifrInt::structural_construct_at(source, nodes[0], token)?,
            SifrInt::structural_construct_at(source, nodes[1], token)?,
        ))
    }
}

#[derive(Debug, PartialEq)]
struct ComplexParts(f64, f64);

impl StructuralType for ComplexParts {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Complex")
    }
}

impl StructuralConstruct for ComplexParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = tuple_nodes(source, node, 2)?;
        Ok(Self(
            f64::structural_construct_at(source, nodes[0], token)?,
            f64::structural_construct_at(source, nodes[1], token)?,
        ))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DateParts(i16, u8, u8);

impl StructuralType for DateParts {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Date")
    }
}

impl StructuralConstruct for DateParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = tuple_nodes(source, node, 3)?;
        Ok(Self(
            i16::structural_construct_at(source, nodes[0], token)?,
            u8::structural_construct_at(source, nodes[1], token)?,
            u8::structural_construct_at(source, nodes[2], token)?,
        ))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TimeParts(u8, u8, u8, u32, Option<i32>);

impl StructuralType for TimeParts {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Time")
    }
}

impl StructuralConstruct for TimeParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = tuple_nodes(source, node, 5)?;
        Ok(Self(
            u8::structural_construct_at(source, nodes[0], token)?,
            u8::structural_construct_at(source, nodes[1], token)?,
            u8::structural_construct_at(source, nodes[2], token)?,
            u32::structural_construct_at(source, nodes[3], token)?,
            Option::<i32>::structural_construct_at(source, nodes[4], token)?,
        ))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DateTimeParts(DateParts, TimeParts);

impl StructuralType for DateTimeParts {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.DateTime")
    }
}

impl StructuralConstruct for DateTimeParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = tuple_nodes(source, node, 2)?;
        Ok(Self(
            DateParts::structural_construct_at(source, nodes[0], token)?,
            TimeParts::structural_construct_at(source, nodes[1], token)?,
        ))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DurationParts(bool, u32, u32, u32);

impl StructuralType for DurationParts {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Duration")
    }
}

impl StructuralConstruct for DurationParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = tuple_nodes(source, node, 4)?;
        Ok(Self(
            bool::structural_construct_at(source, nodes[0], token)?,
            u32::structural_construct_at(source, nodes[1], token)?,
            u32::structural_construct_at(source, nodes[2], token)?,
            u32::structural_construct_at(source, nodes[3], token)?,
        ))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct PatternParts(String, u8);

impl StructuralType for PatternParts {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "pydantic_sifr.special_values.Pattern",
            &[],
            &[
                NominalField {
                    name: "source",
                    identity: primitive("str"),
                    required: true,
                    default_identity: None,
                },
                NominalField {
                    name: "flags",
                    identity: primitive("uint8"),
                    required: true,
                    default_identity: None,
                },
            ],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for PatternParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = record_nodes(
            source,
            node,
            "pydantic_sifr.special_values.Pattern",
            &["source", "flags"],
        )?;
        Ok(Self(
            String::structural_construct_at(source, nodes[0], token)?,
            u8::structural_construct_at(source, nodes[1], token)?,
        ))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct UrlParts(String);

impl StructuralType for UrlParts {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            "pydantic_sifr.special_values.Url",
            &[],
            &[NominalField {
                name: "value",
                identity: primitive("str"),
                required: true,
                default_identity: None,
            }],
            metadata(&[]),
        )
    }
}

impl StructuralConstruct for UrlParts {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let nodes = record_nodes(source, node, "pydantic_sifr.special_values.Url", &["value"])?;
        Ok(Self(String::structural_construct_at(
            source, nodes[0], token,
        )?))
    }
}

fn record_nodes<S: StructuralSource>(
    source: &S,
    node: NodeId,
    identity: &'static str,
    fields: &[&'static str],
) -> Result<Vec<NodeId>, StructuralContractError> {
    let description = source.node(node)?;
    if description.kind() != StructuralKind::Record
        || description.nominal_identity() != Some(identity)
        || description.edges().len() != fields.len()
    {
        return Err(StructuralContractError::ArityMismatch);
    }
    description
        .edges()
        .iter()
        .zip(fields)
        .map(|(edge, field)| {
            if edge.kind() == StructuralEdgeKind::RecordField(field) {
                Ok(edge.node())
            } else {
                Err(StructuralContractError::MemberMismatch)
            }
        })
        .collect()
}

fn tuple_nodes<S: StructuralSource>(
    source: &S,
    node: NodeId,
    expected: usize,
) -> Result<Vec<NodeId>, StructuralContractError> {
    let description = source.node(node)?;
    if description.kind() != StructuralKind::Tuple || description.edges().len() != expected {
        return Err(StructuralContractError::ArityMismatch);
    }
    description
        .edges()
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            if edge.kind() == StructuralEdgeKind::Index(index) {
                Ok(edge.node())
            } else {
                Err(StructuralContractError::MemberMismatch)
            }
        })
        .collect()
}

fn prepared(schema: &Schema) -> PreparedSchema<'_> {
    PreparedSchema::new(schema).unwrap_or_else(|error| panic!("schema preparation failed: {error}"))
}

#[derive(Debug, Eq, PartialEq)]
struct DecimalText(String);

impl StructuralType for DecimalText {
    fn shape_identity() -> ShapeIdentity {
        primitive("bigdecimal")
    }
}

impl StructuralConstruct for DecimalText {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        _token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        if source.node(node)?.kind() != StructuralKind::String {
            return Err(StructuralContractError::KindMismatch);
        }
        match source.take_scalar(node)? {
            StructuralScalar::String(value) => Ok(Self(value)),
            _ => Err(StructuralContractError::ScalarMismatch),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct UuidBytes(Vec<u8>);

impl StructuralType for UuidBytes {
    fn shape_identity() -> ShapeIdentity {
        primitive("bytes")
    }
}

impl StructuralConstruct for UuidBytes {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        sifr_runtime::interop::structural::construct_bytes_at(source, node, token).map(Self)
    }
}

#[test]
fn specialized_scalar_components_construct_without_crate_specific_payloads() {
    let decimal = validate_native_and_construct::<DecimalText>(
        &prepared(&Schema::Decimal(DecimalConstraints::default())),
        &NativeValue::Decimal("12.340".to_owned()),
        JsonLimits::default(),
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("decimal construction failed: {error}"));
    assert_eq!(decimal, DecimalText("12.340".to_owned()));

    let fraction = validate_native_and_construct::<FractionParts>(
        &prepared(&Schema::Fraction(FractionConstraints::default())),
        &NativeValue::Fraction {
            numerator: "6".to_owned(),
            denominator: "-8".to_owned(),
        },
        JsonLimits::default(),
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("fraction construction failed: {error}"));
    assert_eq!(
        fraction,
        FractionParts(SifrInt::from_i64(-3), SifrInt::from_i64(4))
    );

    let complex = validate_native_and_construct::<ComplexParts>(
        &prepared(&Schema::Complex(ComplexConstraints::default())),
        &NativeValue::Complex {
            real: 3.0,
            imaginary: 4.0,
        },
        JsonLimits::default(),
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("complex construction failed: {error}"));
    assert_eq!(complex, ComplexParts(3.0, 4.0));

    let date = validate_json_and_construct::<DateParts>(
        &prepared(&temporal(TemporalKind::Date)),
        br#""2024-02-29""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("date construction failed: {error}"));
    assert_eq!(date, DateParts(2024, 2, 29));

    let time = validate_json_and_construct::<TimeParts>(
        &prepared(&temporal(TemporalKind::Time)),
        br#""12:34:56.123456+02:00""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("time construction failed: {error}"));
    assert_eq!(time, TimeParts(12, 34, 56, 123_456, Some(7_200)));

    let datetime = validate_json_and_construct::<DateTimeParts>(
        &prepared(&temporal(TemporalKind::DateTime)),
        br#""2024-02-29T12:34:56Z""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("datetime construction failed: {error}"));
    assert_eq!(
        datetime,
        DateTimeParts(DateParts(2024, 2, 29), TimeParts(12, 34, 56, 0, Some(0)))
    );

    let duration = validate_json_and_construct::<DurationParts>(
        &prepared(&temporal(TemporalKind::Duration)),
        br#""P2DT3H4M5.6S""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("duration construction failed: {error}"));
    assert_eq!(duration, DurationParts(true, 2, 11_045, 600_000));

    let uuid = validate_json_and_construct::<UuidBytes>(
        &prepared(&Schema::Uuid { version: Some(4) }),
        br#""550e8400-e29b-41d4-a716-446655440000""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("UUID construction failed: {error}"));
    assert_eq!(uuid.0.len(), 16);

    let url = validate_json_and_construct::<UrlParts>(
        &prepared(&Schema::Url(UrlConstraints::default())),
        br#""https://EXAMPLE.com/a/../b""#,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("URL construction failed: {error}"));
    assert_eq!(url, UrlParts("https://example.com/b".to_owned()));

    let pattern = validate_native_and_construct::<PatternParts>(
        &prepared(&Schema::Pattern(PatternSchema {
            case_insensitive: true,
            multi_line: false,
            dot_matches_new_line: false,
        })),
        &NativeValue::Pattern {
            source: "^abc$".to_owned(),
            flags: 0,
        },
        JsonLimits::default(),
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("pattern construction failed: {error}"));
    assert_eq!(pattern, PatternParts("^abc$".to_owned(), 1));
}

fn temporal(kind: TemporalKind) -> Schema {
    Schema::Temporal(TemporalSchema {
        kind,
        relative: None,
    })
}
