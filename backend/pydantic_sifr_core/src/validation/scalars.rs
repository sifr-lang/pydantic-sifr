use std::str::FromStr;

use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_complex::Complex64;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

use crate::InputValue;

use super::{
    BytesConstraints, ComplexConstraints, DecimalConstraints, ErrorDetail, FloatConstraints,
    FractionConstraints, InputProfile, IntegerConstraints, IntegerTarget, Schema,
    StringConstraints, ValidatedValue, ValidationError, ValidationOptions,
};

pub(crate) fn validate_scalar(
    schema: &Schema,
    input: &InputValue,
    options: ValidationOptions,
) -> Option<Result<ValidatedValue, ValidationError>> {
    let strict = options.strict;
    let profile = options.profile;
    let result = match schema {
        Schema::None => validate_none(input),
        Schema::Bool => validate_bool(input, strict, profile),
        Schema::Integer {
            target,
            constraints,
        } => validate_integer(input, options, *target, constraints),
        Schema::Float(constraints) => validate_float(input, strict, profile, constraints),
        Schema::Decimal(constraints) => validate_decimal(input, options, constraints),
        Schema::Fraction(constraints) => validate_fraction(input, options, constraints),
        Schema::Complex(constraints) => validate_complex(input, strict, profile, constraints),
        Schema::String(constraints) => validate_string(input, strict, constraints),
        Schema::Bytes(constraints) => validate_bytes(input, strict, profile, constraints),
        _ => return None,
    };
    Some(result)
}

fn validate_none(input: &InputValue) -> Result<ValidatedValue, ValidationError> {
    if matches!(input, InputValue::Null) {
        Ok(ValidatedValue::None)
    } else {
        Err(type_error("none_required", "Input must be None", "none"))
    }
}

fn validate_bool(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
) -> Result<ValidatedValue, ValidationError> {
    if let InputValue::Bool(value) = input {
        return Ok(ValidatedValue::Bool(*value));
    }
    if strict && profile != InputProfile::Strings {
        return Err(type_error("bool_type", "Input must be a boolean", "bool"));
    }
    let value = match input {
        InputValue::Integer(value) if value == "0" => Some(false),
        InputValue::Integer(value) if value == "1" => Some(true),
        InputValue::Float(value) if *value == 0.0 => Some(false),
        InputValue::Float(value) if *value == 1.0 => Some(true),
        InputValue::String(value) => match value.to_ascii_lowercase().as_str() {
            "0" | "false" | "f" | "no" | "n" | "off" => Some(false),
            "1" | "true" | "t" | "yes" | "y" | "on" => Some(true),
            _ => None,
        },
        _ => None,
    };
    value
        .map(ValidatedValue::Bool)
        .ok_or_else(|| type_error("bool_parsing", "Input must be a valid boolean", "bool"))
}

fn validate_integer(
    input: &InputValue,
    options: ValidationOptions,
    target: IntegerTarget,
    constraints: &IntegerConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let value = exact_integer(input, options)?;
    if let Some((minimum, maximum)) = target.bounds()
        && (value < minimum || value > maximum)
    {
        return Err(ValidationError::one(
            ErrorDetail::new(
                "integer_overflow",
                format!("Input does not fit {}", target.name()),
            )
            .expected(target.name())
            .context("minimum", minimum.to_string())
            .context("maximum", maximum.to_string())
            .context("target", target.name()),
        ));
    }
    validate_integer_constraints(&value, constraints)?;
    if target == IntegerTarget::Exact {
        Ok(ValidatedValue::ExactInt(value))
    } else {
        Ok(ValidatedValue::FixedInt {
            kind: target.name(),
            value,
        })
    }
}

