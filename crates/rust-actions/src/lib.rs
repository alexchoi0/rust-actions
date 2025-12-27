pub mod args;
pub mod determinism;
pub mod error;
pub mod expr;
pub mod hooks;
pub mod matrix;
pub mod outputs;
pub mod parser;
pub mod registry;
pub mod runner;
pub mod workflow_registry;
pub mod world;

pub use error::{Error, Result};
pub use rust_actions_macros::*;

pub use inventory;
pub use serde_json;

pub mod prelude {
    pub use crate::args::{FromArgs, RawArgs};
    pub use crate::determinism::SeededRng;
    pub use crate::error::{Error, Result, StepError};
    pub use crate::expr::JobOutputs;
    pub use crate::hooks::HookDef;
    pub use crate::matrix::{expand_matrix, MatrixCombination};
    pub use crate::outputs::{IntoOutputs, StepOutputs};
    pub use crate::parser::{Job, Step, Strategy, Workflow};
    pub use crate::registry::ErasedStepDef;
    pub use crate::runner::{JobResult, RustActions, StepResult, WorkflowResult};
    pub use crate::workflow_registry::WorkflowRegistry;
    pub use crate::world::World;
    pub use rust_actions_macros::{
        after_all, after_scenario, after_step, before_all, before_scenario, before_step, step,
        Args, Outputs, World,
    };
    pub use serde::{Deserialize, Serialize};
    pub use uuid::Uuid;
}
