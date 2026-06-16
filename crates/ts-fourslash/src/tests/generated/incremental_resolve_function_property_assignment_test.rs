#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_incremental_resolve_function_property_assignment() {
    let mut t = TestingT;
    run_test_incremental_resolve_function_property_assignment(&mut t);
}

fn run_test_incremental_resolve_function_property_assignment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"function bar(indexer: { getLength(): number; getTypeAtIndex(index: number): string; }): string {
    return indexer.getTypeAtIndex(indexer.getLength() - 1);
}
function foo(a: string[]) {
    return bar({
        getLength(): number {
            return "a.length";
        },
        getTypeAtIndex(index: number) {
            switch (index) {
                case 0: return a[0];
                case 1: return a[1];
                case 2: return a[2];
                default: return "invalid";
            }
        }
    });
}
var val = foo(["myString1", "myString2"]);
/*1*/val;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "var val: string", "");
    f.verify_number_of_errors_in_current_file(1);
    done();
}
