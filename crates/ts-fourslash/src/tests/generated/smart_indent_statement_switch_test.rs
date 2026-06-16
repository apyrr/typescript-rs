#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_statement_switch() {
    let mut t = TestingT;
    run_test_smart_indent_statement_switch(&mut t);
}

fn run_test_smart_indent_statement_switch(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentStatementSwitch") {
        return;
    }
    let content = r#"function Foo() {
    var x;
    switch (x) {
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
    switch (x) {
        {| "indentation": 8 |}
        case 1:
            {| "indentation": 12 |}
            break;
            {| "indentation": 8 |} // since we just saw "break"
    }
    {| "indentation": 4 |}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
