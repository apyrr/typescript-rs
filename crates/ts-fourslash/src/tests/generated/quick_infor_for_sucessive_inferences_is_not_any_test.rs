#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_infor_for_sucessive_inferences_is_not_any() {
    let mut t = TestingT;
    run_test_quick_infor_for_sucessive_inferences_is_not_any(&mut t);
}

fn run_test_quick_infor_for_sucessive_inferences_is_not_any(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"declare function schema<T> (value : T) : {field : T};

declare const b: boolean;
const obj/*1*/ = schema(b);
const actualTypeOfNested/*2*/ = schema(obj);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "const obj: {\n    field: boolean;\n}", "");
    f.verify_quick_info_at(
        t,
        "2",
        "const actualTypeOfNested: {\n    field: {\n        field: boolean;\n    };\n}",
        "",
    );
    done();
}
