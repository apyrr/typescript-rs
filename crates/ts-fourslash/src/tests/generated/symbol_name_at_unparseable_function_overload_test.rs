#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_symbol_name_at_unparseable_function_overload() {
    let mut t = TestingT;
    run_test_symbol_name_at_unparseable_function_overload(&mut t);
}

fn run_test_symbol_name_at_unparseable_function_overload(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class TestClass {
    public function foo(x: string): void;
    public function foo(): void;
    foo(x: any): void {
        this.bar(/**/x); // should not error
    }
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_exists(t);
    done();
}
