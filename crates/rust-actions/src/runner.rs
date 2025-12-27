use crate::expr::{evaluate_assertion, evaluate_value, ExprContext, JobOutputs};
use crate::hooks::HookRegistry;
use crate::matrix::{expand_matrix, format_matrix_suffix, MatrixCombination};
use crate::parser::{parse_workflow_file, parse_workflows, Job, Step, Workflow};
use crate::registry::{ErasedStepFn, StepRegistry};
use crate::workflow_registry::{is_file_ref, parse_file_ref, WorkflowRegistry};
use crate::world::World;
use crate::{Error, Result};
use colored::Colorize;
use serde_json::Value;
use std::any::Any;
use std::collections::{HashMap, HashSet};
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
pub struct JobResult {
    pub name: String,
    pub matrix_suffix: String,
    pub steps: Vec<(String, StepResult)>,
    pub outputs: JobOutputs,
    pub duration: Duration,
}

impl JobResult {
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
pub struct WorkflowResult {
    pub name: String,
    pub jobs: Vec<JobResult>,
    pub duration: Duration,
}

impl WorkflowResult {
    pub fn passed(&self) -> bool {
        self.jobs.iter().all(|j| j.passed())
    }

    pub fn jobs_passed(&self) -> usize {
        self.jobs.iter().filter(|j| j.passed()).count()
    }

    pub fn jobs_failed(&self) -> usize {
        self.jobs.iter().filter(|j| !j.passed()).count()
    }

    pub fn total_steps_passed(&self) -> usize {
        self.jobs.iter().map(|j| j.steps_passed()).sum()
    }

    pub fn total_steps_failed(&self) -> usize {
        self.jobs.iter().map(|j| j.steps_failed()).sum()
    }
}

pub struct RustActions<W: World + 'static> {
    workflows_path: PathBuf,
    single_workflow: Option<PathBuf>,
    steps: StepRegistry,
    hooks: HookRegistry<W>,
    session_id: String,
    _phantom: PhantomData<W>,
}

