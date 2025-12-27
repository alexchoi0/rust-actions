use crate::outputs::StepOutputs;
use crate::{Error, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

pub struct ExprContext {
    pub env: HashMap<String, String>,
    pub steps: HashMap<String, StepOutputs>,
    pub background: HashMap<String, StepOutputs>,
    pub containers: HashMap<String, ContainerInfo>,
    pub outputs: Option<StepOutputs>,
    pub needs: HashMap<String, JobOutputs>,
    pub matrix: HashMap<String, Value>,
    pub jobs: HashMap<String, JobOutputs>,
}

#[derive(Debug, Clone, Default)]
pub struct JobOutputs {
    pub outputs: HashMap<String, Value>,
}

impl JobOutputs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.outputs.get(key)
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.outputs.get(key).and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => Some(v.to_string()),
        })
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.outputs.insert(key.into(), value);
    }

    pub fn to_value(&self) -> Value {
        Value::Object(
            self.outputs
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub url: String,
    pub host: String,
    pub port: u16,
}

impl ExprContext {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            steps: HashMap::new(),
            background: HashMap::new(),
            containers: HashMap::new(),
            outputs: None,
            needs: HashMap::new(),
            matrix: HashMap::new(),
            jobs: HashMap::new(),
        }
    }

    pub fn with_outputs(&self, outputs: StepOutputs) -> Self {
        Self {
            env: self.env.clone(),
            steps: self.steps.clone(),
            background: self.background.clone(),
            containers: self.containers.clone(),
            outputs: Some(outputs),
            needs: self.needs.clone(),
            matrix: self.matrix.clone(),
            jobs: self.jobs.clone(),
        }
    }

    pub fn with_matrix(&self, matrix: HashMap<String, Value>) -> Self {
        Self {
            env: self.env.clone(),
            steps: self.steps.clone(),
            background: self.background.clone(),
            containers: self.containers.clone(),
            outputs: self.outputs.clone(),
            needs: self.needs.clone(),
            matrix,
            jobs: self.jobs.clone(),
        }
    }
}

impl Default for ExprContext {
    fn default() -> Self {
        Self::new()
    }
}

pub fn evaluate(input: &str, ctx: &ExprContext) -> Result<String> {
    let re = Regex::new(r"\$\{\{\s*(.+?)\s*\}\}").unwrap();

    let mut result = input.to_string();
    for cap in re.captures_iter(input) {
        let full_match = &cap[0];
        let expr = &cap[1];
        let value = evaluate_expr(expr, ctx)?;
        result = result.replace(full_match, &value);
    }

    Ok(result)
}

pub fn evaluate_value(value: &Value, ctx: &ExprContext) -> Result<Value> {
    match value {
        Value::String(s) => {
            let evaluated = evaluate(s, ctx)?;
            Ok(Value::String(evaluated))
        }
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), evaluate_value(v, ctx)?);
            }
            Ok(Value::Object(new_map))
        }
        Value::Array(arr) => {
            let new_arr: Result<Vec<_>> = arr.iter().map(|v| evaluate_value(v, ctx)).collect();
            Ok(Value::Array(new_arr?))
        }
        _ => Ok(value.clone()),
    }
}

pub fn evaluate_assertion(assertion: &str, ctx: &ExprContext) -> Result<bool> {
    let re = Regex::new(r"\$\{\{\s*(.+?)\s*\}\}").unwrap();

    if let Some(cap) = re.captures(assertion) {
        let expr = &cap[1];
        evaluate_bool_expr(expr, ctx)
    } else {
        Err(Error::Expression(format!(
            "Invalid assertion format: {}",
            assertion
        )))
    }
}

