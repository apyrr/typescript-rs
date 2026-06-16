use std::collections::HashMap;

use super::*;

#[test]
fn test_make_units_from_test() {
    let code = r#"// @strict: true
// @noEmit: true
// @filename: firstFile.ts
function foo() { return "a"; }
// normal comment
// @filename: secondFile.ts
// some other comment
function bar() { return "b"; }"#;

    let test_content = make_units_from_test(code, "simpleTest.ts");

    assert_eq!(
        test_content.test_unit_data,
        vec![
            TestUnit {
                content: "function foo() { return \"a\"; }\n// normal comment".to_string(),
                name: "firstFile.ts".to_string(),
            },
            TestUnit {
                content: "// some other comment\nfunction bar() { return \"b\"; }".to_string(),
                name: "secondFile.ts".to_string(),
            },
        ]
    );
    assert_eq!(test_content.tsconfig, None);
    assert_eq!(test_content.tsconfig_file_unit_data, None);
    assert_eq!(test_content.symlinks, HashMap::new());
}

#[test]
fn extract_compiler_settings_preserves_ignore_deprecations() {
    let settings = extract_compiler_settings("// @ignoreDeprecations: 6.0\nlet x = 1;");

    assert_eq!(
        settings.get("ignoredeprecations").map(String::as_str),
        Some("6.0")
    );

    let configs =
        ts_testutil::harnessutil::get_file_based_test_configurations(&settings, &HashMap::new());
    assert_eq!(
        configs
            .first()
            .and_then(|config| config.config.get("ignoredeprecations"))
            .map(String::as_str),
        Some("6.0")
    );
}
