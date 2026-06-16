#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_wrap_jsx_in_fragment4() {
    let mut t = TestingT;
    run_test_code_fix_wrap_jsx_in_fragment4(&mut t);
}

fn run_test_code_fix_wrap_jsx_in_fragment4(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixWrapJsxInFragment4") {
        return;
    }
    let content = r"// @jsx: react-jsxdev
// @Filename: /a.tsx
[|<a></a><a />|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(t, "<><a></a><a /></>", false, 0, 0);
    done();
}
