use core::fmt;

/// A compact checked index into an arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArenaId(u32);

impl ArenaId {
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    pub fn from_usize(value: usize) -> Result<Self, ArenaError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| ArenaError::CapacityExceeded)
    }
}

/// A move-owned arena whose values are only addressed through checked IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arena<T> {
    values: Vec<T>,
}

impl<T> Arena<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn push(&mut self, value: T) -> Result<ArenaId, ArenaError> {
        let id = ArenaId::from_usize(self.values.len())?;
        self.values.push(value);
        Ok(id)
    }

    #[must_use]
    pub fn get(&self, id: ArenaId) -> Option<&T> {
        self.values.get(id.raw() as usize)
    }

    #[must_use]
    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub(crate) fn into_values(self) -> Vec<T> {
        self.values
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaError {
    CapacityExceeded,
}

impl fmt::Display for ArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("arena capacity exceeds the compact index range")
    }
}

impl std::error::Error for ArenaError {}
