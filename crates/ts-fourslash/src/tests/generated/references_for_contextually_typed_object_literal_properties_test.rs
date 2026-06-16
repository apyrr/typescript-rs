#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_contextually_typed_object_literal_properties() {
    let mut t = TestingT;
    run_test_references_for_contextually_typed_object_literal_properties(&mut t);
}

fn run_test_references_for_contextually_typed_object_literal_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForContextuallyTypedObjectLiteralProperties") {
        return;
    }
    let content = r"interface IFoo { /*xy*/xy: number; }

// Assignment
var a1: IFoo = { xy: 0 };
var a2: IFoo = { xy: 0 };

// Function call
function consumer(f: IFoo) { }
consumer({ xy: 1 });

// Type cast
var c = <IFoo>{ xy: 0 };

// Array literal
var ar: IFoo[] = [{ xy: 1 }, { xy: 2 }];

// Nested object literal
var ob: { ifoo: IFoo } = { ifoo: { xy: 0 } };

// Widened type
var w: IFoo = { xy: undefined };

// Untped -- should not be included
var u = { xy: 0 };";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["xy".to_string()]);
    done();
}
