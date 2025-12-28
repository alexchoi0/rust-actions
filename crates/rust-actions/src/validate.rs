use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

use crate::parser::JobNeeds;
use crate::workflow_registry::{is_file_ref, parse_file_ref, WorkflowRegistry};

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    JobDependencyNotFound {
        workflow: PathBuf,
        job: String,
        dependency: String,
    },
    FileReferenceNotFound {
        workflow: PathBuf,
        job: String,
        file_ref: String,
    },
    InvalidFileReference {
        workflow: PathBuf,
        job: String,
        uses: String,
    },
    CircularJobDependency {
        workflow: PathBuf,
        chain: Vec<String>,
    },
    DuplicateStepId {
        workflow: PathBuf,
        job: String,
        step_id: String,
    },
    InvalidOutputExpression {
        workflow: PathBuf,
        job: String,
        output_name: String,
        expression: String,
        reason: String,
    },
    ReusableWorkflowMissingOutputs {
        workflow: PathBuf,
        job: String,
        file_ref: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::JobDependencyNotFound {
                workflow,
                job,
                dependency,
            } => write!(
                f,
                "[{}] Job '{}' depends on non-existent job '{}'",
                workflow.display(),
                job,
                dependency
            ),
            ValidationError::FileReferenceNotFound {
                workflow,
                job,
                file_ref,
            } => write!(
                f,
                "[{}] Job '{}' references non-existent workflow '{}'",
                workflow.display(),
                job,
                file_ref
            ),
            ValidationError::InvalidFileReference {
                workflow,
                job,
                uses,
            } => write!(
                f,
                "[{}] Job '{}' has invalid file reference: '{}'",
                workflow.display(),
                job,
                uses
            ),
            ValidationError::CircularJobDependency { workflow, chain } => write!(
                f,
                "[{}] Circular job dependency detected: {}",
                workflow.display(),
                chain.join(" -> ")
            ),
            ValidationError::DuplicateStepId {
                workflow,
                job,
                step_id,
            } => write!(
                f,
                "[{}] Job '{}' has duplicate step id: '{}'",
                workflow.display(),
                job,
                step_id
            ),
            ValidationError::InvalidOutputExpression {
                workflow,
                job,
                output_name,
                expression,
                reason,
            } => write!(
                f,
                "[{}] Job '{}' output '{}' has invalid expression '{}': {}",
                workflow.display(),
                job,
                output_name,
                expression,
                reason
            ),
            ValidationError::ReusableWorkflowMissingOutputs {
                workflow,
                job,
                file_ref,
            } => write!(
                f,
                "[{}] Job '{}' uses reusable workflow '{}' but it has no defined outputs",
                workflow.display(),
                job,
                file_ref
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValidationWarning {
    EmptyWorkflow {
        workflow: PathBuf,
    },
    JobWithNoSteps {
        workflow: PathBuf,
        job: String,
    },
    UnusedReusableWorkflow {
        workflow: PathBuf,
    },
    StepWithoutId {
        workflow: PathBuf,
        job: String,
        step_index: usize,
        step_uses: String,
    },
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationWarning::EmptyWorkflow { workflow } => {
                write!(f, "[{}] Workflow has no jobs", workflow.display())
            }
            ValidationWarning::JobWithNoSteps { workflow, job } => write!(
                f,
                "[{}] Job '{}' has no steps and doesn't use a reusable workflow",
                workflow.display(),
                job
            ),
            ValidationWarning::UnusedReusableWorkflow { workflow } => write!(
                f,
                "[{}] Reusable workflow is not referenced by any other workflow",
                workflow.display()
            ),
            ValidationWarning::StepWithoutId {
                workflow,
                job,
                step_index,
                step_uses,
            } => write!(
                f,
                "[{}] Job '{}' step {} ('{}') has no id - outputs won't be accessible",
                workflow.display(),
                job,
                step_index,
                step_uses
            ),
        }
    }
}

pub fn validate_registry(registry: &WorkflowRegistry) -> ValidationReport {
    let mut report = ValidationReport::new();

    let mut referenced_reusables: HashSet<PathBuf> = HashSet::new();

    for (path, workflow) in registry.all_workflows() {
        if workflow.jobs.is_empty() {
            report.add_warning(ValidationWarning::EmptyWorkflow {
                workflow: path.clone(),
            });
            continue;
        }

        let job_names: HashSet<&String> = workflow.jobs.keys().collect();

        for (job_name, job) in &workflow.jobs {
            validate_job_dependencies(path, job_name, &job.needs, &job_names, &mut report);

            if let Some(ref uses) = job.uses {
                validate_job_uses(path, job_name, uses, registry, &mut report, &mut referenced_reusables);
            } else if job.steps.is_empty() {
                report.add_warning(ValidationWarning::JobWithNoSteps {
                    workflow: path.clone(),
                    job: job_name.clone(),
                });
            }

            validate_step_ids(path, job_name, &job.steps, &mut report);

            validate_job_outputs(path, job_name, &job.outputs, &job.steps, &mut report);
        }

        validate_circular_dependencies(path, workflow, &mut report);
    }

    for (path, _workflow) in registry.reusable_workflows() {
        if !referenced_reusables.contains(path) {
            report.add_warning(ValidationWarning::UnusedReusableWorkflow {
                workflow: path.clone(),
            });
        }
    }

    report
}

