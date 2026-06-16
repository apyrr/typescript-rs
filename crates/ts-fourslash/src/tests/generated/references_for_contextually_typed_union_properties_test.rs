#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_contextually_typed_union_properties() {
    let mut t = TestingT;
    run_test_references_for_contextually_typed_union_properties(&mut t);
}

fn run_test_references_for_contextually_typed_union_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForContextuallyTypedUnionProperties") {
        return;
    }
    let content = r#"interface A {
    a: number;
    /*1*/common: string;
}

interface B {
    b: number;
    /*2*/common: number;
}

// Assignment
var v1: A | B = { a: 0, /*3*/common: "" };
var v2: A | B = { b: 0, /*4*/common: 3 };

// Function call
function consumer(f:  A | B) { }
consumer({ a: 0, b: 0, /*5*/common: 1 });

// Type cast
var c = <A | B> { /*6*/common: 0, b: 0 };

// Array literal
var ar: Array<A|B> = [{ a: 0, /*7*/common: "" }, { b: 0, /*8*/common: 0 }];

// Nested object literal
var ob: { aorb: A|B } = { aorb: { b: 0, /*9*/common: 0 } };

// Widened type
var w: A|B = { a:0, /*10*/common: undefined };

// Untped -- should not be included
var u1 = { a: 0, b: 0, common: "" };
var u2 = { b: 0, common: 0 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
        ],
    );
    done();
}
