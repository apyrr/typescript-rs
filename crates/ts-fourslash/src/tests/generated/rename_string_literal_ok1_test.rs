#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_string_literal_ok1() {
    let mut t = TestingT;
    run_test_rename_string_literal_ok1(&mut t);
}

fn run_test_rename_string_literal_ok1(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameStringLiteralOk1") {
        return;
    }
    let content = r"declare function f(): '[|foo|]' | 'bar'
class Foo {
    f = f()
}
const d: 'foo' = 'foo'
declare const ff: Foo
ff.f = '[|foo|]'";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "foo");
    done();
}
