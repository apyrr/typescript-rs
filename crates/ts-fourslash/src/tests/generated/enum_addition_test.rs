#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_enum_addition() {
    let mut t = TestingT;
    run_test_enum_addition(&mut t);
}

fn run_test_enum_addition(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"namespace m { export enum Color { Red } }
var /**/t = m.Color.Red + 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var t: number", "");
    done();
}
