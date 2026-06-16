#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_no_space_between_closing_paren_and_template_string() {
    let mut t = TestingT;
    run_test_format_no_space_between_closing_paren_and_template_string(&mut t);
}

fn run_test_format_no_space_between_closing_paren_and_template_string(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"foo() ` + "`" + `abc` + "`" + `;
bar()` + "`" + `def` + "`" + `;
baz()` + "`" + `a${x}b` + "`" + `;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"foo()`abc`;
bar()`def`;
baz()`a${x}b`;",
    );
    done();
}
