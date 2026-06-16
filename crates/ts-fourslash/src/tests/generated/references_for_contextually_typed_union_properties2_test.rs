#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_contextually_typed_union_properties2() {
    let mut t = TestingT;
    run_test_references_for_contextually_typed_union_properties2(&mut t);
}

fn run_test_references_for_contextually_typed_union_properties2(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForContextuallyTypedUnionProperties2") {
        return;
    }
    let content = r#"interface A {
    a: number;
    common: string;
}

interface B {
    /*1*/b: number;
    common: number;
}

// Assignment
var v1: A | B = { a: 0, common: "" };
var v2: A | B = { b: 0, common: 3 };

// Function call
function consumer(f:  A | B) { }
consumer({ a: 0, b: 0, common: 1 });

// Type cast
var c = <A | B> { common: 0, b: 0 };

// Array literal
var ar: Array<A|B> = [{ a: 0, common: "" }, { b: 0, common: 0 }];

// Nested object literal
var ob: { aorb: A|B } = { aorb: { b: 0, common: 0 } };

// Widened type
var w: A|B = { b:undefined, common: undefined };

// Untped -- should not be included
var u1 = { a: 0, b: 0, common: "" };
var u2 = { b: 0, common: 0 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