fn validate_job_dependencies(
    workflow_path: &PathBuf,
    job_name: &str,
    needs: &JobNeeds,
    all_jobs: &HashSet<&String>,
    report: &mut ValidationReport,
) {
    for dep in needs.as_vec() {
        if !all_jobs.contains(&dep) {
            report.add_error(ValidationError::JobDependencyNotFound {
                workflow: workflow_path.clone(),
                job: job_name.to_string(),
                dependency: dep,
            });
        }
    }
}

fn validate_job_uses(
    workflow_path: &PathBuf,
    job_name: &str,
    uses: &str,
    registry: &WorkflowRegistry,
    report: &mut ValidationReport,
    referenced_reusables: &mut HashSet<PathBuf>,
) {
    if is_file_ref(uses) {
        match parse_file_ref(uses) {
            Ok(file_path) => {
                if registry.get_by_str(file_path).is_none() {
                    report.add_error(ValidationError::FileReferenceNotFound {
                        workflow: workflow_path.clone(),
                        job: job_name.to_string(),
                        file_ref: file_path.to_string(),
                    });
                } else {
                    referenced_reusables.insert(PathBuf::from(file_path));

                    if let Some(reusable) = registry.get_by_str(file_path) {
                        let has_outputs = reusable
                            .on
                            .as_ref()
                            .and_then(|t| t.workflow_call.as_ref())
                            .map(|wc| !wc.outputs.is_empty())
                            .unwrap_or(false);

                        if !has_outputs && !reusable.is_reusable() {
                        }
                    }
                }
            }
            Err(_) => {
                report.add_error(ValidationError::InvalidFileReference {
                    workflow: workflow_path.clone(),
                    job: job_name.to_string(),
                    uses: uses.to_string(),
                });
            }
        }
    }
}

fn validate_step_ids(
    workflow_path: &PathBuf,
    job_name: &str,
    steps: &[crate::parser::Step],
    report: &mut ValidationReport,
) {
    let mut seen_ids: HashSet<String> = HashSet::new();

    for step in steps.iter() {
        if let Some(ref id) = step.id {
            if seen_ids.contains(id) {
                report.add_error(ValidationError::DuplicateStepId {
                    workflow: workflow_path.clone(),
                    job: job_name.to_string(),
                    step_id: id.clone(),
                });
            } else {
                seen_ids.insert(id.clone());
            }
        }
    }
}

fn validate_job_outputs(
    workflow_path: &PathBuf,
    job_name: &str,
    outputs: &std::collections::HashMap<String, String>,
    steps: &[crate::parser::Step],
    report: &mut ValidationReport,
) {
    let step_ids: HashSet<String> = steps.iter().filter_map(|s| s.id.clone()).collect();

    for (output_name, expression) in outputs {
        if let Some(step_ref) = extract_step_reference(expression) {
            if !step_ids.contains(&step_ref) {
                report.add_error(ValidationError::InvalidOutputExpression {
                    workflow: workflow_path.clone(),
                    job: job_name.to_string(),
                    output_name: output_name.clone(),
                    expression: expression.clone(),
                    reason: format!("references non-existent step id '{}'", step_ref),
                });
            }
        }
    }
}

fn extract_step_reference(expression: &str) -> Option<String> {
    let trimmed = expression.trim();
    if !trimmed.starts_with("${{") || !trimmed.ends_with("}}") {
        return None;
    }

    let inner = trimmed[3..trimmed.len() - 2].trim();

    if inner.starts_with("steps.") {
        let rest = &inner[6..];
        if let Some(dot_pos) = rest.find('.') {
            return Some(rest[..dot_pos].to_string());
        }
    }

    None
}

