use sifr_runtime::interop::structural::{
    ConstructToken, MappedValue, NodeId, NominalField, ShapeIdentity, StructuralConstruct,
    StructuralContractError, StructuralEdge, StructuralEdgeKind, StructuralEnter, StructuralKind,
    StructuralMapping, StructuralProject, StructuralSource, StructuralVisitor, VisitControl,
    metadata, nominal_record, primitive,
};
use std::fmt;

const URL_IDENTITY: &str = "pydantic_sifr.special_values.Url";
const MULTI_HOST_URL_IDENTITY: &str = "pydantic_sifr.special_values.MultiHostUrl";
const PATTERN_IDENTITY: &str = "pydantic_sifr.special_values.Pattern";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UrlValue {
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MultiHostUrlValue {
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatternValue {
    source: String,
    flags: u8,
}

#[derive(Debug)]
pub struct SpecialValueError;

impl fmt::Display for SpecialValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("special value access failed")
    }
}

impl std::error::Error for SpecialValueError {}

pub struct UrlMapping;
pub struct MultiHostUrlMapping;
pub struct PatternMapping;

fn text_shape(identity: &'static str) -> ShapeIdentity {
    nominal_record(
        identity,
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

fn construct_text<S: StructuralSource>(
    source: &mut S,
    node: NodeId,
    token: ConstructToken,
    identity: &'static str,
) -> Result<String, StructuralContractError> {
    let description = source.node(node)?;
    if description.kind() != StructuralKind::Record
        || description.nominal_identity() != Some(identity)
    {
        return Err(StructuralContractError::KindMismatch);
    }
    let [edge] = description.edges() else {
        return Err(StructuralContractError::ArityMismatch);
    };
    if edge.kind() != StructuralEdgeKind::RecordField("value") {
        return Err(StructuralContractError::MemberMismatch);
    }
    String::structural_construct_at(source, edge.node(), token)
}

fn project_text<'value, V: StructuralVisitor<'value>>(
    value: &'value String,
    visitor: &mut V,
    identity: &'static str,
) -> Result<(), V::Error> {
    let control = visitor.enter(StructuralEnter::new(
        StructuralKind::Record,
        Some(identity),
        1,
    ))?;
    if control == VisitControl::Continue {
        visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField(
            "value",
        )))?;
        value.structural_project(visitor)?;
    }
    visitor.exit(StructuralKind::Record)
}

impl StructuralMapping<UrlValue> for UrlMapping {
    fn shape_identity() -> ShapeIdentity {
        text_shape(URL_IDENTITY)
    }

    fn nominal_identity() -> Option<&'static str> {
        Some(URL_IDENTITY)
    }

    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<UrlValue, StructuralContractError> {
        construct_text(source, node, token, URL_IDENTITY).map(|value| UrlValue { value })
    }

    fn structural_project<'value, V: StructuralVisitor<'value>>(
        value: &'value UrlValue,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        project_text(&value.value, visitor, URL_IDENTITY)
    }
}

impl StructuralMapping<MultiHostUrlValue> for MultiHostUrlMapping {
    fn shape_identity() -> ShapeIdentity {
        text_shape(MULTI_HOST_URL_IDENTITY)
    }

    fn nominal_identity() -> Option<&'static str> {
        Some(MULTI_HOST_URL_IDENTITY)
    }

    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<MultiHostUrlValue, StructuralContractError> {
        construct_text(source, node, token, MULTI_HOST_URL_IDENTITY)
            .map(|value| MultiHostUrlValue { value })
    }

    fn structural_project<'value, V: StructuralVisitor<'value>>(
        value: &'value MultiHostUrlValue,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        project_text(&value.value, visitor, MULTI_HOST_URL_IDENTITY)
    }
}

impl StructuralMapping<PatternValue> for PatternMapping {
    fn shape_identity() -> ShapeIdentity {
        nominal_record(
            PATTERN_IDENTITY,
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

    fn nominal_identity() -> Option<&'static str> {
        Some(PATTERN_IDENTITY)
    }

    fn structural_construct_at<S: StructuralSource>(
        source: &mut S,
        node: NodeId,
        token: ConstructToken,
    ) -> Result<PatternValue, StructuralContractError> {
        let description = source.node(node)?;
        if description.kind() != StructuralKind::Record
            || description.nominal_identity() != Some(PATTERN_IDENTITY)
        {
            return Err(StructuralContractError::KindMismatch);
        }
        let [source_edge, flags_edge] = description.edges() else {
            return Err(StructuralContractError::ArityMismatch);
        };
        if source_edge.kind() != StructuralEdgeKind::RecordField("source")
            || flags_edge.kind() != StructuralEdgeKind::RecordField("flags")
        {
            return Err(StructuralContractError::MemberMismatch);
        }
        let source_node = source_edge.node();
        let flags_node = flags_edge.node();
        Ok(PatternValue {
            source: String::structural_construct_at(source, source_node, token)?,
            flags: u8::structural_construct_at(source, flags_node, token)?,
        })
    }

    fn structural_project<'value, V: StructuralVisitor<'value>>(
        value: &'value PatternValue,
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        let control = visitor.enter(StructuralEnter::new(
            StructuralKind::Record,
            Some(PATTERN_IDENTITY),
            2,
        ))?;
        if control == VisitControl::Continue {
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField(
                "source",
            )))?;
            value.source.structural_project(visitor)?;
            visitor.edge(StructuralEdge::new(StructuralEdgeKind::RecordField(
                "flags",
            )))?;
            value.flags.structural_project(visitor)?;
        }
        visitor.exit(StructuralKind::Record)
    }
}

pub fn url_text(value: &MappedValue<UrlValue, UrlMapping>) -> Result<String, SpecialValueError> {
    let value = value.inner_ref().map_err(|_| SpecialValueError)?;
    Ok(value.value.clone())
}

pub fn multi_host_url_text(
    value: &MappedValue<MultiHostUrlValue, MultiHostUrlMapping>,
) -> Result<String, SpecialValueError> {
    let value = value.inner_ref().map_err(|_| SpecialValueError)?;
    Ok(value.value.clone())
}

pub fn pattern_source(
    value: &MappedValue<PatternValue, PatternMapping>,
) -> Result<String, SpecialValueError> {
    let value = value.inner_ref().map_err(|_| SpecialValueError)?;
    Ok(value.source.clone())
}

pub fn pattern_flags(
    value: &MappedValue<PatternValue, PatternMapping>,
) -> Result<u8, SpecialValueError> {
    let value = value.inner_ref().map_err(|_| SpecialValueError)?;
    Ok(value.flags)
}
