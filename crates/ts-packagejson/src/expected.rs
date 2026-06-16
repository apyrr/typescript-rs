use std::any::{TypeId, type_name};

use serde::de::{Deserialize, DeserializeOwned, Deserializer, Error};

pub trait ExpectedJsonValue {
    fn expected_json_type() -> &'static str;
}

fn is_type<T: 'static, U: 'static>() -> bool {
    TypeId::of::<T>() == TypeId::of::<U>()
}

fn is_number_type<T: 'static>() -> bool {
    is_type::<T, i8>()
        || is_type::<T, i16>()
        || is_type::<T, i32>()
        || is_type::<T, i64>()
        || is_type::<T, isize>()
        || is_type::<T, u8>()
        || is_type::<T, u16>()
        || is_type::<T, u32>()
        || is_type::<T, u64>()
        || is_type::<T, usize>()
}

impl<T: 'static> ExpectedJsonValue for T {
    fn expected_json_type() -> &'static str {
        if is_type::<T, String>() {
            return "string";
        }
        if is_type::<T, bool>() {
            return "boolean";
        }
        if is_number_type::<T>() {
            return "number";
        }

        let name = type_name::<T>();
        if name.starts_with("alloc::vec::Vec<") || name.starts_with('[') {
            return "array";
        }
        if name.starts_with("std::collections::hash::map::HashMap<") {
            return "object";
        }
        "unknown"
    }
}

pub type AnyJson = serde_json::Value;

#[derive(Clone)]
pub struct Expected<T> {
    actual_json_type: String,
    pub null: bool,
    pub valid: bool,
    pub value: T,
}

impl<T> Default for Expected<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            actual_json_type: String::new(),
            null: false,
            valid: false,
            value: T::default(),
        }
    }
}

impl<T> Expected<T>
where
    T: Default + DeserializeOwned,
{
    pub fn unmarshal_json(&mut self, data: &[u8]) -> Result<(), String> {
        if data == b"null" {
            *self = Expected {
                null: true,
                actual_json_type: "null".to_owned(),
                valid: false,
                value: T::default(),
            };
            return Ok(());
        }
        if let Ok(value) = serde_json::from_slice(data) {
            self.value = value;
            self.valid = true;
        }
        match data[0] {
            b'"' => self.actual_json_type = "string".to_owned(),
            b't' | b'f' => self.actual_json_type = "boolean".to_owned(),
            b'[' => self.actual_json_type = "array".to_owned(),
            b'{' => self.actual_json_type = "object".to_owned(),
            _ => self.actual_json_type = "number".to_owned(),
        }
        Ok(())
    }
}

impl<T> Expected<T> {
    pub fn is_present(&self) -> bool {
        !self.actual_json_type.is_empty()
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn actual_json_type(&self) -> String {
        self.actual_json_type.clone()
    }
}

impl<T> Expected<T>
where
    T: Clone,
{
    pub fn get_value(&self) -> (T, bool) {
        (self.value.clone(), self.valid)
    }
}

impl<T> Expected<T>
where
    T: ExpectedJsonValue,
{
    pub fn expected_json_type(&self) -> String {
        T::expected_json_type().to_owned()
    }
}

impl<'de, T> Deserialize<'de> for Expected<T>
where
    T: Default + DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let data = serde_json::to_string(&value).map_err(D::Error::custom)?;
        let mut expected = Expected::default();
        expected
            .unmarshal_json(data.as_bytes())
            .map_err(D::Error::custom)?;
        Ok(expected)
    }
}

impl<T> crate::TypeValidatedField for Expected<T>
where
    T: ExpectedJsonValue,
{
    fn is_present(&self) -> bool {
        self.is_present()
    }

    fn is_valid(&self) -> bool {
        self.is_valid()
    }

    fn expected_json_type(&self) -> String {
        self.expected_json_type()
    }

    fn actual_json_type(&self) -> String {
        self.actual_json_type()
    }
}

pub fn expected_of<T>(value: T) -> Expected<T>
where
    T: ExpectedJsonValue,
{
    Expected {
        value,
        valid: true,
        actual_json_type: T::expected_json_type().to_owned(),
        null: false,
    }
}
