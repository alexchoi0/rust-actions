use crate::expr::{evaluate_assertion, evaluate_value, ContainerInfo, ExprContext};
use crate::hooks::HookRegistry;
use crate::parser::{parse_features, Feature, Scenario, Step};
use crate::registry::{ErasedStepFn, StepRegistry};
use crate::world::World;
use crate::Result;
use colored::Colorize;
use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum StepResult {
    Passed(Duration),
    Failed(Duration, String),
    Skipped,
}

impl StepResult {
    pub fn is_passed(&self) -> bool {
        matches!(self, StepResult::Passed(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, StepResult::Failed(_, _))
    }
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub name: String,
    pub steps: Vec<(String, StepResult)>,
    pub duration: Duration,
}

impl ScenarioResult {
    pub fn passed(&self) -> bool {
        self.steps.iter().all(|(_, r)| r.is_passed())
    }

    pub fn steps_passed(&self) -> usize {
        self.steps.iter().filter(|(_, r)| r.is_passed()).count()
    }

    pub fn steps_failed(&self) -> usize {
        self.steps.iter().filter(|(_, r)| r.is_failed()).count()
    }
}

#[derive(Debug)]
pub struct FeatureResult {
    pub name: String,
    pub scenarios: Vec<ScenarioResult>,
    pub duration: Duration,
}

impl FeatureResult {
    pub fn passed(&self) -> bool {
        self.scenarios.iter().all(|s| s.passed())
    }

    pub fn scenarios_passed(&self) -> usize {
        self.scenarios.iter().filter(|s| s.passed()).count()
    }

    pub fn scenarios_failed(&self) -> usize {
        self.scenarios.iter().filter(|s| !s.passed()).count()
    }

    pub fn total_steps_passed(&self) -> usize {
        self.scenarios.iter().map(|s| s.steps_passed()).sum()
    }

    pub fn total_steps_failed(&self) -> usize {
        self.scenarios.iter().map(|s| s.steps_failed()).sum()
    }
}

pub struct RustActions<W: World + 'static> {
    features_path: PathBuf,
    steps: StepRegistry,
    hooks: HookRegistry<W>,
    _phantom: PhantomData<W>,
}

