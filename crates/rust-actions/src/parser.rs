use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Feature {
    pub name: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub containers: HashMap<String, String>,
    #[serde(default)]
    pub background: Vec<Step>,
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Step {
    #[serde(default)]
    pub name: String,
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

impl Feature {
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let feature: Feature = serde_yaml::from_str(yaml)?;
        Ok(feature)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }
}

pub fn parse_features(path: impl AsRef<Path>) -> Result<Vec<Feature>> {
    let path = path.as_ref();
    let mut features = Vec::new();

    if path.is_file() {
        features.push(Feature::from_file(path)?);
    } else if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());
                if matches!(ext, Some("yaml") | Some("yml")) {
                    features.push(Feature::from_file(&path)?);
                }
            }
        }
    }

    Ok(features)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_feature() {
        let yaml = r#"
name: User Management

env:
  DB_URL: postgres://localhost/test

containers:
  postgres: postgres:15

scenarios:
  - name: Create user
    steps:
      - name: Create user
        id: user
        uses: user/create
        with:
          username: alice
          email: alice@test.com

      - name: Verify
        uses: assert/not_empty
        with:
          value: ${{ steps.user.outputs.id }}
"#;

        let feature = Feature::from_yaml(yaml).unwrap();
        assert_eq!(feature.name, "User Management");
        assert_eq!(feature.scenarios.len(), 1);
        assert_eq!(feature.scenarios[0].steps.len(), 2);
        assert_eq!(feature.scenarios[0].steps[0].uses, "user/create");
    }
}
