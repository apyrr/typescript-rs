use super::expected::{AnyJson, Expected};
use serde::Deserialize;

#[test]
fn test_expected() {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct PackageJson {
        name: Expected<String>,
        version: Expected<String>,
        exports: Expected<AnyJson>,
        main: Expected<String>,
    }

    let json_string = r#"{
        "name": "test",
        "version": 2,
        "exports": null
    }"#;

    let p: PackageJson = serde_json::from_str(json_string).unwrap();

    assert_eq!(p.name.valid, true);
    assert_eq!(p.name.value, "test");

    assert_eq!(p.version.valid, false);
    assert_eq!(p.version.value, "");

    assert!(p.exports.null);
    assert_eq!(p.exports.valid, false);

    assert_eq!(p.main.valid, false);
    assert_eq!(p.main.null, false);
    assert_eq!(p.main.value, "");
}
