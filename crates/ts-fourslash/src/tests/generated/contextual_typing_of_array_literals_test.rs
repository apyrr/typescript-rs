#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextual_typing_of_array_literals() {
    let mut t = TestingT;
    run_test_contextual_typing_of_array_literals(&mut t);
}

fn run_test_contextual_typing_of_array_literals(t: &mut TestingT) {
    if should_skip_if_failing("TestContextualTypingOfArrayLiterals") {
        return;
    }
    let content = r"// @strict: false
class C {
    name: string;
    age: number;
}
interface I {
    [x: number]: C;
}
var /*1*/x = [null, null];
var x2: I = [null, null];
var /*2*/r = x2[0];
var a = { name: 'bob', age: 20 };
var b = { name: 'jim', age: 20, dob: new Date() };
var c: C;
var d = { name: 'jim', age: 20, address: 'springfield' };
var x3: I = [a, b];
var /*3*/r3 = x3[1];
var x4: I = [a, b, c];
var /*4*/r4 = x4[1];
var /*5*/x5 = [a, b];
var /*6*/r5 = x5[1];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_error_exists_between_markers(&f.marker_by_name("1"), &f.marker_by_name("6"));
    f.verify_quick_info_at(t, "1", "var x: any[]", "");
    f.verify_quick_info_at(t, "2", "var r: C", "");
    f.verify_quick_info_at(t, "3", "var r3: C", "");
    f.verify_quick_info_at(t, "4", "var r4: C", "");
    f.verify_quick_info_at(
        t,
        "5",
        "var x5: {\n    name: string;\n    age: number;\n}[]",
        "",
    );
    f.verify_quick_info_at(
        t,
        "6",
        "var r5: {\n    name: string;\n    age: number;\n}",
        "",
    );
    done();
}
