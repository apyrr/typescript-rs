use super::parse_pseudo_big_int;

#[test]
fn test_parse_pseudo_big_int_strip_base_10_strings() {
    let mut test_numbers: Vec<i64> = (0..1_000).collect();
    for bits in 0..53 {
        test_numbers.push(1_i64 << bits);
        test_numbers.push((1_i64 << bits) - 1);
    }

    for test_number in test_numbers {
        for leading_zeros in 0..10 {
            assert_eq!(
                parse_pseudo_big_int(&format!("{}{}n", "0".repeat(leading_zeros), test_number)),
                test_number.to_string()
            );
        }
    }
}

#[test]
fn test_parse_pseudo_big_int_parse_non_decimal_bases_small_numbers() {
    struct Case {
        lit: &'static str,
        out: &'static str,
    }
    let cases = [
        // binary
        Case {
            lit: "0b0n",
            out: "0",
        },
        Case {
            lit: "0b1n",
            out: "1",
        },
        Case {
            lit: "0b1010n",
            out: "10",
        },
        Case {
            lit: "0b1010_0101n",
            out: "165",
        },
        Case {
            lit: "0B1101n",
            out: "13",
        }, // uppercase prefix
        // octal
        Case {
            lit: "0o0n",
            out: "0",
        },
        Case {
            lit: "0o7n",
            out: "7",
        },
        Case {
            lit: "0o755n",
            out: "493",
        },
        Case {
            lit: "0o7_5_5n",
            out: "493",
        },
        Case {
            lit: "0O12n",
            out: "10",
        }, // uppercase prefix
        // hex
        Case {
            lit: "0x0n",
            out: "0",
        },
        Case {
            lit: "0xFn",
            out: "15",
        },
        Case {
            lit: "0xFFn",
            out: "255",
        },
        Case {
            lit: "0xF_Fn",
            out: "255",
        },
        Case {
            lit: "0X1Fn",
            out: "31",
        }, // uppercase prefix
    ];

    for c in cases {
        let got = parse_pseudo_big_int(c.lit);
        assert_eq!(got, c.out, "literal: {:?}", c.lit);
    }
}

#[test]
fn test_parse_pseudo_big_int_can_parse_large_literals() {
    assert_eq!(
        parse_pseudo_big_int("123456789012345678901234567890n"),
        "123456789012345678901234567890"
    );
    assert_eq!(
        parse_pseudo_big_int(
            "0b1100011101110100100001111111101101100001101110011111000001110111001001110001111110000101011010010n"
        ),
        "123456789012345678901234567890"
    );
    assert_eq!(
        parse_pseudo_big_int("0o143564417755415637016711617605322n"),
        "123456789012345678901234567890"
    );
    assert_eq!(
        parse_pseudo_big_int("0x18ee90ff6c373e0ee4e3f0ad2n"),
        "123456789012345678901234567890"
    );
}