fn exact_integer(
    input: &InputValue,
    options: ValidationOptions,
) -> Result<BigInt, ValidationError> {
    if let InputValue::Integer(value) = input {
        return parse_canonical_integer(value, options.limits.max_numeric_digits);
    }
    if options.strict && options.profile != InputProfile::Strings {
        return Err(type_error("int_type", "Input must be an integer", "int"));
    }
    match input {
        InputValue::Bool(value) => Ok(BigInt::from(u8::from(*value))),
        InputValue::Float(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value.abs() <= 9_007_199_254_740_992.0 =>
        {
            Ok(BigInt::from(*value as i64))
        }
        InputValue::String(value) => {
            parse_canonical_integer(value, options.limits.max_numeric_digits)
        }
        _ => Err(type_error(
            "int_parsing",
            "Input must be a valid integer",
            "int",
        )),
    }
}

fn parse_canonical_integer(value: &str, max_digits: usize) -> Result<BigInt, ValidationError> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.len() > max_digits {
        return Err(resource_error("Integer digit limit exceeded"));
    }
    let canonical = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && value != "-0";
    if !canonical {
        return Err(type_error(
            "int_parsing",
            "Input must be a canonical integer",
            "int",
        ));
    }
    BigInt::from_str(value)
        .map_err(|_| type_error("int_parsing", "Input must be a valid integer", "int"))
}

fn validate_integer_constraints(
    value: &BigInt,
    constraints: &IntegerConstraints,
) -> Result<(), ValidationError> {
    if constraints
        .greater_than
        .as_ref()
        .is_some_and(|bound| value <= bound)
    {
        return Err(bound_error(
            "greater_than",
            "Input must be greater",
            constraints.greater_than.as_ref(),
        ));
    }
    if constraints
        .greater_or_equal
        .as_ref()
        .is_some_and(|bound| value < bound)
    {
        return Err(bound_error(
            "greater_than_equal",
            "Input must be greater than or equal",
            constraints.greater_or_equal.as_ref(),
        ));
    }
    if constraints
        .less_than
        .as_ref()
        .is_some_and(|bound| value >= bound)
    {
        return Err(bound_error(
            "less_than",
            "Input must be less",
            constraints.less_than.as_ref(),
        ));
    }
    if constraints
        .less_or_equal
        .as_ref()
        .is_some_and(|bound| value > bound)
    {
        return Err(bound_error(
            "less_than_equal",
            "Input must be less than or equal",
            constraints.less_or_equal.as_ref(),
        ));
    }
    if let Some(multiple) = &constraints.multiple_of {
        if multiple.is_zero() {
            return Err(type_error(
                "schema_invalid",
                "multiple_of cannot be zero",
                "nonzero integer",
            ));
        }
        if value % multiple != BigInt::ZERO {
            return Err(ValidationError::one(
                ErrorDetail::new("multiple_of", "Input must be a multiple")
                    .context("multiple_of", multiple.to_string()),
            ));
        }
    }
    Ok(())
}

fn bound_error(
    code: &'static str,
    message: &'static str,
    bound: Option<&BigInt>,
) -> ValidationError {
    let detail = ErrorDetail::new(code, message);
    ValidationError::one(match bound {
        Some(bound) => detail.context("bound", bound.to_string()),
        None => detail,
    })
}

fn validate_float(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    constraints: &FloatConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let value = match input {
        InputValue::Float(value) => *value,
        InputValue::Integer(value) => value
            .parse::<f64>()
            .map_err(|_| type_error("float_parsing", "Input must be a finite float", "float"))?,
        InputValue::String(value) if !strict || profile == InputProfile::Strings => value
            .parse::<f64>()
            .map_err(|_| type_error("float_parsing", "Input must be a valid float", "float"))?,
        InputValue::Bool(value) if !strict => f64::from(u8::from(*value)),
        _ => return Err(type_error("float_type", "Input must be a float", "float")),
    };
    if !constraints.allow_non_finite && !value.is_finite() {
        return Err(type_error(
            "finite_number",
            "Input must be a finite number",
            "finite float",
        ));
    }
    if constraints.greater_than.is_some_and(|bound| value <= bound) {
        return Err(float_bound_error("greater_than", constraints.greater_than));
    }
    if constraints
        .greater_or_equal
        .is_some_and(|bound| value < bound)
    {
        return Err(float_bound_error(
            "greater_than_equal",
            constraints.greater_or_equal,
        ));
    }
    if constraints.less_than.is_some_and(|bound| value >= bound) {
        return Err(float_bound_error("less_than", constraints.less_than));
    }
    if constraints.less_or_equal.is_some_and(|bound| value > bound) {
        return Err(float_bound_error(
            "less_than_equal",
            constraints.less_or_equal,
        ));
    }
    if let Some(multiple) = constraints.multiple_of {
        if !multiple.is_finite() || multiple == 0.0 {
            return Err(type_error(
                "schema_invalid",
                "multiple_of must be finite and nonzero",
                "finite float",
            ));
        }
        let quotient = value / multiple;
        if !quotient.is_finite()
            || (quotient - quotient.round()).abs() > 1e-9 * quotient.abs().max(1.0)
        {
            return Err(float_bound_error("multiple_of", Some(multiple)));
        }
    }
    Ok(ValidatedValue::Float(value))
}

