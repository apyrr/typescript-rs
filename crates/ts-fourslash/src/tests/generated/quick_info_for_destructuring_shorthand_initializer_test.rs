#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_destructuring_shorthand_initializer() {
    let mut t = TestingT;
    run_test_quick_info_for_destructuring_shorthand_initializer(&mut t);
}

fn run_test_quick_info_for_destructuring_shorthand_initializer(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"let a = '';
let b: string;
({b = /**/a} = {b: 'b'});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "let a: string", "");
    done();
}
