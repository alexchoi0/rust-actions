use crate::parser::{Matrix, Strategy};
use serde_json::Value;
use std::collections::HashMap;

pub type MatrixCombination = HashMap<String, Value>;

pub fn expand_matrix(strategy: &Strategy) -> Vec<MatrixCombination> {
    expand_matrix_inner(&strategy.matrix)
}

pub fn expand_matrix_inner(matrix: &Matrix) -> Vec<MatrixCombination> {
    if matrix.dimensions.is_empty() && matrix.include.is_empty() {
        return vec![HashMap::new()];
    }

    let mut combinations = cartesian_product(&matrix.dimensions);

    combinations.retain(|combo| !matches_any_exclude(combo, &matrix.exclude));

    for include in &matrix.include {
        let mut new_combo = HashMap::new();
        for (key, value) in include {
            new_combo.insert(key.clone(), value.clone());
        }
        combinations.push(new_combo);
    }

    if combinations.is_empty() {
        vec![HashMap::new()]
    } else {
        combinations
    }
}

fn cartesian_product(matrix: &HashMap<String, Vec<Value>>) -> Vec<MatrixCombination> {
    if matrix.is_empty() {
        return vec![];
    }

    let keys: Vec<&String> = matrix.keys().collect();
    let mut result = vec![HashMap::new()];

    for key in keys {
        let values = &matrix[key];
        let mut new_result = Vec::new();

        for combo in &result {
            for value in values {
                let mut new_combo = combo.clone();
                new_combo.insert(key.clone(), value.clone());
                new_result.push(new_combo);
            }
        }

        result = new_result;
    }

    result
}

fn matches_any_exclude(combo: &MatrixCombination, excludes: &[HashMap<String, Value>]) -> bool {
    excludes.iter().any(|exclude| matches_exclude(combo, exclude))
}

fn matches_exclude(combo: &MatrixCombination, exclude: &HashMap<String, Value>) -> bool {
    exclude.iter().all(|(key, value)| {
        combo
            .get(key)
            .map(|v| values_equal(v, value))
            .unwrap_or(false)
    })
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Null, Value::Null) => true,
        _ => a == b,
    }
}

pub fn format_matrix_suffix(combo: &MatrixCombination) -> String {
    if combo.is_empty() {
        return String::new();
    }

    let mut parts: Vec<String> = combo
        .iter()
        .map(|(k, v)| format!("{}={}", k, format_value(v)))
        .collect();
    parts.sort();

    format!(" [{}]", parts.join(", "))
}

fn format_value(value: &Value) -> String {
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
    use serde_json::json;

    #[test]
    fn test_empty_matrix() {
        let matrix = Matrix {
            dimensions: HashMap::new(),
            include: vec![],
            exclude: vec![],
        };

        let combos = expand_matrix_inner(&matrix);
        assert_eq!(combos.len(), 1);
        assert!(combos[0].is_empty());
    }

    #[test]
    fn test_single_dimension_matrix() {
        let mut dimensions = HashMap::new();
        dimensions.insert("version".to_string(), vec![json!("v1"), json!("v2")]);

        let matrix = Matrix {
            dimensions,
            include: vec![],
            exclude: vec![],
        };

        let combos = expand_matrix_inner(&matrix);
        assert_eq!(combos.len(), 2);
    }

    #[test]
    fn test_cartesian_product() {
        let mut dimensions = HashMap::new();
        dimensions.insert("a".to_string(), vec![json!(true), json!(false)]);
        dimensions.insert("b".to_string(), vec![json!(true), json!(false)]);

        let matrix = Matrix {
            dimensions,
            include: vec![],
            exclude: vec![],
        };

        let combos = expand_matrix_inner(&matrix);
        assert_eq!(combos.len(), 4);
    }

    #[test]
    fn test_exclude() {
        let mut dimensions = HashMap::new();
        dimensions.insert("a".to_string(), vec![json!("v1"), json!("v2")]);
        dimensions.insert("b".to_string(), vec![json!("v1"), json!("v2")]);

        let mut exclude = HashMap::new();
        exclude.insert("a".to_string(), json!("v1"));
        exclude.insert("b".to_string(), json!("v2"));

        let matrix = Matrix {
            dimensions,
            include: vec![],
            exclude: vec![exclude],
        };

        let combos = expand_matrix_inner(&matrix);
        assert_eq!(combos.len(), 3);

        let excluded_combo: MatrixCombination =
            [("a".to_string(), json!("v1")), ("b".to_string(), json!("v2"))]
                .into_iter()
                .collect();

        assert!(!combos.contains(&excluded_combo));
    }

    #[test]
    fn test_include() {
        let mut dimensions = HashMap::new();
        dimensions.insert("a".to_string(), vec![json!("v1")]);

        let mut include = HashMap::new();
        include.insert("a".to_string(), json!("v3-beta"));
        include.insert("experimental".to_string(), json!(true));

        let matrix = Matrix {
            dimensions,
            include: vec![include],
            exclude: vec![],
        };

        let combos = expand_matrix_inner(&matrix);
        assert_eq!(combos.len(), 2);

        let has_beta = combos
            .iter()
            .any(|c| c.get("a") == Some(&json!("v3-beta")));
        assert!(has_beta);

        let has_experimental = combos.iter().any(|c| c.get("experimental").is_some());
        assert!(has_experimental);
    }

    #[test]
    fn test_format_matrix_suffix() {
        let combo: MatrixCombination = [
            ("feature_x".to_string(), json!(true)),
            ("feature_y".to_string(), json!(false)),
        ]
        .into_iter()
        .collect();

        let suffix = format_matrix_suffix(&combo);
        assert!(suffix.contains("feature_x=true"));
        assert!(suffix.contains("feature_y=false"));
    }

    #[test]
    fn test_format_empty_matrix() {
        let combo: MatrixCombination = HashMap::new();
        let suffix = format_matrix_suffix(&combo);
        assert!(suffix.is_empty());
    }
}
