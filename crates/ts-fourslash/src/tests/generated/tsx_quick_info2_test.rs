#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_quick_info2() {
    let mut t = TestingT;
    run_test_tsx_quick_info2(&mut t);
}

fn run_test_tsx_quick_info2(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxQuickInfo2") {
        return;
    }
    let content = r"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
        div: any
    }
}
var x1 = <di/*1*/v></di/*2*/v>
class MyElement {}
var z = <My/*3*/Element></My/*4*/Element>";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) JSX.IntrinsicElements.div: any", "");
    f.verify_quick_info_at(t, "2", "(property) JSX.IntrinsicElements.div: any", "");
    f.verify_quick_info_at(t, "3", "class MyElement", "");
    f.verify_quick_info_at(t, "4", "class MyElement", "");
    done();
}
