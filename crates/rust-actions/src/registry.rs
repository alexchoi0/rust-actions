use crate::args::RawArgs;
use crate::outputs::StepOutputs;
use crate::world::World;
use crate::Result;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub type StepFn<W> =
    for<'a> fn(&'a mut W, RawArgs) -> Pin<Box<dyn Future<Output = Result<StepOutputs>> + Send + 'a>>;

pub type ErasedStepFn = for<'a> fn(
    &'a mut dyn Any,
    RawArgs,
) -> Pin<Box<dyn Future<Output = Result<StepOutputs>> + Send + 'a>>;

pub struct ErasedStepDef {
    pub name: &'static str,
    pub world_type_id: TypeId,
    pub func: ErasedStepFn,
}

impl ErasedStepDef {
    pub const fn new(
        name: &'static str,
        world_type_id: TypeId,
        func: ErasedStepFn,
    ) -> Self {
        Self {
            name,
            world_type_id,
            func,
        }
    }
}

inventory::collect!(ErasedStepDef);

pub struct StepRegistry {
    steps: HashMap<String, ErasedStepFn>,
}

impl StepRegistry {
    pub fn new() -> Self {
        Self {
            steps: HashMap::new(),
        }
    }

    pub fn collect_for<W: World + 'static>(&mut self) {
        let target_type_id = TypeId::of::<W>();

        for step in inventory::iter::<ErasedStepDef> {
            if step.world_type_id == target_type_id {
                self.steps.insert(step.name.to_string(), step.func);
            }
        }
    }

    pub fn register(&mut self, name: impl Into<String>, func: ErasedStepFn) {
        self.steps.insert(name.into(), func);
    }

    pub fn get(&self, name: &str) -> Option<&ErasedStepFn> {
        self.steps.get(name)
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Default for StepRegistry {
    fn default() -> Self {
        Self::new()
    }
}
