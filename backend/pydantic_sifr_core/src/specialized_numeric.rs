use core::fmt;

use num_rational::BigRational;
use num_traits::Zero;
use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::{
    ConstructToken, NodeId, ShapeIdentity, StructuralConstruct, StructuralContractError,
    StructuralEdgeKind, StructuralKind, StructuralProject, StructuralScalarRef, StructuralSource,
    StructuralType, StructuralVisitor, primitive,
};

#[derive(Clone, Debug, Eq, PartialEq)]
/// Exact rational value constructed by `TypeAdapter<Fraction>`.
///
/// Its structural projection is the canonical serialization string. Strict
/// native validation instead requires `NativeValue::Fraction`, because strict
/// input does not accept serialized strings.
pub struct Fraction {
    numerator: SifrInt,
    denominator: SifrInt,
    canonical: String,
}

impl Fraction {
    pub fn new(numerator: SifrInt, denominator: SifrInt) -> Result<Self, FractionError> {
        let denominator = denominator.as_bigint();
        if denominator.is_zero() {
            return Err(FractionError);
        }
        Ok(Self::from_rational(BigRational::new(
            numerator.as_bigint(),
            denominator,
        )))
    }

    #[must_use]
    pub const fn numerator(&self) -> &SifrInt {
        &self.numerator
    }

    #[must_use]
    pub const fn denominator(&self) -> &SifrInt {
        &self.denominator
    }

    fn from_rational(value: BigRational) -> Self {
        let numerator = SifrInt::from_bigint(value.numer().clone());
        let denominator = SifrInt::from_bigint(value.denom().clone());
        let canonical = if value.is_integer() {
            numerator.to_string()
        } else {
            format!("{numerator}/{denominator}")
        };
        Self {
            numerator,
            denominator,
            canonical,
        }
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical)
    }
}

impl StructuralType for Fraction {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Fraction")
    }
}

impl StructuralConstruct for Fraction {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let [numerator, denominator] = tuple_nodes::<2, _>(source, node)?;
        let numerator = SifrInt::structural_construct_at(source, numerator, token)?;
        let denominator = SifrInt::structural_construct_at(source, denominator, token)?;
        Self::new(numerator, denominator).map_err(|_| StructuralContractError::ScalarMismatch)
    }
}

impl StructuralProject for Fraction {
    fn structural_project<'value, V: StructuralVisitor<'value>>(
        &'value self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        visitor.scalar(StructuralScalarRef::String(&self.canonical))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionError;

impl fmt::Display for FractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fraction denominator must not be zero")
    }
}

impl std::error::Error for FractionError {}

#[derive(Clone, Debug, PartialEq)]
/// Complex value constructed by `TypeAdapter<Complex>`.
///
/// Its structural projection is the canonical serialization string. Strict
/// native validation instead requires `NativeValue::Complex`, because strict
/// input does not accept serialized strings.
pub struct Complex {
    real: f64,
    imaginary: f64,
    canonical: String,
}

impl Complex {
    #[must_use]
    pub fn new(real: f64, imaginary: f64) -> Self {
        Self {
            real,
            imaginary,
            canonical: canonical_complex(real, imaginary),
        }
    }

    #[must_use]
    pub const fn real(&self) -> f64 {
        self.real
    }

    #[must_use]
    pub const fn imaginary(&self) -> f64 {
        self.imaginary
    }
}

impl fmt::Display for Complex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical)
    }
}

impl StructuralType for Complex {
    fn shape_identity() -> ShapeIdentity {
        primitive("pydantic_sifr.Complex")
    }
}

impl StructuralConstruct for Complex {
    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<Self, StructuralContractError> {
        let [real, imaginary] = tuple_nodes::<2, _>(source, node)?;
        Ok(Self::new(
            f64::structural_construct_at(source, real, token)?,
            f64::structural_construct_at(source, imaginary, token)?,
        ))
    }
}

impl StructuralProject for Complex {
    fn structural_project<'value, V: StructuralVisitor<'value>>(
        &'value self,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        visitor.scalar(StructuralScalarRef::String(&self.canonical))
    }
}

fn tuple_nodes<const N: usize, S: StructuralSource>(
    source: &S,
    node: NodeId,
) -> Result<[NodeId; N], StructuralContractError> {
    let description = source.node(node)?;
    if description.kind() != StructuralKind::Tuple || description.edges().len() != N {
        return Err(StructuralContractError::ArityMismatch);
    }
    let mut nodes = [NodeId::new(0); N];
    for (index, edge) in description.edges().iter().enumerate() {
        if edge.kind() != StructuralEdgeKind::Index(index) {
            return Err(StructuralContractError::MemberMismatch);
        }
        nodes[index] = edge.node();
    }
    Ok(nodes)
}

fn canonical_complex(real: f64, imaginary: f64) -> String {
    if real == 0.0 && !real.is_sign_negative() {
        return format!("{}j", float_text(imaginary));
    }
    let separator = if imaginary.is_sign_negative() {
        ""
    } else {
        "+"
    };
    format!("{}{separator}{}j", float_text(real), float_text(imaginary))
}

fn float_text(value: f64) -> String {
    if value.is_nan() {
        "nan".to_owned()
    } else if value == f64::INFINITY {
        "inf".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-inf".to_owned()
    } else {
        let mut output = serde_json::Number::from_f64(value)
            .map_or_else(|| value.to_string(), |number| number.to_string());
        if let Some(integer) = output.strip_suffix(".0") {
            output = integer.to_owned();
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_normalizes_sign_and_common_factors() {
        let value = Fraction::new(SifrInt::from_i64(6), SifrInt::from_i64(-8))
            .unwrap_or_else(|error| panic!("fraction failed: {error}"));
        assert_eq!(value.to_string(), "-3/4");
        assert!(Fraction::new(SifrInt::from_i64(1), SifrInt::from_i64(0)).is_err());
    }

    #[test]
    fn complex_text_is_canonical() {
        assert_eq!(Complex::new(1.0, 2.0).to_string(), "1+2j");
        assert_eq!(Complex::new(0.0, 1.0).to_string(), "1j");
        assert_eq!(Complex::new(0.0, 0.0).to_string(), "0j");
        assert_eq!(Complex::new(-1.25, -4.5).to_string(), "-1.25-4.5j");
        assert_eq!(Complex::new(-0.0, 4.0).to_string(), "-0+4j");
        assert_eq!(Complex::new(1e300, 1.0).to_string(), "1e+300+1j");
    }
}
