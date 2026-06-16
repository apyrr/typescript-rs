use crate::{new_fourslash, TestingT};

pub fn test_quick_info_index_signature_mapped_type(t: &mut TestingT) {
    // Regression test for https://github.com/microsoft/typescript-go/issues/3018
    // Quick info for property access resolved from an index signature on a mapped type
    // (e.g. Record<string, string>) should show the value type rather than nothing.
    let content = r#"
// @strict: true
// @filename: main.ts
declare const record: Record<string, string>;
record.fo/*1*/o;
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "string", "");
    done();
}