fn float_bound_error(code: &'static str, bound: Option<f64>) -> ValidationError {
    let detail = ErrorDetail::new(code, "Input violates a numeric constraint");
    ValidationError::one(match bound {
        Some(bound) => detail.context("bound", bound.to_string()),
        None => detail,
    })
}

fn validate_decimal(
    input: &InputValue,
    options: ValidationOptions,
    constraints: &DecimalConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let source = match input {
        InputValue::Decimal(value) => value.as_str(),
        InputValue::Integer(value)
            if !options.strict || options.profile != InputProfile::Native =>
        {
            value.as_str()
        }
        InputValue::Float(value) if !options.strict && value.is_finite() => {
            return validate_decimal_text(&value.to_string(), constraints, options);
        }
        InputValue::String(value) if !options.strict || options.profile != InputProfile::Native => {
            value.as_str()
        }
        _ => {
            return Err(type_error(
                "decimal_type",
                "Input must be an exact decimal",
                "decimal",
            ));
        }
    };
    validate_decimal_text(source, constraints, options)
}

fn validate_decimal_text(
    source: &str,
    constraints: &DecimalConstraints,
    options: ValidationOptions,
) -> Result<ValidatedValue, ValidationError> {
    validate_decimal_source(source, options)?;
    let value = BigDecimal::from_str(source).map_err(|_| {
        type_error(
            "decimal_parsing",
            "Input must be a valid decimal",
            "decimal",
        )
    })?;
    validate_decimal_bounds(&value, constraints)?;
    if constraints.max_digits.is_some() || constraints.decimal_places.is_some() {
        validate_decimal_digits(&value, constraints)?;
    }
    Ok(ValidatedValue::Decimal(value))
}

fn validate_decimal_source(
    source: &str,
    options: ValidationOptions,
) -> Result<(), ValidationError> {
    let unsigned = source.strip_prefix(['-', '+']).unwrap_or(source);
    let (mantissa, exponent) = unsigned
        .split_once(['e', 'E'])
        .map_or((unsigned, None), |(mantissa, exponent)| {
            (mantissa, Some(exponent))
        });
    let digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
    if digits == 0 {
        return Err(type_error(
            "decimal_parsing",
            "Input must be a valid decimal",
            "decimal",
        ));
    }
    if digits > options.limits.max_numeric_digits {
        return Err(resource_error("Decimal digit limit exceeded"));
    }
    if let Some(exponent) = exponent {
        let exponent = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
        if exponent.is_empty() || !exponent.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(type_error(
                "decimal_parsing",
                "Input must be a valid decimal",
                "decimal",
            ));
        }
        let maximum = options.limits.max_decimal_exponent.to_string();
        let significant = exponent.trim_start_matches('0');
        if significant.len() > maximum.len()
            || (significant.len() == maximum.len() && significant > maximum.as_str())
        {
            return Err(resource_error("Decimal exponent limit exceeded"));
        }
    }
    Ok(())
}

