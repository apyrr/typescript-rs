#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_object_literal() {
    let mut t = TestingT;
    run_test_formatting_object_literal(&mut t);
}

fn run_test_formatting_object_literal(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"var clear = {
"a": 1/**/
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "");
    f.verify_current_line_content(t, "    \"a\": 1");
    done();
}