fn evaluate_bool_expr(expr: &str, ctx: &ExprContext) -> Result<bool> {
    let ops = [" contains ", "==", "!=", ">=", "<=", ">", "<"];

    for op in ops {
        if let Some(pos) = find_operator(expr, op) {
            let left = expr[..pos].trim();
            let right = expr[pos + op.len()..].trim();

            let left_val = evaluate_operand(left, ctx)?;
            let right_val = evaluate_operand(right, ctx)?;

            return Ok(compare_values(&left_val, &right_val, op.trim()));
        }
    }

    Err(Error::Expression(format!(
        "No comparison operator found in expression: {}",
        expr
    )))
}

fn find_operator(expr: &str, op: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    let chars: Vec<char> = expr.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if in_string {
            if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
            continue;
        }

        if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
            continue;
        }

        if c == '{' || c == '[' {
            depth += 1;
        } else if c == '}' || c == ']' {
            depth -= 1;
        }

        if depth == 0 && i + op.len() <= expr.len() {
            if &expr[i..i + op.len()] == op {
                return Some(i);
            }
        }
    }
    None
}

fn evaluate_operand(operand: &str, ctx: &ExprContext) -> Result<Value> {
    let operand = operand.trim();

    if operand.starts_with('{') || operand.starts_with('[') {
        serde_json::from_str(operand)
            .map_err(|e| Error::Expression(format!("Invalid JSON: {}", e)))
    } else if operand.starts_with('"') {
        Ok(Value::String(operand[1..operand.len() - 1].to_string()))
    } else if operand.starts_with('\'') {
        Ok(Value::String(operand[1..operand.len() - 1].to_string()))
    } else if operand == "true" {
        Ok(Value::Bool(true))
    } else if operand == "false" {
        Ok(Value::Bool(false))
    } else if operand == "null" {
        Ok(Value::Null)
    } else if let Ok(num) = operand.parse::<i64>() {
        Ok(Value::Number(num.into()))
    } else if let Ok(num) = operand.parse::<f64>() {
        Ok(serde_json::Number::from_f64(num)
            .map(Value::Number)
            .unwrap_or(Value::Null))
    } else {
        evaluate_expr_value(operand, ctx)
    }
}

fn evaluate_expr_value(expr: &str, ctx: &ExprContext) -> Result<Value> {
    let parts: Vec<&str> = expr.split('.').collect();

    match parts.as_slice() {
        ["outputs"] => ctx
            .outputs
            .as_ref()
            .map(|o| o.to_value())
            .ok_or_else(|| Error::Expression("No outputs context available".to_string())),

        ["outputs", field] => ctx
            .outputs
            .as_ref()
            .and_then(|o| o.get(field).cloned())
            .ok_or_else(|| Error::Expression(format!("Output not found: {}", field))),

        ["outputs", rest @ ..] => {
            let field = rest[0];
            let remaining: Vec<&str> = rest[1..].to_vec();
            let base = ctx
                .outputs
                .as_ref()
                .and_then(|o| o.get(field).cloned())
                .ok_or_else(|| Error::Expression(format!("Output not found: {}", field)))?;
            navigate_value(&base, &remaining)
        }

        ["env", var_name] => ctx
            .env
            .get(*var_name)
            .map(|s| Value::String(s.clone()))
            .ok_or_else(|| Error::EnvVar((*var_name).to_string())),

        ["steps", step_id, "outputs"] => ctx
            .steps
            .get(*step_id)
            .map(|o| o.to_value())
            .ok_or_else(|| Error::Expression(format!("Step not found: {}", step_id))),

        ["steps", step_id, "outputs", field] => ctx
            .steps
            .get(*step_id)
            .and_then(|o| o.get(field).cloned())
            .ok_or_else(|| {
                Error::Expression(format!("Step output not found: {}.{}", step_id, field))
            }),

        ["containers", name, prop] => {
            let container = ctx
                .containers
                .get(*name)
                .ok_or_else(|| Error::Expression(format!("Container not found: {}", name)))?;
            match *prop {
                "url" => Ok(Value::String(container.url.clone())),
                "host" => Ok(Value::String(container.host.clone())),
                "port" => Ok(Value::Number(container.port.into())),
                _ => Err(Error::Expression(format!(
                    "Unknown container property: {}",
                    prop
                ))),
            }
        }

        // needs.job_name.outputs.field
        ["needs", job_name, "outputs"] => ctx
            .needs
            .get(*job_name)
            .map(|o| o.to_value())
            .ok_or_else(|| Error::Expression(format!("Job not found in needs: {}", job_name))),

        ["needs", job_name, "outputs", field] => ctx
            .needs
            .get(*job_name)
            .and_then(|o| o.get(field).cloned())
            .ok_or_else(|| {
                Error::Expression(format!("Job output not found: {}.{}", job_name, field))
            }),

        ["needs", job_name, "outputs", field, rest @ ..] => {
            let base = ctx
                .needs
                .get(*job_name)
                .and_then(|o| o.get(field).cloned())
                .ok_or_else(|| {
                    Error::Expression(format!("Job output not found: {}.{}", job_name, field))
                })?;
            navigate_value(&base, &rest.to_vec())
        }

        // matrix.key
        ["matrix", key] => ctx
            .matrix
            .get(*key)
            .cloned()
            .ok_or_else(|| Error::Expression(format!("Matrix key not found: {}", key))),

        // jobs.job_name.outputs.field (for workflow-level references)
        ["jobs", job_name, "outputs"] => ctx
            .jobs
            .get(*job_name)
            .map(|o| o.to_value())
            .ok_or_else(|| Error::Expression(format!("Job not found: {}", job_name))),

        ["jobs", job_name, "outputs", field] => ctx
            .jobs
            .get(*job_name)
            .and_then(|o| o.get(field).cloned())
            .ok_or_else(|| {
                Error::Expression(format!("Job output not found: {}.{}", job_name, field))
            }),

        _ => Err(Error::Expression(format!("Unknown expression: {}", expr))),
    }
}

