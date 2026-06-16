#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_contextual_typing_from_type_assertion1() {
    let mut t = TestingT;
    run_test_contextual_typing_from_type_assertion1(&mut t);
}

fn run_test_contextual_typing_from_type_assertion1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var f3 = <(x: string) => string> function (/**/x) { return x.toLowerCase(); };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "(parameter) x: string", "");
    done();
}
