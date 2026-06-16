use super::packagejson::parse;

#[test]
fn parse_duplicate_names() {
    let fields = parse(
        br#"{
            "name": "test-package",
            "name": "test-package",
            "version": "1.0.0"
        }"#,
    )
    .unwrap();

    assert_eq!(fields.header_fields.name.value, "test-package");
    assert!(fields.header_fields.name.valid);
    assert_eq!(fields.header_fields.version.value, "1.0.0");
    assert!(fields.header_fields.version.valid);
}