fn navigate_value(value: &Value, path: &[&str]) -> Result<Value> {
    if path.is_empty() {
        return Ok(value.clone());
    }

    match value {
        Value::Object(map) => {
            let field = path[0];
            let next = map
                .get(field)
                .ok_or_else(|| Error::Expression(format!("Field not found: {}", field)))?;
            navigate_value(next, &path[1..])
        }
        Value::Array(arr) => {
            let index: usize = path[0]
                .parse()
                .map_err(|_| Error::Expression(format!("Invalid array index: {}", path[0])))?;
            let next = arr
                .get(index)
                .ok_or_else(|| Error::Expression(format!("Array index out of bounds: {}", index)))?;
            navigate_value(next, &path[1..])
        }
        _ => Err(Error::Expression(format!(
            "Cannot navigate into non-object/array value"
        ))),
    }
}

fn compare_values(left: &Value, right: &Value, op: &str) -> bool {
    match op {
        "==" => left == right,
        "!=" => left != right,
        "contains" => value_contains(left, right),
        ">" => compare_numeric(left, right, |a, b| a > b),
        "<" => compare_numeric(left, right, |a, b| a < b),
        ">=" => compare_numeric(left, right, |a, b| a >= b),
        "<=" => compare_numeric(left, right, |a, b| a <= b),
        _ => false,
    }
}

