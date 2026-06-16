#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_merged_interfaces_with_incremental_edits() {
    let mut t = TestingT;
    run_test_quick_info_on_merged_interfaces_with_incremental_edits(&mut t);
}

fn run_test_quick_info_on_merged_interfaces_with_incremental_edits(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @strict: false
namespace MM {
    interface B<T> {
        foo: number;
    }
    interface B<T> {
        bar: string;
    }
    var b: B<string>;
    var r3 = b.foo; // number
    var r/*2*/4 = b.b/*1*/ar; // string
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(t, "(property) B<string>.bar: string", "");
    f.delete_at_caret(t, 1);
    f.insert(t, "z");
    f.verify_quick_info_is(t, "any", "");
    f.verify_number_of_errors_in_current_file(1);
    f.backspace(t, 1);
    f.insert(t, "a");
    f.verify_quick_info_is(t, "(property) B<string>.bar: string", "");
    f.go_to_marker(t, "2");
    f.verify_quick_info_is(t, "var r4: string", "");
    f.verify_no_errors();
    done();
}