fn validate_decimal_bounds(
    value: &BigDecimal,
    constraints: &DecimalConstraints,
) -> Result<(), ValidationError> {
    if constraints
        .greater_than
        .as_ref()
        .is_some_and(|bound| value <= bound)
    {
        return Err(decimal_bound_error(
            "greater_than",
            constraints.greater_than.as_ref(),
        ));
    }
    if constraints
        .greater_or_equal
        .as_ref()
        .is_some_and(|bound| value < bound)
    {
        return Err(decimal_bound_error(
            "greater_than_equal",
            constraints.greater_or_equal.as_ref(),
        ));
    }
    if constraints
        .less_than
        .as_ref()
        .is_some_and(|bound| value >= bound)
    {
        return Err(decimal_bound_error(
            "less_than",
            constraints.less_than.as_ref(),
        ));
    }
    if constraints
        .less_or_equal
        .as_ref()
        .is_some_and(|bound| value > bound)
    {
        return Err(decimal_bound_error(
            "less_than_equal",
            constraints.less_or_equal.as_ref(),
        ));
    }
    if let Some(multiple) = &constraints.multiple_of {
        if multiple.is_zero() {
            return Err(type_error(
                "schema_invalid",
                "multiple_of cannot be zero",
                "nonzero decimal",
            ));
        }
        if value % multiple != BigDecimal::zero() {
            return Err(decimal_bound_error("multiple_of", Some(multiple)));
        }
    }
    Ok(())
}

fn decimal_bound_error(code: &'static str, bound: Option<&BigDecimal>) -> ValidationError {
    let detail = ErrorDetail::new(code, "Input violates a decimal constraint");
    ValidationError::one(match bound {
        Some(bound) => detail.context("bound", bound.to_string()),
        None => detail,
    })
}

fn validate_decimal_digits(
    value: &BigDecimal,
    constraints: &DecimalConstraints,
) -> Result<(), ValidationError> {
    let normalized = decimal_counts(&value.normalized());
    if let Some(maximum) = constraints.max_digits
        && normalized.0 > maximum
    {
        return Err(decimal_digit_error(
            "decimal_max_digits",
            "max_digits",
            maximum,
        ));
    }
    if let Some(maximum) = constraints.decimal_places
        && normalized.1 > maximum
    {
        return Err(decimal_digit_error(
            "decimal_max_places",
            "decimal_places",
            maximum,
        ));
    }
    if let (Some(max_digits), Some(decimal_places)) =
        (constraints.max_digits, constraints.decimal_places)
    {
        let allowed = max_digits.saturating_sub(decimal_places);
        if normalized.0.saturating_sub(normalized.1) > allowed {
            return Err(decimal_digit_error(
                "decimal_whole_digits",
                "whole_digits",
                allowed,
            ));
        }
    }
    Ok(())
}

fn decimal_counts(value: &BigDecimal) -> (usize, usize) {
    let (coefficient, scale) = value.as_bigint_and_exponent();
    let coefficient_digits = coefficient.abs().to_string().len();
    let decimal_digits = usize::try_from(scale.max(0)).unwrap_or(usize::MAX);
    let whole_trailing_zeros = usize::try_from((-scale).max(0)).unwrap_or(usize::MAX);
    (
        coefficient_digits.saturating_add(whole_trailing_zeros),
        decimal_digits,
    )
}

fn decimal_digit_error(code: &'static str, key: &str, allowance: usize) -> ValidationError {
    ValidationError::one(
        ErrorDetail::new(code, "Decimal digit constraint failed")
            .context(key, allowance.to_string()),
    )
}

fn validate_fraction(
    input: &InputValue,
    options: ValidationOptions,
    constraints: &FractionConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let value = match input {
        InputValue::Fraction {
            numerator,
            denominator,
        } => rational_from_parts(numerator, denominator, options.limits.max_numeric_digits)?,
        InputValue::Integer(value)
            if !options.strict || options.profile != InputProfile::Native =>
        {
            BigRational::from_integer(parse_canonical_integer(
                value,
                options.limits.max_numeric_digits,
            )?)
        }
        InputValue::Decimal(value) if !options.strict => rational_from_decimal(value, options)?,
        InputValue::String(value)
            if !options.strict || options.profile == InputProfile::Strings =>
        {
            if let Some((numerator, denominator)) = value.split_once('/') {
                rational_from_parts(numerator, denominator, options.limits.max_numeric_digits)?
            } else if value.contains('.') || value.contains('e') || value.contains('E') {
                rational_from_decimal(value, options)?
            } else {
                BigRational::from_integer(parse_canonical_integer(
                    value,
                    options.limits.max_numeric_digits,
                )?)
            }
        }
        _ => {
            return Err(type_error(
                "fraction_type",
                "Input must be an exact fraction",
                "fraction",
            ));
        }
    };
    validate_fraction_constraints(&value, constraints)?;
    Ok(ValidatedValue::Fraction(value))
}

