#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_templates_with_newline() {
    let mut t = TestingT;
    run_test_formatting_templates_with_newline(&mut t);
}

fn run_test_formatting_templates_with_newline(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingTemplatesWithNewline") {
        return;
    }
    let content = r"`${1}`;
`
`;/**/1";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "\n");
    f.verify_current_line_content(t, "1");
    done();
}
