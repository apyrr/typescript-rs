use std::fmt;

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JsonValueType {
    #[default]
    NotPresent,
    Null,
    String,
    Number,
    Boolean,
    Array,
    Object,
}

impl fmt::Display for JsonValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::String => f.write_str("string"),
            Self::Number => f.write_str("number"),
            Self::Boolean => f.write_str("boolean"),
            Self::Array => f.write_str("array"),
            Self::Object => f.write_str("object"),
            Self::NotPresent => f.write_str("unknown(0)"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JsonValue {
    pub type_: JsonValueType,
    pub value: Value,
}

impl JsonValue {
    pub fn is_present(&self) -> bool {
        self.type_ != JsonValueType::NotPresent
    }

    pub fn is_falsy(&self) -> bool {
        match self.type_ {
            JsonValueType::NotPresent | JsonValueType::Null => true,
            JsonValueType::String => self.value.as_str().is_some_and(str::is_empty),
            JsonValueType::Number => self.value.as_f64() == Some(0.0),
            JsonValueType::Boolean => self.value.as_bool() == Some(false),
            JsonValueType::Array | JsonValueType::Object => false,
        }
    }

    pub fn as_object(&self) -> &Map<String, Value> {
        self.value
            .as_object()
            .unwrap_or_else(|| panic!("expected object, got {}", self.type_))
    }

    pub fn as_array(&self) -> &[Value] {
        self.value
            .as_array()
            .unwrap_or_else(|| panic!("expected array, got {}", self.type_))
    }

    pub fn as_string(&self) -> &str {
        self.value
            .as_str()
            .unwrap_or_else(|| panic!("expected string, got {}", self.type_))
    }

    pub fn from_json_value(value: Value) -> Self {
        let type_ = match &value {
            Value::Null => JsonValueType::Null,
            Value::String(_) => JsonValueType::String,
            Value::Number(_) => JsonValueType::Number,
            Value::Bool(_) => JsonValueType::Boolean,
            Value::Array(_) => JsonValueType::Array,
            Value::Object(_) => JsonValueType::Object,
        };
        Self { type_, value }
    }

    pub fn unmarshal_json(&mut self, data: &[u8]) -> serde_json::Result<()> {
        *self = Self::from_json_value(serde_json::from_slice(data)?);
        Ok(())
    }
}
