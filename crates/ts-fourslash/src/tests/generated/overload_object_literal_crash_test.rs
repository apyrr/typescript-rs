#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_overload_object_literal_crash() {
    let mut t = TestingT;
    run_test_overload_object_literal_crash(&mut t);
}

fn run_test_overload_object_literal_crash(t: &mut TestingT) {
    if should_skip_if_failing("TestOverloadObjectLiteralCrash") {
        return;
    }
    let content = r#"interface Foo {
    extend<T>(...objs: any[]): T;
    extend<T>(deep, target: T): T;
}
var $: Foo;
$.extend({ /**/foo: 0 }, "");
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
