#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_for_array_destructuring() {
    let mut t = TestingT;
    run_test_get_outlining_for_array_destructuring(&mut t);
}

fn run_test_get_outlining_for_array_destructuring(t: &mut TestingT) {
    if should_skip_if_failing("TestGetOutliningForArrayDestructuring") {
        return;
    }
    let content = r"const[| [
    a,
    b,
    c
]|] =[| [
    1,
    2,
    3
]|];
const[| [
    [|[
        [|[
            [|[
                a,
                b,
                c
            ]|]
        ]|]
    ]|],
    [|[
        a1,
        b1,
        c1
    ]|]
]|] =[| [
    [|[
        [|[
            [|[
                1,
                2,
                3
            ]|]
        ]|]
    ]|],
    [|[
        1,
        2,
        3
    ]|]
]|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
