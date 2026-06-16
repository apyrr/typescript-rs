#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_require() {
    let mut t = TestingT;
    run_test_quick_info_for_require(&mut t);
}

fn run_test_quick_info_for_require(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename: AA/BB.ts
export class a{}
//@Filename: quickInfoForRequire_input.ts
import a = require("./AA/B/*1*/B");
import b = require(` + "`" + `./AA/B/*2*/B` + "`" + `);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(t, "module a", "");
    f.go_to_marker(t, "2");
    f.verify_quick_info_is(t, "module a", "");
    done();
}
