#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsx_element_extends_no_crash2() {
    let mut t = TestingT;
    run_test_jsx_element_extends_no_crash2(&mut t);
}

fn run_test_jsx_element_extends_no_crash2(t: &mut TestingT) {
    if should_skip_if_failing("TestJsxElementExtendsNoCrash2") {
        return;
    }
    let content = r"// @filename: index.tsx
<T extends/>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
