use crate::parser::{parse_workflows, Workflow};
use crate::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const FILE_REF_PREFIX: &str = "@file:";

pub struct WorkflowRegistry {
    base_path: PathBuf,
    workflows: HashMap<PathBuf, Workflow>,
}

impl WorkflowRegistry {
    pub fn build(workflows_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = workflows_path.as_ref().to_path_buf();
        let parsed = parse_workflows(&base_path)?;
        let workflows: HashMap<PathBuf, Workflow> = parsed.into_iter().collect();

        Ok(Self {
            base_path,
            workflows,
        })
    }

    pub fn get(&self, path: &Path) -> Option<&Workflow> {
        self.workflows.get(path)
    }

    pub fn get_by_str(&self, path: &str) -> Option<&Workflow> {
        self.workflows.get(&PathBuf::from(path))
    }

    pub fn is_reusable(&self, path: &Path) -> bool {
        self.workflows
            .get(path)
            .map(|w| w.is_reusable())
            .unwrap_or(false)
    }

    pub fn resolve_file_ref(&self, uses: &str) -> Result<&Workflow> {
        let path = parse_file_ref(uses)?;
        self.get_by_str(path).ok_or_else(|| {
            Error::WorkflowNotFound {
                path: path.to_string(),
            }
        })
    }

    pub fn runnable_workflows(&self) -> impl Iterator<Item = (&PathBuf, &Workflow)> {
        self.workflows.iter().filter(|(_, w)| !w.is_reusable())
    }

    pub fn reusable_workflows(&self) -> impl Iterator<Item = (&PathBuf, &Workflow)> {
        self.workflows.iter().filter(|(_, w)| w.is_reusable())
    }

    pub fn all_workflows(&self) -> impl Iterator<Item = (&PathBuf, &Workflow)> {
        self.workflows.iter()
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn workflow_count(&self) -> usize {
        self.workflows.len()
    }

    pub fn runnable_count(&self) -> usize {
        self.workflows.values().filter(|w| !w.is_reusable()).count()
    }

    pub fn reusable_count(&self) -> usize {
        self.workflows.values().filter(|w| w.is_reusable()).count()
    }
}

pub fn is_file_ref(uses: &str) -> bool {
    uses.starts_with(FILE_REF_PREFIX)
}

pub fn parse_file_ref(uses: &str) -> Result<&str> {
    if !uses.starts_with(FILE_REF_PREFIX) {
        return Err(Error::InvalidFileRef {
            uses: uses.to_string(),
        });
    }
    Ok(&uses[FILE_REF_PREFIX.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_file_ref() {
        assert!(is_file_ref("@file:setup/user-setup.yaml"));
        assert!(!is_file_ref("user/create"));
        assert!(!is_file_ref("file:something"));
    }

    #[test]
    fn test_parse_file_ref() {
        let path = parse_file_ref("@file:setup/user-setup.yaml").unwrap();
        assert_eq!(path, "setup/user-setup.yaml");
    }

    #[test]
    fn test_parse_file_ref_invalid() {
        let result = parse_file_ref("user/create");
        assert!(result.is_err());
    }
}
