use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workflow {
    pub name: String,
    #[serde(default)]
    pub on: Option<WorkflowTrigger>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub jobs: HashMap<String, Job>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowTrigger {
    #[serde(default)]
    pub workflow_call: Option<WorkflowCallConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowCallConfig {
    #[serde(default)]
    pub inputs: HashMap<String, InputDef>,
    #[serde(default)]
    pub outputs: HashMap<String, OutputDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputDef {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(rename = "type", default)]
    pub input_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputDef {
    #[serde(default)]
    pub description: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Job {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub needs: JobNeeds,
    #[serde(default)]
    pub uses: Option<String>,
    #[serde(default)]
    pub with: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub strategy: Option<Strategy>,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(untagged)]
pub enum JobNeeds {
    #[default]
    None,
    Single(String),
    Multiple(Vec<String>),
}

impl JobNeeds {
    pub fn as_vec(&self) -> Vec<String> {
        match self {
            JobNeeds::None => vec![],
            JobNeeds::Single(s) => vec![s.clone()],
            JobNeeds::Multiple(v) => v.clone(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            JobNeeds::None => true,
            JobNeeds::Single(_) => false,
            JobNeeds::Multiple(v) => v.is_empty(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Strategy {
    #[serde(default)]
    pub matrix: Matrix,
    #[serde(default = "default_true", rename = "fail-fast")]
    pub fail_fast: bool,
    #[serde(default, rename = "max-parallel")]
    pub max_parallel: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Matrix {
    #[serde(default)]
    pub include: Vec<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub exclude: Vec<HashMap<String, serde_json::Value>>,
    #[serde(flatten)]
    pub dimensions: HashMap<String, Vec<serde_json::Value>>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Step {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    pub uses: String,
    #[serde(default)]
    pub with: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "continue-on-error")]
    pub continue_on_error: bool,
    #[serde(default, rename = "pre-assert")]
    pub pre_assert: Vec<String>,
    #[serde(default, rename = "post-assert")]
    pub post_assert: Vec<String>,
}

impl Workflow {
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let workflow: Workflow = serde_yaml::from_str(yaml)?;
        Ok(workflow)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }

    pub fn is_reusable(&self) -> bool {
        self.on
            .as_ref()
            .map(|t| t.workflow_call.is_some())
            .unwrap_or(false)
    }
}

pub fn parse_workflows(path: impl AsRef<Path>) -> Result<Vec<(PathBuf, Workflow)>> {
    let path = path.as_ref();
    let mut workflows = Vec::new();

    if path.is_file() {
        workflows.push((path.to_path_buf(), Workflow::from_file(path)?));
    } else if path.is_dir() {
        parse_workflows_recursive(path, path, &mut workflows)?;
    }

    Ok(workflows)
}

fn parse_workflows_recursive(
    base_path: &Path,
    current_path: &Path,
    workflows: &mut Vec<(PathBuf, Workflow)>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            parse_workflows_recursive(base_path, &path, workflows)?;
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str());
            if matches!(ext, Some("yaml") | Some("yml")) {
                let rel_path = path
                    .strip_prefix(base_path)
                    .unwrap_or(&path)
                    .to_path_buf();
                workflows.push((rel_path, Workflow::from_file(&path)?));
            }
        }
    }
    Ok(())
}

pub fn parse_workflow_file(path: impl AsRef<Path>) -> Result<(PathBuf, Workflow)> {
    let path = path.as_ref();
    Ok((path.to_path_buf(), Workflow::from_file(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_reusable_workflow() {
        let yaml = r#"
name: User Setup
on:
  workflow_call:
    outputs:
      user_id:
        value: ${{ jobs.setup.outputs.user_id }}
      session_token:
        value: ${{ jobs.setup.outputs.session_token }}

jobs:
  setup:
    outputs:
      user_id: ${{ steps.user.outputs.id }}
      session_token: ${{ steps.session.outputs.token }}
    steps:
      - uses: user/create
        id: user
      - uses: auth/login
        id: session
        with:
          user_id: ${{ steps.user.outputs.id }}
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "User Setup");
        assert!(workflow.is_reusable());
        assert!(workflow.jobs.contains_key("setup"));

        let setup_job = &workflow.jobs["setup"];
        assert_eq!(setup_job.steps.len(), 2);
        assert_eq!(setup_job.outputs.len(), 2);
    }

    #[test]
    fn test_parse_runnable_workflow() {
        let yaml = r#"
name: Order Tests

jobs:
  setup:
    uses: "@file:setup/user-setup.yaml"

  place-order:
    needs: [setup]
    steps:
      - uses: order/create
        with:
          token: ${{ needs.setup.outputs.session_token }}
        post-assert:
          - ${{ outputs.order_id != "" }}
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "Order Tests");
        assert!(!workflow.is_reusable());

        let setup_job = &workflow.jobs["setup"];
        assert_eq!(
            setup_job.uses.as_deref(),
            Some("@file:setup/user-setup.yaml")
        );

        let order_job = &workflow.jobs["place-order"];
        assert_eq!(order_job.needs.as_vec(), vec!["setup"]);
        assert_eq!(order_job.steps.len(), 1);
    }

    #[test]
    fn test_parse_matrix_workflow() {
        let yaml = r#"
name: Feature Flag Compatibility

jobs:
  test-flags:
    strategy:
      matrix:
        service_a_feature_x: [true, false]
        service_b_feature_y: [true, false]
      fail-fast: false
    steps:
      - uses: service-a/configure
        with:
          feature_x: ${{ matrix.service_a_feature_x }}
      - uses: service-b/configure
        with:
          feature_y: ${{ matrix.service_b_feature_y }}
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "Feature Flag Compatibility");

        let job = &workflow.jobs["test-flags"];
        let strategy = job.strategy.as_ref().unwrap();
        assert!(!strategy.fail_fast);
        assert_eq!(strategy.matrix.dimensions.len(), 2);
        assert_eq!(strategy.matrix.dimensions["service_a_feature_x"].len(), 2);
    }

    #[test]
    fn test_parse_matrix_with_include_exclude() {
        let yaml = r#"
name: Matrix Test

jobs:
  test:
    strategy:
      matrix:
        service_a: [v1, v2]
        service_b: [v1, v2]
        include:
          - service_a: v3-beta
            service_b: v2
            experimental: true
        exclude:
          - service_a: v1
            service_b: v2
    steps:
      - uses: test/run
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        let job = &workflow.jobs["test"];
        let strategy = job.strategy.as_ref().unwrap();

        assert_eq!(strategy.matrix.include.len(), 1);
        assert_eq!(strategy.matrix.exclude.len(), 1);
        assert_eq!(
            strategy.matrix.include[0]["experimental"],
            serde_json::Value::Bool(true)
        );
    }
}
