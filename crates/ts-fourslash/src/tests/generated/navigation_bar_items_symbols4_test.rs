#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_symbols4() {
    let mut t = TestingT;
    run_test_navigation_bar_items_symbols4(&mut t);
}

fn run_test_navigation_bar_items_symbols4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @checkJs: true
// @allowJs: true
// @target: es6
// @Filename: file.js
const _sym = Symbol("_sym");
class MyClass {
    constructor() {
        // Dynamic assignment properties can't show up in navigation,
        // as they're not syntactic members
        // Additonally, late bound members are always filtered out, besides
        this[_sym] = "ok";
    }

    method() {
        this[_sym] = "yep";
        const x = this[_sym];
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
