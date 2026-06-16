#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_link3() {
    let mut t = TestingT;
    run_test_quick_info_link3(&mut t);
}

fn run_test_quick_info_link3(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoLink3") {
        return;
    }
    let content = r"class Foo<T> {
    /**
     * {@link Foo}
     * {@link Foo<T>}
     * {@link Foo<Array<X>>}
     * {@link Foo<>}
     * {@link Foo>}
     * {@link Foo<}
     */
    bar/**/(){}
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_hover(t, &[]);
    done();
}
