#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_named_tuple_members() {
    let mut t = TestingT;
    run_test_quick_info_named_tuple_members(&mut t);
}

fn run_test_quick_info_named_tuple_members(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoNamedTupleMembers") {
        return;
    }
    let content = r"export type /*1*/Segment = [length: number, count: number];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "type Segment = [length: number, count: number]", "");
    done();
}