fn compare_numeric<F>(left: &Value, right: &Value, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    match (value_to_f64(left), value_to_f64(right)) {
        (Some(l), Some(r)) => cmp(l, r),
        _ => false,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn value_contains(haystack: &Value, needle: &Value) -> bool {
    match (haystack, needle) {
        (Value::Object(h), Value::Object(n)) => n.iter().all(|(k, v)| {
            h.get(k).map_or(false, |hv| {
                if v.is_object() || v.is_array() {
                    value_contains(hv, v)
                } else {
                    hv == v
                }
            })
        }),

        (Value::Array(h), Value::Array(n)) => n.iter().all(|needle_item| {
            h.iter().any(|hay_item| {
                if needle_item.is_object() {
                    value_contains(hay_item, needle_item)
                } else {
                    hay_item == needle_item
                }
            })
        }),

        (Value::Array(h), needle) => h.iter().any(|item| {
            if needle.is_object() {
                value_contains(item, needle)
            } else {
                item == needle
            }
        }),

        (Value::String(h), Value::String(n)) => h.contains(n.as_str()),

        _ => false,
    }
}

fn evaluate_expr(expr: &str, ctx: &ExprContext) -> Result<String> {
    let parts: Vec<&str> = expr.split('.').collect();

    match parts.as_slice() {
        ["env", var_name] => ctx
            .env
            .get(*var_name)
            .cloned()
            .ok_or_else(|| Error::EnvVar((*var_name).to_string())),

        ["steps", step_id, "outputs", field] => ctx
            .steps
            .get(*step_id)
            .and_then(|outputs| outputs.get_string(field))
            .ok_or_else(|| {
                Error::Expression(format!("Step output not found: {}.{}", step_id, field))
            }),

        ["background", step_id, "outputs", field] => ctx
            .background
            .get(*step_id)
            .and_then(|outputs| outputs.get_string(field))
            .ok_or_else(|| {
                Error::Expression(format!(
                    "Background output not found: {}.{}",
                    step_id, field
                ))
            }),

        ["containers", name, "url"] => ctx
            .containers
            .get(*name)
            .map(|c| c.url.clone())
            .ok_or_else(|| Error::Expression(format!("Container not found: {}", name))),

        ["containers", name, "host"] => ctx
            .containers
            .get(*name)
            .map(|c| c.host.clone())
            .ok_or_else(|| Error::Expression(format!("Container not found: {}", name))),

        ["containers", name, "port"] => ctx
            .containers
            .get(*name)
            .map(|c| c.port.to_string())
            .ok_or_else(|| Error::Expression(format!("Container not found: {}", name))),

        // needs.job_name.outputs.field
        ["needs", job_name, "outputs", field] => ctx
            .needs
            .get(*job_name)
            .and_then(|outputs| outputs.get_string(field))
            .ok_or_else(|| {
                Error::Expression(format!("Job output not found: {}.{}", job_name, field))
            }),

        // matrix.key
        ["matrix", key] => ctx
            .matrix
            .get(*key)
            .map(|v| value_to_string(v))
            .ok_or_else(|| Error::Expression(format!("Matrix key not found: {}", key))),

        // jobs.job_name.outputs.field
        ["jobs", job_name, "outputs", field] => ctx
            .jobs
            .get(*job_name)
            .and_then(|outputs| outputs.get_string(field))
            .ok_or_else(|| {
                Error::Expression(format!("Job output not found: {}.{}", job_name, field))
            }),

        _ => Err(Error::Expression(format!("Unknown expression: {}", expr))),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_env() {
        let mut ctx = ExprContext::new();
        ctx.env.insert("DB_URL".to_string(), "postgres://localhost".to_string());

        let result = evaluate("${{ env.DB_URL }}", &ctx).unwrap();
        assert_eq!(result, "postgres://localhost");
    }

    #[test]
    fn test_evaluate_step_output() {
        let mut ctx = ExprContext::new();
        let mut outputs = StepOutputs::new();
        outputs.insert("id", "user-123");
        ctx.steps.insert("user".to_string(), outputs);

        let result = evaluate("User ID: ${{ steps.user.outputs.id }}", &ctx).unwrap();
        assert_eq!(result, "User ID: user-123");
    }

    #[test]
    fn test_evaluate_container() {
        let mut ctx = ExprContext::new();
        ctx.containers.insert(
            "postgres".to_string(),
            ContainerInfo {
                url: "postgres://localhost:5432".to_string(),
                host: "localhost".to_string(),
                port: 5432,
            },
        );

        let result = evaluate("${{ containers.postgres.url }}", &ctx).unwrap();
        assert_eq!(result, "postgres://localhost:5432");
    }
}
