use crate::JsonLimits;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SelectionSegment {
    Field(String),
    Index(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPath {
    segments: Vec<SelectionSegment>,
}

impl SelectionPath {
    #[must_use]
    pub fn new(segments: Vec<SelectionSegment>) -> Self {
        Self { segments }
    }

    #[must_use]
    pub fn field(name: impl Into<String>) -> Self {
        Self::new(vec![SelectionSegment::Field(name.into())])
    }

    #[must_use]
    pub fn segments(&self) -> &[SelectionSegment] {
        &self.segments
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SerializationOptions {
    pub limits: JsonLimits,
    pub by_alias: bool,
    pub exclude_none: bool,
    pub exclude_defaults: bool,
    pub include: Vec<SelectionPath>,
    pub exclude: Vec<SelectionPath>,
}

pub(super) fn selected(options: &SerializationOptions, path: &[SelectionSegment]) -> bool {
    let included = options.include.is_empty()
        || options.include.iter().any(|selection| {
            is_prefix(selection.segments(), path) || is_prefix(path, selection.segments())
        });
    included
        && !options
            .exclude
            .iter()
            .any(|selection| is_prefix(selection.segments(), path))
}

fn is_prefix(prefix: &[SelectionSegment], path: &[SelectionSegment]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}
