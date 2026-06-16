#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_extend_interface_overloaded_method() {
    let mut t = TestingT;
    run_test_extend_interface_overloaded_method(&mut t);
}

fn run_test_extend_interface_overloaded_method(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
interface A<T> {
    foo(a: T): B<T>;
    foo(): void ;
    foo2(): B<number>;
}
interface B<T> extends A<T> {
    bar(): void ;
}
var b: B<number>;
var /**/x = b.foo2().foo(5).foo(); // 'x' is of type 'void'";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var x: void", "");
    f.verify_no_errors();
    done();
}
