use super::jsonvalue::{JsonValue, JsonValueType};
use serde_json::Value;

#[test]
fn json_value() {
    let json_string = r##"{
        "private": true,
        "false": false,
        "name": "test",
        "version": 2,
        "exports": {
            ".": {
                "import": "./test.ts",
                "default": "./test.ts"
            },
            "./test": [
                "./test1.ts",
                "./test2.ts",
                null
            ],
            "./null": null
        },
        "imports": null
    }"##;

    let root: Value = serde_json::from_str(json_string).unwrap();

    let private = field(&root, "private");
    assert_eq!(private.type_, JsonValueType::Boolean);
    assert_eq!(private.value, Value::Bool(true));

    let name = field(&root, "name");
    assert_eq!(name.type_, JsonValueType::String);
    assert_eq!(name.value, Value::String("test".to_owned()));

    let version = field(&root, "version");
    assert_eq!(version.type_, JsonValueType::Number);
    assert_eq!(version.value.as_f64(), Some(2.0));

    let exports = field(&root, "exports");
    assert_eq!(exports.type_, JsonValueType::Object);
    assert_eq!(exports.as_object().len(), 3);

    let dot = json_value_from_object(&exports, ".");
    assert_eq!(dot.type_, JsonValueType::Object);
    assert_eq!(
        json_value_from_object(&dot, "import").value,
        Value::String("./test.ts".to_owned())
    );

    let test = json_value_from_object(&exports, "./test");
    assert_eq!(test.type_, JsonValueType::Array);
    assert_eq!(test.as_array().len(), 3);
    assert_eq!(
        JsonValue::from_json_value(test.as_array()[0].clone()).value,
        Value::String("./test1.ts".to_owned())
    );
    assert_eq!(
        JsonValue::from_json_value(test.as_array()[1].clone()).value,
        Value::String("./test2.ts".to_owned())
    );
    assert_eq!(
        JsonValue::from_json_value(test.as_array()[2].clone()).type_,
        JsonValueType::Null
    );

    assert_eq!(
        json_value_from_object(&exports, "./null").type_,
        JsonValueType::Null
    );

    let imports = field(&root, "imports");
    assert_eq!(imports.type_, JsonValueType::Null);
    assert!(imports.value.is_null());

    let not_present = field(&root, "notPresent");
    assert_eq!(not_present.type_, JsonValueType::NotPresent);
    assert!(not_present.value.is_null());
}

fn field(root: &Value, name: &str) -> JsonValue {
    root.as_object()
        .and_then(|object| object.get(name))
        .cloned()
        .map(JsonValue::from_json_value)
        .unwrap_or_default()
}

fn json_value_from_object(value: &JsonValue, name: &str) -> JsonValue {
    JsonValue::from_json_value(value.as_object().get(name).unwrap().clone())
}
