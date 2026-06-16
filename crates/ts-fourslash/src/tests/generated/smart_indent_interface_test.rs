#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_interface() {
    let mut t = TestingT;
    run_test_smart_indent_interface(&mut t);
}

fn run_test_smart_indent_interface(t: &mut TestingT) {
    if should_skip_if_failing("TestSmartIndentInterface") {
        return;
    }
    let content = r#"interface Foo {
    {| "indentation" : 4 |}
    x: number;
    {| "indentation" : 4 |}
    foo(): number;
    {| "indentation" : 4 |}
}
{| "indentation" : 0 |}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
