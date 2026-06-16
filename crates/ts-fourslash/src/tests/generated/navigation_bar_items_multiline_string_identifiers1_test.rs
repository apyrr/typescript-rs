#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_bar_items_multiline_string_identifiers1() {
    let mut t = TestingT;
    run_test_navigation_bar_items_multiline_string_identifiers1(&mut t);
}

fn run_test_navigation_bar_items_multiline_string_identifiers1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"declare module "Multiline\r\nMadness" {
}

declare module "Multiline\
Madness" {
}
declare module "MultilineMadness" {}

declare module "Multiline\
Madness2" {
}

interface Foo {
    "a1\\\r\nb";
    "a2\
    \
    b"(): Foo;
}

class Bar implements Foo {
    'a1\\\r\nb': Foo;

    'a2\
    \
    b'(): Foo {
        return this;
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_document_symbol(t);
    done();
}
