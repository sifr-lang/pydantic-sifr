use crate::schema::VerifiedSchemaProgram;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanOp {
    EnterProgram,
    LeaveProgram,
}

/// Foundation for an execution plan tied to one sealed static program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    program_identity: [u8; 32],
    operations: [PlanOp; 2],
}

impl ExecutionPlan {
    #[must_use]
    pub fn from_verified(program: &VerifiedSchemaProgram<'_>) -> Self {
        Self {
            program_identity: program.header().program_identity,
            operations: [PlanOp::EnterProgram, PlanOp::LeaveProgram],
        }
    }

    #[must_use]
    pub const fn program_identity(&self) -> [u8; 32] {
        self.program_identity
    }

    #[must_use]
    pub const fn operations(&self) -> &[PlanOp; 2] {
        &self.operations
    }
}
