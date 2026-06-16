use super::exportsorimports::ExportsOrImports;
use super::jsonvalue::{JsonValue, JsonValueType};
use serde_json::Value;

#[test]
fn exports() {
    let json_string = r##"{
        "imports": {
            "#foo": {
                "import": "./foo.ts"
            }
        },
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
        }
    }"##;

    let root: Value = serde_json::from_str(json_string).unwrap();
    let mut exports = exports_or_imports_field(&root, "exports");
    let mut imports = exports_or_imports_field(&root, "imports");

    assert!(exports.is_subpaths());
    assert_eq!(exports.as_object().len(), 3);

    let mut dot = exports_or_imports_from_value(exports.as_object().get(".").unwrap());
    assert!(dot.is_conditions());
    assert_eq!(
        json_value_from_object(&dot.json_value, "import").type_,
        JsonValueType::String
    );

    assert_eq!(
        JsonValue::from_json_value(
            json_value_from_object(&exports.json_value, "./test").as_array()[2].clone()
        )
        .type_,
        JsonValueType::Null
    );
    assert_eq!(
        json_value_from_object(&exports.json_value, "./null").type_,
        JsonValueType::Null
    );

    assert!(imports.is_imports());
    assert_eq!(imports.as_object().len(), 1);

    let mut foo = exports_or_imports_from_value(imports.as_object().get("#foo").unwrap());
    assert!(foo.is_conditions());
    assert_eq!(
        json_value_from_object(&foo.json_value, "import").type_,
        JsonValueType::String
    );
}

#[test]
fn conditional_exports_preserve_package_json_order() {
    let json_string = r##"{
        "exports": {
            ".": {
                "node": "./index.node.js",
                "default": "./index.web.js"
            }
        }
    }"##;

    let root: Value = serde_json::from_str(json_string).unwrap();
    let exports = exports_or_imports_field(&root, "exports");
    let dot = exports_or_imports_from_value(exports.as_object().get(".").unwrap());
    let keys: Vec<_> = dot.as_object().keys().map(String::as_str).collect();

    assert_eq!(keys, ["node", "default"]);
}

fn exports_or_imports_field(root: &Value, name: &str) -> ExportsOrImports {
    exports_or_imports_from_value(root.as_object().unwrap().get(name).unwrap())
}

fn exports_or_imports_from_value(value: &Value) -> ExportsOrImports {
    let mut exports_or_imports = ExportsOrImports::default();
    exports_or_imports
        .unmarshal_json(value.to_string().as_bytes())
        .unwrap();
    exports_or_imports
}

fn json_value_from_object(value: &JsonValue, name: &str) -> JsonValue {
    JsonValue::from_json_value(value.as_object().get(name).unwrap().clone())
}