impl<W: World + 'static> RustActions<W> {
    pub fn new() -> Self {
        let mut steps = StepRegistry::new();
        steps.collect_for::<W>();

        let session_id = uuid::Uuid::new_v4().to_string().replace("-", "")[..8].to_string();

        Self {
            workflows_path: PathBuf::from("tests/workflows"),
            single_workflow: None,
            steps,
            hooks: HookRegistry::new(),
            session_id,
            _phantom: PhantomData,
        }
    }

    pub fn workflows(mut self, path: impl Into<PathBuf>) -> Self {
        self.workflows_path = path.into();
        self
    }

    pub fn features(self, path: impl Into<PathBuf>) -> Self {
        self.workflows(path)
    }

    pub fn workflow(mut self, path: impl Into<PathBuf>) -> Self {
        self.single_workflow = Some(path.into());
        self
    }

    pub fn register_step(mut self, name: impl Into<String>, func: ErasedStepFn) -> Self {
        self.steps.register(name, func);
        self
    }

    pub async fn run(self) {
        std::env::set_var("RUST_ACTIONS_SESSION_ID", &self.session_id);

        let registry = if self.single_workflow.is_some() {
            None
        } else {
            match WorkflowRegistry::build(&self.workflows_path) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!(
                        "{} Failed to build workflow registry: {}",
                        "Error:".red().bold(),
                        e
                    );
                    std::process::exit(1);
                }
            }
        };

        let workflows: Vec<(PathBuf, Workflow)> = if let Some(ref path) = self.single_workflow {
            match parse_workflow_file(path) {
                Ok(w) => vec![w],
                Err(e) => {
                    eprintln!("{} Failed to parse workflow: {}", "Error:".red().bold(), e);
                    std::process::exit(1);
                }
            }
        } else {
            match parse_workflows(&self.workflows_path) {
                Ok(w) => w.into_iter().filter(|(_, w)| !w.is_reusable()).collect(),
                Err(e) => {
                    eprintln!(
                        "{} Failed to parse workflows: {}",
                        "Error:".red().bold(),
                        e
                    );
                    std::process::exit(1);
                }
            }
        };

        self.hooks.run_before_all().await;

        let mut all_results = Vec::new();
        let mut total_passed = 0;
        let mut total_failed = 0;

        for (path, workflow) in workflows {
            let result = self.run_workflow(&path, workflow, registry.as_ref()).await;
            total_passed += result.jobs_passed();
            total_failed += result.jobs_failed();
            all_results.push(result);
        }

        self.hooks.run_after_all().await;

        println!();
        let total_jobs = total_passed + total_failed;
        let total_steps_passed: usize = all_results.iter().map(|r| r.total_steps_passed()).sum();
        let total_steps_failed: usize = all_results.iter().map(|r| r.total_steps_failed()).sum();
        let total_steps = total_steps_passed + total_steps_failed;

        if total_failed == 0 {
            println!(
                "{} {} ({} passed)",
                format!("{} jobs", total_jobs).green(),
                "✓".green(),
                total_passed
            );
        } else {
            println!(
                "{} ({} passed, {} failed)",
                format!("{} jobs", total_jobs).yellow(),
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

    async fn run_workflow(
        &self,
        _path: &PathBuf,
        workflow: Workflow,
        registry: Option<&WorkflowRegistry>,
    ) -> WorkflowResult {
        let start = Instant::now();
        println!("\n{} {}", "Workflow:".bold(), workflow.name);

        let job_order = match toposort_jobs(&workflow.jobs) {
            Ok(order) => order,
            Err(e) => {
                eprintln!("{} {}", "Error:".red().bold(), e);
                return WorkflowResult {
                    name: workflow.name,
                    jobs: vec![],
                    duration: start.elapsed(),
                };
            }
        };

        let mut job_outputs: HashMap<String, JobOutputs> = HashMap::new();
        let mut job_results = Vec::new();

        for job_name in job_order {
            let job = &workflow.jobs[&job_name];

            if let Some(uses) = &job.uses {
                if is_file_ref(uses) {
                    if let Some(reg) = registry {
                        match self
                            .run_file_ref_job(&job_name, uses, job, reg, &job_outputs)
                            .await
                        {
                            Ok(result) => {
                                job_outputs.insert(job_name.clone(), result.outputs.clone());
                                job_results.push(result);
                            }
                            Err(e) => {
                                eprintln!(
                                    "  {} {} ({})",
                                    "✗".red(),
                                    job_name,
                                    e
                                );
                            }
                        }
                    }
                    continue;
                }
            }

            let matrix_combos = job
                .strategy
                .as_ref()
                .map(|s| expand_matrix(s))
                .unwrap_or_else(|| vec![HashMap::new()]);

            for matrix_values in matrix_combos {
                let result = self
                    .run_job(&job_name, job, &workflow.env, &job_outputs, &matrix_values)
                    .await;
                job_outputs.insert(job_name.clone(), result.outputs.clone());
                job_results.push(result);
            }
        }

        WorkflowResult {
            name: workflow.name,
            jobs: job_results,
            duration: start.elapsed(),
        }
    }

    async fn run_file_ref_job(
        &self,
        job_name: &str,
        uses: &str,
        _job: &Job,
        registry: &WorkflowRegistry,
        parent_outputs: &HashMap<String, JobOutputs>,
    ) -> Result<JobResult> {
        let start = Instant::now();
        let file_path = parse_file_ref(uses)?;
        let ref_workflow = registry.resolve_file_ref(uses)?;

        println!(
            "  {} {} (via @file:{})",
            "Job:".dimmed(),
            job_name,
            file_path
        );

        let mut combined_outputs = JobOutputs::new();

        let ref_job_order = toposort_jobs(&ref_workflow.jobs)?;

        let mut ref_job_outputs: HashMap<String, JobOutputs> = HashMap::new();
        let mut all_step_results = Vec::new();

        for ref_job_name in ref_job_order {
            let ref_job = &ref_workflow.jobs[&ref_job_name];

            let mut world = match W::new().await {
                Ok(w) => w,
                Err(_) => {
                    return Ok(JobResult {
                        name: job_name.to_string(),
                        matrix_suffix: String::new(),
                        steps: vec![],
                        outputs: JobOutputs::new(),
                        duration: start.elapsed(),
                    });
                }
            };

            let mut ctx = ExprContext::new();
            ctx.env = ref_workflow.env.clone();

            for (dep_name, dep_outputs) in &ref_job_outputs {
                ctx.needs.insert(dep_name.clone(), dep_outputs.clone());
            }
            for (dep_name, dep_outputs) in parent_outputs {
                ctx.needs.insert(dep_name.clone(), dep_outputs.clone());
            }

            #[allow(unused_variables)]
            let step_outputs: HashMap<String, Value> = HashMap::new();

            for step in &ref_job.steps {
                let result = self.run_step(&mut world, step, &mut ctx).await;
                let step_name = step.name.clone().unwrap_or_else(|| step.uses.clone());

                match &result {
                    StepResult::Passed(_) => {
                        println!("    {} {}", "✓".green(), step_name);
                    }
                    StepResult::Failed(_, msg) => {
                        println!("    {} {}", "✗".red(), step_name);
                        println!("      {}: {}", "Error".red(), msg);
                    }
                    StepResult::Skipped => {
                        println!("    {} {} (skipped)", "○".dimmed(), step_name);
                    }
                }

                all_step_results.push((step_name, result));
            }

            let mut ref_job_output = JobOutputs::new();
            for (key, expr) in &ref_job.outputs {
                if let Ok(value) = evaluate_value(&Value::String(expr.clone()), &ctx) {
                    ref_job_output.insert(key.clone(), value);
                }
            }
            ref_job_outputs.insert(ref_job_name.clone(), ref_job_output.clone());
        }

        if let Some(trigger) = &ref_workflow.on {
            if let Some(call_config) = &trigger.workflow_call {
                for (key, output_def) in &call_config.outputs {
                    let mut eval_ctx = ExprContext::new();
                    for (job_name, outputs) in &ref_job_outputs {
                        eval_ctx.jobs.insert(job_name.clone(), outputs.clone());
                    }
                    if let Ok(value) =
                        evaluate_value(&Value::String(output_def.value.clone()), &eval_ctx)
                    {
                        combined_outputs.insert(key.clone(), value);
                    }
                }
            }
        }

        Ok(JobResult {
            name: job_name.to_string(),
            matrix_suffix: String::new(),
            steps: all_step_results,
            outputs: combined_outputs,
            duration: start.elapsed(),
        })
    }

    async fn run_job(
        &self,
        job_name: &str,
        job: &Job,
        workflow_env: &HashMap<String, String>,
        parent_outputs: &HashMap<String, JobOutputs>,
        matrix_values: &MatrixCombination,
    ) -> JobResult {
        let start = Instant::now();
        let matrix_suffix = format_matrix_suffix(matrix_values);

        let mut world = match W::new().await {
            Ok(w) => w,
            Err(e) => {
                println!(
                    "  {} {}{} (world init failed: {})",
                    "✗".red(),
                    job_name,
                    matrix_suffix,
                    e
                );
                return JobResult {
                    name: job_name.to_string(),
                    matrix_suffix,
                    steps: vec![],
                    outputs: JobOutputs::new(),
                    duration: start.elapsed(),
                };
            }
        };

        self.hooks.run_before_scenario(&mut world).await;

        let mut ctx = ExprContext::new();
        ctx.env = workflow_env.clone();
        ctx.env.extend(job.env.clone());
        ctx.matrix = matrix_values.clone();

        for need in job.needs.as_vec() {
            if let Some(outputs) = parent_outputs.get(&need) {
                ctx.needs.insert(need.clone(), outputs.clone());
            }
        }

        let mut step_results = Vec::new();
        let mut should_skip = false;

        for step in &job.steps {
            let step_name = step.name.clone().unwrap_or_else(|| step.uses.clone());

            if should_skip {
                step_results.push((step_name, StepResult::Skipped));
                continue;
            }

            self.hooks.run_before_step(&mut world, step).await;

            let result = self.run_step(&mut world, step, &mut ctx).await;

            self.hooks.run_after_step(&mut world, step, &result).await;

            if result.is_failed() && !step.continue_on_error {
                should_skip = true;
            }

            step_results.push((step_name, result));
        }

        self.hooks.run_after_scenario(&mut world).await;

        let duration = start.elapsed();
        let all_passed = step_results.iter().all(|(_, r)| r.is_passed());

        if all_passed {
            println!(
                "  {} {}{} ({:?})",
                "✓".green(),
                job_name,
                matrix_suffix,
                duration
            );
        } else {
            println!(
                "  {} {}{} ({:?})",
                "✗".red(),
                job_name,
                matrix_suffix,
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

        let mut outputs = JobOutputs::new();
        for (key, expr) in &job.outputs {
            if let Ok(value) = evaluate_value(&Value::String(expr.clone()), &ctx) {
                outputs.insert(key.clone(), value);
            }
        }

        JobResult {
            name: job_name.to_string(),
            matrix_suffix,
            steps: step_results,
            outputs,
            duration,
        }
    }

    async fn run_step(&self, world: &mut W, step: &Step, ctx: &mut ExprContext) -> StepResult {
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

fn toposort_jobs(jobs: &HashMap<String, Job>) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut temp_visited = HashSet::new();

    fn visit(
        name: &str,
        jobs: &HashMap<String, Job>,
        visited: &mut HashSet<String>,
        temp_visited: &mut HashSet<String>,
        result: &mut Vec<String>,
        path: &mut Vec<String>,
    ) -> Result<()> {
        if temp_visited.contains(name) {
            path.push(name.to_string());
            return Err(Error::CircularDependency {
                chain: path.join(" -> "),
            });
        }

        if visited.contains(name) {
            return Ok(());
        }

        temp_visited.insert(name.to_string());
        path.push(name.to_string());

        if let Some(job) = jobs.get(name) {
            for dep in job.needs.as_vec() {
                if !jobs.contains_key(&dep) {
                    return Err(Error::JobDependencyNotFound {
                        job: name.to_string(),
                        dependency: dep.clone(),
                    });
                }
                visit(&dep, jobs, visited, temp_visited, result, path)?;
            }
        }

        path.pop();
        temp_visited.remove(name);
        visited.insert(name.to_string());
        result.push(name.to_string());

        Ok(())
    }

    let job_names: Vec<String> = jobs.keys().cloned().collect();
    for name in &job_names {
        let mut path = Vec::new();
        visit(name, jobs, &mut visited, &mut temp_visited, &mut result, &mut path)?;
    }

    Ok(result)
}