fn rational_from_parts(
    numerator: &str,
    denominator: &str,
    max_digits: usize,
) -> Result<BigRational, ValidationError> {
    let numerator = parse_canonical_integer(numerator, max_digits)?;
    let denominator = parse_canonical_integer(denominator, max_digits)?;
    if denominator.is_zero() {
        return Err(type_error(
            "fraction_zero_denominator",
            "Fraction denominator cannot be zero",
            "nonzero denominator",
        ));
    }
    Ok(BigRational::new(numerator, denominator))
}

fn rational_from_decimal(
    value: &str,
    options: ValidationOptions,
) -> Result<BigRational, ValidationError> {
    validate_decimal_source(value, options)?;
    let decimal = BigDecimal::from_str(value).map_err(|_| {
        type_error(
            "fraction_parsing",
            "Input must be an exact fraction",
            "fraction",
        )
    })?;
    let (coefficient, scale) = decimal.as_bigint_and_exponent();
    let magnitude = u32::try_from(scale.unsigned_abs()).map_err(|_| {
        type_error(
            "fraction_parsing",
            "Decimal exponent is too large",
            "bounded decimal",
        )
    })?;
    let power = BigInt::from(10_u8).pow(magnitude);
    if scale >= 0 {
        Ok(BigRational::new(coefficient, power))
    } else {
        Ok(BigRational::from_integer(coefficient * power))
    }
}

fn validate_fraction_constraints(
    value: &BigRational,
    constraints: &FractionConstraints,
) -> Result<(), ValidationError> {
    if let Some(bound) = &constraints.greater_than
        && value <= bound
    {
        return Err(fraction_bound_error("greater_than", bound));
    }
    if let Some(bound) = &constraints.greater_or_equal
        && value < bound
    {
        return Err(fraction_bound_error("greater_than_equal", bound));
    }
    if let Some(bound) = &constraints.less_than
        && value >= bound
    {
        return Err(fraction_bound_error("less_than", bound));
    }
    if let Some(bound) = &constraints.less_or_equal
        && value > bound
    {
        return Err(fraction_bound_error("less_than_equal", bound));
    }
    if let Some(multiple) = &constraints.multiple_of {
        if multiple.is_zero() {
            return Err(type_error(
                "schema_invalid",
                "multiple_of cannot be zero",
                "nonzero fraction",
            ));
        }
        if !(value / multiple).is_integer() {
            return Err(ValidationError::one(
                ErrorDetail::new("multiple_of", "Input must be a multiple")
                    .context("multiple_of", multiple.to_string()),
            ));
        }
    }
    Ok(())
}

fn fraction_bound_error(code: &'static str, bound: &BigRational) -> ValidationError {
    ValidationError::one(
        ErrorDetail::new(code, "Input violates a fraction constraint")
            .context("bound", bound.to_string()),
    )
}

fn validate_complex(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    constraints: &ComplexConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let value = match input {
        InputValue::Complex { real, imaginary } => Complex64::new(*real, *imaginary),
        InputValue::Float(value) if !strict => Complex64::new(*value, 0.0),
        InputValue::Integer(value) if !strict => Complex64::new(
            value.parse::<f64>().map_err(|_| {
                type_error(
                    "complex_parsing",
                    "Input must be a valid complex",
                    "complex",
                )
            })?,
            0.0,
        ),
        InputValue::String(value) if !strict || profile == InputProfile::Strings => {
            let source = value.replace('j', "i");
            Complex64::from_str(&source).map_err(|_| {
                type_error(
                    "complex_parsing",
                    "Input must be a valid complex",
                    "complex",
                )
            })?
        }
        _ => {
            return Err(type_error(
                "complex_type",
                "Input must be a complex number",
                "complex",
            ));
        }
    };
    if !constraints.allow_non_finite && (!value.re.is_finite() || !value.im.is_finite()) {
        return Err(type_error(
            "finite_number",
            "Complex components must be finite",
            "finite complex",
        ));
    }
    let magnitude = value.norm();
    if constraints
        .magnitude_greater_or_equal
        .is_some_and(|minimum| magnitude < minimum)
    {
        return Err(float_bound_error(
            "complex_magnitude_too_small",
            constraints.magnitude_greater_or_equal,
        ));
    }
    if constraints
        .magnitude_less_or_equal
        .is_some_and(|maximum| magnitude > maximum)
    {
        return Err(float_bound_error(
            "complex_magnitude_too_large",
            constraints.magnitude_less_or_equal,
        ));
    }
    Ok(ValidatedValue::Complex(value))
}