fn validate_circular_dependencies(
    workflow_path: &PathBuf,
    workflow: &crate::parser::Workflow,
    report: &mut ValidationReport,
) {
    use std::collections::HashMap;

    let mut in_degree: HashMap<&String, usize> = HashMap::new();
    let mut dependents: HashMap<&String, Vec<&String>> = HashMap::new();

    for job_name in workflow.jobs.keys() {
        in_degree.insert(job_name, 0);
        dependents.insert(job_name, Vec::new());
    }

    for (job_name, job) in &workflow.jobs {
        for dep in job.needs.as_vec() {
            if let Some(deg) = in_degree.get_mut(&job_name) {
                *deg += 1;
            }
            if let Some(dep_key) = workflow.jobs.keys().find(|k| **k == dep) {
                if let Some(deps) = dependents.get_mut(dep_key) {
                    deps.push(job_name);
                }
            }
        }
    }

    let mut queue: Vec<&String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();

    let mut processed = 0;

    while let Some(job) = queue.pop() {
        processed += 1;
        if let Some(deps) = dependents.get(job) {
            for dependent in deps {
                if let Some(deg) = in_degree.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(dependent);
                    }
                }
            }
        }
    }

    if processed < workflow.jobs.len() {
        let cycle_jobs: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(&name, _)| name.clone())
            .collect();

        report.add_error(ValidationError::CircularJobDependency {
            workflow: workflow_path.clone(),
            chain: cycle_jobs,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_registry(workflows: Vec<(&str, &str)>) -> WorkflowRegistry {
        use std::fs;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        for (name, content) in workflows {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
        }

        WorkflowRegistry::build(dir.path()).unwrap()
    }

    #[test]
    fn test_validate_missing_job_dependency() {
        let yaml = r#"
name: Test
jobs:
  job1:
    needs: [nonexistent]
    steps:
      - uses: test/step
"#;
        let registry = create_test_registry(vec![("test.yaml", yaml)]);
        let report = validate_registry(&registry);

        assert!(!report.is_valid());
        assert!(report.errors.iter().any(|e| matches!(
            e,
            ValidationError::JobDependencyNotFound { dependency, .. } if dependency == "nonexistent"
        )));
    }

    #[test]
    fn test_validate_circular_dependency() {
        let yaml = r#"
name: Test
jobs:
  job1:
    needs: [job2]
    steps:
      - uses: test/step
  job2:
    needs: [job1]
    steps:
      - uses: test/step
"#;
        let registry = create_test_registry(vec![("test.yaml", yaml)]);
        let report = validate_registry(&registry);

        assert!(!report.is_valid());
        assert!(report.errors.iter().any(|e| matches!(e, ValidationError::CircularJobDependency { .. })));
    }

    #[test]
    fn test_validate_duplicate_step_id() {
        let yaml = r#"
name: Test
jobs:
  job1:
    steps:
      - uses: test/step1
        id: same_id
      - uses: test/step2
        id: same_id
"#;
        let registry = create_test_registry(vec![("test.yaml", yaml)]);
        let report = validate_registry(&registry);

        assert!(!report.is_valid());
        assert!(matches!(
            &report.errors[0],
            ValidationError::DuplicateStepId { step_id, .. } if step_id == "same_id"
        ));
    }

    #[test]
    fn test_validate_invalid_output_reference() {
        let yaml = r#"
name: Test
jobs:
  job1:
    outputs:
      result: ${{ steps.nonexistent.outputs.value }}
    steps:
      - uses: test/step
        id: real_step
"#;
        let registry = create_test_registry(vec![("test.yaml", yaml)]);
        let report = validate_registry(&registry);

        assert!(!report.is_valid());
        assert!(matches!(
            &report.errors[0],
            ValidationError::InvalidOutputExpression { reason, .. } if reason.contains("nonexistent")
        ));
    }

    #[test]
    fn test_validate_missing_file_reference() {
        let yaml = r#"
name: Test
jobs:
  job1:
    uses: "@file:nonexistent.yaml"
"#;
        let registry = create_test_registry(vec![("test.yaml", yaml)]);
        let report = validate_registry(&registry);

        assert!(!report.is_valid());
        assert!(matches!(
            &report.errors[0],
            ValidationError::FileReferenceNotFound { file_ref, .. } if file_ref == "nonexistent.yaml"
        ));
    }

    #[test]
    fn test_validate_valid_workflow() {
        let reusable = r#"
name: Setup
on:
  workflow_call:
    outputs:
      user_id:
        value: ${{ jobs.setup.outputs.user_id }}

jobs:
  setup:
    outputs:
      user_id: ${{ steps.create.outputs.id }}
    steps:
      - uses: user/create
        id: create
"#;

        let main = r#"
name: Main
jobs:
  setup:
    uses: "@file:setup.yaml"
  test:
    needs: [setup]
    steps:
      - uses: test/run
        id: run
"#;

        let registry = create_test_registry(vec![
            ("setup.yaml", reusable),
            ("main.yaml", main),
        ]);
        let report = validate_registry(&registry);

        assert!(report.is_valid(), "Errors: {:?}", report.errors);
    }

    #[test]
    fn test_extract_step_reference() {
        assert_eq!(
            extract_step_reference("${{ steps.alice.outputs.id }}"),
            Some("alice".to_string())
        );
        assert_eq!(
            extract_step_reference("${{ steps.my_step.outputs.value }}"),
            Some("my_step".to_string())
        );
        assert_eq!(extract_step_reference("${{ jobs.job1.outputs.x }}"), None);
        assert_eq!(extract_step_reference("plain string"), None);
    }
}
