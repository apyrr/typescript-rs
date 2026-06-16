use super::from_string;

#[test]
fn from_string_parses_prefixed_integer_literals_as_js_numbers() {
    assert_eq!(from_string("0b11010").to_string(), "26");
    assert_eq!(from_string("0B11010").to_string(), "26");
    assert_eq!(from_string("0o755").to_string(), "493");
    assert_eq!(from_string("0xFF").to_string(), "255");
}

#[test]
fn from_string_parses_large_prefixed_integer_literals_as_js_numbers() {
    assert_eq!(
        from_string(
            "0B11111111111111111111111111111111111111111111111101001010100000010111110001111111111"
        )
        .to_string(),
        "9.671406556917009e+24"
    );
    let overflowing_binary = format!("0B{}", "1".repeat(2048));
    assert_eq!(from_string(&overflowing_binary).to_string(), "Infinity");
}
