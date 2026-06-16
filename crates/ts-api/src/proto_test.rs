use crate::proto::DocumentIdentifier;

#[test]
fn test_document_identifier_unmarshal_json() {
    // t.Parallel()
    struct Test {
        name: &'static str,
        input: &'static str,
        file_name: &'static str,
        uri: &'static str,
        err: &'static str,
    }

    let tests = [
        Test {
            name: "plain string",
            input: r#""foo.ts""#,
            file_name: "foo.ts",
            uri: "",
            err: "",
        },
        Test {
            name: "uri object",
            input: r#"{"uri":"file:///foo.ts"}"#,
            file_name: "",
            uri: "file:///foo.ts",
            err: "",
        },
        Test {
            name: "uri object with unknown fields",
            input: r#"{"uri":"file:///foo.ts","extra":true}"#,
            file_name: "",
            uri: "file:///foo.ts",
            err: "",
        },
        Test {
            name: "empty object",
            input: r#"{}"#,
            file_name: "",
            uri: "",
            err: "",
        },
        Test {
            name: "invalid type",
            input: r#"42"#,
            file_name: "",
            uri: "",
            err: "expected string or object, got number",
        },
    ];

    for tt in tests {
        // t.Run(tt.name, func(t *testing.T) {
        //     t.Parallel()
        let result = serde_json::from_str::<DocumentIdentifier>(tt.input);
        if !tt.err.is_empty() {
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains(tt.err),
                "{}: expected error containing {:?}, got {:?}",
                tt.name,
                tt.err,
                err
            );
            continue;
        }
        let d = result.unwrap_or_else(|err| panic!("{}: {err}", tt.name));
        assert_eq!(d.file_name, tt.file_name, "{}", tt.name);
        assert_eq!(d.uri.to_string(), tt.uri, "{}", tt.name);
        // })
    }
}