impl<W: World + 'static> RustActions<W> {
    pub fn new() -> Self {
        let mut steps = StepRegistry::new();
        steps.collect_for::<W>();

        Self {
            features_path: PathBuf::from("tests/features"),
            steps,
            hooks: HookRegistry::new(),
            _phantom: PhantomData,
        }
    }

    pub fn features(mut self, path: impl Into<PathBuf>) -> Self {
        self.features_path = path.into();
        self
    }

    pub fn register_step(mut self, name: impl Into<String>, func: ErasedStepFn) -> Self {
        self.steps.register(name, func);
        self
    }

    pub async fn run(self) {
        tokio::time::pause();

        let features = match parse_features(&self.features_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{} Failed to parse features: {}", "Error:".red().bold(), e);
                std::process::exit(1);
            }
        };

        self.hooks.run_before_all().await;

        let mut all_results = Vec::new();
        let mut total_passed = 0;
        let mut total_failed = 0;

        for feature in features {
            let result = self.run_feature(feature).await;
            total_passed += result.scenarios_passed();
            total_failed += result.scenarios_failed();
            all_results.push(result);
        }

        self.hooks.run_after_all().await;

        println!();
        let total_scenarios = total_passed + total_failed;
        let total_steps_passed: usize = all_results.iter().map(|r| r.total_steps_passed()).sum();
        let total_steps_failed: usize = all_results.iter().map(|r| r.total_steps_failed()).sum();
        let total_steps = total_steps_passed + total_steps_failed;

        if total_failed == 0 {
            println!(
                "{} {} ({} passed)",
                format!("{} scenarios", total_scenarios).green(),
                "✓".green(),
                total_passed
            );
        } else {
            println!(
                "{} ({} passed, {} failed)",
                format!("{} scenarios", total_scenarios).yellow(),
                total_passed,
                total_failed
            );
        }

        println!(
            "{} ({} passed, {} failed)",
            format!("{} steps", total_steps),
            total_steps_passed,
            total_steps_failed
        );

        if total_failed > 0 {
            std::process::exit(1);
        }
    }

    async fn run_feature(&self, feature: Feature) -> FeatureResult {
        let start = Instant::now();
        println!("\n{} {}", "Feature:".bold(), feature.name);

        let mut scenario_results = Vec::new();

        for scenario in feature.scenarios {
            let result = self
                .run_scenario(&scenario, &feature.env, &feature.containers)
                .await;
            scenario_results.push(result);
        }

        FeatureResult {
            name: feature.name,
            scenarios: scenario_results,
            duration: start.elapsed(),
        }
    }

    async fn run_scenario(
        &self,
        scenario: &Scenario,
        env: &HashMap<String, String>,
        containers: &HashMap<String, String>,
    ) -> ScenarioResult {
        let start = Instant::now();

        let mut world = match W::new().await {
            Ok(w) => w,
            Err(e) => {
                println!(
                    "  {} {} (world init failed: {})",
                    "✗".red(),
                    scenario.name,
                    e
                );
                return ScenarioResult {
                    name: scenario.name.clone(),
                    steps: vec![],
                    duration: start.elapsed(),
                };
            }
        };

        self.hooks.run_before_scenario(&mut world).await;

        let mut ctx = ExprContext::new();
        ctx.env = env.clone();

        for (name, _image) in containers {
            ctx.containers.insert(
                name.clone(),
                ContainerInfo {
                    url: format!("{}://localhost:5432", name),
                    host: "localhost".to_string(),
                    port: 5432,
                },
            );
        }

        let mut step_results = Vec::new();
        let mut should_skip = false;

        for step in &scenario.steps {
            if should_skip {
                step_results.push((step.name.clone(), StepResult::Skipped));
                continue;
            }

            self.hooks.run_before_step(&mut world, step).await;

            let result = self.run_step(&mut world, step, &mut ctx).await;

            self.hooks.run_after_step(&mut world, step, &result).await;

            if result.is_failed() && !step.continue_on_error {
                should_skip = true;
            }

            step_results.push((step.name.clone(), result));
        }

        self.hooks.run_after_scenario(&mut world).await;

        let duration = start.elapsed();
        let all_passed = step_results.iter().all(|(_, r)| r.is_passed());

        if all_passed {
            println!(
                "  {} {} ({:?})",
                "✓".green(),
                scenario.name,
                duration
            );
        } else {
            println!(
                "  {} {} ({:?})",
                "✗".red(),
                scenario.name,
                duration
            );
        }

        for (name, result) in &step_results {
            match result {
                StepResult::Passed(_) => {
                    println!("    {} {}", "✓".green(), name);
                }
                StepResult::Failed(_, msg) => {
                    println!("    {} {}", "✗".red(), name);
                    println!("      {}: {}", "Error".red(), msg);
                }
                StepResult::Skipped => {
                    println!("    {} {} (skipped)", "○".dimmed(), name);
                }
            }
        }

        ScenarioResult {
            name: scenario.name.clone(),
            steps: step_results,
            duration,
        }
    }

    async fn run_step(
        &self,
        world: &mut W,
        step: &Step,
        ctx: &mut ExprContext,
    ) -> StepResult {
        let start = Instant::now();

        for assertion in &step.pre_assert {
            match evaluate_assertion(assertion, ctx) {
                Ok(true) => {}
                Ok(false) => {
                    return StepResult::Failed(
                        start.elapsed(),
                        format!("Pre-assertion failed: {}", assertion),
                    );
                }
                Err(e) => {
                    return StepResult::Failed(
                        start.elapsed(),
                        format!("Pre-assertion error: {}", e),
                    );
                }
            }
        }

        let step_fn = match self.steps.get(&step.uses) {
            Some(f) => f,
            None => {
                return StepResult::Failed(
                    start.elapsed(),
                    format!("Step not found: {}", step.uses),
                );
            }
        };

        let evaluated_args = match step
            .with
            .iter()
            .map(|(k, v)| evaluate_value(v, ctx).map(|ev| (k.clone(), ev)))
            .collect::<Result<HashMap<_, _>>>()
        {
            Ok(args) => args,
            Err(e) => {
                return StepResult::Failed(
                    start.elapsed(),
                    format!("Args evaluation failed: {}", e),
                );
            }
        };

        let world_any: &mut dyn Any = world;
        let outputs = match step_fn(world_any, evaluated_args).await {
            Ok(outputs) => outputs,
            Err(e) => return StepResult::Failed(start.elapsed(), e.to_string()),
        };

        if let Some(id) = &step.id {
            ctx.steps.insert(id.clone(), outputs.clone());
        }

        if !step.post_assert.is_empty() {
            let assert_ctx = ctx.with_outputs(outputs);

            for assertion in &step.post_assert {
                match evaluate_assertion(assertion, &assert_ctx) {
                    Ok(true) => {}
                    Ok(false) => {
                        return StepResult::Failed(
                            start.elapsed(),
                            format!("Post-assertion failed: {}", assertion),
                        );
                    }
                    Err(e) => {
                        return StepResult::Failed(
                            start.elapsed(),
                            format!("Post-assertion error: {}", e),
                        );
                    }
                }
            }
        }

        StepResult::Passed(start.elapsed())
    }
}

impl<W: World + 'static> Default for RustActions<W> {
    fn default() -> Self {
        Self::new()
    }
}
