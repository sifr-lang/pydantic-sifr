use core::fmt;

use crate::schema::{NodeIndex, VerifiedSchemaProgram};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanOp {
    Enter(NodeIndex),
    Leave(NodeIndex),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    root: NodeIndex,
    operations: Vec<PlanOp>,
}

impl ExecutionPlan {
    pub fn from_verified(program: &VerifiedSchemaProgram) -> Result<Self, PlanError> {
        let root = program.program().root;
        let operations = vec![PlanOp::Enter(root), PlanOp::Leave(root)];
        Ok(Self { root, operations })
    }

    #[must_use]
    pub const fn root(&self) -> NodeIndex {
        self.root
    }

    #[must_use]
    pub fn operations(&self) -> &[PlanOp] {
        &self.operations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanError {
    InvalidVerifiedProgram,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("verified schema program cannot form an execution plan")
    }
}

impl std::error::Error for PlanError {}
