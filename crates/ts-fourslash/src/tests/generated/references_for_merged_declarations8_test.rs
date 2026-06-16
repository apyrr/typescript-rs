#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_merged_declarations8() {
    let mut t = TestingT;
    run_test_references_for_merged_declarations8(&mut t);
}

fn run_test_references_for_merged_declarations8(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForMergedDeclarations8") {
        return;
    }
    let content = r"interface Foo { }
namespace Foo {
    export interface Bar { }
    /*1*/export module /*2*/Bar { export interface Baz { } }
    export function Bar() { }
}

// module
import a3 = Foo./*3*/Bar.Baz;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string(), "3".to_string()]);
    done();
}
