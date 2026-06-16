#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_inlay_hints_overload_call2() {
    let mut t = TestingT;
    run_test_inlay_hints_overload_call2(&mut t);
}

fn run_test_inlay_hints_overload_call2(t: &mut TestingT) {
    if should_skip_if_failing("TestInlayHintsOverloadCall2") {
        return;
    }
    let content = r"type HasID = {
    id: number;
}

type Numbers = {
    n: number[];
}

declare function func(bad1: number, bad2: HasID): void;
declare function func(ok_1: Numbers, ok_2: HasID): void;

func(
    { n: [1, 2, 3] },
    {
        id: 1,
    },
);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_inlay_hints(t);
    done();
}
