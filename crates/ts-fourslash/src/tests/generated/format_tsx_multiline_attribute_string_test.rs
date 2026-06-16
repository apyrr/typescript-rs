#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_tsx_multiline_attribute_string() {
    let mut t = TestingT;
    run_test_format_tsx_multiline_attribute_string(&mut t);
}

fn run_test_format_tsx_multiline_attribute_string(t: &mut TestingT) {
    if should_skip_if_failing("TestFormatTsxMultilineAttributeString") {
        return;
    }
    let content = r#"// @Filename: foo.tsx
(
    <input
        value="x
        x"
    />
);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r#"(
    <input
        value="x
        x"
    />
);"#,
    );
    done();
}
