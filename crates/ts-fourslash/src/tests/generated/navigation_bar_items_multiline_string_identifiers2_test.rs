#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_multiline_string_identifiers2() {
    let mut t = TestingT;
    run_test_navigation_bar_items_multiline_string_identifiers2(&mut t);
}

fn run_test_navigation_bar_items_multiline_string_identifiers2(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationBarItemsMultilineStringIdentifiers2") {
        return;
    }
    let content = r"function f(p1: () => any, p2: string) { }
f(() => { }, `line1\
line2\
line3`);

class c1 {
    const a = ' ''line1\
        line2';
}

f(() => { }, `unterminated backtick 1
unterminated backtick 2
unterminated backtick 3";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
