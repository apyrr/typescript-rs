#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_member_list_in_with_block2() {
    let mut t = TestingT;
    run_test_member_list_in_with_block2(&mut t);
}

fn run_test_member_list_in_with_block2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface IFoo {
    a: number;
}

with (x) {
    var y: IFoo = { /*1*/ };
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Name("1".to_string()), None);
    done();
}