fn validate_string(
    input: &InputValue,
    strict: bool,
    constraints: &StringConstraints,
) -> Result<ValidatedValue, ValidationError> {
    if constraints.to_upper && constraints.to_lower {
        return Err(type_error(
            "schema_invalid",
            "to_upper and to_lower cannot both be enabled",
            "one case conversion",
        ));
    }
    let converted = match input {
        InputValue::String(value) => value.clone(),
        InputValue::Bytes(value) if !strict => String::from_utf8(value.clone()).map_err(|_| {
            type_error("string_unicode", "Bytes must contain UTF-8", "UTF-8 string")
        })?,
        InputValue::Integer(value) if !strict && constraints.coerce_numbers_to_str => value.clone(),
        InputValue::Float(value)
            if !strict && constraints.coerce_numbers_to_str && value.is_finite() =>
        {
            value.to_string()
        }
        InputValue::Decimal(value) if !strict && constraints.coerce_numbers_to_str => value.clone(),
        _ => return Err(type_error("string_type", "Input must be a string", "str")),
    };
    let mut value = if constraints.strip_whitespace {
        converted.trim().to_owned()
    } else {
        converted
    };
    if constraints.ascii_only && !value.is_ascii() {
        return Err(type_error(
            "string_not_ascii",
            "String must contain only ASCII characters",
            "ASCII string",
        ));
    }
    let length = value.chars().count();
    validate_length(
        length,
        constraints.min_length,
        constraints.max_length,
        "string",
    )?;
    if let Some(pattern) = &constraints.pattern
        && !pattern.is_match(&value)
    {
        return Err(ValidationError::one(
            ErrorDetail::new(
                "string_pattern_mismatch",
                "String does not match the pattern",
            )
            .context("pattern", pattern.source()),
        ));
    }
    if constraints.to_upper {
        value = value.to_uppercase();
    } else if constraints.to_lower {
        value = value.to_lowercase();
    }
    Ok(ValidatedValue::String(value))
}

fn validate_bytes(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    constraints: &BytesConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let value = match input {
        InputValue::Bytes(value) => value.clone(),
        InputValue::String(value) if !strict || profile == InputProfile::Strings => {
            value.as_bytes().to_vec()
        }
        _ => return Err(type_error("bytes_type", "Input must be bytes", "bytes")),
    };
    validate_length(
        value.len(),
        constraints.min_length,
        constraints.max_length,
        "bytes",
    )?;
    Ok(ValidatedValue::Bytes(value))
}

pub(crate) fn validate_length(
    length: usize,
    minimum: Option<usize>,
    maximum: Option<usize>,
    kind: &str,
) -> Result<(), ValidationError> {
    if minimum.is_some_and(|minimum| length < minimum) {
        return Err(ValidationError::one(
            ErrorDetail::new("too_short", format!("{kind} is too short"))
                .context("minimum", minimum.unwrap_or_default().to_string()),
        ));
    }
    if maximum.is_some_and(|maximum| length > maximum) {
        return Err(ValidationError::one(
            ErrorDetail::new("too_long", format!("{kind} is too long"))
                .context("maximum", maximum.unwrap_or_default().to_string()),
        ));
    }
    Ok(())
}

pub(crate) fn type_error(
    code: &'static str,
    message: &'static str,
    expected: &'static str,
) -> ValidationError {
    ValidationError::one(ErrorDetail::new(code, message).expected(expected))
}

fn resource_error(message: &'static str) -> ValidationError {
    type_error("resource_limit", message, "bounded numeric input")
}
