use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Step not found: {0}")]
    StepNotFound(String),

    #[error("Args error: {0}")]
    Args(String),

    #[error("Expression error: {0}")]
    Expression(String),

    #[error("Assertion failed: {0}")]
    Assertion(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Step error: {0}")]
    Step(#[from] StepError),

    #[error("Container error: {0}")]
    Container(String),

    #[error("Environment variable not found: {0}")]
    EnvVar(String),

    #[error("Workflow not found: {path}")]
    WorkflowNotFound { path: String },

    #[error("Job not found: {job} in workflow {workflow}")]
    JobNotFound { workflow: String, job: String },

    #[error("Invalid file reference: {uses}")]
    InvalidFileRef { uses: String },

    #[error("Circular dependency detected: {chain}")]
    CircularDependency { chain: String },

    #[error("Job dependency not found: {job} requires {dependency}")]
    JobDependencyNotFound { job: String, dependency: String },

    #[error("{0}")]
    Custom(String),
}

#[derive(Error, Debug)]
pub enum StepError {
    #[error("Assertion failed: {0}")]
    Assertion(String),

    #[error("{0}")]
    Custom(String),
}

impl StepError {
    pub fn assertion(msg: impl Into<String>) -> Self {
        StepError::Assertion(msg.into())
    }

    pub fn custom(msg: impl Into<String>) -> Self {
        StepError::Custom(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
