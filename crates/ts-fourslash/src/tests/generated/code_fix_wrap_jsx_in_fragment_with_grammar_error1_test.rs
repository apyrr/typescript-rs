#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_wrap_jsx_in_fragment_with_grammar_error1() {
    let mut t = TestingT;
    run_test_code_fix_wrap_jsx_in_fragment_with_grammar_error1(&mut t);
}

fn run_test_code_fix_wrap_jsx_in_fragment_with_grammar_error1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @jsx: react-jsxdev
// @Filename: /a.tsx
[|<div abc={{ foo = 10 }}></div><div abc={{ foo = 10 }}></div>|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "<><div abc={{ foo = 10 }}></div><div abc={{ foo = 10 }}></div></>",
        false,
        0,
        0,
    );
    done();
}
