use super::encode_uri;

#[test]
fn test_encode_uri() {
    struct Test {
        name: &'static str,
        input: &'static str,
        expected: &'static str,
    }

    let tests = [
        Test {
            name: "encodes spaces as percent20",
            input: "a b",
            expected: "a%20b",
        },
        Test {
            name: "preserves reserved uri characters",
            input: ";/?:@&=+$,#",
            expected: ";/?:@&=+$,#",
        },
        Test {
            name: "encodes brackets and unicode using utf8 bytes",
            input: "①Ⅻㄨㄩ U1[abc]",
            expected: "%E2%91%A0%E2%85%AB%E3%84%A8%E3%84%A9%20U1%5Babc%5D",
        },
    ];

    for tt in tests {
        let got = encode_uri(tt.input);
        assert_eq!(
            got, tt.expected,
            "EncodeURI({:?}) = {:?}, expected {:?}",
            tt.input, got, tt.expected
        );
        let _ = tt.name;
    }
}
