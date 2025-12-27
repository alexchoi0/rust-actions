use crate::parser::Step;
use crate::runner::StepResult;
use crate::world::World;
use std::future::Future;
use std::pin::Pin;

pub type BeforeAllFn = fn() -> Pin<Box<dyn Future<Output = ()> + Send>>;
pub type AfterAllFn = fn() -> Pin<Box<dyn Future<Output = ()> + Send>>;
pub type BeforeScenarioFn<W> = for<'a> fn(&'a mut W) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
pub type AfterScenarioFn<W> = for<'a> fn(&'a mut W) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
pub type BeforeStepFn<W> = for<'a> fn(&'a mut W, &'a Step) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
pub type AfterStepFn<W> =
    for<'a> fn(&'a mut W, &'a Step, &'a StepResult) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub enum HookDef<W: World> {
    BeforeAll(BeforeAllFn),
    AfterAll(AfterAllFn),
    BeforeScenario(BeforeScenarioFn<W>),
    AfterScenario(AfterScenarioFn<W>),
    BeforeStep(BeforeStepFn<W>),
    AfterStep(AfterStepFn<W>),
}

impl<W: World> HookDef<W> {
    pub fn before_all(f: BeforeAllFn) -> Self {
        HookDef::BeforeAll(f)
    }

    pub fn after_all(f: AfterAllFn) -> Self {
        HookDef::AfterAll(f)
    }

    pub fn before_scenario(f: BeforeScenarioFn<W>) -> Self {
        HookDef::BeforeScenario(f)
    }

    pub fn after_scenario(f: AfterScenarioFn<W>) -> Self {
        HookDef::AfterScenario(f)
    }

    pub fn before_step(f: BeforeStepFn<W>) -> Self {
        HookDef::BeforeStep(f)
    }

    pub fn after_step(f: AfterStepFn<W>) -> Self {
        HookDef::AfterStep(f)
    }
}

pub struct HookRegistry<W: World> {
    before_all: Vec<BeforeAllFn>,
    after_all: Vec<AfterAllFn>,
    before_scenario: Vec<BeforeScenarioFn<W>>,
    after_scenario: Vec<AfterScenarioFn<W>>,
    before_step: Vec<BeforeStepFn<W>>,
    after_step: Vec<AfterStepFn<W>>,
}

impl<W: World> HookRegistry<W> {
    pub fn new() -> Self {
        Self {
            before_all: Vec::new(),
            after_all: Vec::new(),
            before_scenario: Vec::new(),
            after_scenario: Vec::new(),
            before_step: Vec::new(),
            after_step: Vec::new(),
        }
    }

    pub fn register(&mut self, hook: HookDef<W>) {
        match hook {
            HookDef::BeforeAll(f) => self.before_all.push(f),
            HookDef::AfterAll(f) => self.after_all.push(f),
            HookDef::BeforeScenario(f) => self.before_scenario.push(f),
            HookDef::AfterScenario(f) => self.after_scenario.push(f),
            HookDef::BeforeStep(f) => self.before_step.push(f),
            HookDef::AfterStep(f) => self.after_step.push(f),
        }
    }

    pub async fn run_before_all(&self) {
        for hook in &self.before_all {
            hook().await;
        }
    }

    pub async fn run_after_all(&self) {
        for hook in &self.after_all {
            hook().await;
        }
    }

    pub async fn run_before_scenario(&self, world: &mut W) {
        for hook in &self.before_scenario {
            hook(world).await;
        }
    }

    pub async fn run_after_scenario(&self, world: &mut W) {
        for hook in &self.after_scenario {
            hook(world).await;
        }
    }

    pub async fn run_before_step(&self, world: &mut W, step: &Step) {
        for hook in &self.before_step {
            hook(world, step).await;
        }
    }

    pub async fn run_after_step(&self, world: &mut W, step: &Step, result: &StepResult) {
        for hook in &self.after_step {
            hook(world, step, result).await;
        }
    }
}

impl<W: World> Default for HookRegistry<W> {
    fn default() -> Self {
        Self::new()
    }
}
