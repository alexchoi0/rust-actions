use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct StepOutputs {
    values: HashMap<String, Value>,
}

impl StepOutputs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_value(value: Value) -> Self {
        match value {
            Value::Object(map) => Self {
                values: map.into_iter().collect(),
            },
            _ => Self::default(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.values.get(key).and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => Some(v.to_string()),
        })
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn to_value(&self) -> Value {
        Value::Object(self.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}

pub trait IntoOutputs {
    fn into_outputs(self) -> StepOutputs;
}

impl IntoOutputs for () {
    fn into_outputs(self) -> StepOutputs {
        StepOutputs::new()
    }
}

impl IntoOutputs for StepOutputs {
    fn into_outputs(self) -> StepOutputs {
        self
    }
}
