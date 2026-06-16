use crate::CommandLineOption;
use crate::Tristate;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerOptionsParseMode {
    CommandLine,
    TsConfig,
}

pub fn parse_tristate(value: Option<&Value>) -> Tristate {
    match value {
        None | Some(Value::Null) => Tristate::Unknown,
        Some(Value::Bool(true)) => Tristate::True,
        Some(Value::Bool(false)) => Tristate::False,
        _ => Tristate::False,
    }
}

pub fn parse_string_array(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub fn parse_string_map(value: Option<&Value>) -> BTreeMap<String, Vec<String>> {
    let Some(Value::Object(map)) = value else {
        return BTreeMap::new();
    };
    map.iter()
        .map(|(key, value)| (key.clone(), parse_string_array(Some(value))))
        .collect()
}

pub fn parse_string(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or_default().to_owned()
}

pub fn parse_number(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| value.and_then(Value::as_f64).map(|value| value as i32))
}

pub fn parse_json_to_string_key(json: Option<&Value>) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    let Some(Value::Object(map)) = json else {
        return result;
    };
    for key in [
        "include",
        "exclude",
        "files",
        "references",
        "extends",
        "compilerOptions",
        "excludes",
        "typeAcquisition",
    ] {
        if let Some(value) = map.get(key) {
            if key == "extends"
                && let Some(path) = value.as_str()
            {
                result.insert(
                    key.to_owned(),
                    Value::Array(vec![Value::String(path.to_owned())]),
                );
                continue;
            }
            result.insert(key.to_owned(), value.clone());
        }
    }
    result
}

pub fn validate_json_option_value(
    option: &CommandLineOption,
    value: Option<&str>,
    errors: &mut Vec<String>,
) -> bool {
    if option.disallow_null_or_undefined() && value.is_none() {
        errors.push(format!("{} cannot be null or undefined", option.name));
        return false;
    }
    if option.min_value != 0
        && let Some(value) = value.and_then(|value| value.parse::<i32>().ok())
        && value < option.min_value
    {
        errors.push(format!(
            "{} must be at least {}",
            option.name, option.min_value
        ));
        return false;
    }
    true
}

pub fn convert_json_option(
    option: &CommandLineOption,
    value: Option<&str>,
    errors: &mut Vec<String>,
) -> Option<String> {
    validate_json_option_value(option, value, errors).then(|| value.unwrap_or_default().to_owned())
}

pub fn get_option_name(option: &CommandLineOption) -> &str {
    &option.name
}

pub fn is_option_value_empty(value: Option<&str>) -> bool {
    value.map(str::is_empty).unwrap_or(true)
}

pub fn has_property(map: &std::collections::BTreeMap<String, String>, key: &str) -> bool {
    map.contains_key(key)
}
