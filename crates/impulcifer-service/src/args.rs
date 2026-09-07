//! Positional pywebview arguments: omitted defaults differ from explicit null.
use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Value, json};

pub struct Args(pub Vec<Value>);

pub fn invalid(message: impl Into<String>) -> Value {
    ipc::error(ErrorCode::InvalidRequest, message, json!({}), false)
}

impl Args {
    pub fn count(&self, min: usize, max: usize) -> Result<(), Value> {
        if self.0.len() < min || self.0.len() > max {
            Err(invalid(format!(
                "Expected {min} to {max} positional arguments."
            )))
        } else {
            Ok(())
        }
    }
    pub fn get(&self, index: usize) -> &Value {
        self.0.get(index).unwrap_or(&Value::Null)
    }
    pub fn string(&self, index: usize, name: &str, default: Option<&str>) -> Result<String, Value> {
        match self.0.get(index) {
            None if default.is_some() => Ok(default.unwrap_or_default().to_owned()),
            Some(Value::String(value)) => Ok(value.clone()),
            _ => Err(invalid(format!("{name} must be a string."))),
        }
    }
    pub fn optional_string(&self, index: usize, name: &str) -> Result<Option<String>, Value> {
        match self.get(index) {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(value.clone())),
            _ => Err(invalid(format!("{name} must be a string."))),
        }
    }
    pub fn after_seq(&self) -> Result<u64, Value> {
        match self.0.get(1) {
            None => Ok(0),
            Some(value) => value
                .as_u64()
                .ok_or_else(|| invalid("after_seq must be a non-negative integer.")),
        }
    }
    pub fn job_id(&self) -> Result<String, Value> {
        match self.get(0).as_str() {
            Some(value) if !value.is_empty() => Ok(value.to_owned()),
            _ => Err(invalid("job_id is required.")),
        }
    }
}
