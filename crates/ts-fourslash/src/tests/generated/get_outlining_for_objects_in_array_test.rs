#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_for_objects_in_array() {
    let mut t = TestingT;
    run_test_get_outlining_for_objects_in_array(&mut t);
}

fn run_test_get_outlining_for_objects_in_array(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"const x =[| [
    [|{ a: 0 }|],
    [|{ b: 1 }|],
    [|{ c: 2 }|]
]|];

const y =[| [
    [|{
        a: 0
    }|],
    [|{
        b: 1
    }|],
    [|{
        c: 2
    }|]
]|];

const w =[| [
    [|[ 0 ]|],
    [|[ 1 ]|],
    [|[ 2 ]|]
]|];

const z =[| [
    [|[
        0
    ]|],
    [|[
        1
    ]|],
    [|[
        2
    ]|]
]|];

const z =[| [
    [|[
        [|{ hello: 0 }|]
    ]|],
    [|[
        [|{ hello: 3 }|]
    ]|],
    [|[
        [|{ hello: 5 }|],
        [|{ hello: 7 }|]
    ]|]
]|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
