#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_smart_indent_nested_module() {
    let mut t = TestingT;
    run_test_smart_indent_nested_module(&mut t);
}

fn run_test_smart_indent_nested_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"namespace Foo {
    namespace Foo2 {
        {| "indentation": 8 |}
        function f() {
        }
        {| "indentation": 8 |}
        var x: number;
        {| "indentation": 8 |}
    }
    {| "indentation": 4 |}
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_indentation_at_markers_from_data(t);
    done();
}
