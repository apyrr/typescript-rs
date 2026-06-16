#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_return_recursive_type() {
    let mut t = TestingT;
    run_test_return_recursive_type(&mut t);
}

fn run_test_return_recursive_type(t: &mut TestingT) {
    if should_skip_if_failing("TestReturnRecursiveType") {
        return;
    }
    let content = r"interface MyInt {
    (): void;
}
function MyFn() { return <MyInt>MyFn; }
var My/**/Var = MyFn();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var MyVar: MyInt", "");
    done();
}
